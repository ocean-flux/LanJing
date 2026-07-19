//! 存储实例与 single writer 配置 DTO。
//!
//! 配置仅描述真实文件系统路径、固定容量的 writer queue 与独立读连接上限；artifact/secret
//! material 的持久化合同位于 sibling `artifact` DTO 与 artifact store。

use std::path::PathBuf;

/// 单 writer 队列的固定容量。
pub const WRITER_CAPACITY: usize = 256;
/// 默认 execution archive 保留时长：30 天。
pub const DEFAULT_ARCHIVE_TTL_MS: i64 = 30 * 24 * 60 * 60 * 1_000;
/// candidate staging 的固定默认保留时长：24 小时。
pub const DEFAULT_CANDIDATE_TTL_MS: i64 = 24 * 60 * 60 * 1_000;

/// SQLite、artifact 根目录与保留策略。
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// `SQLite` 文件路径；测试和生产都必须使用真实文件，禁止 `:memory:`。
    pub database_path: PathBuf,
    /// artifact 根目录；body 与 secret 会在其下按 BLAKE3 fan-out 写入。
    pub artifact_root: PathBuf,
    /// 安装级主密钥的 keyring service 名。
    pub keyring_service: String,
    /// artifact 配额字节数。
    pub quota_bytes: u64,
    /// archive TTL；`None` 表示仅按容量回收。
    pub archive_ttl_ms: Option<i64>,
    /// 同时打开的独立读连接上限。
    pub read_concurrency: usize,
}

impl StorageConfig {
    /// 创建桌面默认配置。
    #[must_use]
    pub fn desktop(database_path: PathBuf, artifact_root: PathBuf) -> Self {
        Self {
            database_path,
            artifact_root,
            keyring_service: "lanjing.event-store.master-key".to_string(),
            quota_bytes: 2 * 1024 * 1024 * 1024,
            archive_ttl_ms: Some(DEFAULT_ARCHIVE_TTL_MS),
            read_concurrency: 4,
        }
    }

    /// 创建移动端默认配置。
    #[must_use]
    pub fn mobile(database_path: PathBuf, artifact_root: PathBuf) -> Self {
        Self {
            quota_bytes: 512 * 1024 * 1024,
            ..Self::desktop(database_path, artifact_root)
        }
    }
}
