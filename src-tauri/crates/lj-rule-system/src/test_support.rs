//! 集成测试共享 harness：TempRuleSystem、mock keyring、SQLite 测试数据库与 vault key 清理。
//!
//! 本模块按 `#[cfg(feature = "test-support")]` 编译，提供两个测试文件（legado / maccms）中最窄的
//! 重复基础设施。格式特有 fixture、断言与 `captured_response_secret_files` 等 legado 特有函数留在各自测试文件。

use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::Duration;

use keyring_core::{Entry, mock, set_default_store};
use sea_orm::{
    ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, FromQueryResult, Statement,
};
use uuid::Uuid;

use crate::{RuleError, RuleErrorStage, RuleSystem, RuleSystemConfig};

/// 内存测试用临时 `RuleSystem`。
///
/// 封装 temp dir、mock keyring service name 和生命周期。创建临时目录后自动清理。
/// 签名统一为候选 TTL 版（maccms 超集），legado 调用处传默认 `Duration::from_secs(300)`。
pub struct TempRuleSystem {
    root: PathBuf,
    keyring_service: String,
}

impl TempRuleSystem {
    /// 创建新临时 `RuleSystem`，名称用于区分目录。
    ///
    /// # Panics
    ///
    /// 临时目录创建失败时 panic。
    #[must_use]
    pub fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("lj-rule-system-{name}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("创建 RuleSystem 测试根目录");
        let keyring_service = format!("lanjing.rule-system.test.{}", Uuid::new_v4());
        Self {
            root,
            keyring_service,
        }
    }

    /// 临时目录路径。
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Mock keyring service name。
    #[must_use]
    pub fn keyring_service(&self) -> &str {
        &self.keyring_service
    }

    /// `SQLite` 事件存储路径。
    #[must_use]
    pub fn database_path(&self) -> PathBuf {
        self.root.join("event-store.db")
    }

    /// 打开 `RuleSystem`。
    ///
    /// # Panics
    ///
    /// 打开失败时 panic。
    pub async fn open(&self, candidate_ttl: Duration) -> RuleSystem {
        self.open_result(candidate_ttl)
            .await
            .expect("打开 concrete RuleSystem")
    }

    /// Drop 后重开（带重试适应 storage writer 退出延迟）。
    ///
    /// # Panics
    ///
    /// 重试耗尽仍无法重开时 panic。
    pub async fn reopen_after_drop(&self, candidate_ttl: Duration) -> RuleSystem {
        for attempt in 0..20 {
            match self.open_result(candidate_ttl).await {
                Ok(system) => return system,
                Err(error) if error.stage == RuleErrorStage::Persistence && attempt < 19 => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(error) => panic!("同进程 storage writer 退出后无法重开 RuleSystem: {error:?}"),
            }
        }
        unreachable!("有界重开循环应在成功或最终错误时退出")
    }

    /// 尝试打开 `RuleSystem`，返回 `Result`。
    ///
    /// # Errors
    ///
    /// 返回底层 `RuleSystem::open` 的原始错误。
    pub async fn open_result(&self, candidate_ttl: Duration) -> Result<RuleSystem, RuleError> {
        RuleSystem::open(
            RuleSystemConfig::local_fixture(
                self.root.join("event-store.db"),
                self.root.join("artifacts"),
            )
            .with_keyring_service(self.keyring_service.clone())
            .with_candidate_ttl(candidate_ttl),
        )
        .await
    }
}

impl Drop for TempRuleSystem {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// 初始化一次 mock keyring store。
///
/// 使用 `keyring_core::mock::Store` 替代系统 keyring，确保测试不写真实凭据。
///
/// # Panics
///
/// mock store 创建失败时 panic。
pub fn init_mock_keyring() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        set_default_store(mock::Store::new().expect("keyring-core mock store"));
    });
}

/// 打开 sqlite 测试数据库连接。
///
/// 路径中反斜杠自动转正斜杠以兼容 Windows。
///
/// # Panics
///
/// 数据库连接失败时 panic。
pub async fn open_test_database(path: &Path) -> DatabaseConnection {
    let normalized = path.to_string_lossy().replace('\\', "/");
    Database::connect(format!("sqlite://{normalized}?mode=rw"))
        .await
        .expect("打开真实 SQLite")
}

#[derive(FromQueryResult)]
struct VaultKeyRow {
    key_id: String,
}

/// 删除指定 database 中 `vault_key_metadata` id=1 对应的 keyring entry。
///
/// 用于测试丢失 master key 后的拒绝行为。
///
/// # Panics
///
/// 查询 vault key 元数据、读取 keyring entry 或删除 credential 失败时 panic。
pub async fn wipe_vault_key(database_path: &Path, keyring_service: &str) {
    let connection = open_test_database(database_path).await;
    let query = Statement::from_string(
        DatabaseBackend::Sqlite,
        "SELECT key_id FROM vault_key_metadata WHERE id = 1",
    );
    let row = connection
        .query_one_raw(query)
        .await
        .expect("读取 vault key ID");
    let row = VaultKeyRow::from_query_result(&row.expect("vault key metadata"), "")
        .expect("map vault key metadata");
    let entry = Entry::new(keyring_service, &format!("vault-key-v1/{}", row.key_id))
        .expect("vault key entry");
    entry.delete_credential().expect("删除 mock vault key");
}
