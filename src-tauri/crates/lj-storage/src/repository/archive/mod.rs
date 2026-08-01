//! Execution effect archive 的持久化与 replay 边界。
//!
//! 此模块只处理 runtime 已执行 effect 的输出、typed witness 与可选 HTTP request material。
//! **持久化不变量**：body/secret 文件先在 writer blocking lane durable 写入，随后同一
//! `BEGIN IMMEDIATE` transaction 追加 execution Event、artifact refs、global sequence 与
//! `effect_captures` 行；任何一步失败都不得返回 receipt。**加密不变量**：request body 一律
//! 是 AES-256-GCM `Secret` Artifact，Event 仅含 BLAKE3 ref/hash。**replay 不变量**：C2
//! 在返回 runtime 前验证 artifact、secret、typed witness 与 output 的完整性，绝不重新暴露原始
//! request material。

use async_trait::async_trait;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use crate::database::DatabaseSession;
use crate::database::OptionalResultExt;
use crate::database::statement;
use lj_rule_model::{ControlTrace, EventType, InvocationPath, SensitiveNamePolicy, canonical_json};
use lj_runtime::{
    ArchivedEffectCapture, ControlReplayLookup, ControlTraceCapture, ControlTraceReceipt,
    DurableCaptureReceipt, EffectArchive, EffectArchiveError, EffectArchiveErrorCode,
    EffectCapture, EffectCaptureMaterialSensitivity, EffectOutput, EffectReplayLookup,
    EffectWitness, ReplayCompletionLookup, control_trace_hash,
};
use sea_orm::FromQueryResult;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::artifact::{ArtifactStore, PendingSecretArtifact};
use crate::repository::event::{
    EventDraft, append_event_transaction, database_error, deserialize, ensure_blake3_hash,
    now_millis, push_new_artifact_link, read_body_by_hash, serialize, stream_version, to_i64,
};
use crate::repository::execution::{
    execution_stream_id, get_execution_sync, update_execution_revision,
};
use crate::repository::secret::{read_owned_secret, retain_pending_secret, write_secret};
use crate::storage::EventProjectionStorage;

include!("prepare.rs");
include!("persist.rs");
include!("load.rs");
include!("control.rs");
include!("model.rs");
include!("ledger.rs");
include!("trait_impl.rs");
