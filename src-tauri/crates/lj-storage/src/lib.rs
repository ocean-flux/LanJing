//! `SQLite` Event Store、规范化投影与 durable artifact archive。
//!
//! `EventProjectionStorage` 是 C4 `RuleSystem` 的具体存储模块：所有 event/projection
//! 写入经过容量固定为 256 的单 writer；同步 Diesel、文件系统与 keyring 工作只在
//! blocking lane 执行；读请求使用独立 `SQLite` 连接。旧 Graph Repository 与单 JSON
//! media graph 已被完全移除。

mod artifact;
mod candidate_install;
mod connection;
mod event_store;
mod execution;
mod execution_archive;
mod projection_query;
mod retention_recovery;
mod schema;
mod storage;
pub mod types;
mod writer;

pub use storage::EventProjectionStorage;
pub use types::{
    AppendRequest, ArtifactInput, ArtifactKind, CandidateDraft, CandidateSummary,
    CheckpointReceipt, CommitReceipt, DEFAULT_ARCHIVE_TTL_MS, DEFAULT_CANDIDATE_TTL_MS,
    DeltaCommit, ExecutionFinish, ExecutionPin, ExecutionRecord, ExecutionReplayPin,
    ExecutionSourceCredentials, ExecutionStart, ExecutionStatus, GcReport, GcState,
    InstallCandidateRequest, InstalledSource, InstalledSourceRecord, LibraryEntry, LibraryProgress,
    LibraryProjection, LibraryProjectionEntry, LibraryProjectionSnapshot, LibraryUpdate,
    OrphanRecovery, ProjectionDelta, ProjectionTombstones, RelationTombstone, ReplayExecutionStart,
    RetentionPolicy, SourceCredentialInput, SourceCredentialSnapshot, SourceProjectionSnapshot,
    SourceProjectionView, StorageConfig, StorageError, StoredEvent, WRITER_CAPACITY,
};
