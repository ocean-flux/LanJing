//! Execution archive、replay pin 与两阶段 GC DTO。
//!
//! replay DTO 只在 source-version、Plan 与 artifact 引用已经验证后返回；GC 状态必须按
//! `active → marked → external_refs_removed → finalized` 单向推进。

use lj_media::SourceProfile;
use lj_rule_model::{ExecutionPlan, PolicyCapabilities};
use lj_runtime::ExecutionMode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::StorageError;

/// 仅供执行内部使用的已解密 source credential material。
///
/// 不实现 `Debug`、`Clone` 或序列化，防止 secret 进入日志、DTO 或默认导出。
pub struct ExecutionSourceCredentials {
    cookie_namespace: String,
    secret_bytes: Option<Vec<u8>>,
}

impl ExecutionSourceCredentials {
    pub(crate) fn new(cookie_namespace: String, secret_bytes: Option<Vec<u8>>) -> Self {
        Self {
            cookie_namespace,
            secret_bytes,
        }
    }

    /// 返回 source-owned cookie namespace。
    #[must_use]
    pub fn cookie_namespace(&self) -> &str {
        &self.cookie_namespace
    }

    /// 返回只应传给执行适配器的解密 credential bytes。
    #[must_use]
    pub fn secret_bytes(&self) -> Option<&[u8]> {
        self.secret_bytes.as_deref()
    }

    /// 消费 carrier 并把 secret bytes 的所有权移交给执行适配器，避免额外复制。
    #[must_use]
    pub fn into_secret_bytes(self) -> Option<Vec<u8>> {
        self.secret_bytes
    }
}

/// 建立一个 execution archive 的请求。
#[derive(Debug, Clone)]
pub struct ExecutionStart {
    /// 新 execution ID。
    pub execution_id: Uuid,
    /// 已安装来源稳定身份。
    pub source_identity: String,
    /// 可重试事件 ID。
    pub event_id: Uuid,
    /// 安全 trace 标识。
    pub trace_id: String,
    /// 启动时刻（UTC epoch milliseconds）。
    pub started_at_ms: i64,
    /// 可选关联 ID。
    pub correlation_id: Option<Uuid>,
}

/// 基于已验证历史 pin 建立新 execution archive 的请求。
#[derive(Debug, Clone)]
pub struct ReplayExecutionStart {
    /// 新 replay execution ID。
    pub execution_id: Uuid,
    /// 必须来自 [`crate::EventProjectionStorage::load_execution_replay_pin`] 的历史 pin。
    pub pin: ExecutionReplayPin,
    /// 可重试事件 ID。
    pub event_id: Uuid,
    /// 安全 trace 标识。
    pub trace_id: String,
    /// 启动时刻（UTC epoch milliseconds）。
    pub started_at_ms: i64,
    /// 可选关联 ID。
    pub correlation_id: Option<Uuid>,
}

/// execution 终态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// 正在执行。
    Running,
    /// 正常完成。
    Completed,
    /// 执行失败。
    Failed,
    /// 被调用方取消。
    Cancelled,
    /// 进程恢复时发现缺少终态。
    Incomplete,
}

impl ExecutionStatus {
    /// 判断是否已经进入唯一终态。
    #[must_use]
    pub fn is_terminal(self) -> bool {
        !matches!(self, Self::Running)
    }

    /// 返回 `SQLite` 使用的稳定文本。
    #[must_use]
    pub fn as_db(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Incomplete => "incomplete",
        }
    }

    /// 从 `SQLite` 稳定文本还原状态。
    ///
    /// # Errors
    ///
    /// 数据库出现未知状态时返回 [`StorageError::InvalidInput`]。
    pub fn from_db(value: &str) -> Result<Self, StorageError> {
        match value {
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "incomplete" => Ok(Self::Incomplete),
            _ => Err(StorageError::InvalidInput(
                "未知 execution 状态".to_string(),
            )),
        }
    }
}

/// execution archive 的两阶段回收状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcState {
    /// archive 正常保留。
    Active,
    /// 已写入并验证 aggregate checkpoint，等待移除外部引用。
    Marked,
    /// 已从 `SQLite` ref 移除，等待文件/keyring metadata finalization。
    ExternalRefsRemoved,
    /// archive Event/ref 已删除，不可 replay。
    Finalized,
}

