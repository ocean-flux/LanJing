//! 规范化媒体投影、checkpoint 与保留策略 DTO。
//!
//! projection 的 upsert/tombstone 与事件在同一 `SQLite` transaction 中提交；公开读取 DTO
//! 只包含领域状态与 BLAKE3 引用，不暴露 Diesel 行或 secret plaintext。

use lj_media::{MediaGraphDelta, MediaResourceId, SourceProfile};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 规范化投影的 upsert 与 tombstone 集合。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionDelta {
    /// 需要按稳定 ID upsert 的标准资源。
    pub upserts: MediaGraphDelta,
    /// 需要删除的稳定资源或关系键。
    pub tombstones: ProjectionTombstones,
}

/// 投影 tombstone；删除与对应 event 在同一 `SQLite` transaction 生效。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectionTombstones {
    /// 删除的来源 profile ID。
    pub sources: Vec<MediaResourceId>,
    /// 删除的媒体主体 ID。
    pub items: Vec<MediaResourceId>,
    /// 删除的媒体集合 ID。
    pub collections: Vec<MediaResourceId>,
    /// 删除的消费单元 ID。
    pub units: Vec<MediaResourceId>,
    /// 删除的资产 ID。
    pub assets: Vec<MediaResourceId>,
    /// 删除的关系复合键。
    pub relations: Vec<RelationTombstone>,
    /// 删除的 action ID。
    pub actions: Vec<MediaResourceId>,
    /// 删除的展示提示所绑定资源 ID。
    pub hints: Vec<MediaResourceId>,
}

/// 可稳定定位一条 relation 的复合键。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationTombstone {
    /// 关系所属来源。
    pub source_id: MediaResourceId,
    /// 起点资源 ID。
    pub from_id: MediaResourceId,
    /// 终点资源 ID。
    pub to_id: MediaResourceId,
    /// relation enum 的 serde 值。
    pub relation_kind: String,
}

/// 一个执行 Delta 的事件与 O(delta) 投影提交请求。
#[derive(Debug, Clone)]
pub struct DeltaCommit {
    /// execution ID。
    pub execution_id: Uuid,
    /// optimistic concurrency 所需的 execution stream 版本。
    pub expected_version: u64,
    /// 可重试事件 ID。
    pub event_id: Uuid,
    /// 安全 trace 标识。
    pub trace_id: String,
    /// 发生时刻（UTC epoch milliseconds）。
    pub occurred_at_ms: i64,
    /// 待提交的规范化变更。
    pub delta: ProjectionDelta,
}

/// 按来源分区的 checkpoint 内容。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceProjectionSnapshot {
    /// 稳定来源身份。
    pub source_identity: String,
    /// snapshot 覆盖到的全局序号。
    pub global_seq: u64,
    /// 生成时 source stream revision。
    pub source_revision: u64,
    /// 该来源的当前规范化资源。
    pub delta: MediaGraphDelta,
}

/// 资源查询返回的简短组合视图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceProjectionView {
    /// 来源 profile；尚未安装或已 tombstone 时为空。
    pub profile: Option<SourceProfile>,
    /// 当前规范化资源。
    pub delta: MediaGraphDelta,
}
