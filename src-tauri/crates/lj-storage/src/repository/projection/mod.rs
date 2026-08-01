//! 规范化媒体 projection、library aggregate 与只读查询实现。
//!
//! 每个 Delta 在 writer transaction 内按稳定 resource ID O(delta) upsert/tombstone；绝不读取
//! 或重写整张 JSON graph。source/item/unit/asset 查询只走独立只读 connection，结果以稳定 SQL
//! 顺序返回。library 是用户独有 aggregate，共享 resource identity 但不复制 source Graph、Plan
//! 或 secret。

use crate::database::DatabaseSession;
use crate::database::OptionalResultExt;
use crate::database::statement;
use lj_media::{
    MediaAction, MediaAsset, MediaCollection, MediaGraphDelta, MediaItem, MediaRelation,
    MediaResourceId, MediaUnit, PresentationHint, SourceProfile,
};
use lj_rule_model::EventType;
use sea_orm::FromQueryResult;

use crate::repository::event::{
    EventDraft, append_event_transaction, current_global_seq, database_error, deserialize,
    from_i64, idempotent_event, serialize, to_i64,
};
use crate::types::{
    CommitReceipt, LibraryEntry, LibraryProgress, LibraryProjection, LibraryProjectionEntry,
    LibraryUpdate, ProjectionDelta, ProjectionTombstones, SourceProjectionView, StorageError,
};

include!("write.rs");
include!("read.rs");
