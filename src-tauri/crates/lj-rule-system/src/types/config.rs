//! `RuleSystem` 本地配置。

use std::path::PathBuf;
use std::time::Duration;

/// `RuleSystem` 本地持久化与有界执行配置。
#[derive(Debug, Clone)]
pub struct RuleSystemConfig {
    pub(crate) database_path: PathBuf,
    pub(crate) artifact_root: PathBuf,
    pub(crate) keyring_service: String,
    pub(crate) candidate_ttl: Duration,
    pub(crate) session_event_capacity: usize,
    pub(crate) max_concurrent_executions: usize,
    pub(crate) max_concurrent_effects: usize,
    pub(crate) max_concurrent_effects_per_source: usize,
    pub(crate) local_fixture_http: bool,
}

impl RuleSystemConfig {
    /// 用桌面默认容量创建配置。
    #[must_use]
    pub fn desktop(database_path: PathBuf, artifact_root: PathBuf) -> Self {
        Self {
            database_path,
            artifact_root,
            keyring_service: "lanjing.event-store.master-key".to_string(),
            candidate_ttl: Duration::from_hours(24),
            session_event_capacity: 64,
            max_concurrent_executions: 16,
            max_concurrent_effects: 16,
            max_concurrent_effects_per_source: 4,
            local_fixture_http: false,
        }
    }

    /// 创建允许环回地址的真实本地 fixture 配置。
    #[cfg(feature = "test-support")]
    #[must_use]
    pub fn local_fixture(database_path: PathBuf, artifact_root: PathBuf) -> Self {
        Self {
            local_fixture_http: true,
            ..Self::desktop(database_path, artifact_root)
        }
    }

    /// 覆盖 keyring service。
    #[must_use]
    pub fn with_keyring_service(mut self, keyring_service: String) -> Self {
        self.keyring_service = keyring_service;
        self
    }

    /// 覆盖 candidate TTL。
    #[must_use]
    pub fn with_candidate_ttl(mut self, candidate_ttl: Duration) -> Self {
        self.candidate_ttl = candidate_ttl;
        self
    }
}
