//! live execution、offline replay 与启动失败收尾。

use std::collections::BTreeMap;
use std::sync::Arc;

use futures::{StreamExt, stream};
use lj_importer::legado::LegadoImporter;
use lj_rule_model::SystemCapabilities;
use lj_runtime::{
    ExecutionMode as RuntimeExecutionMode, HttpExecutionCredentials, PlanExecutionRequest,
};
use lj_storage::{
    ExecutionFinish, ExecutionRecord, ExecutionStart, ExecutionStatus, InstalledSourceSnapshot,
    ProjectionDelta, ReplayExecutionStart,
};
use serde_json::Value;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::super::error_mapping::{runtime_error, storage_error};
use super::super::session_delivery::{
    SessionRun, continue_action_error, flush_persisted, replay_snapshot, run_session,
};
use super::super::{RuleSystem, lock, now_millis};
use crate::{
    ExecuteRequest, ExecutionId, ExecutionMode, ExecutionSession, RuleError, RuleErrorStage,
    SourceId,
};

struct ExecutionSnapshot {
    source_identity: String,
    plan: lj_rule_model::ExecutionPlan,
    grant: SystemCapabilities,
    base_url: String,
    credentials: HttpExecutionCredentials,
    mode: RuntimeExecutionMode,
    replay_continue_actions: Option<BTreeMap<String, Value>>,
}

impl RuleSystem {
    /// 启动 installed source 的标准意图 execution。
    ///
    /// # Errors
    ///
    /// source、intent、Plan、replay pin 或 durable start 失败时返回 `RuleError`。
    pub async fn execute(
        &self,
        mut request: ExecuteRequest,
    ) -> Result<ExecutionSession, RuleError> {
        let trace_id = super::super::trace_id();
        Self::normalize_continue_action(&mut request, &trace_id)?;
        let execution_uuid = Uuid::new_v4();
        let (snapshot, record) = self
            .start_execution_snapshot(&request, execution_uuid, &trace_id)
            .await?;

        let (delivery_sender, delivery_receiver) = mpsc::channel(self.state.session_event_capacity);
        let mut persisted_sequence = 0;
        if let Err(error) = flush_persisted(
            &self.state.storage,
            execution_uuid,
            &mut persisted_sequence,
            &delivery_sender,
            &trace_id,
        )
        .await
        {
            return Err(self
                .finish_started_execution_failure(execution_uuid, record.revision, error, &trace_id)
                .await);
        }
        if persisted_sequence != record.revision {
            let error = RuleError::new(
                RuleErrorStage::Persistence,
                "execution_start_sequence_mismatch",
                "execution start 的持久序列不连续",
                trace_id.clone(),
                false,
                Vec::new(),
            );
            return Err(self
                .finish_started_execution_failure(execution_uuid, record.revision, error, &trace_id)
                .await);
        }

        let ExecutionSnapshot {
            source_identity,
            plan,
            grant,
            base_url,
            credentials,
            mode,
            replay_continue_actions,
        } = snapshot;
        let runtime_session = match self.state.runtime.execute(
            PlanExecutionRequest {
                execution_id: execution_uuid,
                source_id: source_identity.clone(),
                trace_id: trace_id.clone(),
                plan,
                intent: request.intent,
                input: request.input,
                mode,
                capabilities: grant,
                base_url,
                credentials,
            },
            self.state.registry.clone(),
            Arc::new(self.state.storage.clone()),
        ) {
            Ok(session) => session,
            Err(error) => {
                let error = runtime_error(&error, &trace_id);
                return Err(self
                    .finish_started_execution_failure(
                        execution_uuid,
                        record.revision,
                        error,
                        &trace_id,
                    )
                    .await);
            }
        };
        let cancellation = runtime_session.cancellation_handle();
        lock(&self.state.executions).insert(execution_uuid, cancellation.clone());
        let runtime_events = runtime_session.into_events();
        let state = self.state.clone();
        let runner_trace_id = trace_id.clone();
        tokio::spawn(async move {
            run_session(
                state,
                execution_uuid,
                SessionRun {
                    source_identity,
                    replay_continue_actions,
                    runtime_events,
                    delivery_sender,
                    persisted_sequence,
                    trace_id: runner_trace_id,
                },
            )
            .await;
        });
        let events = stream::unfold(delivery_receiver, |mut receiver| async {
            receiver.recv().await.map(|event| (event, receiver))
        })
        .boxed();
        Ok(ExecutionSession::new(
            ExecutionId::from_uuid(execution_uuid),
            events,
            cancellation,
            self.state.storage.clone(),
        ))
    }

