//! 规则生命周期与来源文档保险库的 concrete façade。
//!
//! 本 crate 公开 document CRUD/credential、candidate/install 与 execute 的安全 DTO；Definition、
//! immutable Plan、node effect adapter、`EventProjectionStorage` 与 execution registry 全部保持
//! 私有组合，Tauri 不直接依赖 storage/importer/runtime。

mod error;
pub(crate) mod system;
mod types;

pub use error::{RuleError, RuleErrorStage};
pub use system::RuleSystem;
pub use types::{
    CandidateId, CapabilityGrant, ClearSourceDocumentCredentialRequest,
    CreateSourceDocumentRequest, CredentialSlotSummary, DeleteSourceDocumentRequest,
    DocumentMutationOutcome, DocumentRef, DocumentValidationIssue, ExecuteRequest,
    ExecutionCancellation, ExecutionEvent, ExecutionEventKind, ExecutionId, ExecutionMode,
    ExecutionSession, GetSourceDocumentRequest, InstallCandidate, InstalledSource,
    LibraryEntryUpdate, LibraryProgress, LibraryProjection, LibraryProjectionEntry,
    LibraryUpdateReceipt, ListSourceDocumentsRequest, MaskedSourceDocument, MediaAssetPage,
    MediaUnitPage, PinSourceDocumentRevisionRequest, RebaseSourceDocumentRequest,
    ReleaseSourceDocumentRevisionPinRequest, RenameSourceDocumentRequest,
    ReplaceSourceDocumentCredentialRequest, RevealSourceDocumentCredentialRequest, RuleInput,
    RuleSystemConfig, SaveSourceDocumentRequest, SourceDocumentCredentialAction,
    SourceDocumentCredentialResolution, SourceDocumentCredentialReveal,
    SourceDocumentCredentialTarget, SourceDocumentFormat, SourceDocumentId,
    SourceDocumentRebaseMode, SourceDocumentRevisionPin, SourceDocumentRevisionPinReleaseOutcome,
    SourceDocumentState, SourceDocumentSummary, SourceId,
};
// 标准媒体模型经 façade 再导出，避免根 package 依赖 lj-media path。
pub use lj_media::{MediaAsset, MediaItem, MediaUnit};

#[cfg(feature = "test-support")]
pub use types::{
    EffectWitnessCaptureForTest, EffectWitnessForTest, ExtractEffectWitnessForTest,
    HttpDnsTargetKindForTest, HttpDnsTargetWitnessForTest, HttpEffectErrorKindForTest,
    HttpEffectWitnessForTest, HttpMethodForTest, HttpRedirectWitnessForTest,
    HttpRequestBodyWitnessForTest, HttpRequestHeaderWitnessForTest, HttpRequestWitnessForTest,
    QuickJsEffectWitnessForTest, QuickJsErrorKindForTest, QuickJsHostCallForTest,
    QuickJsHostCallWitnessForTest,
};
