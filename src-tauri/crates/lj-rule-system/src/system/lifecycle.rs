//! candidate → install → execute 生命周期。
//!
//! 该模块是 concrete façade 的三条命令实现。它只把 importer Definition 交给 compiler，
//! 将 candidate/source/execution 耐久状态委托给 C2，并将 immutable Plan 交给 C3 runtime。
//! 调用方不会接触 Graph、Plan JSON、processor registry 或 storage handle。
//!
//! 持久化不变量：candidate 先 durable stage；install 由 C2 原子消费 candidate 并固定 source
//! version/package/Plan/grant/credential namespace；execute 在 runtime 启动前先 durable `Started`，
//! 后续 session 只能通过 `session_delivery` 提交并 delivery。

use std::collections::BTreeMap;
use std::sync::Arc;

use futures::{StreamExt, stream};
use lj_compiler::{canonicalize, validate};
use lj_importer::legado::LegadoImporter;
use lj_importer::maccms::MaccmsImporter;
use lj_importer::{ImportDiagnostic, ImportedNativeRule};
use lj_media::{MediaResourceId, SourceProfile};
use lj_node_http::ImportFetchError;
use lj_rule_model::{Diagnostic, PolicyCapabilities, RulePackage, SourceSpan};
use lj_runtime::{
    ExecutionMode as RuntimeExecutionMode, HttpExecutionCredentials, PlanExecutionRequest,
};
use lj_storage::{
    CandidateDraft, CandidateSummary, ExecutionFinish, ExecutionRecord, ExecutionStart,
    ExecutionStatus, InstallCandidateRequest, InstalledSource as StorageInstalledSource,
    InstalledSourceSnapshot, ProjectionDelta, ReplayExecutionStart, RuntimeCredentialMaterial,
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

struct ExecutionSnapshot {
    source_identity: String,
    plan: lj_rule_model::ExecutionPlan,
    grant: PolicyCapabilities,
    base_url: String,
    credentials: HttpExecutionCredentials,
    mode: RuntimeExecutionMode,
    replay_continue_actions: Option<BTreeMap<String, Value>>,
}

fn import_diagnostic(diagnostic: &ImportDiagnostic) -> Diagnostic {
    Diagnostic {
        code: diagnostic.code.clone(),
        severity: diagnostic.severity,
        message: "第三方来源字段需要审阅".to_string(),
        span: Some(SourceSpan {
            start: diagnostic.byte_offset,
            end: diagnostic
                .byte_offset
                .saturating_add(diagnostic.byte_length),
            path: Some(diagnostic.path.clone()),
        }),
    }
}

fn import_fetch_error(error: ImportFetchError, trace_id: &str) -> RuleError {
    let (code, message) = match error {
        ImportFetchError::InvalidUrl => (
            "import_src_url_invalid",
            "仅支持不含用户凭据的 HTTP(S) 导入地址",
        ),
        ImportFetchError::TargetBlocked => {
            ("import_src_target_blocked", "导入地址未通过网络安全校验")
        }
        ImportFetchError::Timeout => ("import_src_timeout", "获取导入内容超时"),
        ImportFetchError::RequestFailed => ("import_src_request_failed", "无法安全获取导入内容"),
        ImportFetchError::RedirectInvalid => (
            "import_src_redirect_invalid",
            "导入地址重定向无效或超过上限",
        ),
        ImportFetchError::HttpStatus(_) => ("import_src_http_status", "远程地址返回非成功状态"),
        ImportFetchError::BodyTooLarge => ("import_src_body_too_large", "导入内容超过大小上限"),
        ImportFetchError::InvalidUtf8 => ("import_src_invalid_utf8", "导入内容不是有效 UTF-8"),
    };
    RuleError::new(
        RuleErrorStage::Import,
        code,
        message,
        trace_id,
        matches!(
            error,
            ImportFetchError::Timeout | ImportFetchError::RequestFailed
        ),
        Vec::new(),
    )
}

struct PreparedRuleInput {
    definition: lj_rule_model::RuleDefinition,
    runtime_credentials: Option<Vec<u8>>,
    display_title: Option<String>,
    display_group: Option<String>,
    diagnostics: Vec<Diagnostic>,
}

fn prepare_legado_input(source_json: &str, trace_id: &str) -> Result<PreparedRuleInput, RuleError> {
    let imported = LegadoImporter.import(source_json).map_err(|error| {
        let code = error.to_string();
        let diagnostics = error.diagnostics().iter().map(import_diagnostic).collect();
        RuleError::new(
            RuleErrorStage::Import,
            code,
            "Legado 输入未通过安全校验",
            trace_id,
            false,
            diagnostics,
        )
    })?;
    Ok(prepared_import(imported))
}

fn prepare_transient_rule_input(
    input: RuleInput,
    trace_id: &str,
) -> Result<PreparedRuleInput, RuleError> {
    match input {
        RuleInput::Legado { source_json } => prepare_legado_input(&source_json, trace_id),
        RuleInput::MaccmsJson { url } => {
            let imported = MaccmsImporter.import_url(&url).map_err(|error| {
                RuleError::new(
                    RuleErrorStage::Import,
                    error.to_string(),
                    "Maccms JSON endpoint URL 无效",
                    trace_id,
                    false,
                    error.diagnostics().iter().map(import_diagnostic).collect(),
                )
            })?;
            Ok(prepared_import(imported))
        }
    }
}

fn prepared_import(mut imported: ImportedNativeRule) -> PreparedRuleInput {
    let runtime_credentials = imported
        .take_credentials()
        .map(lj_importer::ImportCredentialMaterial::into_bytes);
    PreparedRuleInput {
        definition: imported.definition,
        runtime_credentials,
        display_title: imported.display_title,
        display_group: imported.display_group,
        diagnostics: imported.diagnostics.iter().map(import_diagnostic).collect(),
    }
}

impl RuleSystem {
    /// 通过受限 HTTP transport 拉取一次性导入文本。
    ///
    /// 此入口复用 production SSRF、逐跳 redirect/DNS 校验、30 秒超时与 2 MiB body 上限；
    /// Tauri root 不直接依赖 node adapter。返回值只在调用栈中短暂存在，不进入文档生命周期。
    ///
    /// # Errors
    ///
    /// URL、目标地址、redirect、HTTP 状态、响应大小或 UTF-8 校验失败时返回安全
    /// [`RuleError`]。
    pub async fn fetch_import_source(url: &str) -> Result<String, RuleError> {
        let trace_id = super::trace_id();
        lj_node_http::fetch_import_source(url)
            .await
            .map_err(|error| import_fetch_error(error, &trace_id))
    }

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
        let prepared = prepare_transient_rule_input(input, &trace_id)?;
        let expected_installed_revision = self
            .state
            .storage
            .get_installed_source(prepared.definition.source_identity().id.clone())
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Candidate, &trace_id))?
            .map_or(0, |source| source.source_revision);
        self.stage_prepared_candidate(prepared, expected_installed_revision, &trace_id)
            .await
    }

    async fn stage_prepared_candidate(
        &self,
        prepared: PreparedRuleInput,
        expected_installed_revision: u64,
        trace_id: &str,
    ) -> Result<InstallCandidate, RuleError> {
        let PreparedRuleInput {
            definition,
            runtime_credentials,
            display_title,
            display_group,
            diagnostics: import_diagnostics,
        } = prepared;
        let definition = canonicalize(&definition);
        let mut diagnostics = validate(&definition);
        diagnostics.extend(import_diagnostics);
        let plan = self
            .state
            .compiler
            .compile(&definition)
            .map_err(|error| compiler_error(&error, trace_id))?;
        // Candidate staging remains the single RuleSystem support gate. Compiler success is not
        // enough on its own: the exact runtime configuration must accept the sealed current Plan
        // before storage can make it installable.
        if let Err(error) = self.state.runtime.validate_plan(&plan) {
            let mapped = runtime_error(&error, trace_id);
            return Err(RuleError::new(
                RuleErrorStage::Candidate,
                mapped.code,
                mapped.message,
                trace_id,
                mapped.retryable,
                diagnostics,
            ));
        }
        let now = now_millis(trace_id)?;
        let expires_at_ms = now
            .checked_add(self.state.candidate_ttl_ms)
            .ok_or_else(|| {
                RuleError::new(
                    RuleErrorStage::Candidate,
                    "candidate_expiry_overflow",
                    "candidate 到期时刻超出支持范围",
                    trace_id,
                    false,
                    Vec::new(),
                )
            })?;
        let profile = source_profile(
            &definition,
            plan.definition_hash(),
            display_title,
            display_group,
        );
        let required_grant = definition.capability_manifest().required.clone();
        let package = RulePackage::new(
            definition.source_identity().clone(),
            plan.definition_hash(),
            definition,
        );
        let summary = self
            .state
            .storage
            .stage_candidate(CandidateDraft {
                candidate_id: Uuid::new_v4(),
                package,
                plan,
                profile,
                required_grant,
                diagnostics,
                runtime_credentials: runtime_credentials.map(RuntimeCredentialMaterial::new),
                expected_installed_revision,
                expires_at_ms: Some(expires_at_ms),
                trace_id: trace_id.to_string(),
                correlation_id: None,
                created_at_ms: now,
            })
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Candidate, trace_id))?;
        Ok(candidate_from_summary(summary))
    }

    /// 原子消费 composite candidate；install 时不重新读取 current source 或 credential ref。
    ///
    /// # Errors
    ///
    /// candidate 缺失、过期、篡改、已消费、schema/source 基线变化、grant 不足或 C2
    /// install transaction 失败时返回 [`RuleError`]。
    pub async fn install(
        &self,
        candidate_id: CandidateId,
        grant: CapabilityGrant,
    ) -> Result<InstalledSource, RuleError> {
        let trace_id = super::trace_id();
        let installed = self
            .state
            .storage
            .install_candidate(InstallCandidateRequest {
                candidate_id: candidate_id.as_uuid(),
                grant: grant.policy().clone(),
                event_id: Uuid::new_v4(),
                trace_id: trace_id.clone(),
                occurred_at_ms: now_millis(&trace_id)?,
                correlation_id: None,
            })
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Install, &trace_id))?;
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

        let (delivery_sender, delivery_receiver) = mpsc::channel(self.state.session_event_capacity);
        let mut persisted_sequence = 0;
        // `Started` 必须先从 C2 catch-up/delivery，再允许 runtime 调度任何 effect。
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
            self.state.handlers.clone(),
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
        let cookie_namespace = runtime_credentials.cookie_namespace().to_string();
        let credentials = HttpExecutionCredentials::from_source_secret(
            cookie_namespace,
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
                                    trace_id.to_string(),
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
                if !archived.replayable || archived.replay_unavailable_reason.is_some() {
                    return Err(RuleError::new(
                        RuleErrorStage::Replay,
                        "replay_revision_unavailable",
                        "历史 execution 无法唯一固定来源 revision",
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
                if !pin.plan.intent_entries().contains_key(&request.intent) {
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
                        credentials: HttpExecutionCredentials::default(),
                        mode: pin.mode,
                        replay_continue_actions,
                    },
                    record,
                ))
            }
        }
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

