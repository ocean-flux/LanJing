//! `RuleSystem` 边界错误映射。
//!
//! 本模块把 compiler、runtime 与 C2 storage 的内部错误收敛成稳定、脱敏的
//! [`RuleError`]。不变量：任何 message、diagnostic 或 trace 都不得包含 body、cookie、
//! token、完整 URL query、Plan JSON 或 opaque payload。

use lj_compiler::CompilerError;
use lj_plugin_contract::PluginError;
use lj_runtime::{PlanRuntimeError, RuntimeFailureCode};
use lj_storage::StorageError;

use crate::{RuleError, RuleErrorStage};

/// 将 runtime 已持久化失败映射为调用方可见的安全错误。
pub(super) fn runtime_failure_error(code: RuntimeFailureCode, trace_id: &str) -> RuleError {
    let (stage, code, message) = match code {
        RuntimeFailureCode::CapabilityDenied => (
            RuleErrorStage::Capability,
            "runtime_capability_denied",
            "运行时拒绝未批准的 capability",
        ),
        RuntimeFailureCode::OperationUnavailable => (
            RuleErrorStage::Internal,
            "runtime_operation_unavailable",
            "执行绑定的 registry 缺少该 operation 的 handler",
        ),
        RuntimeFailureCode::EffectFailed => (
            RuleErrorStage::Effect,
            "effect_failed",
            "执行外部 effect 失败",
        ),
        RuntimeFailureCode::CaptureFailed => (
            RuleErrorStage::Effect,
            "effect_capture_failed",
            "effect durable capture 失败",
        ),
        RuntimeFailureCode::CaptureReceiptMismatch => (
            RuleErrorStage::Effect,
            "effect_capture_receipt_mismatch",
            "effect durable capture 收据不匹配",
        ),
        RuntimeFailureCode::CaptureWitnessInvalid => (
            RuleErrorStage::Effect,
            "effect_capture_witness_invalid",
            "effect durable capture witness 完整性校验失败",
        ),
        RuntimeFailureCode::ReplayCaptureMissing => (
            RuleErrorStage::Replay,
            "replay_capture_missing",
            "历史 execution 缺少 invocation archive",
        ),
        RuntimeFailureCode::ReplayRecordMismatch => (
            RuleErrorStage::Replay,
            "replay_record_mismatch",
            "历史 invocation archive 不属于固定 replay pin",
        ),
        RuntimeFailureCode::ReplayFingerprintMismatch => (
            RuleErrorStage::Replay,
            "replay_fingerprint_mismatch",
            "历史 effect fingerprint 不匹配",
        ),
        RuntimeFailureCode::ReplayOutputHashMismatch => (
            RuleErrorStage::Replay,
            "replay_output_hash_mismatch",
            "历史 effect 输出 hash 不匹配",
        ),
        RuntimeFailureCode::ReplayWitnessMismatch => (
            RuleErrorStage::Replay,
            "replay_witness_mismatch",
            "历史 effect witness 不匹配",
        ),
        RuntimeFailureCode::InputTypeMismatch => (
            RuleErrorStage::Execution,
            "runtime_input_type_mismatch",
            "immutable Plan 节点输入类型不匹配",
        ),
        RuntimeFailureCode::LegacyRuleContractUnsupported => (
            RuleErrorStage::Replay,
            "LEGACY_RULE_CONTRACT_UNSUPPORTED",
            "历史 invocation archive 不受当前版本支持",
        ),
        RuntimeFailureCode::Internal => (
            RuleErrorStage::Internal,
            "runtime_internal_failure",
            "运行时出现内部失败",
        ),
    };
    RuleError::new(
        stage,
        code,
        message,
        trace_id.to_string(),
        false,
        Vec::new(),
    )
}

