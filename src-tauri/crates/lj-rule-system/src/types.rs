//! `RuleSystem` façade DTO 分区。

mod candidate;
mod config;
mod execution;
mod query;

pub use candidate::{
    CandidateId, CapabilityGrant, InstallCandidate, InstalledSource, RuleInput, SourceId,
};
pub use config::RuleSystemConfig;
pub use execution::{
    ExecuteRequest, ExecutionCancellation, ExecutionEvent, ExecutionEventKind, ExecutionId,
    ExecutionMode, ExecutionSession,
};
pub use query::{
    LibraryEntryUpdate, LibraryProgress, LibraryProjection, LibraryProjectionEntry,
    LibraryUpdateReceipt, MediaAssetPage, MediaUnitPage,
};

#[cfg(feature = "test-support")]
pub use execution::{
    EffectWitnessCaptureForTest, EffectWitnessForTest, ExtractEffectWitnessForTest,
    HttpDnsTargetKindForTest, HttpDnsTargetWitnessForTest, HttpEffectErrorKindForTest,
    HttpEffectWitnessForTest, HttpMethodForTest, HttpRedirectWitnessForTest,
    HttpRequestBodyWitnessForTest, HttpRequestHeaderWitnessForTest, HttpRequestWitnessForTest,
    QuickJsEffectWitnessForTest, QuickJsErrorKindForTest, QuickJsHostCallForTest,
    QuickJsHostCallWitnessForTest,
};
