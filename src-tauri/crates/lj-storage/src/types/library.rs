//! 用户拥有的 library aggregate 与规范化投影读取 DTO。
//!
//! 资料库状态与媒体空间共享稳定 resource ID，但只保存用户收藏、固定和进度；它不复制
//! source Graph、规则 Plan 或任何 secret。查询在同一只读 transaction 取得 global sequence
//! 与按资源 ID 排序的 entries，写回使用每条 entry 的 expected revision。

use lj_media::MediaResourceId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 用户唯一拥有的资料库进度。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryProgress {
    /// 当前消费单元；没有单元时为空。
    pub unit_id: Option<MediaResourceId>,
    /// 当前消费位置。
    pub position: u64,
    /// 可选总长度。
    pub total: Option<u64>,
}

/// 用户唯一拥有的资料库状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryEntry {
    /// 标准资源 ID。
    pub resource_id: MediaResourceId,
    /// 是否收藏。
    pub favorite: bool,
    /// 是否固定。
    pub pinned: bool,
    /// 最近打开时间（RFC3339 文本）。
    pub last_opened_at: Option<String>,
    /// 消费进度。
    pub progress: Option<LibraryProgress>,
}

/// 单个资料库投影条目及其并发控制版本。
///
/// 所有字段均为用户拥有的资料库状态；不含媒体 Graph、规则 Plan 或 secret。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryProjectionEntry {
    /// 标准资源 ID。
    pub resource_id: MediaResourceId,
    /// 是否收藏。
    pub favorite: bool,
    /// 是否固定。
    pub pinned: bool,
    /// 最近打开时间（RFC3339 文本）。
    pub last_opened_at: Option<String>,
    /// 消费进度。
    pub progress: Option<LibraryProgress>,
    /// 此资源 library stream 的当前 revision；写回时作为 expected version。
    pub revision: u64,
    /// 最后一次投影变更的全局序号。
    pub updated_global_seq: u64,
}

/// 完整、稳定的资料库投影读取 DTO。
///
/// `global_seq` 与 `entries` 在同一 `SQLite` read transaction 中取得；`entries` 按稳定资源 ID
/// 升序排列。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryProjection {
    /// 该完整快照覆盖到的全局事件序号。
    pub global_seq: u64,
    /// 当前全部用户拥有的资料库条目。
    pub entries: Vec<LibraryProjectionEntry>,
}

impl LibraryEntry {
    /// 创建没有用户状态的资料库条目。
    #[must_use]
    pub fn new(resource_id: MediaResourceId) -> Self {
        Self {
            resource_id,
            favorite: false,
            pinned: false,
            last_opened_at: None,
            progress: None,
        }
    }
}

/// 资料库投影更新请求。
#[derive(Debug, Clone)]
pub struct LibraryUpdate {
    /// 待持久化的用户状态。
    pub entry: LibraryEntry,
    /// 对该资源 library stream 的 expected version。
    pub expected_version: u64,
    /// 可重试事件 ID。
    pub event_id: Uuid,
    /// 发生时刻（UTC epoch milliseconds）。
    pub occurred_at_ms: i64,
    /// 安全 trace 标识。
    pub trace_id: String,
}

/// library aggregate 的 checkpoint 内容。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryProjectionSnapshot {
    /// snapshot 覆盖到的全局序号。
    pub global_seq: u64,
    /// 当前全部资料库用户状态。
    pub entries: Vec<LibraryEntry>,
}
