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
    CandidateId, CapabilityGrant, ExecuteRequest, ExecutionCancellation, ExecutionEvent,
    ExecutionEventKind, ExecutionId, ExecutionMode, ExecutionSession, InstallCandidate,
    InstalledSource, LibraryEntryUpdate, LibraryProgress, LibraryProjection,
    LibraryProjectionEntry, LibraryUpdateReceipt, MediaAssetPage, MediaUnitPage, RuleInput,
    RuleSystemConfig, SourceId,
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
