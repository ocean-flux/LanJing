//! candidate → install → execute 生命周期。
//!
//! 该模块是 concrete façade 的三条命令实现。它只把 importer Definition 交给 compiler，
//! 将 candidate/source/execution 耐久状态委托给 C2，并将 immutable Plan 交给 C3 runtime。
//! 调用方不会接触 Graph、Plan JSON、processor registry 或 storage handle。
//!
//! 持久化不变量：candidate 先 durable stage；install 由 C2 原子消费 candidate 并固定 source
//! version/package/Plan/grant/credential namespace；execute 在 runtime 启动前先 durable `Started`，
//! 后续 session 只能通过 `session_delivery` 提交并 delivery。

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::Arc;

use futures::{StreamExt, stream};
use lj_compiler::{canonicalize, validate};
use lj_importer::legado::{LegadoImporter, LegadoSourceJson};
use lj_importer::maccms::{MaccmsFormat, MaccmsImporter, MaccmsSourceUrl};
use lj_media::{MediaResourceId, SourceProfile};
use lj_rule_model::{PolicyCapabilities, RulePackage};
use lj_runtime::{
    ExecutionMode as RuntimeExecutionMode, HttpExecutionCredentials, PlanExecutionRequest,
};
use lj_storage::{
    CandidateDraft, CandidateSummary, ExecutionFinish, ExecutionStart, ExecutionStatus,
    InstalledSource as StorageInstalledSource, ProjectionDelta, ReplayExecutionStart,
    SourceCredentialInput,
};
use serde_json::Value;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::error_mapping::{compiler_error, runtime_error, storage_error};
use super::session_delivery::{
    SessionRun, continue_action_error, flush_persisted, replay_snapshot, run_session,
};
use super::{RuleSystem, lock, now_millis};
use crate::{
    CandidateId, CapabilityGrant, ExecuteRequest, ExecutionId, ExecutionMode, ExecutionSession,
    InstallCandidate, InstalledSource, RuleError, RuleErrorStage, RuleInput, SourceId,
};

/// 只缓存已安装来源的 Plan，避免启动时反序列化全部来源。
pub(super) struct PlanCache {
    capacity: usize,
    entries: HashMap<String, StorageInstalledSource>,
    order: VecDeque<String>,
}

impl PlanCache {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub(super) fn get(&mut self, source_identity: &str) -> Option<StorageInstalledSource> {
        let source = self.entries.get(source_identity).cloned()?;
        self.touch(source_identity);
        Some(source)
    }

