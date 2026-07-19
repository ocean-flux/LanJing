//! `RuleSystem` 边界错误映射。
//!
//! 本模块把 compiler、runtime 与 C2 storage 的内部错误收敛成稳定、脱敏的
//! [`RuleError`]。不变量：任何 message、diagnostic 或 trace 都不得包含 body、cookie、
//! token、完整 URL query、Plan JSON 或 opaque payload。

use lj_compiler::CompilerError;
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
            "历史 execution 缺少 effect capture",
        ),
        RuntimeFailureCode::ReplayRecordMismatch => (
            RuleErrorStage::Replay,
            "replay_record_mismatch",
            "历史 effect capture 不属于固定 replay pin",
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
        PlanRuntimeError::SchemaVersionMismatch { .. }
        | PlanRuntimeError::CompilerVersionMismatch
        | PlanRuntimeError::PlanHashMismatch => {
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
        PlanRuntimeError::UnsupportedControlFlow => (
            "plan_control_flow_unsupported",
            "immutable Plan 包含不支持的控制流",
        ),
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
pub(super) fn storage_error(
    error: &StorageError,
    default_stage: RuleErrorStage,
    trace_id: &str,
) -> RuleError {
    let (stage, code, message, retryable) = match error {
        StorageError::CandidateMissing => (
            RuleErrorStage::Candidate,
            "candidate_missing",
            "candidate 不存在",
            false,
        ),
        StorageError::CandidateExpired => (
            RuleErrorStage::Candidate,
            "candidate_expired",
            "candidate 已过期",
            false,
        ),
        StorageError::CandidateUnavailable => (
            RuleErrorStage::Candidate,
            "candidate_unavailable",
            "candidate 已被消费或不可安装",
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
        StorageError::VersionConflict { .. } => (
            default_stage,
            "stream_version_conflict",
            "持久化流版本发生冲突",
            true,
        ),
        StorageError::ArtifactUnavailable(_)
        | StorageError::MasterKeyUnavailable
        | StorageError::SecretUnavailable
        | StorageError::ReplayUnavailable(_) => (
            RuleErrorStage::Replay,
            "replay_pin_unavailable",
            "历史 execution 的固定 archive 不可 replay",
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
    };
    RuleError::new(
        stage,
        code,
        message,
        trace_id.to_string(),
        retryable,
        Vec::new(),
    )
}
