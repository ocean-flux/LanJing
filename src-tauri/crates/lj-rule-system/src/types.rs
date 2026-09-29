//! `RuleSystem` façade DTO 分区。

mod candidate;
mod config;
mod document;
mod execution;
mod query;

pub use candidate::{
    CandidateId, CapabilityGrant, InstallCandidate, InstalledSource, RuleInput, SourceId,
    SourceRevisionSummary,
};
pub use config::RuleSystemConfig;
pub use document::{
    CreateMode, CreateNativeRuleDocumentRequest, CredentialMutationAction,
    CredentialMutationRequest, DeleteNativeRuleDocumentRequest, DomainOutcome, ExpectedDataType,
    GetNativeRuleDocumentRequest, GetNativeRuleProvenanceRequest, LayoutSave,
    NativeRuleDocumentDetail, NativeRuleDocumentSummary, NativeRuleProvenanceView,
    NativeRuleRevisionSummary, ProvenanceSummaryView, RenameNativeRuleDocumentRequest,
    RestoreNativeRuleRevisionOutcome, RestoreNativeRuleRevisionRequest, RevisionConflict,
    SaveNativeRuleDocumentOutcome, SaveNativeRuleDocumentRequest, SemanticActivation, SemanticSave,
    ValidateNativeRuleDocumentPreview, ValidateNativeRuleDocumentRequest,
};
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