    async fn start_execution_snapshot(
        &self,
        request: &ExecuteRequest,
        execution_id: Uuid,
        trace_id: &str,
    ) -> Result<(ExecutionSnapshot, ExecutionRecord), RuleError> {
        match request.mode {
            ExecutionMode::Live => {
                let receipt = self
                    .state
                    .storage
                    .start_execution(ExecutionStart {
                        execution_id,
                        source_identity: request.source_id.as_identity().to_string(),
                        event_id: Uuid::new_v4(),
                        trace_id: trace_id.to_string(),
                        started_at_ms: now_millis(trace_id)?,
                        correlation_id: None,
                    })
                    .await
                    .map_err(|error| storage_error(&error, RuleErrorStage::Execution, trace_id))?;
                let record = receipt.record;
                let source = receipt.installed_snapshot;
                let validation =
                    validate_live_receipt(&record, &source, &request.source_id, trace_id)
                        .and_then(|()| {
                            if source.plan.intent_entries().contains_key(&request.intent) {
                                Ok(())
                            } else {
                                Err(RuleError::new(
                                    RuleErrorStage::Execution,
                                    "unsupported_intent",
                                    "已安装来源未声明该标准意图",
                                    trace_id,
                                    false,
                                    Vec::new(),
                                ))
                            }
                        })
                        .and_then(|()| {
                            self.state
                                .runtime
                                .validate_plan(&source.plan)
                                .map_err(|error| runtime_error(&error, trace_id))
                        });
                if let Err(error) = validation {
                    return Err(self
                        .finish_started_execution_failure(
                            execution_id,
                            record.revision,
                            error,
                            trace_id,
                        )
                        .await);
                }
                Ok(Self::live_execution_snapshot(source, record))
            }
            ExecutionMode::Replay {
                execution_id: archived_execution_id,
            } => {
                self.start_replay_snapshot(request, execution_id, archived_execution_id, trace_id)
                    .await
            }
        }
    }

