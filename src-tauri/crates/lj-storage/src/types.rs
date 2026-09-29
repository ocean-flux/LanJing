//! `lj-storage` 的公开 DTO 门面。
//!
//! 对外类型按配置、错误、candidate、execution 与 projection 职责拆分；本门面维持原有
//! `lj_storage::types::*` 路径，且不暴露 ORM 行模型、连接或泛型 Repository。

mod artifact;
mod candidate;
mod config;
mod document;
mod error;
mod event;
mod execution;
mod library;
mod projection;
mod retention;

pub(crate) use artifact::ArtifactKind;
pub use artifact::{ArtifactInput, SecretArtifactId};
pub use candidate::{
    CandidateDraft, CandidateSummary, INSTALL_CANDIDATE_SCHEMA_VERSION, InstallCandidateRequest,
    InstalledSource, InstalledSourceRecord, RuntimeCredentialMaterial, SourceRevisionRecord,
    SourceRollbackRequest,
};
pub use config::{
    DEFAULT_ARCHIVE_TTL_MS, DEFAULT_CANDIDATE_TTL_MS, StorageConfig, WRITER_CAPACITY,
};
pub use document::{
    CreateDocumentRequest, DeleteDocumentRequest, DocumentCredentialMutation,
    DocumentCredentialMutationAction, DocumentDetail, DocumentInitial, DocumentSummary,
    DomainSaveOutcome, LayoutSaveInput, LayoutSnapshot, ProvenanceCreateInput, ProvenanceSummary,
    RenameDocumentRequest, RestoreDocumentRevisionOutcome, RestoreDocumentRevisionRequest,
    RevisionConflict, RuleRevisionHistoryRecord, SaveDocumentOutcome, SaveDocumentRequest,
    SemanticActivation, SemanticSaveInput, SemanticSnapshot,
};
pub use error::StorageError;
pub use event::{AppendRequest, CommitReceipt, StoredEvent};
pub use execution::{
    ExecutionFinish, ExecutionPin, ExecutionRecord, ExecutionReplayPin, ExecutionSourceCredentials,
    ExecutionStart, ExecutionStartReceipt, ExecutionStatus, GcState, InstalledSourceSnapshot,
    ReplayExecutionStart,
};
pub use library::{
    LibraryEntry, LibraryProgress, LibraryProjection, LibraryProjectionEntry,
    LibraryProjectionSnapshot, LibraryUpdate,
};
pub use projection::{
    DeltaCommit, ProjectionDelta, ProjectionTombstones, RelationTombstone,
    SourceProjectionSnapshot, SourceProjectionView,
};
pub use retention::{CheckpointReceipt, GcReport, OrphanRecovery, RetentionPolicy};