/// 将 plugin contract 与注册错误映射为调用方可见的安全错误。
///
/// message 只包含稳定错误码，不包含 plugin payload 或 identity 以外的声明内容。
pub(super) fn plugin_error(error: &PluginError, trace_id: &str) -> RuleError {
    RuleError::new(
        RuleErrorStage::Internal,
        "plugin_registration_failed",
        format!("内置 plugin 注册失败: {}", error.code()),
        trace_id.to_string(),
        false,
        Vec::new(),
    )
}

/// 将 compiler 错误收敛为安全安装前错误。
pub(super) fn compiler_error(error: &CompilerError, trace_id: &str) -> RuleError {
    let diagnostics = error.diagnostics().to_vec();
    let (stage, code, message) = match error {
        CompilerError::Validation { .. } => (
            RuleErrorStage::Validation,
            "definition_validation_failed",
            "来源 Definition 未通过校验",
        ),
        CompilerError::SyntaxError(_) => (
            RuleErrorStage::Compile,
            "source_syntax_invalid",
            "来源规则语法无效",
        ),
        CompilerError::UnsupportedSelector(_) => (
            RuleErrorStage::Compile,
            "selector_unsupported",
            "来源规则包含不支持的选择器",
        ),
        CompilerError::UnsupportedVersion(_) => (
            RuleErrorStage::Compile,
            "source_version_unsupported",
            "来源规则版本不受支持",
        ),
        CompilerError::Serialization(_) => (
            RuleErrorStage::Compile,
            "plan_serialization_failed",
            "immutable Plan 序列化失败",
        ),
        CompilerError::Internal(_) => (
            RuleErrorStage::Internal,
            "compiler_internal_failure",
            "compiler 出现内部失败",
        ),
    };
    RuleError::new(
        stage,
        code,
        message,
        trace_id.to_string(),
        false,
        diagnostics,
    )
}

/// 将 Plan runtime 的启动/校验错误映射为安全执行错误。
pub(super) fn runtime_error(error: &PlanRuntimeError, trace_id: &str) -> RuleError {
    let (code, message) = match error {
        PlanRuntimeError::MissingIntent => {
            ("unsupported_intent", "immutable Plan 未声明该标准意图")
        }
        PlanRuntimeError::CompilerVersionMismatch | PlanRuntimeError::PlanHashMismatch => {
            ("plan_pin_invalid", "immutable Plan 版本或 hash 无效")
        }
        PlanRuntimeError::InvalidConfiguration(_) => {
            ("runtime_configuration_invalid", "PlanRuntime 配置无效")
        }
        PlanRuntimeError::MissingTokioRuntime => {
            ("runtime_unavailable", "当前线程没有可用 Tokio runtime")
        }
        PlanRuntimeError::MissingNode(_) | PlanRuntimeError::InvalidPlan(_) => {
            ("plan_invalid", "immutable Plan 结构无效")
        }
        PlanRuntimeError::CanonicalSerialization => (
            "plan_canonicalization_failed",
            "immutable Plan 无法验证 canonical hash",
        ),
    };
    RuleError::new(
        RuleErrorStage::Execution,
        code,
        message,
        trace_id.to_string(),
        false,
        Vec::new(),
    )
}

