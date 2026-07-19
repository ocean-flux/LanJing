//! Checkpoint、retention 与 orphan recovery DTO。
//!
//! checkpoint 始终按 source/library aggregate 分区，不能复制整个 `SQLite` 文件。GC 只能沿
//! `active → marked → external_refs_removed → finalized` 单向推进；每一步可重试，避免跨
//! `SQLite`、文件与 keyring 的非 ACID 边界被误报为已完成。

/// checkpoint 成功写入后的 metadata。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointReceipt {
    /// aggregate 标识。
    pub aggregate_id: String,
    /// 覆盖到的全局序号。
    pub global_seq: u64,
    /// checkpoint body artifact BLAKE3 hash。
    pub artifact_hash: String,
}

/// artifact GC 的策略；使用量达到配额 90% 即触发容量路径。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// 最大 artifact 存储字节。
    pub quota_bytes: u64,
    /// execution archive TTL；`None` 表示禁用时间路径。
    pub archive_ttl_ms: Option<i64>,
}

/// GC 处理的状态汇总。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct GcReport {
    /// 被标记进入两阶段删除的 execution 数。
    pub marked: usize,
    /// 已移除外部 artifact 引用的 execution 数。
    pub external_refs_removed: usize,
    /// 已完成 metadata/file cleanup 的 execution 数。
    pub finalized: usize,
    /// 因 24 小时 staging 到期而失效的 candidate 数。
    pub expired_candidates: usize,
}

/// artifact orphan 恢复统计。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OrphanRecovery {
    /// 删除的 temp 或无 metadata 文件数。
    pub removed_files: usize,
    /// 发现但保留的 metadata 引用数。
    pub referenced_files: usize,
}
