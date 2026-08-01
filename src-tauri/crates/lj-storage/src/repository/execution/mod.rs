//! Execution archive 生命周期、revision pin 与事件 catch-up。
//!
//! live start 由单 writer transaction 同时追加 Started Event 和固定 `(source identity,
//! source revision)`；成功只返回 [`ExecutionStartReceipt`]，其中 installed snapshot 与 pin 来自
//! 同一 immutable `source_versions` 行。所有 execution 都必须固定 current source revision，
//! replay 绝不读取 current source 猜测。

use std::str::FromStr;

use crate::database::DatabaseSession;
use crate::database::OptionalResultExt;
use crate::database::statement;
use lj_media::SourceProfile;
use lj_rule_model::{EventType, PolicyCapabilities};
use lj_runtime::ExecutionMode;
use sea_orm::FromQueryResult;
use uuid::Uuid;

use crate::artifact::ArtifactStore;
use crate::mapper::event::stored_event_from_row;
use crate::repository::candidate_source::{
    canonical_plan_hash, get_source_row, grant_covers, read_execution_plan_artifact,
    read_rule_package_artifact, source_cookie_namespace, source_version_owner_id,
    validate_candidate_package_and_plan,
};
use crate::repository::event::{
    ArtifactLink, EventDraft, append_event_transaction, database_error, deserialize,
    ensure_blake3_hash, from_i64, idempotent_event, read_body_by_hash, serialize, to_i64,
};
use crate::repository::projection::{apply_projection_delta, validate_delta_source};
use crate::repository::secret::read_owned_secret;
use crate::types::{
    ArtifactKind, CommitReceipt, DeltaCommit, ExecutionFinish, ExecutionPin, ExecutionRecord,
    ExecutionReplayPin, ExecutionSourceCredentials, ExecutionStart, ExecutionStartReceipt,
    ExecutionStatus, GcState, InstalledSourceSnapshot, ReplayExecutionStart, SecretArtifactId,
    StorageError, StoredEvent,
};

include!("start.rs");
include!("commit.rs");
include!("read.rs");
include!("events_rows.rs");