    async fn start_replay_snapshot(
        &self,
        request: &ExecuteRequest,
        execution_id: Uuid,
        archived_execution_id: ExecutionId,
        trace_id: &str,
    ) -> Result<(ExecutionSnapshot, ExecutionRecord), RuleError> {
        let archived = self
            .state
            .storage
            .get_execution(archived_execution_id.as_uuid())
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Replay, trace_id))?
            .ok_or_else(|| {
                RuleError::new(
                    RuleErrorStage::Replay,
                    "replay_execution_missing",
                    "历史 execution 不存在",
                    trace_id,
                    false,
                    Vec::new(),
                )
            })?;
        if archived.status != ExecutionStatus::Completed {
            return Err(RuleError::new(
                RuleErrorStage::Replay,
                "replay_execution_not_completed",
                "历史 execution 未以可 replay 的完成终态结束",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        if !archived.replayable {
            return Err(RuleError::new(
                RuleErrorStage::Replay,
                "replay_revision_unavailable",
                "历史 execution 无法唯一固定来源 revision",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        let pin = self
            .state
            .storage
            .load_execution_replay_pin(archived_execution_id.as_uuid())
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Replay, trace_id))?;
        replay_snapshot(&request.source_id, archived_execution_id, &pin, trace_id)?;
        if !pin.plan.intent_entries().contains_key(&request.intent) {
            return Err(RuleError::new(
                RuleErrorStage::Replay,
                "unsupported_pinned_intent",
                "历史 execution 的固定 Plan 未声明该标准意图",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        self.state
            .runtime
            .validate_plan(&pin.plan)
            .map_err(|error| runtime_error(&error, trace_id))?;
        let replay_continue_actions = if LegadoImporter::owns_source(&pin.source_identity) {
            Some(
                self.load_replay_continue_actions(archived_execution_id, trace_id)
                    .await?,
            )
        } else {
            None
        };
        let record = self
            .state
            .storage
            .start_replay_execution(ReplayExecutionStart {
                execution_id,
                pin: pin.clone(),
                event_id: Uuid::new_v4(),
                trace_id: trace_id.to_string(),
                started_at_ms: now_millis(trace_id)?,
                correlation_id: Some(archived_execution_id.as_uuid()),
            })
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Replay, trace_id))?;
        Ok((
            ExecutionSnapshot {
                source_identity: pin.source_identity,
                plan: pin.plan,
                grant: pin.grant,
                base_url: pin.base_url,
                credentials: HttpExecutionCredentials::default(),
                mode: pin.mode,
                replay_continue_actions,
            },
            record,
        ))
    }

    fn live_execution_snapshot(
        source: InstalledSourceSnapshot,
        record: ExecutionRecord,
    ) -> (ExecutionSnapshot, ExecutionRecord) {
        let InstalledSourceSnapshot {
            source_identity,
            plan,
            grant,
            base_url,
            runtime_credentials,
            ..
        } = source;
        let credentials = HttpExecutionCredentials::from_source_secret(
            runtime_credentials.cookie_namespace().to_string(),
            runtime_credentials.into_secret_bytes(),
        );
        (
            ExecutionSnapshot {
                source_identity,
                plan,
                grant,
                base_url,
                credentials,
                mode: RuntimeExecutionMode::Live,
                replay_continue_actions: None,
            },
            record,
        )
    }

    async fn finish_started_execution_failure(
        &self,
        execution_id: Uuid,
        expected_version: u64,
        failure: RuleError,
        trace_id: &str,
    ) -> RuleError {
        let Ok(finished_at_ms) = now_millis(trace_id) else {
            return failure;
        };
        match self
            .state
            .storage
            .finish_execution(ExecutionFinish {
                execution_id,
                expected_version,
                event_id: Uuid::new_v4(),
                status: ExecutionStatus::Failed,
                finished_at_ms,
                trace_id: trace_id.to_string(),
            })
            .await
        {
            Ok(_) => failure,
            Err(error) => storage_error(&error, RuleErrorStage::Persistence, trace_id),
        }
    }

    fn normalize_continue_action(
        request: &mut ExecuteRequest,
        trace_id: &str,
    ) -> Result<(), RuleError> {
        if request.intent != lj_capability::StandardIntent::ContinueAction
            || !LegadoImporter::owns_source(request.source_id.as_identity())
        {
            return Ok(());
        }
        request.input = LegadoImporter::consume_continue_action(
            &request.input,
            request.source_id.as_identity(),
            now_millis(trace_id)?,
        )
        .map_err(|error| continue_action_error(error, RuleErrorStage::Execution, trace_id))?;
        Ok(())
    }

    async fn load_replay_continue_actions(
        &self,
        archived_execution_id: ExecutionId,
        trace_id: &str,
    ) -> Result<BTreeMap<String, Value>, RuleError> {
        let events = self
            .state
            .storage
            .catch_up_execution(archived_execution_id.as_uuid(), 0)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Replay, trace_id))?;
        let mut actions = BTreeMap::new();
        for event in events {
            if event.envelope.payload.get("kind").and_then(Value::as_str) != Some("delta") {
                continue;
            }
            let projection = event
                .envelope
                .payload
                .get("delta")
                .cloned()
                .ok_or_else(|| replay_delta_error("replay_delta_missing", trace_id))?;
            let projection = serde_json::from_value::<ProjectionDelta>(projection)
                .map_err(|_| replay_delta_error("replay_delta_invalid", trace_id))?;
            for action in projection.upserts.actions {
                if action.intent == lj_capability::StandardIntent::ContinueAction {
                    actions.insert(action.id.0, action.payload);
                }
            }
        }
        Ok(actions)
    }
}

fn replay_delta_error(code: &str, trace_id: &str) -> RuleError {
    RuleError::new(
        RuleErrorStage::Replay,
        code,
        "历史 execution 的 Delta archive 无效",
        trace_id,
        false,
        Vec::new(),
    )
}

fn validate_live_receipt(
    record: &ExecutionRecord,
    source: &InstalledSourceSnapshot,
    requested_source: &SourceId,
    trace_id: &str,
) -> Result<(), RuleError> {
    let valid = source.source_revision > 0
        && record.source_revision == source.source_revision
        && record.source_identity == source.source_identity
        && requested_source.as_identity() == source.source_identity
        && source.profile.id.0 == source.source_identity
        && source.package.source_identity().id == source.source_identity
        && source.package.definition().source_identity().id == source.source_identity
        && source.version == source.package.version()
        && source.plan.definition_hash() == source.version
        && source.plan.plan_hash() == record.plan_hash
        && source.package.definition().base_url() == source.base_url
        && !source.base_url.trim().is_empty();
    if valid {
        return Ok(());
    }
    Err(RuleError::new(
        RuleErrorStage::Execution,
        "execution_start_receipt_invalid",
        "execution start receipt 的来源 revision 快照不一致",
        trace_id,
        false,
        Vec::new(),
    ))
}
