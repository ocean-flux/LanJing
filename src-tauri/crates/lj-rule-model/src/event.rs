//! `EventEnvelope` — Event Store 统一事件信封。

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 事件类型标签。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// 候选/安装相关。
    Candidate,
    /// 来源版本/grant/profile。
    Source,
    /// 执行过程。
    Execution,
    /// 资料库用户真相。
    Library,
    /// 其他/扩展。
    Other(String),
}

/// 明文/压缩 body 内容寻址引用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRef {
    /// blake3 内容 hash（hex）。
    pub hash: String,
    /// 编解码标识（如 `zstd`）。
    pub codec: String,
}

/// 加密 secret 引用（Event 不存明文）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecretRef {
    /// 加密后内容 hash。
    pub hash: String,
    /// 算法标识（如 `aes-256-gcm`）。
    pub algorithm: String,
}

/// 统一事件信封。
///
/// 含 stream/global sequence、schema version、correlation/trace 字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// 全局单调序号（单 writer 分配）。
    pub global_seq: u64,
    /// 流 ID（如 `execution/{id}`）。
    pub stream_id: String,
    /// 流内版本（optimistic concurrency）。
    pub stream_version: u64,
    /// 事件 ID。
    pub event_id: Uuid,
    /// 事件类型。
    pub event_type: EventType,
    /// 事件 payload schema 版本。
    pub schema_version: u32,
    /// 关联 ID（同一业务请求）。
    pub correlation_id: Option<Uuid>,
    /// 因果父事件。
    pub causation_id: Option<Uuid>,
    /// 分布式/本地 trace。
    pub trace_id: String,
    /// 发生时间（RFC3339 或 epoch millis 字符串，调用方约定）。
    pub occurred_at: String,
    /// 业务载荷。
    pub payload: serde_json::Value,
    /// body artifact 引用。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_refs: Vec<ArtifactRef>,
    /// secret artifact 引用。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secret_refs: Vec<SecretRef>,
}
