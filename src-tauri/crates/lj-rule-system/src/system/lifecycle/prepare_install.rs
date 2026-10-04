//! 一次性 import、candidate prepare 与 install。

use lj_compiler::{canonicalize, validate};
use lj_importer::legado::LegadoImporter;
use lj_importer::maccms::MaccmsImporter;
use lj_importer::{ImportDiagnostic, ImportedNativeRule};
use lj_media::{MediaResourceId, SourceProfile};
use lj_node_http::ImportFetchError;
use lj_rule_model::{Diagnostic, RulePackage, SourceSpan};
use lj_storage::{
    CandidateDraft, CandidateSummary, InstallCandidateRequest,
    InstalledSource as StorageInstalledSource, RuntimeCredentialMaterial, SourceRollbackRequest,
};
use uuid::Uuid;

use super::super::error_mapping::{compiler_error, runtime_error, storage_error};
use super::super::{RuleSystem, now_millis};
use crate::{
    CandidateId, CapabilityGrant, InstallCandidate, InstalledSource, RuleError, RuleErrorStage,
    RuleInput, SourceId, SourceOperation,
};

pub(super) struct PreparedRuleInput {
    pub(super) definition: lj_rule_model::RuleDefinition,
    pub(super) runtime_credentials: Option<Vec<u8>>,
    pub(super) display_title: Option<String>,
    pub(super) display_group: Option<String>,
    pub(super) diagnostics: Vec<Diagnostic>,
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

fn prepare_legado_input(source_json: &str, trace_id: &str) -> Result<PreparedRuleInput, RuleError> {
    let imported = LegadoImporter.import(source_json).map_err(|error| {
        RuleError::new(
            RuleErrorStage::Import,
            error.to_string(),
            "Legado 输入未通过安全校验",
            trace_id,
            false,
            error.diagnostics().iter().map(import_diagnostic).collect(),
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
        RuleInput::Package { source_json } => prepare_package_input(&source_json, trace_id),
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

/// 从导入的 Rule Package 读取 Definition；package 不承载 runtime credential。
fn prepare_package_input(
    package_json: &str,
    trace_id: &str,
) -> Result<PreparedRuleInput, RuleError> {
    let package = super::package::read_package(package_json.as_bytes(), trace_id)?;
    Ok(PreparedRuleInput {
        definition: package.definition().clone(),
        runtime_credentials: None,
        display_title: None,
        display_group: None,
        diagnostics: Vec::new(),
    })
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
    /// # Errors
    ///
    /// URL、目标、redirect、HTTP、大小或 UTF-8 校验失败时返回 `RuleError`。
    pub async fn fetch_import_source(url: &str) -> Result<String, RuleError> {
        let trace_id = super::super::trace_id();
        lj_node_http::fetch_import_source(url)
            .await
            .map_err(|error| import_fetch_error(error, &trace_id))
    }

    /// 从一次性来源输入生成 durable candidate。
    ///
    /// # Errors
    ///
    /// import、validation、compile、runtime gate 或 staging 失败时返回 `RuleError`。
    pub async fn prepare_install(&self, input: RuleInput) -> Result<InstallCandidate, RuleError> {
        let trace_id = super::super::trace_id();
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

    pub(super) async fn stage_prepared_candidate(
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

    /// 从历史 source revision 准备需要重新审阅的 rollback candidate。
    ///
    /// # Errors
    ///
    /// 来源、历史 revision、artifact、凭证或 candidate staging 失败时返回 `RuleError`。
    pub async fn prepare_source_rollback(
        &self,
        source_id: SourceId,
        source_revision: u64,
    ) -> Result<InstallCandidate, RuleError> {
        let trace_id = super::super::trace_id();
        let source_identity = source_id.as_identity().to_string();
        if source_identity.trim().is_empty() || source_revision == 0 {
            return Err(RuleError::new(
                RuleErrorStage::Validation,
                "source_revision_invalid",
                "来源 revision 无效",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        let created_at_ms = now_millis(&trace_id)?;
        let expires_at_ms = created_at_ms
            .checked_add(self.state.candidate_ttl_ms)
            .ok_or_else(|| {
                RuleError::new(
                    RuleErrorStage::Candidate,
                    "candidate_ttl_overflow",
                    "candidate 到期时长超出支持范围",
                    trace_id.clone(),
                    false,
                    Vec::new(),
                )
            })?;
        let summary = self
            .state
            .storage
            .stage_source_rollback(SourceRollbackRequest {
                candidate_id: Uuid::new_v4(),
                source_identity,
                source_revision,
                expires_at_ms: Some(expires_at_ms),
                trace_id: trace_id.clone(),
                correlation_id: None,
                created_at_ms,
            })
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Candidate, &trace_id))?;
        Ok(candidate_from_summary(summary))
    }

    /// 原子消费 candidate。
    ///
    /// # Errors
    ///
    /// candidate、schema、grant、source baseline 或 transaction 失败时返回 `RuleError`。
    pub async fn install(
        &self,
        candidate_id: CandidateId,
        grant: CapabilityGrant,
    ) -> Result<InstalledSource, RuleError> {
        let trace_id = super::super::trace_id();
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
}

fn candidate_from_summary(summary: CandidateSummary) -> InstallCandidate {
    InstallCandidate {
        id: CandidateId::from_uuid(summary.candidate_id),
        expected_installed_revision: summary.expected_installed_revision,
        operation: SourceOperation::from_installed_revision(summary.expected_installed_revision),
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
        grant: CapabilityGrant::from_policy(source.grant),
        revision: source.source_revision,
    }
}

pub(super) fn source_profile(
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