/// 将 C2 错误映射为不泄露存储路径、artifact ref 或 secret 的 façade 错误。
// 安全 façade 的穷举映射集中在一个 match，避免新增 StorageError 时遗漏脱敏分支。
pub(super) fn storage_error(
    error: &StorageError,
    default_stage: RuleErrorStage,
    trace_id: &str,
) -> RuleError {
    let contract = match error {
        StorageError::CandidateMissing
        | StorageError::CandidateExpired
        | StorageError::CandidateUnavailable
        | StorageError::CandidateTampered
        | StorageError::CandidateStale
        | StorageError::CandidateSchemaMismatch
        | StorageError::SourceRevisionMissing => candidate_storage_contract(error),
        StorageError::DocumentMissing => (
            RuleErrorStage::Persistence,
            "document_not_found",
            "文档不存在",
            false,
        ),
        StorageError::RuleRevisionMissing => (
            RuleErrorStage::Persistence,
            "rule_revision_not_found",
            "请求的规则历史 revision 不存在",
            false,
        ),
        StorageError::ContractSchemaUnsupported { .. }
        | StorageError::GrantInsufficient
        | StorageError::SourceCredentialUnavailable
        | StorageError::SecretOwnershipMismatch
        | StorageError::SourceMissing
        | StorageError::ExecutionMissing => contract_storage_contract(error, default_stage),
        StorageError::CurrentSchemaRequired
        | StorageError::VersionConflict { .. }
        | StorageError::ArtifactUnavailable(_)
        | StorageError::SecretUnavailable
        | StorageError::ReplayUnavailable(_)
        | StorageError::KeyringUnavailable
        | StorageError::KeyringLocked
        | StorageError::KeyLost
        | StorageError::ArtifactCorrupt
        | StorageError::SecretOwnershipCorrupt
        | StorageError::IdempotencyMismatch
        | StorageError::WriterClosed
        | StorageError::WriterUnavailable
        | StorageError::Database(_)
        | StorageError::FileSystem(_)
        | StorageError::Keyring
        | StorageError::Serialization
        | StorageError::InvalidInput(_) => durability_storage_contract(error, default_stage),
    };
    storage_rule_error(contract, trace_id)
}

fn candidate_storage_contract(
    error: &StorageError,
) -> (RuleErrorStage, &'static str, &'static str, bool) {
    let (code, message) = match error {
        StorageError::CandidateMissing => ("candidate_missing", "candidate 不存在"),
        StorageError::CandidateExpired => ("candidate_expired", "candidate 已过期"),
        StorageError::CandidateUnavailable => {
            ("candidate_unavailable", "candidate 已被消费或不可安装")
        }
        StorageError::CandidateTampered => (
            "candidate_tampered",
            "candidate durable metadata 与安装内容不一致",
        ),
        StorageError::CandidateStale => ("candidate_stale", "candidate 的已安装来源基线已经变化"),
        StorageError::CandidateSchemaMismatch => (
            "candidate_schema_mismatch",
            "candidate schema 已不受当前版本支持",
        ),
        StorageError::SourceRevisionMissing => (
            "source_revision_not_found",
            "请求的来源历史 revision 不存在",
        ),
        _ => unreachable!("candidate storage error category is exhaustive"),
    };
    (RuleErrorStage::Candidate, code, message, false)
}

fn contract_storage_contract(
    error: &StorageError,
    default_stage: RuleErrorStage,
) -> (RuleErrorStage, &'static str, &'static str, bool) {
    match error {
        StorageError::ContractSchemaUnsupported { .. } => (
            default_stage,
            "RULE_CONTRACT_SCHEMA_UNSUPPORTED",
            "已安装规则合同 schema 不受当前版本支持",
            false,
        ),
        StorageError::GrantInsufficient => (
            RuleErrorStage::Capability,
            "grant_insufficient",
            "批准的 capability 未覆盖来源所需能力",
            false,
        ),
        StorageError::SourceCredentialUnavailable => (
            default_stage,
            "source_credentials_unavailable",
            "来源凭证快照缺失、篡改或不可读取",
            false,
        ),
        StorageError::SecretOwnershipMismatch => (
            default_stage,
            "secret_owner_mismatch",
            "secret artifact 不属于请求的持久化 owner",
            false,
        ),
        StorageError::SourceMissing => (
            RuleErrorStage::Execution,
            "source_not_installed",
            "来源尚未安装",
            false,
        ),
        StorageError::ExecutionMissing => (
            RuleErrorStage::Execution,
            "execution_missing",
            "execution 不存在",
            false,
        ),
        _ => unreachable!("contract storage error category is exhaustive"),
    }
}

