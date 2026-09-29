//! Candidate composite publish、source install/update 与 revision-keyed snapshot。
//!
//! 一个 writer command 先 durable 写完 package/Plan 与全部随机 secret 文件，再由同一个 Event
//! transaction 发布 candidate row/owners。install 重新验证 schema、expiry、contract hash 与
//! installed-source baseline，在一个 source Event transaction 中消费 candidate、追加 source
//! revision、固定 runtime credential ownership；失败不推进任一 projection。

use std::str::FromStr;

use crate::database::DatabaseSession;
use crate::database::OptionalResultExt;
use crate::database::statement;
use lj_media::SourceProfile;
use lj_rule_model::{
    ArtifactRef, EventType, ExecutionPlan, PolicyCapabilities, RulePackage, SchemaReadError,
    SecretRef, definition_hash, execution_plan_hash, read_execution_plan, read_rule_package,
};
use sea_orm::FromQueryResult;
use uuid::Uuid;

use crate::artifact::ArtifactStore;
use crate::repository::event::{
    ArtifactLink, EventDraft, append_event_transaction, database_error, deserialize, from_i64,
    idempotent_event, read_body_by_hash, remove_candidate_event_refs, serialize, to_i64,
};
use crate::repository::projection::upsert_projection_source;
use crate::repository::secret::{
    read_owned_secret, release_secret_owner, retain_existing_secret, retain_pending_secret,
    write_secret,
};
use crate::types::{
    ArtifactKind, CandidateDraft, CandidateSummary, DEFAULT_CANDIDATE_TTL_MS,
    INSTALL_CANDIDATE_SCHEMA_VERSION, InstallCandidateRequest, InstalledSource,
    InstalledSourceRecord, RuntimeCredentialMaterial, SecretArtifactId, SourceRevisionRecord,
    SourceRollbackRequest, StorageError,
};

include!("staging.rs");
include!("install.rs");
include!("read.rs");
include!("contract.rs");
include!("rows.rs");

#[cfg(test)]
mod tests {
    use super::*;
    include!("tests.rs");
}