impl GcState {
    /// 从 `SQLite` 稳定文本还原状态。
    ///
    /// # Errors
    ///
    /// 数据库出现未知状态时返回 [`StorageError::InvalidInput`]。
    pub fn from_db(value: &str) -> Result<Self, StorageError> {
        match value {
            "active" => Ok(Self::Active),
            "marked" => Ok(Self::Marked),
            "external_refs_removed" => Ok(Self::ExternalRefsRemoved),
            "finalized" => Ok(Self::Finalized),
            _ => Err(StorageError::InvalidInput(
                "未知 archive GC 状态".to_string(),
            )),
        }
    }

    /// 返回 `SQLite` 使用的稳定文本。
    #[must_use]
    pub const fn as_db(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Marked => "marked",
            Self::ExternalRefsRemoved => "external_refs_removed",
            Self::Finalized => "finalized",
        }
    }
}

/// 将 execution 推进到终态的请求。
#[derive(Debug, Clone)]
pub struct ExecutionFinish {
    /// execution ID。
    pub execution_id: Uuid,
    /// optimistic concurrency 所需的 execution stream 版本。
    pub expected_version: u64,
    /// 可重试事件 ID。
    pub event_id: Uuid,
    /// 终态；不得为 `Running`。
    pub status: ExecutionStatus,
    /// 终态发生时刻（UTC epoch milliseconds）。
    pub finished_at_ms: i64,
    /// 安全 trace 标识。
    pub trace_id: String,
}

/// 修改 execution archive pin 的请求。
#[derive(Debug, Clone)]
pub struct ExecutionPin {
    /// execution ID。
    pub execution_id: Uuid,
    /// optimistic concurrency 所需的 execution stream 版本。
    pub expected_version: u64,
    /// 可重试事件 ID。
    pub event_id: Uuid,
    /// 是否固定 archive。
    pub pinned: bool,
    /// 发生时刻（UTC epoch milliseconds）。
    pub occurred_at_ms: i64,
    /// 安全 trace 标识。
    pub trace_id: String,
}

/// execution summary 查询结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRecord {
    /// execution ID。
    pub execution_id: Uuid,
    /// 关联来源。
    pub source_identity: String,
    /// 已 pin 的 Plan hash。
    pub plan_hash: String,
    /// 当前状态。
    pub status: ExecutionStatus,
    /// archive 是否固定。
    pub pinned: bool,
    /// archive 是否还可 replay。
    pub replayable: bool,
    /// 两阶段 archive 回收状态。
    pub gc_state: GcState,
    /// 起始时刻。
    pub started_at_ms: i64,
    /// 可选终态时刻。
    pub finished_at_ms: Option<i64>,
    /// execution stream 当前版本。
    pub revision: u64,
}

/// 从 execution archive 读取的不可变 replay pin。
///
/// 此 DTO 只在 archive 仍可 replay，且 source-version snapshot、artifact BLAKE3 与 Plan
/// canonical hash 均已验证后返回；调用方必须使用其中字段，不得回退到当前安装来源配置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionReplayPin {
    /// 被固定的历史 execution。
    pub execution_id: Uuid,
    /// 历史 execution 所属的稳定来源身份。
    pub source_identity: String,
    /// execution 启动时固定的来源版本。
    pub source_version: String,
    /// 固定 source version 的展示资料。
    pub profile: SourceProfile,
    /// 固定 source version 的已批准能力。
    pub grant: PolicyCapabilities,
    /// 固定 source version 的 canonical base URL。
    pub base_url: String,
    /// source package body artifact 的 BLAKE3 ref。
    pub package_artifact_hash: String,
    /// 已验证的 immutable Plan artifact 内容。
    pub plan: ExecutionPlan,
    /// Plan canonical BLAKE3 hash。
    pub plan_hash: String,
    /// Plan body artifact 的 BLAKE3 ref。
    pub plan_artifact_hash: String,
    /// 只能用于该 archive 的 replay 模式。
    pub mode: ExecutionMode,
}