fn durability_storage_contract(
    error: &StorageError,
    default_stage: RuleErrorStage,
) -> (RuleErrorStage, &'static str, &'static str, bool) {
    match error {
        StorageError::CurrentSchemaRequired => (
            RuleErrorStage::Persistence,
            "CURRENT_SCHEMA_REQUIRED",
            "本地数据库不是当前 schema，需要删除后重建",
            false,
        ),
        StorageError::VersionConflict { .. } => (
            default_stage,
            "stream_version_conflict",
            "持久化流版本发生冲突",
            true,
        ),
        StorageError::ArtifactUnavailable(_)
        | StorageError::SecretUnavailable
        | StorageError::ReplayUnavailable(_) => (
            RuleErrorStage::Replay,
            "replay_pin_unavailable",
            "历史 execution 的固定 archive 不可 replay",
            false,
        ),
        StorageError::KeyringUnavailable => (
            default_stage,
            "keyring_unavailable",
            "当前平台没有可用的安全凭证存储",
            false,
        ),
        StorageError::KeyringLocked => (
            default_stage,
            "keyring_locked",
            "安全凭证存储当前已锁定",
            true,
        ),
        StorageError::KeyLost => (
            default_stage,
            "vault_key_lost",
            "secret artifact 加密密钥已经丢失",
            false,
        ),
        StorageError::ArtifactCorrupt => (
            default_stage,
            "vault_artifact_corrupt",
            "secret artifact 加密内容已损坏",
            false,
        ),
        StorageError::SecretOwnershipCorrupt => (
            RuleErrorStage::Persistence,
            "secret_ownership_corrupt",
            "secret artifact ownership 数据不一致",
            false,
        ),
        StorageError::IdempotencyMismatch => (
            default_stage,
            "idempotency_mismatch",
            "持久化请求与已有事件不一致",
            false,
        ),
        StorageError::WriterClosed | StorageError::WriterUnavailable => (
            RuleErrorStage::Persistence,
            "storage_writer_unavailable",
            "本地事件存储当前不可用",
            true,
        ),
        StorageError::Database(_)
        | StorageError::FileSystem(_)
        | StorageError::Keyring
        | StorageError::Serialization
        | StorageError::InvalidInput(_) => (
            default_stage,
            "storage_operation_failed",
            "本地持久化操作失败",
            false,
        ),
        _ => unreachable!("durability storage error category is exhaustive"),
    }
}

fn storage_rule_error(
    contract: (RuleErrorStage, &'static str, &'static str, bool),
    trace_id: &str,
) -> RuleError {
    let (stage, code, message, retryable) = contract;
    RuleError::new(
        stage,
        code,
        message,
        trace_id.to_string(),
        retryable,
        Vec::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use lj_rule_model::SchemaContract;

    #[test]
    fn storage_schema_errors_map_to_stable_contract_codes() {
        let schema = storage_error(
            &StorageError::ContractSchemaUnsupported {
                contract: SchemaContract::ExecutionPlan,
                version: 99,
            },
            RuleErrorStage::Execution,
            "trace-schema",
        );
        assert_eq!(schema.stage, RuleErrorStage::Execution);
        assert_eq!(schema.code, "RULE_CONTRACT_SCHEMA_UNSUPPORTED");
        assert!(schema.diagnostics.is_empty());
        assert!(!schema.message.contains("script"));
        assert!(!schema.message.contains("secret"));
    }

    #[test]
    fn runtime_legacy_archive_rejection_stays_replay_safe() {
        let error = runtime_failure_error(
            RuntimeFailureCode::LegacyRuleContractUnsupported,
            "trace-legacy-invocation",
        );
        assert_eq!(error.stage, RuleErrorStage::Replay);
        assert_eq!(error.code, "LEGACY_RULE_CONTRACT_UNSUPPORTED");
        assert!(error.diagnostics.is_empty());
        assert!(!error.message.contains("script"));
        assert!(!error.message.contains("result"));
    }
}