fn validate_live_receipt(
    record: &ExecutionRecord,
    source: &InstalledSourceSnapshot,
    requested_source: &SourceId,
    trace_id: &str,
) -> Result<(), RuleError> {
    let revision_matches = source.source_revision > 0
        && record.source_revision == Some(source.source_revision)
        && record.source_identity == source.source_identity;
    let identity_matches = requested_source.as_identity() == source.source_identity
        && source.profile.id.0 == source.source_identity
        && source.package.source_identity().id == source.source_identity
        && source.package.definition().source_identity().id == source.source_identity;
    let package_matches = source.version == source.package.version()
        && source.plan.definition_hash() == source.version
        && source.plan.plan_hash() == record.plan_hash
        && source.package.definition().base_url() == source.base_url
        && !source.base_url.trim().is_empty();
    if revision_matches && identity_matches && package_matches {
        return Ok(());
    }
    Err(RuleError::new(
        RuleErrorStage::Execution,
        "execution_start_receipt_invalid",
        "execution start receipt 的来源 revision 快照不一致",
        trace_id.to_string(),
        false,
        Vec::new(),
    ))
}

fn candidate_from_summary(summary: CandidateSummary) -> InstallCandidate {
    InstallCandidate {
        id: CandidateId::from_uuid(summary.candidate_id),
        expected_installed_revision: summary.expected_installed_revision,
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
        revision: source.source_revision,
    }
}

