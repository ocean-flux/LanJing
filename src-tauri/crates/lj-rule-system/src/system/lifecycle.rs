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
use lj_importer::authoring::{CredentialCodecError, CredentialSlotCodec};
use lj_importer::legado::{LegadoImporter, LegadoSourceJson};
use lj_importer::maccms::MaccmsImporter;
use lj_media::{MediaResourceId, SourceProfile};
use lj_rule_model::{
    AuthoringDiagnostic, CredentialTargetIdentity, Diagnostic, PolicyCapabilities, RulePackage,
    SourceSpan,
};
use lj_runtime::{
    ExecutionMode as RuntimeExecutionMode, HttpExecutionCredentials, PlanExecutionRequest,
};
use lj_storage::{
    CandidateDocumentInput, CandidateDraft, CandidateSummary, ExecutionFinish, ExecutionRecord,
    ExecutionStart, ExecutionStatus, InstallCandidateRequest,
    InstalledSource as StorageInstalledSource, InstalledSourceSnapshot, ProjectionDelta,
    ReplayExecutionStart, RuntimeCredentialMaterial, TransientSourceDocumentInput,
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
    CandidateId, CapabilityGrant, DocumentRef, ExecuteRequest, ExecutionId, ExecutionMode,
    ExecutionSession, InstallCandidate, InstalledSource, RuleError, RuleErrorStage, RuleInput,
    SourceDocumentFormat, SourceDocumentId, SourceId,
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

fn authoring_diagnostic(diagnostic: &AuthoringDiagnostic) -> Diagnostic {
    Diagnostic {
        code: diagnostic.code.clone(),
        severity: diagnostic.severity,
        message: "来源作者文档字段需要审阅".to_string(),
        span: Some(SourceSpan {
            start: diagnostic.byte_offset,
            end: diagnostic
                .byte_offset
                .saturating_add(diagnostic.byte_length),
            path: Some(diagnostic.path.clone()),
        }),
    }
}

struct PreparedRuleInput {
    definition: lj_rule_model::RuleDefinition,
    runtime_credentials: Option<Vec<u8>>,
    display_title: Option<String>,
    display_group: Option<String>,
    diagnostics: Vec<Diagnostic>,
}

fn prepare_rule_document(
    format: SourceDocumentFormat,
    raw_text: &str,
    trace_id: &str,
) -> Result<PreparedRuleInput, RuleError> {
    match format {
        SourceDocumentFormat::Legado => prepare_legado_input(raw_text, trace_id),
        SourceDocumentFormat::Maccms10Endpoint => prepare_maccms_input(raw_text, trace_id),
    }
}

fn prepare_legado_input(source_json: &str, trace_id: &str) -> Result<PreparedRuleInput, RuleError> {
    let imported = LegadoImporter
        .import_document(source_json)
        .map_err(|error| {
            let code = error
                .diagnostics()
                .first()
                .map_or("legado_source_invalid", |diagnostic| {
                    diagnostic.code.as_str()
                })
                .to_string();
            let diagnostics = error
                .diagnostics()
                .iter()
                .map(authoring_diagnostic)
                .collect();
            RuleError::new(
                RuleErrorStage::Import,
                code,
                "Legado 作者文档未通过安全校验",
                trace_id,
                false,
                diagnostics,
            )
        })?;
    let source = serde_json::from_str::<LegadoSourceJson>(source_json).map_err(|_| {
        RuleError::new(
            RuleErrorStage::Import,
            "legado_source_invalid",
            "Legado 书源 JSON 无效",
            trace_id,
            false,
            Vec::new(),
        )
    })?;
    let display_title = {
        let name = source.book_source_name.trim();
        (!name.is_empty()).then(|| name.to_string())
    };
    let display_group = source
        .book_source_group
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let diagnostics = imported
        .diagnostics
        .iter()
        .map(authoring_diagnostic)
        .collect();
    let runtime_credentials = imported.adapted.credential_snapshot_bytes().map_err(|_| {
        RuleError::new(
            RuleErrorStage::Import,
            "legado_credentials_invalid",
            "Legado 静态凭证无法安全快照",
            trace_id,
            false,
            Vec::new(),
        )
    })?;
    Ok(PreparedRuleInput {
        definition: imported.adapted.definition,
        runtime_credentials,
        display_title,
        display_group,
        diagnostics,
    })
}

fn prepare_maccms_input(raw_text: &str, trace_id: &str) -> Result<PreparedRuleInput, RuleError> {
    let imported = MaccmsImporter.import_document(raw_text).map_err(|_| {
        RuleError::new(
            RuleErrorStage::Import,
            "maccms_document_invalid",
            "Maccms JSON endpoint 文档无效",
            trace_id,
            false,
            Vec::new(),
        )
    })?;
    let runtime_credentials = imported.credential_snapshot_bytes().map_err(|_| {
        RuleError::new(
            RuleErrorStage::Import,
            "maccms_credentials_invalid",
            "Maccms 静态凭证无法安全快照",
            trace_id,
            false,
            Vec::new(),
        )
    })?;
    Ok(PreparedRuleInput {
        definition: imported.definition,
        runtime_credentials,
        display_title: None,
        display_group: None,
        diagnostics: Vec::new(),
    })
}

fn prepare_transient_rule_input(
    input: RuleInput,
    trace_id: &str,
) -> Result<(PreparedRuleInput, CandidateDocumentInput), RuleError> {
    match input {
        RuleInput::Legado { source_json } => {
            let format = SourceDocumentFormat::Legado;
            let target = CredentialTargetIdentity {
                format,
                document_id: SourceDocumentId::new().to_string(),
                revision: 1,
            };
            let split = CredentialSlotCodec::split(&source_json, target)
                .map_err(|error| credential_codec_error(&error, trace_id))?;
            let prepared = prepare_legado_input(&source_json, trace_id)?;
            Ok((
                prepared,
                CandidateDocumentInput::Transient(TransientSourceDocumentInput {
                    format,
                    masked_text: split.masked_text,
                    raw_text: source_json,
                    manifest: split.manifest,
                }),
            ))
        }
        RuleInput::MaccmsJson { url } => {
            let format = SourceDocumentFormat::Maccms10Endpoint;
            let raw_text = serde_json::to_string(&serde_json::json!({
                "base_url": url.as_str(),
                "format": "json",
                "headers": {},
            }))
            .map_err(|_| {
                RuleError::new(
                    RuleErrorStage::Import,
                    "maccms_document_invalid",
                    "Maccms JSON endpoint 文档无法构造",
                    trace_id,
                    false,
                    Vec::new(),
                )
            })?;
            let target = CredentialTargetIdentity {
                format,
                document_id: SourceDocumentId::new().to_string(),
                revision: 1,
            };
            let split = CredentialSlotCodec::split(&raw_text, target)
                .map_err(|error| credential_codec_error(&error, trace_id))?;
            let prepared = prepare_maccms_input(&raw_text, trace_id)?;
            Ok((
                prepared,
                CandidateDocumentInput::Transient(TransientSourceDocumentInput {
                    format,
                    masked_text: split.masked_text,
                    raw_text,
                    manifest: split.manifest,
                }),
            ))
        }
    }
}

fn credential_codec_error(error: &CredentialCodecError, trace_id: &str) -> RuleError {
    let code = error.diagnostic.code.clone();
    let diagnostic = authoring_diagnostic(&error.diagnostic);
    RuleError::new(
        RuleErrorStage::Import,
        code,
        "来源作者文档未通过 credential 安全校验",
        trace_id,
        false,
        vec![diagnostic],
    )
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
        let (prepared, document) = prepare_transient_rule_input(input, &trace_id)?;
        let expected_installed_revision = self
            .state
            .storage
            .get_installed_source(prepared.definition.source_identity().id.clone())
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Candidate, &trace_id))?
            .map_or(0, |source| source.source_revision);
        self.stage_prepared_candidate(prepared, document, expected_installed_revision, &trace_id)
            .await
    }

    /// 只从保险库中已显式保存的精确 [`DocumentRef`] 准备 composite candidate-v2。
    ///
    /// 该路径只在当前 saved revision 上运行 importer/compiler；dirty text、旧 ordinary-save
    /// revision 或缺失 secret material 都不能进入 staging。
    ///
    /// # Errors
    ///
    /// 文档 revision 不存在/已过期、secret 无法解密、authoring/import/compiler 失败，或 C2 单 writer
    /// composite publish 失败时返回 [`RuleError`]。
    pub async fn prepare_install_from_document(
        &self,
        document_ref: DocumentRef,
    ) -> Result<InstallCandidate, RuleError> {
        let trace_id = super::trace_id();
        if document_ref.document_revision == 0 {
            return Err(RuleError::new(
                RuleErrorStage::Candidate,
                "document_revision_invalid",
                "来源文档 revision 无效",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        let current = self
            .state
            .storage
            .get_source_document(document_ref.document_id)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Candidate, &trace_id))?
            .ok_or_else(|| {
                RuleError::new(
                    RuleErrorStage::Candidate,
                    "document_not_found",
                    "来源文档不存在",
                    trace_id.clone(),
                    false,
                    Vec::new(),
                )
            })?;
        if current.summary.revision != document_ref.document_revision {
            return Err(RuleError::new(
                RuleErrorStage::Candidate,
                "document_revision_stale",
                "来源文档 revision 已不是当前保存版本",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        let material = self
            .state
            .storage
            .load_source_document_material(document_ref)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Candidate, &trace_id))?
            .ok_or_else(|| {
                RuleError::new(
                    RuleErrorStage::Candidate,
                    "document_revision_unavailable",
                    "来源文档 revision 已不可用于安装",
                    trace_id.clone(),
                    false,
                    Vec::new(),
                )
            })?;
        let mut prepared =
            prepare_rule_document(material.format(), material.expose_raw_text(), &trace_id)?;
        let source_identity = prepared.definition.source_identity().id.clone();
        if current
            .summary
            .source_identity
            .as_deref()
            .is_some_and(|linked| linked != source_identity.as_str())
        {
            return Err(RuleError::new(
                RuleErrorStage::Candidate,
                "document_source_link_mismatch",
                "来源文档已关联另一来源，不能改写来源身份",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        let installed = self
            .state
            .storage
            .get_installed_source(&source_identity)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Candidate, &trace_id))?;
        let expected_installed_revision = installed
            .as_ref()
            .map_or(0, |source| source.source_revision);
        if prepared.display_title.is_none() {
            prepared.display_title = Some(current.summary.title);
        }
        self.stage_prepared_candidate(
            prepared,
            CandidateDocumentInput::Saved(document_ref),
            expected_installed_revision,
            &trace_id,
        )
        .await
    }

    async fn stage_prepared_candidate(
        &self,
        prepared: PreparedRuleInput,
        document: CandidateDocumentInput,
        expected_installed_revision: u64,
        trace_id: &str,
    ) -> Result<InstallCandidate, RuleError> {
        let PreparedRuleInput {
            definition,
            runtime_credentials,
            display_title,
            display_group,
            diagnostics: authoring_diagnostics,
        } = prepared;
        let definition = canonicalize(&definition);
        let mut diagnostics = validate(&definition);
        diagnostics.extend(authoring_diagnostics);
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
                document,
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

    /// 原子消费 composite candidate-v2；install 时不重新读取 current source 或 credential ref。
    ///
    /// # Errors
    ///
    /// candidate 缺失、过期、篡改、已消费、schema/document/source 基线变化、grant 不足或 C2
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
        document_ref: summary.document_ref,
        transient: summary.transient,
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
        document_ref: source.document_id.zip(source.document_revision).map(
            |(document_id, document_revision)| DocumentRef {
                document_id,
                document_revision,
            },
        ),
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
        CREDENTIAL_SCHEMA_VERSION, CapabilityManifest, ConditionConfig, ControlExpression,
        ControlledMapper, CredentialSlotManifest, FlowEdge, FlowGraph, FlowNode, FlowNodeConfig,
        FlowPortRef, JsConfig, JsOutputKind, LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE,
        MERGE_OUTPUT_HANDLE, MergeConfig, MergeInput, MergeInputActivation, MergeStrategy,
        RuleDefinition, SourceIdentity, SystemCapabilities,
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
        let format = SourceDocumentFormat::Maccms10Endpoint;
        let candidate = system
            .stage_prepared_candidate(
                PreparedRuleInput {
                    definition: current_control_definition(),
                    runtime_credentials: None,
                    display_title: Some("控制流测试".to_string()),
                    display_group: None,
                    diagnostics: Vec::new(),
                },
                CandidateDocumentInput::Transient(TransientSourceDocumentInput {
                    format,
                    masked_text: "{}".to_string(),
                    raw_text: "{}".to_string(),
                    manifest: CredentialSlotManifest {
                        schema_version: CREDENTIAL_SCHEMA_VERSION,
                        target: CredentialTargetIdentity {
                            format,
                            document_id: "transient-control-fixture".to_string(),
                            revision: 1,
                        },
                        slots: Vec::new(),
                    },
                }),
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
