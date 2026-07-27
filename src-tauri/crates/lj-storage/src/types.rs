//! `lj-storage` 的公开 DTO 门面。
//!
//! 对外类型按配置、错误、candidate、execution 与 projection 职责拆分；本门面维持原有
//! `lj_storage::types::*` 路径，且不暴露 Diesel 行模型、连接或泛型 Repository。

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

pub use artifact::{ArtifactInput, ArtifactKind};
pub use candidate::{
    CandidateDocumentInput, CandidateDraft, CandidateSummary, INSTALL_CANDIDATE_SCHEMA_VERSION,
    InstallCandidateRequest, InstalledSource, InstalledSourceRecord, RuntimeCredentialMaterial,
    TransientSourceDocumentInput,
};
pub use config::{
    DEFAULT_ARCHIVE_TTL_MS, DEFAULT_CANDIDATE_TTL_MS, StorageConfig, WRITER_CAPACITY,
};
pub use document::{
    CreateSourceDocumentInput, CredentialSlotMaterial, CredentialSlotSummary,
    DeleteSourceDocumentInput, DocumentMutationOutcome, DocumentRef, DocumentValidationIssue,
    EditSourceDocumentCredentialInput, LoadSourceDocumentRebaseMaterialInput,
    MAX_SOURCE_DOCUMENT_BYTES, MaskedSourceDocument, PinSourceDocumentRevisionInput,
    RebaseSourceDocumentInput, RenameSourceDocumentInput, RevealedSourceDocumentCredential,
    SOURCE_DOCUMENT_SCHEMA_VERSION, SaveSourceDocumentInput, SecretArtifactId,
    SourceDocumentCredentialTarget, SourceDocumentId, SourceDocumentMaterial,
    SourceDocumentRebaseCommitMode, SourceDocumentRebaseInvalidReason,
    SourceDocumentRebaseMaterialOutcome, SourceDocumentRebaseMaterials,
    SourceDocumentRevisionInput, SourceDocumentRevisionPin, SourceDocumentState,
    SourceDocumentSummary,
};
pub use error::StorageError;
pub use event::{AppendRequest, CommitReceipt, StoredEvent};
pub use execution::{
    ExecutionFinish, ExecutionPin, ExecutionRecord, ExecutionReplayPin, ExecutionSourceCredentials,
    ExecutionStart, ExecutionStartReceipt, ExecutionStatus, GcState, InstalledSourceSnapshot,
    ReplayExecutionStart, ReplayUnavailableReason,
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