fn source_profile(
    definition: &lj_rule_model::RuleDefinition,
    version: &str,
    display_title: Option<String>,
    display_group: Option<String>,
) -> SourceProfile {
    SourceProfile {
        id: MediaResourceId(definition.source_identity().id.clone()),
        title: display_title
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| definition.base_url().to_string()),
        icon_url: None,
        version: Some(version.to_string()),
        group: display_group,
        supported_intents: definition.intent_exports().keys().copied().collect(),
        risk_notes: vec!["该来源可能按已声明的 capability 发起外部请求".to_string()],
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::sync::Once;

    use futures::StreamExt as _;
    use keyring_core::{mock, set_default_store};
    use lj_capability::{IntentExport, IntentInput, StandardIntent};
    use lj_rule_model::definition::MapperOutputKind;
    use lj_rule_model::{
        CapabilityManifest, ConditionConfig, ControlExpression, ControlledMapper, FlowEdge,
        FlowGraph, FlowNode, FlowNodeConfig, FlowPortRef, JsConfig, JsOutputKind,
        LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE, MERGE_OUTPUT_HANDLE, MergeConfig, MergeInput,
        MergeInputActivation, MergeStrategy, RuleDefinition, SourceIdentity, SystemCapabilities,
    };

    use super::*;
    use crate::{CapabilityGrant, ExecutionEventKind, RuleSystemConfig};

    fn init_mock_keyring() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            set_default_store(mock::Store::new().expect("keyring-core mock store"));
        });
    }

    fn current_control_definition() -> RuleDefinition {
        let entry = Uuid::from_u128(2_001);
        let condition = Uuid::from_u128(2_002);
        let alpha = Uuid::from_u128(2_003);
        let beta = Uuid::from_u128(2_004);
        let merge = Uuid::from_u128(2_005);
        let mapper = Uuid::from_u128(2_006);
        RuleDefinition::new(
            SourceIdentity {
                id: "source:rule-system-control-test".to_string(),
            },
            "https://example.invalid",
            BTreeMap::from([(
                StandardIntent::Search,
                IntentExport::new(entry, mapper),
            )]),
            FlowGraph {
                nodes: vec![
                    FlowNode::new(
                        entry,
                        FlowNodeConfig::Js(JsConfig {
                            code: "JSON.stringify([{ enabled: true, title: '入口', url: 'https://example.invalid/entry' }])".to_string(),
                            output: JsOutputKind::Json,
                        }),
                    ),
                    FlowNode::new(
                        condition,
                        FlowNodeConfig::Condition(ConditionConfig {
                            branches: vec!["alpha".to_string(), "beta".to_string()],
                            expression: ControlExpression::Js {
                                code: "'alpha'".to_string(),
                            },
                        }),
                    ),
                    FlowNode::new(
                        alpha,
                        FlowNodeConfig::Js(JsConfig {
                            code: "JSON.stringify([{ title: '命中', url: 'https://example.invalid/alpha' }])".to_string(),
                            output: JsOutputKind::Json,
                        }),
                    ),
                    FlowNode::new(
                        beta,
                        FlowNodeConfig::Js(JsConfig {
                            code: "JSON.stringify([{ title: '未命中', url: 'https://example.invalid/beta' }])".to_string(),
                            output: JsOutputKind::Json,
                        }),
                    ),
                    FlowNode::new(
                        merge,
                        FlowNodeConfig::Merge(MergeConfig {
                            inputs: vec![
                                MergeInput {
                                    input_id: "alpha".to_string(),
                                    handle: "alpha".to_string(),
                                    order: 0,
                                    activation: MergeInputActivation::Required,
                                },
                                MergeInput {
                                    input_id: "beta".to_string(),
                                    handle: "beta".to_string(),
                                    order: 1,
                                    activation: MergeInputActivation::Optional,
                                },
                            ],
                            strategy: MergeStrategy::SingleActive,
                        }),
                    ),
                    FlowNode::new(
                        mapper,
                        FlowNodeConfig::Mapper(ControlledMapper {
                            output: MapperOutputKind::Items,
                            identity_fields: vec!["url".to_string()],
                        }),
                    ),
                ],
                edges: vec![
                    FlowEdge::new(
                        FlowPortRef::new(entry, LINEAR_OUTPUT_HANDLE),
                        FlowPortRef::new(condition, lj_rule_model::CONDITION_INPUT_HANDLE),
                    ),
                    FlowEdge::new(
                        FlowPortRef::new(condition, "alpha"),
                        FlowPortRef::new(alpha, LINEAR_INPUT_HANDLE),
                    ),
                    FlowEdge::new(
                        FlowPortRef::new(condition, "beta"),
                        FlowPortRef::new(beta, LINEAR_INPUT_HANDLE),
                    ),
                    FlowEdge::new(
                        FlowPortRef::new(alpha, LINEAR_OUTPUT_HANDLE),
                        FlowPortRef::new(merge, "alpha"),
                    ),
                    FlowEdge::new(
                        FlowPortRef::new(beta, LINEAR_OUTPUT_HANDLE),
                        FlowPortRef::new(merge, "beta"),
                    ),
                    FlowEdge::new(
                        FlowPortRef::new(merge, MERGE_OUTPUT_HANDLE),
                        FlowPortRef::new(mapper, LINEAR_INPUT_HANDLE),
                    ),
                ],
            },
            CapabilityManifest {
                required: PolicyCapabilities {
                    network: true,
                    system: SystemCapabilities::default(),
                },
            },
            vec!["url".to_string()],
        )
    }

    #[tokio::test]
    async fn current_control_candidate_passes_gate_and_installs_executes_and_replays() {
        init_mock_keyring();
        let root = std::env::temp_dir().join(format!("lj-rule-system-control-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create RuleSystem control fixture root");
        let system = RuleSystem::open(
            RuleSystemConfig::desktop(root.join("event-store.db"), root.join("artifacts"))
                .with_keyring_service(format!(
                    "lanjing.rule-system.control.test.{}",
                    Uuid::new_v4()
                )),
        )
        .await
        .expect("open RuleSystem control fixture");
        let candidate = system
            .stage_prepared_candidate(
                PreparedRuleInput {
                    definition: current_control_definition(),
                    runtime_credentials: None,
                    display_title: Some("控制流测试".to_string()),
                    display_group: None,
                    diagnostics: Vec::new(),
                },
                0,
                "trace-control-candidate",
            )
            .await
            .expect("current control candidate stages");
        assert!(
            candidate
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != "RUNTIME_CONTROL_UNAVAILABLE")
        );
        let installed = system
            .install(candidate.id, CapabilityGrant::network_only())
            .await
            .expect("current control candidate installs");

        let live = system
            .execute(ExecuteRequest {
                source_id: installed.source_id.clone(),
                intent: StandardIntent::Search,
                input: IntentInput::Query("control".to_string()),
                mode: ExecutionMode::Live,
            })
            .await
            .expect("current control live session starts");
        let live_execution_id = live.id;
        let live_events = live.into_events().collect::<Vec<_>>().await;
        assert!(
            matches!(
                live_events.last().map(|event| &event.kind),
                Some(ExecutionEventKind::Completed)
            ),
            "current control live events: {live_events:?}"
        );
        assert!(
            live_events
                .iter()
                .any(|event| matches!(event.kind, ExecutionEventKind::EffectCaptured { .. }))
        );

        let replay = system
            .execute(ExecuteRequest {
                source_id: installed.source_id,
                intent: StandardIntent::Search,
                input: IntentInput::Query("control".to_string()),
                mode: ExecutionMode::Replay {
                    execution_id: live_execution_id,
                },
            })
            .await
            .expect("current control replay session starts");
        let replay_events = replay.into_events().collect::<Vec<_>>().await;
        assert!(matches!(
            replay_events.last().map(|event| &event.kind),
            Some(ExecutionEventKind::Completed)
        ));
        assert!(
            replay_events
                .iter()
                .all(|event| !matches!(event.kind, ExecutionEventKind::EffectCaptured { .. }))
        );
        drop(system);
        let _ = fs::remove_dir_all(root);
    }
}
