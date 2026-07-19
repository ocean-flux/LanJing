//! 规则生命周期 concrete façade。
//!
//! 本 crate 只公开 `prepare_install`、`install`、`execute` 三个生命周期命令及其安全 DTO。
//! Definition、immutable Plan、node effect adapter、EventProjectionStorage 与 execution registry
//! 全部保持私有组合；Maccms JSON 是首条真实来源路径。

mod error;
pub(crate) mod system;
mod types;

pub use error::{RuleError, RuleErrorStage};
pub use system::RuleSystem;
pub use types::{
    CandidateId, CapabilityGrant, ExecuteRequest, ExecutionCancellation, ExecutionEvent,
    ExecutionEventKind, ExecutionId, ExecutionMode, ExecutionSession, InstallCandidate,
    InstalledSource, LibraryEntryUpdate, LibraryProgress, LibraryProjection,
    LibraryProjectionEntry, LibraryUpdateReceipt, RuleInput, RuleSystemConfig, SourceId,
};

#[cfg(feature = "test-support")]
pub use types::{
    EffectWitnessCaptureForTest, EffectWitnessForTest, ExtractEffectWitnessForTest,
    HttpDnsTargetKindForTest, HttpDnsTargetWitnessForTest, HttpEffectErrorKindForTest,
    HttpEffectWitnessForTest, HttpMethodForTest, HttpRedirectWitnessForTest,
    HttpRequestBodyWitnessForTest, HttpRequestHeaderWitnessForTest, HttpRequestWitnessForTest,
    QuickJsEffectWitnessForTest, QuickJsErrorKindForTest, QuickJsHostCallForTest,
    QuickJsHostCallWitnessForTest,
};
