//! library 与标准媒体查询 DTO。

use lj_media::{MediaAsset, MediaUnit};
use serde::{Deserialize, Serialize};

/// 资料库消费进度。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryProgress {
    /// 当前消费单元。
    pub unit_id: Option<String>,
    /// 当前位置。
    pub position: u64,
    /// 可选总长度。
    pub total: Option<u64>,
}

/// 单个资源的安全资料库条目。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryProjectionEntry {
    /// 标准媒体资源 ID。
    pub resource_id: String,
    /// 是否收藏。
    pub favorite: bool,
    /// 是否固定。
    pub pinned: bool,
    /// 最近打开时间。
    pub last_opened_at: Option<String>,
    /// 可选进度。
    pub progress: Option<LibraryProgress>,
    /// resource stream revision。
    pub revision: u64,
    /// 最近更新 global sequence。
    pub updated_global_seq: u64,
}

/// 完整安全资料库快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryProjection {
    /// 快照 global sequence。
    pub global_seq: u64,
    /// 按稳定 ID 排序的条目。
    pub entries: Vec<LibraryProjectionEntry>,
}

/// 更新资料库条目请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryEntryUpdate {
    /// 标准媒体资源 ID。
    pub resource_id: String,
    /// 是否收藏。
    pub favorite: bool,
    /// 是否固定。
    pub pinned: bool,
    /// 最近打开时间。
    pub last_opened_at: Option<String>,
    /// 可选进度。
    pub progress: Option<LibraryProgress>,
    /// optimistic version。
    pub expected_version: u64,
}

/// 资料库更新 durable receipt。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryUpdateReceipt {
    /// 提交后 global sequence。
    pub global_seq: u64,
    /// 提交后 resource revision。
    pub revision: u64,
}

/// 有界消费单元页。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaUnitPage {
    /// 本页单元。
    pub items: Vec<MediaUnit>,
    /// 请求 offset。
    pub offset: u32,
    /// 请求 limit。
    pub limit: u32,
    /// 是否还有后续页。
    pub has_more: bool,
    /// 父 item 是否存在。
    pub parent_found: bool,
}

/// 有界资产页。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAssetPage {
    /// 本页资产。
    pub items: Vec<MediaAsset>,
    /// 请求 offset。
    pub offset: u32,
    /// 请求 limit。
    pub limit: u32,
    /// 是否还有后续页。
    pub has_more: bool,
    /// 父 unit 是否存在。
    pub parent_found: bool,
}
