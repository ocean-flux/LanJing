//! 规则导入、安装与执行的 concrete façade。
//!
//! 本 crate 公开 import/candidate/install 与 execute 的安全 DTO；Definition、
//! immutable Plan、node effect adapter、`EventProjectionStorage` 与 execution registry 全部保持
//! 私有组合，Tauri 不直接依赖 storage/importer/runtime。

mod error;
pub(crate) mod system;
mod types;

pub use error::{RuleError, RuleErrorStage};
pub use system::RuleSystem;
pub use types::{
    CandidateId, CapabilityGrant, CreateMode, CreateNativeRuleDocumentRequest,
    CredentialMutationAction, CredentialMutationRequest, DeleteNativeRuleDocumentRequest,
    DomainOutcome, ExecuteRequest, ExecutionCancellation, ExecutionEvent, ExecutionEventKind,
    ExecutionId, ExecutionMode, ExecutionSession, ExpectedDataType, GetNativeRuleDocumentRequest,
    GetNativeRuleProvenanceRequest, InstallCandidate, InstalledSource, LayoutSave,
    LibraryEntryUpdate, LibraryProgress, LibraryProjection, LibraryProjectionEntry,
    LibraryUpdateReceipt, MediaAssetPage, MediaUnitPage, NativeRuleDocumentDetail,
    NativeRuleDocumentSummary, NativeRuleProvenanceView, NativeRuleRevisionSummary,
    ProvenanceSummaryView, RenameNativeRuleDocumentRequest, RestoreNativeRuleRevisionOutcome,
    RestoreNativeRuleRevisionRequest, RevisionConflict, RuleInput, RulePackageInspection,
    RuleSystemConfig, SaveNativeRuleDocumentOutcome, SaveNativeRuleDocumentRequest,
    SemanticActivation, SemanticSave, SourceId, SourceRevisionSummary, UnavailableNodeSummary,
    ValidateNativeRuleDocumentPreview, ValidateNativeRuleDocumentRequest,
};
// 标准媒体模型经 façade 再导出，避免根 package 依赖 lj-media path。
pub use lj_media::{MediaAsset, MediaItem, MediaUnit};

#[cfg(feature = "test-support")]
pub mod test_support;

#[cfg(feature = "test-support")]
pub use types::{
    EffectWitnessCaptureForTest, EffectWitnessForTest, ExtractEffectWitnessForTest,
    HttpDnsTargetKindForTest, HttpDnsTargetWitnessForTest, HttpEffectErrorKindForTest,
    HttpEffectWitnessForTest, HttpMethodForTest, HttpRedirectWitnessForTest,
    HttpRequestBodyWitnessForTest, HttpRequestHeaderWitnessForTest, HttpRequestWitnessForTest,
    QuickJsEffectWitnessForTest, QuickJsErrorKindForTest, QuickJsHostCallForTest,
    QuickJsHostCallWitnessForTest,
};
