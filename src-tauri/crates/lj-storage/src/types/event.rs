//! Event envelope 与执行投影写入 DTO。
//!
//! `AppendRequest` 的 expected version、Event、artifact ref 与投影更新由单 writer 在同一个
//! `BEGIN IMMEDIATE` transaction 中提交；成功 receipt 的 sequence/revision 因而可作为
//! catch-up 和 replay 的稳定边界。

use lj_rule_model::{ArtifactRef, EventEnvelope, EventType, SecretRef};
use uuid::Uuid;

use super::ArtifactInput;

/// 待追加的无投影事件。
#[derive(Debug, Clone)]
pub struct AppendRequest {
    /// 事件流标识。
    pub stream_id: String,
    /// optimistic concurrency 所需的当前流版本。
    pub expected_version: u64,
    /// 全局唯一、可用于幂等重试的事件 ID。
    pub event_id: Uuid,
    /// 事件分类。
    pub event_type: EventType,
    /// payload schema 版本。
    pub schema_version: u32,
    /// 可选关联 ID。
    pub correlation_id: Option<Uuid>,
    /// 可选因果父事件 ID。
    pub causation_id: Option<Uuid>,
    /// 安全 trace 标识。
    pub trace_id: String,
    /// 发生时刻（UTC epoch milliseconds）。
    pub occurred_at_ms: i64,
    /// 业务 payload；不得包含 secret。
    pub payload: serde_json::Value,
    /// 归属来源；用于 checkpoint 后来源事件扫描。
    pub source_id: Option<String>,
    /// 需要与 Event 同时建立引用的 artifact。
    pub artifacts: Vec<ArtifactInput>,
}

/// 事件提交成功后的 durable 收据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitReceipt {
    /// 单调全局序号。
    pub global_seq: u64,
    /// 事件流标识。
    pub stream_id: String,
    /// 已提交的 stream 版本。
    pub stream_version: u64,
    /// 明文 body artifact 引用。
    pub artifact_refs: Vec<ArtifactRef>,
    /// encrypted secret artifact 引用。
    pub secret_refs: Vec<SecretRef>,
}

/// 将 `SQLite` 事件还原为带 epoch 时间的领域信封。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEvent {
    /// 原始 event envelope；`occurred_at` 为十进制 epoch milliseconds。
    pub envelope: EventEnvelope,
    /// 事件归属来源，供 source checkpoint catch-up 使用。
    pub source_identity: Option<String>,
}
