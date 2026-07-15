//! 共享资料库状态与标准媒体资源图投影。

use lj_core::media::{MediaGraphDelta, MediaResourceId};
use serde::{Deserialize, Serialize};

/// 用户对单个标准媒体资源持有的进度。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryProgress {
    /// 当前消费单元。资源没有单元时为空。
    pub unit_id: Option<MediaResourceId>,
    /// 当前消费位置。
    pub position: u64,
    /// 可选的总长度。
    pub total: Option<u64>,
}

/// 资料库唯一拥有的用户状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryEntry {
    /// 标准媒体资源 ID。
    pub resource_id: MediaResourceId,
    /// 是否收藏。
    pub favorite: bool,
    /// 是否固定。
    pub pinned: bool,
    /// 最近打开时间，使用 RFC 3339 文本。
    pub last_opened_at: Option<String>,
    /// 消费进度。
    pub progress: Option<LibraryProgress>,
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

    /// 判断条目是否应该出现在资料库投影中。
    #[must_use]
    pub fn is_owned(&self) -> bool {
        self.favorite || self.pinned || self.last_opened_at.is_some() || self.progress.is_some()
    }
}

/// 标准媒体资源图和共享用户状态的资料库投影。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryProjection {
    /// 权威标准媒体资源图，包含来源与明确关系。
    pub graph: MediaGraphDelta,
    /// 资源库唯一拥有的用户状态。
    pub entries: Vec<LibraryEntry>,
}
