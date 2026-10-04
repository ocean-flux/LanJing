//! immutable Execution Plan runtime。
//!
//! 只接收 compiler 产出的 `ExecutionPlan`，通过 typed HTTP、QuickJS、Extract effect
//! seam 执行。公开执行合同只包含 immutable Plan 与 typed effect seam。

pub mod capability;
pub mod effect;
pub mod effect_registry;
pub(crate) mod mapper;
pub(crate) mod mapper_fields;
pub mod node_data;
pub mod plan_runtime;

pub use capability::{check_capability, default_capabilities, merge};
pub use effect::{
    ArchivedEffectCapture, CancellationHandle, CapturedEffectOutput, ControlReplayLookup,
    ControlTraceCapture, ControlTraceReceipt, DurableCaptureReceipt, EffectArchive,
    EffectArchiveError, EffectArchiveErrorCode, EffectCancellation, EffectCapture,
    EffectCaptureMaterialSensitivity, EffectError, EffectErrorCode, EffectFailure, EffectInput,
    EffectOutput, EffectReplayLookup, EffectWitness, EffectWitnessError, ExtractEffectHandler,
    ExtractEffectRequest, ExtractEffectWitness, ExtractOutput, HttpCredentialsError,
    HttpDnsTargetKind, HttpDnsTargetWitness, HttpEffectErrorKind, HttpEffectHandler,
    HttpEffectRequest, HttpEffectWitness, HttpExecutionCredentials, HttpRedirectWitness,
    HttpRequestBodyWitness, HttpRequestHeaderWitness, HttpRequestWitness, QuickJsEffectHandler,
    QuickJsEffectRequest, QuickJsEffectWitness, QuickJsErrorKind, QuickJsHostCall,
    QuickJsHostCallWitness, QuickJsOutput, ReplayCompletionLookup, SecretHttpHeaders,
    control_trace_hash, effect_bytes_hash, effect_input_hash, effect_output_hash,
    quickjs_script_hash,
};
pub use node_data::{HttpResponse, NodeData};
pub use plan_runtime::{
    ExecutionEvent, ExecutionEventKind, ExecutionFailure, ExecutionMode, ExecutionSession,
    PlanExecutionRequest, PlanRuntime, PlanRuntimeConfig, PlanRuntimeError, PlanSupport,
    RuntimeFailureCode,
};
