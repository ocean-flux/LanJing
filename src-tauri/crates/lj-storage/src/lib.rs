//! `SQLite` Event Store、规范化投影与 durable artifact archive。
//!
//! `EventProjectionStorage` 是 C4 `RuleSystem` 的具体存储模块：所有 event/projection
//! 写入经过容量固定为 256 的单 writer；同步 Diesel、文件系统与 keyring 工作只在
//! blocking lane 执行；读请求使用独立 `SQLite` 连接。旧 Graph Repository 与单 JSON
//! media graph 已被完全移除。

mod artifact;
mod candidate_install;
mod connection;
mod document_vault;
mod event_store;
mod execution;
mod execution_archive;
mod keyring_init;
mod projection_query;
mod retention_recovery;
mod schema;
mod secret_artifact;
mod storage;
pub mod types;
mod writer;

pub use storage::EventProjectionStorage;
pub use types::{
    AppendRequest, ArtifactInput, ArtifactKind, CandidateDocumentInput, CandidateDraft,
    CandidateSummary, CheckpointReceipt, CommitReceipt, CreateSourceDocumentInput,
    CredentialSlotMaterial, CredentialSlotSummary, DEFAULT_ARCHIVE_TTL_MS,
    DEFAULT_CANDIDATE_TTL_MS, DeleteSourceDocumentInput, DeltaCommit, DocumentMutationOutcome,
    DocumentRef, DocumentValidationIssue, EditSourceDocumentCredentialInput, ExecutionFinish,
    ExecutionPin, ExecutionRecord, ExecutionReplayPin, ExecutionSourceCredentials, ExecutionStart,
    ExecutionStartReceipt, ExecutionStatus, GcReport, GcState, INSTALL_CANDIDATE_SCHEMA_VERSION,
    InstallCandidateRequest, InstalledSource, InstalledSourceRecord, InstalledSourceSnapshot,
    LibraryEntry, LibraryProgress, LibraryProjection, LibraryProjectionEntry,
    LibraryProjectionSnapshot, LibraryUpdate, LoadSourceDocumentRebaseMaterialInput,
    MAX_SOURCE_DOCUMENT_BYTES, MaskedSourceDocument, OrphanRecovery,
    PinSourceDocumentRevisionInput, ProjectionDelta, ProjectionTombstones,
    RebaseSourceDocumentInput, RelationTombstone, RenameSourceDocumentInput, ReplayExecutionStart,
    ReplayUnavailableReason, RetentionPolicy, RevealedSourceDocumentCredential,
    RuntimeCredentialMaterial, SOURCE_DOCUMENT_SCHEMA_VERSION, SaveSourceDocumentInput,
    SecretArtifactId, SourceDocumentCredentialTarget, SourceDocumentId, SourceDocumentMaterial,
    SourceDocumentRebaseCommitMode, SourceDocumentRebaseInvalidReason,
    SourceDocumentRebaseMaterialOutcome, SourceDocumentRebaseMaterials,
    SourceDocumentRevisionInput, SourceDocumentRevisionPin, SourceDocumentState,
    SourceDocumentSummary, SourceProjectionSnapshot, SourceProjectionView, StorageConfig,
    StorageError, StoredEvent, TransientSourceDocumentInput, WRITER_CAPACITY,
};
