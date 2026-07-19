//! Plan runtime 的类型化 effect seam。
//!
//! 对外保留 HTTP、QuickJS、Extract 的 typed 合同以及 capture/replay API。内部按 seam 合同、
//! witness 完整性、凭据安全、archive 与取消职责拆分；live 输出只有获得 durable 收据才可推进，
//! replay 永远不接触 live 凭据或 adapter。

mod archive;
mod cancellation;
mod contracts;
mod credentials;
mod witness;

pub use archive::{
    ArchivedEffectCapture, DurableCaptureReceipt, EffectArchive, EffectArchiveError, EffectCapture,
    EffectReplayLookup,
};
pub use cancellation::{CancellationHandle, EffectCancellation};
pub use contracts::{
    EffectError, EffectErrorCode, EffectFailure, EffectHandlers, EffectInput, EffectOutput,
    ExtractEffectHandler, ExtractEffectRequest, ExtractOutput, HttpEffectErrorKind,
    HttpEffectHandler, HttpEffectRequest, QuickJsEffectHandler, QuickJsEffectRequest,
    QuickJsErrorKind, QuickJsOutput,
};
pub use credentials::{HttpCredentialsError, HttpExecutionCredentials, SecretHttpHeaders};
pub use witness::{
    CapturedEffectOutput, EffectCaptureMaterial, EffectCaptureMaterialSensitivity, EffectWitness,
    EffectWitnessError, ExtractEffectWitness, HttpDnsTargetKind, HttpDnsTargetWitness,
    HttpEffectWitness, HttpRedirectWitness, HttpRequestBodyWitness, HttpRequestHeaderWitness,
    HttpRequestWitness, QuickJsEffectWitness, QuickJsHostCall, QuickJsHostCallWitness,
    effect_bytes_hash, effect_input_hash, effect_output_hash, quickjs_script_hash,
};