    pub(super) fn insert(&mut self, source: StorageInstalledSource) {
        let source_identity = source.source_identity.clone();
        self.entries.insert(source_identity.clone(), source);
        self.touch(&source_identity);
        while self.entries.len() > self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    fn touch(&mut self, source_identity: &str) {
        if let Some(position) = self.order.iter().position(|value| value == source_identity) {
            self.order.remove(position);
        }
        self.order.push_back(source_identity.to_string());
    }
}

struct ExecutionSnapshot {
    source_identity: String,
    plan: lj_rule_model::ExecutionPlan,
    grant: PolicyCapabilities,
    base_url: String,
    mode: RuntimeExecutionMode,
    replay_continue_actions: Option<BTreeMap<String, Value>>,
}

impl RuleSystem {
    /// 来源输入生成 durable candidate，但不安装来源或执行网络 effect。
    ///
    /// `prepare_install` 只执行 importer、canonicalization、validation 与 compiler；不会访问来源
    /// 站点。返回 DTO 不含 Definition、Plan 或 Graph JSON。
    ///
    /// # Errors
    ///
    /// 来源格式无效、Definition/Plan 校验或编译失败，或 C2 candidate staging 失败时返回
    /// [`RuleError`]。
    pub async fn prepare_install(&self, input: RuleInput) -> Result<InstallCandidate, RuleError> {
        let trace_id = super::trace_id();
        let (definition, credential_snapshot_bytes) = match input {
            RuleInput::MaccmsJson { url } => (
                MaccmsImporter
                    .definition(&MaccmsSourceUrl {
                        url,
                        at: MaccmsFormat::Json,
                    })
                    .map_err(|_| {
                        RuleError::new(
                            RuleErrorStage::Import,
                            "maccms_definition_invalid",
                            "Maccms JSON 来源输入无效",
                            trace_id.clone(),
                            false,
                            Vec::new(),
                        )
                    })?,
                None,
            ),
            RuleInput::Legado { source_json } => {
                let source =
                    serde_json::from_str::<LegadoSourceJson>(&source_json).map_err(|_| {
                        RuleError::new(
                            RuleErrorStage::Import,
                            "legado_source_invalid",
                            "Legado 书源 JSON 无效",
                            trace_id.clone(),
                            false,
                            Vec::new(),
                        )
                    })?;
                let adapted = LegadoImporter.adapt(&source).map_err(|_| {
                    RuleError::new(
                        RuleErrorStage::Import,
                        "legado_definition_invalid",
                        "Legado 书源无法转换为 Definition",
                        trace_id.clone(),
                        false,
                        Vec::new(),
                    )
                })?;
                let credential_snapshot_bytes =
                    adapted.credential_snapshot_bytes().map_err(|_| {
                        RuleError::new(
                            RuleErrorStage::Import,
                            "legado_credentials_invalid",
                            "Legado 静态凭证无法安全快照",
                            trace_id.clone(),
                            false,
                            Vec::new(),
                        )
                    })?;
                (adapted.definition, credential_snapshot_bytes)
            }
        };
        let definition = canonicalize(&definition);
        let diagnostics = validate(&definition);
        let plan = self
            .state
            .compiler
            .compile(&definition)
            .map_err(|error| compiler_error(&error, &trace_id))?;
        let now = now_millis(&trace_id)?;
        let expires_at_ms = now
            .checked_add(self.state.candidate_ttl_ms)
            .ok_or_else(|| {
                RuleError::new(
                    RuleErrorStage::Candidate,
                    "candidate_expiry_overflow",
                    "candidate 到期时刻超出支持范围",
                    trace_id.clone(),
                    false,
                    Vec::new(),
                )
            })?;
        let profile = source_profile(&definition, &plan.definition_hash);
        let required_grant = definition.capability_manifest.required.clone();
        let candidate_id = Uuid::new_v4();
        let package = RulePackage {
            schema_version: 1,
            source_identity: definition.source_identity.clone(),
            version: plan.definition_hash.clone(),
            definition,
        };
        let summary = self
            .state
            .storage
            .stage_candidate(CandidateDraft {
                candidate_id,
                package,
                plan,
                profile,
                required_grant,
                diagnostics,
                expires_at_ms: Some(expires_at_ms),
                trace_id: trace_id.clone(),
                correlation_id: None,
                created_at_ms: now,
            })
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Candidate, &trace_id))?;
        if let Some(secret_bytes) = credential_snapshot_bytes {
            self.state
                .storage
                .stage_source_credentials(SourceCredentialInput {
                    candidate_id,
                    source_identity: summary.source_identity.clone(),
                    secret_bytes,
                    created_at_ms: now,
                })
                .await
                .map_err(|error| storage_error(&error, RuleErrorStage::Candidate, &trace_id))?;
        }
        lock(&self.state.candidates).insert(candidate_id, summary.clone());
        Ok(candidate_from_summary(summary))
    }

    /// 重读并重验 durable candidate，原子安装稳定来源版本与 immutable Plan。
    ///
    /// C2 负责在一个安装 transaction 中消费 candidate、固定 package/Plan/grant/profile 及可选
    /// credential namespace；本地 preview cache 只用于侦测同进程篡改，绝不是安装真相。
    ///
    /// # Errors
    ///
    /// candidate 缺失、过期、篡改、已消费、hash 不一致、grant 不足、source revision 冲突或 C2
    /// 原子安装失败时返回 [`RuleError`]。
    pub async fn install(
        &self,
        candidate_id: CandidateId,
        grant: CapabilityGrant,
    ) -> Result<InstalledSource, RuleError> {
        let trace_id = super::trace_id();
        let candidate_uuid = candidate_id.as_uuid();
        let summary = self
            .state
            .storage
            .get_candidate_summary(candidate_uuid)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Candidate, &trace_id))?
            .ok_or_else(|| {
                RuleError::new(
                    RuleErrorStage::Candidate,
                    "candidate_missing",
                    "candidate 不存在或不可读取",
                    trace_id.clone(),
                    false,
                    Vec::new(),
                )
            })?;
        if let Some(cached) = lock(&self.state.candidates).get(&candidate_uuid)
            && cached != &summary
        {
            return Err(RuleError::new(
                RuleErrorStage::Candidate,
                "candidate_preview_mismatch",
                "candidate 安全预览与 durable staging 不一致",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        if summary.expires_at_ms <= now_millis(&trace_id)? {
            return Err(RuleError::new(
                RuleErrorStage::Candidate,
                "candidate_expired",
                "candidate 已过期",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        if !grant.covers(&summary.required_grant) {
            return Err(RuleError::new(
                RuleErrorStage::Capability,
                "grant_insufficient",
                "批准的 capability 未覆盖来源所需能力",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        let source_credentials = self
            .state
            .storage
            .get_candidate_source_credentials_ref(candidate_uuid)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Install, &trace_id))?;
        let expected_source_version = self
            .state
            .storage
            .get_installed_source(summary.source_identity.clone())
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Install, &trace_id))?
            .map_or(0, |source| source.revision);
        let installed = self
            .state
            .storage
            .install_candidate(lj_storage::InstallCandidateRequest {
                candidate_id: candidate_uuid,
                grant: grant.policy().clone(),
                source_credentials,
                expected_source_version,
                event_id: Uuid::new_v4(),
                trace_id: trace_id.clone(),
                occurred_at_ms: now_millis(&trace_id)?,
                correlation_id: None,
            })
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Install, &trace_id))?;
        lock(&self.state.candidates).remove(&candidate_uuid);
        lock(&self.state.plans).insert(installed.clone());
        Ok(installed_source_from_storage(installed))
    }

    /// 仅用已安装 source ID、标准 intent/input 与 execution mode 启动一个持久 session。
    ///
    /// 启动前失败直接返回错误；启动成功后所有错误由 `session_delivery` 写为唯一 `Failed`
    /// terminal。delivery stream 被丢弃不隐式取消 execution；显式 cancellation handle 才会
    /// 请求 runtime 停止后续 effect。
    ///
    /// # Errors
    ///
    /// source 未安装、intent 未导出、live Plan 无效、replay pin 缺失/篡改/GC，或 C2 execution
    /// start 失败时返回 [`RuleError`]。
    pub async fn execute(
        &self,
        mut request: ExecuteRequest,
    ) -> Result<ExecutionSession, RuleError> {
        let trace_id = super::trace_id();
        Self::normalize_continue_action(&mut request, &trace_id)?;
        let execution_uuid = Uuid::new_v4();
        let (snapshot, record) = self
            .start_execution_snapshot(&request, execution_uuid, &trace_id)
            .await?;
        let credentials = if matches!(&request.mode, ExecutionMode::Live) {
            self.load_execution_http_credentials(execution_uuid, record.revision, &trace_id)
                .await?
        } else {
            HttpExecutionCredentials::default()
        };

        let (delivery_sender, delivery_receiver) = mpsc::channel(self.state.session_event_capacity);
        let mut persisted_sequence = 0;
        // `Started` 必须先从 C2 catch-up/delivery，再允许 runtime 调度任何 effect。
        flush_persisted(
            &self.state.storage,
            execution_uuid,
            &mut persisted_sequence,
            &delivery_sender,
            &trace_id,
        )
        .await?;
        if persisted_sequence != record.revision {
            return Err(RuleError::new(
                RuleErrorStage::Persistence,
                "execution_start_sequence_mismatch",
                "execution start 的持久序列不连续",
                trace_id,
                false,
                Vec::new(),
            ));
        }

        let replay_continue_actions = snapshot.replay_continue_actions;
        let runtime_session = self
            .state
            .runtime
            .execute(
                PlanExecutionRequest {
                    execution_id: execution_uuid,
                    source_id: snapshot.source_identity.clone(),
                    trace_id: trace_id.clone(),
                    plan: snapshot.plan,
                    intent: request.intent,
                    input: request.input,
                    mode: snapshot.mode,
                    capabilities: snapshot.grant,
                    base_url: snapshot.base_url,
                    credentials,
                },
                self.state.handlers.clone(),
                Arc::new(self.state.storage.clone()),
            )
            .map_err(|error| runtime_error(&error, &trace_id))?;
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
                    source_identity: snapshot.source_identity,
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
    ) -> Result<(ExecutionSnapshot, lj_storage::ExecutionRecord), RuleError> {
        match request.mode {
            ExecutionMode::Live => {
                let source = self.load_live_source(&request.source_id, trace_id).await?;
                if !source.plan.intent_entries.contains_key(&request.intent) {
                    return Err(RuleError::new(
                        RuleErrorStage::Execution,
                        "unsupported_intent",
                        "已安装来源未声明该标准意图",
                        trace_id.to_string(),
                        false,
                        Vec::new(),
                    ));
                }
                self.state
                    .runtime
                    .validate_plan(&source.plan)
                    .map_err(|error| runtime_error(&error, trace_id))?;
                let record = self
                    .state
                    .storage
                    .start_execution(ExecutionStart {
                        execution_id,
                        source_identity: source.source_identity.clone(),
                        event_id: Uuid::new_v4(),
                        trace_id: trace_id.to_string(),
                        started_at_ms: now_millis(trace_id)?,
                        correlation_id: None,
                    })
                    .await
                    .map_err(|error| storage_error(&error, RuleErrorStage::Execution, trace_id))?;
                Ok((
                    ExecutionSnapshot {
                        source_identity: source.source_identity,
                        plan: source.plan,
                        grant: source.grant,
                        base_url: source.package.definition.base_url,
                        mode: RuntimeExecutionMode::Live,
                        replay_continue_actions: None,
                    },
                    record,
                ))
            }
            ExecutionMode::Replay {
                execution_id: archived_execution_id,
            } => {
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
                            trace_id.to_string(),
                            false,
                            Vec::new(),
                        )
                    })?;
                if archived.status != ExecutionStatus::Completed {
                    return Err(RuleError::new(
                        RuleErrorStage::Replay,
                        "replay_execution_not_completed",
                        "历史 execution 未以可 replay 的完成终态结束",
                        trace_id.to_string(),
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
                if !pin.plan.intent_entries.contains_key(&request.intent) {
                    return Err(RuleError::new(
                        RuleErrorStage::Replay,
                        "unsupported_pinned_intent",
                        "历史 execution 的固定 Plan 未声明该标准意图",
                        trace_id.to_string(),
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
                        mode: pin.mode,
                        replay_continue_actions,
                    },
                    record,
                ))
            }
        }
    }

    async fn load_live_source(
        &self,
        source_id: &SourceId,
        trace_id: &str,
    ) -> Result<StorageInstalledSource, RuleError> {
        if let Some(source) = lock(&self.state.plans).get(source_id.as_identity()) {
            return Ok(source);
        }
        let source = self
            .state
            .storage
            .get_installed_source(source_id.as_identity().to_string())
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Execution, trace_id))?
            .ok_or_else(|| {
                RuleError::new(
                    RuleErrorStage::Execution,
                    "source_not_installed",
                    "来源尚未安装",
                    trace_id.to_string(),
                    false,
                    Vec::new(),
                )
            })?;
        lock(&self.state.plans).insert(source.clone());
        Ok(source)
    }

    /// 仅在 live execution start 后解密其 source credential snapshot。
    ///
    /// replay 绝不能调用此方法；C2 会在建立 replay execution 时验证 Secret Artifact，而
    /// `PlanRuntime` 只消费 archive effect output。
    async fn load_execution_http_credentials(
        &self,
        execution_id: Uuid,
        expected_version: u64,
        trace_id: &str,
    ) -> Result<HttpExecutionCredentials, RuleError> {
        match self
            .state
            .storage
            .load_execution_source_credentials(execution_id)
            .await
        {
            Ok(credentials) => {
                let cookie_namespace = credentials.cookie_namespace().to_string();
                Ok(HttpExecutionCredentials::from_source_secret(
                    cookie_namespace,
                    credentials.into_secret_bytes(),
                ))
            }
            Err(storage_failure) => {
                let error = storage_error(&storage_failure, RuleErrorStage::Execution, trace_id);
                self.state
                    .storage
                    .finish_execution(ExecutionFinish {
                        execution_id,
                        expected_version,
                        event_id: Uuid::new_v4(),
                        status: ExecutionStatus::Failed,
                        finished_at_ms: now_millis(trace_id)?,
                        trace_id: trace_id.to_string(),
                    })
                    .await
                    .map_err(|finish_failure| {
                        storage_error(&finish_failure, RuleErrorStage::Persistence, trace_id)
                    })?;
                Err(error)
            }
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
                .ok_or_else(|| {
                    RuleError::new(
                        RuleErrorStage::Replay,
                        "replay_delta_missing",
                        "历史 execution 的 Delta archive 无效",
                        trace_id.to_string(),
                        false,
                        Vec::new(),
                    )
                })?;
            let projection =
                serde_json::from_value::<ProjectionDelta>(projection).map_err(|_| {
                    RuleError::new(
                        RuleErrorStage::Replay,
                        "replay_delta_invalid",
                        "历史 execution 的 Delta archive 无效",
                        trace_id.to_string(),
                        false,
                        Vec::new(),
                    )
                })?;
            for action in projection.upserts.actions {
                if action.intent == lj_capability::StandardIntent::ContinueAction {
                    actions.insert(action.id.0, action.payload);
                }
            }
        }
        Ok(actions)
    }
}

fn candidate_from_summary(summary: CandidateSummary) -> InstallCandidate {
    InstallCandidate {
        id: CandidateId::from_uuid(summary.candidate_id),
        profile: summary.profile,
        required_grant: CapabilityGrant::from_policy(summary.required_grant),
        diagnostics: summary.diagnostics,
        definition_hash: summary.definition_hash,
        plan_hash: summary.plan_hash,
        expires_at_ms: summary.expires_at_ms,
    }
}

fn installed_source_from_storage(source: StorageInstalledSource) -> InstalledSource {
    InstalledSource {
        source_id: SourceId::from_identity(source.source_identity),
        version: source.version,
        profile: source.profile,
        revision: source.revision,
    }
}

fn source_profile(definition: &lj_rule_model::RuleDefinition, version: &str) -> SourceProfile {
    SourceProfile {
        id: MediaResourceId(definition.source_identity.id.clone()),
        title: definition.base_url.clone(),
        icon_url: None,
        version: Some(version.to_string()),
        supported_intents: definition.intent_exports.keys().copied().collect(),
        risk_notes: vec!["该来源可能按已声明的 capability 发起外部请求".to_string()],
    }
}
