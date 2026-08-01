//! 原生规则文档生命周期与 provenance/凭证 secret 的真实 `SQLite` 合同测试。
//!
//! 每个测试创建真实临时 `SQLite` 文件与 artifact 目录；不使用 `:memory:` 或 mock ORM。
//! keyring 使用 keyring-core 官方 mock store；provenance 原文与凭证值仅以密文落盘。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

use keyring_core::{mock, set_default_store};
use lj_storage::{
    ClearDocumentCredentialSecretRequest, CreateDocumentRequest, DeleteDocumentRequest,
    DocumentInitial, EventProjectionStorage, LayoutSaveInput, LayoutSnapshot,
    ProvenanceCreateInput, RenameDocumentRequest, RevisionConflict, SaveDocumentOutcome,
    SaveDocumentRequest, SemanticSaveInput, SemanticSnapshot, StorageConfig, StorageError,
    WriteDocumentCredentialSecretRequest,
};
use sea_orm::{
    ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, FromQueryResult, Statement,
    Value,
};
use uuid::Uuid;

const NOW_MS: i64 = 1_750_000_000_000;

struct TestStatement {
    sql: String,
    values: Vec<Value>,
}

fn test_statement(sql: impl Into<String>) -> TestStatement {
    TestStatement {
        sql: sql.into(),
        values: Vec::new(),
    }
}

impl TestStatement {
    fn bind(mut self, value: impl Into<Value>) -> Self {
        self.values.push(value.into());
        self
    }

    async fn execute(self, connection: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
        connection
            .execute_raw(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                self.sql,
                self.values,
            ))
            .await
            .map(|_| ())
    }

    async fn load<T: FromQueryResult>(
        self,
        connection: &DatabaseConnection,
    ) -> Result<Vec<T>, sea_orm::DbErr> {
        connection
            .query_all_raw(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                self.sql,
                self.values,
            ))
            .await?
            .iter()
            .map(|row| T::from_query_result(row, ""))
            .collect()
    }
}

async fn open_raw(path: &Path) -> DatabaseConnection {
    let normalized = path.to_string_lossy().replace('\\', "/");
    Database::connect(format!("sqlite://{normalized}?mode=rw"))
        .await
        .expect("打开真实 SQLite 数据库")
}

fn init_mock_keyring() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        set_default_store(mock::Store::new().expect("keyring-core mock store"));
    });
}

struct TempStore {
    root: PathBuf,
    config: StorageConfig,
}

impl TempStore {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("lj-native-doc-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("创建测试根目录");
        let mut config =
            StorageConfig::desktop(root.join("event-store.db"), root.join("artifacts"));
        config.keyring_service = format!("lanjing.storage.test.{}", Uuid::new_v4());
        Self { root, config }
    }

    async fn open(&self) -> EventProjectionStorage {
        EventProjectionStorage::open(self.config.clone())
            .await
            .expect("打开真实 SQLite Event Store")
    }
}

impl Drop for TempStore {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn create_request(document_id: &str, title: &str) -> CreateDocumentRequest {
    CreateDocumentRequest {
        document_id: document_id.to_string(),
        format: "native_rule".to_string(),
        title: title.to_string(),
        source_identity: format!("native:{document_id}"),
        initial: DocumentInitial {
            semantic: None,
            layout: None,
        },
        provenance: None,
        trace_id: "trace".to_string(),
        occurred_at_ms: NOW_MS,
    }
}

fn hash(label: &str) -> String {
    blake3::hash(label.as_bytes()).to_hex().to_string()
}

fn with_semantic(
    mut request: CreateDocumentRequest,
    definition_json: &str,
    hash_seed: &str,
) -> CreateDocumentRequest {
    request.initial.semantic = Some(SemanticSnapshot {
        revision: 1,
        definition_json: definition_json.to_string(),
        definition_hash: hash(hash_seed),
        manifest_json: "{}".to_string(),
    });
    request
}

fn with_layout(mut request: CreateDocumentRequest, layout_json: &str) -> CreateDocumentRequest {
    request.initial.layout = Some(LayoutSnapshot {
        revision: 1,
        layout_json: layout_json.to_string(),
    });
    request
}

fn save_request(document_id: &str) -> SaveDocumentRequest {
    SaveDocumentRequest {
        document_id: document_id.to_string(),
        semantic: None,
        layout: None,
        trace_id: "trace".to_string(),
        occurred_at_ms: NOW_MS + 1,
    }
}

#[derive(FromQueryResult)]
struct SecretOwnerRow {
    owner_id: String,
    secret_id: String,
}

#[derive(FromQueryResult)]
struct RefCountRow {
    ref_count: i64,
}

#[derive(FromQueryResult)]
struct CountRow {
    count: i64,
}

#[tokio::test]
async fn create_and_reopen_round_trips_semantic_and_layout() {
    init_mock_keyring();
    let temp = TempStore::new("create-reopen");
    let storage = temp.open().await;

    let request = with_layout(
        with_semantic(create_request("doc-1", "初版标题"), r#"{"nodes":[]}"#, "a"),
        r#"{"nodes":[]}"#,
    );
    let summary = storage
        .create_native_rule_document(request)
        .await
        .expect("创建文档");
    assert_eq!(summary.document_id, "doc-1");
    assert_eq!(summary.format, "native_rule");
    assert_eq!(summary.state, "draft");
    assert_eq!(summary.semantic_revision, 1);
    assert_eq!(summary.layout_revision, 1);
    assert_eq!(summary.link_revision, 0);

    // 重开存储（同一真实文件）后读回，验证 durable。
    storage.shutdown().await.expect("关闭存储");
    let storage = temp.open().await;
    let detail = storage
        .get_native_rule_document("doc-1")
        .await
        .expect("读取文档")
        .expect("文档存在");
    assert_eq!(detail.summary.title, "初版标题");
    assert_eq!(detail.summary.source_identity, "native:doc-1");
    assert_eq!(detail.summary.semantic_revision, 1);
    assert_eq!(detail.summary.layout_revision, 1);
    let semantic = detail.semantic.expect("有 semantic 快照");
    assert_eq!(semantic.revision, 1);
    assert_eq!(semantic.definition_json, r#"{"nodes":[]}"#);
    assert_eq!(semantic.definition_hash.len(), 64);
    assert_eq!(detail.layout.expect("有 layout 快照").revision, 1);
    assert!(detail.provenance.is_none());

    let list = storage
        .list_native_rule_documents()
        .await
        .expect("列出文档");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].document_id, "doc-1");
    storage.shutdown().await.expect("关闭存储");
}

#[tokio::test]
async fn save_writes_valid_domain_and_reports_conflict_per_domain() {
    init_mock_keyring();
    let temp = TempStore::new("save-domains");
    let storage = temp.open().await;

    let request = with_layout(
        with_semantic(create_request("doc-save", "标题"), r#"{"v":1}"#, "b"),
        r#"{"v":1}"#,
    );
    storage
        .create_native_rule_document(request)
        .await
        .expect("创建文档");

    // semantic 期望 revision=1（有效，写 2）；layout 期望 999（冲突，不写）。
    let mut request = save_request("doc-save");
    request.semantic = Some(SemanticSaveInput {
        expected_revision: 1,
        definition_json: r#"{"v":2}"#.to_string(),
        definition_hash: "c".repeat(64),
        manifest_json: "{}".to_string(),
    });
    request.layout = Some(LayoutSaveInput {
        expected_revision: 999,
        layout_json: r#"{"v":99}"#.to_string(),
    });
    let outcome: SaveDocumentOutcome = storage
        .save_native_rule_document(request)
        .await
        .expect("保存文档");

    let semantic = outcome.semantic.expect("semantic outcome");
    assert_eq!(semantic.revision, 2);
    assert!(semantic.conflict.is_none(), "有效域不应冲突");
    let layout = outcome.layout.expect("layout outcome");
    assert_eq!(layout.revision, 1);
    assert_eq!(
        layout.conflict,
        Some(RevisionConflict {
            expected: 999,
            current: 1,
        })
    );

    // 冲突域未写、有效域已写。
    let detail = storage
        .get_native_rule_document("doc-save")
        .await
        .expect("读取")
        .expect("存在");
    assert_eq!(detail.summary.semantic_revision, 2);
    assert_eq!(detail.summary.layout_revision, 1);
    assert_eq!(detail.semantic.unwrap().definition_hash, "c".repeat(64));
    assert_eq!(detail.layout.unwrap().layout_json, r#"{"v":1}"#);
    storage.shutdown().await.expect("关闭存储");
}

#[tokio::test]
async fn save_both_conflicts_leave_document_untouched() {
    init_mock_keyring();
    let temp = TempStore::new("save-both-conflict");
    let storage = temp.open().await;

    let request = with_layout(
        with_semantic(create_request("doc-both", "标题"), r#"{"v":1}"#, "d"),
        r#"{"v":1}"#,
    );
    storage
        .create_native_rule_document(request)
        .await
        .expect("创建文档");

    let mut request = save_request("doc-both");
    request.semantic = Some(SemanticSaveInput {
        expected_revision: 42,
        definition_json: r#"{"v":2}"#.to_string(),
        definition_hash: "e".repeat(64),
        manifest_json: "{}".to_string(),
    });
    request.layout = Some(LayoutSaveInput {
        expected_revision: 43,
        layout_json: r#"{"v":2}"#.to_string(),
    });
    let outcome = storage
        .save_native_rule_document(request)
        .await
        .expect("保存文档");
    assert_eq!(
        outcome.semantic.unwrap().conflict,
        Some(RevisionConflict {
            expected: 42,
            current: 1,
        })
    );
    assert_eq!(
        outcome.layout.unwrap().conflict,
        Some(RevisionConflict {
            expected: 43,
            current: 1,
        })
    );

    // 双域都冲突 → 文档完全未动。
    let detail = storage
        .get_native_rule_document("doc-both")
        .await
        .expect("读取")
        .expect("存在");
    assert_eq!(detail.summary.semantic_revision, 1);
    assert_eq!(detail.summary.layout_revision, 1);
    assert_eq!(detail.semantic.unwrap().definition_hash, hash("d"));
    assert_eq!(detail.layout.unwrap().layout_json, r#"{"v":1}"#);
    storage.shutdown().await.expect("关闭存储");
}

#[tokio::test]
async fn first_semantic_save_after_blank_create_inserts_revision_one() {
    init_mock_keyring();
    let temp = TempStore::new("blank-first-save");
    let storage = temp.open().await;

    // blank 创建：无任何初始快照，revision 从 0 开始。
    let summary = storage
        .create_native_rule_document(create_request("doc-blank", "空白文档"))
        .await
        .expect("创建空白文档");
    assert_eq!(summary.semantic_revision, 0);
    assert_eq!(summary.layout_revision, 0);

    let mut request = save_request("doc-blank");
    request.semantic = Some(SemanticSaveInput {
        expected_revision: 0,
        definition_json: r#"{"nodes":[]}"#.to_string(),
        definition_hash: "f".repeat(64),
        manifest_json: "{}".to_string(),
    });
    let outcome = storage
        .save_native_rule_document(request)
        .await
        .expect("首次保存 semantic");
    let semantic = outcome.semantic.expect("semantic outcome");
    assert_eq!(semantic.revision, 1);
    assert!(semantic.conflict.is_none());

    let detail = storage
        .get_native_rule_document("doc-blank")
        .await
        .expect("读取")
        .expect("存在");
    assert_eq!(detail.summary.semantic_revision, 1);
    assert_eq!(detail.semantic.expect("有 semantic").revision, 1);
    storage.shutdown().await.expect("关闭存储");
}

#[tokio::test]
async fn rename_updates_title_without_advancing_semantic_revision() {
    init_mock_keyring();
    let temp = TempStore::new("rename");
    let storage = temp.open().await;

    let request = with_semantic(create_request("doc-rename", "旧标题"), r#"{"v":1}"#, "a1");
    storage
        .create_native_rule_document(request)
        .await
        .expect("创建文档");

    let summary = storage
        .rename_native_rule_document(RenameDocumentRequest {
            document_id: "doc-rename".to_string(),
            title: "新标题".to_string(),
            expected_revision: 1,
            trace_id: "trace".to_string(),
            occurred_at_ms: NOW_MS + 2,
        })
        .await
        .expect("重命名");
    assert_eq!(summary.title, "新标题");
    assert_eq!(
        summary.semantic_revision, 1,
        "重命名不推进 semantic revision"
    );

    let detail = storage
        .get_native_rule_document("doc-rename")
        .await
        .expect("读取")
        .expect("存在");
    assert_eq!(detail.summary.title, "新标题");
    assert_eq!(
        detail.semantic.unwrap().definition_hash,
        hash("a1"),
        "重命名不改 definition hash"
    );

    // 过期 expected revision 拒绝。
    let error = storage
        .rename_native_rule_document(RenameDocumentRequest {
            document_id: "doc-rename".to_string(),
            title: "过期重命名".to_string(),
            expected_revision: 99,
            trace_id: "trace".to_string(),
            occurred_at_ms: NOW_MS + 3,
        })
        .await
        .expect_err("过期 revision 必须拒绝");
    assert!(matches!(error, StorageError::InvalidInput(_)));
    storage.shutdown().await.expect("关闭存储");
}

#[tokio::test]
async fn delete_linked_document_requires_confirmation() {
    init_mock_keyring();
    let temp = TempStore::new("delete-guard");
    let storage = temp.open().await;

    // draft 文档可直接删除。
    storage
        .create_native_rule_document(create_request("doc-draft", "草稿"))
        .await
        .expect("创建草稿");
    storage
        .delete_native_rule_document(DeleteDocumentRequest {
            document_id: "doc-draft".to_string(),
            confirm_linked: false,
            trace_id: "trace".to_string(),
            occurred_at_ms: NOW_MS,
        })
        .await
        .expect("draft 删除无需确认");
    assert!(
        storage
            .get_native_rule_document("doc-draft")
            .await
            .expect("读取")
            .is_none(),
        "删除后文档应不存在"
    );

    // 模拟安装流程把文档置为 linked。
    storage
        .create_native_rule_document(create_request("doc-link", "已链接"))
        .await
        .expect("创建文档");
    let raw = open_raw(&temp.config.database_path).await;
    test_statement("UPDATE rule_documents SET state = 'linked' WHERE document_id = ?")
        .bind("doc-link")
        .execute(&raw)
        .await
        .expect("置为 linked");
    drop(raw);

    let error = storage
        .delete_native_rule_document(DeleteDocumentRequest {
            document_id: "doc-link".to_string(),
            confirm_linked: false,
            trace_id: "trace".to_string(),
            occurred_at_ms: NOW_MS,
        })
        .await
        .expect_err("linked 删除必须守卫");
    assert!(matches!(error, StorageError::InvalidInput(_)));

    storage
        .delete_native_rule_document(DeleteDocumentRequest {
            document_id: "doc-link".to_string(),
            confirm_linked: true,
            trace_id: "trace".to_string(),
            occurred_at_ms: NOW_MS,
        })
        .await
        .expect("确认后删除");
    assert!(
        storage
            .get_native_rule_document("doc-link")
            .await
            .expect("读取")
            .is_none()
    );
    storage.shutdown().await.expect("关闭存储");
}

#[tokio::test]
async fn provenance_secret_owner_refcount_and_gc_without_orphans() {
    init_mock_keyring();
    let temp = TempStore::new("provenance");
    let storage = temp.open().await;

    let mut request = create_request("doc-prov", "导入文档");
    request.provenance = Some(ProvenanceCreateInput {
        format: "legado".to_string(),
        adapter_version: "1.0".to_string(),
        input_hash: "11".repeat(32),
        source_text: "原始来源文本（含敏感 URL）".to_string(),
        diagnostics_json: "[]".to_string(),
        imported_at_ms: NOW_MS,
    });
    storage
        .create_native_rule_document(request)
        .await
        .expect("创建带 provenance 的文档");

    // 读回解密原文。
    let text = storage
        .get_native_rule_provenance_text("doc-prov")
        .await
        .expect("读取 provenance")
        .expect("有原文");
    assert_eq!(text, "原始来源文本（含敏感 URL）");

    // detail 只返回安全摘要，不暴露 secret_id。
    let detail = storage
        .get_native_rule_document("doc-prov")
        .await
        .expect("读取")
        .expect("存在");
    let provenance = detail.provenance.expect("有 provenance 摘要");
    assert_eq!(provenance.format, "legado");
    assert_eq!(provenance.adapter_version, "1.0");
    assert_eq!(provenance.input_hash, "11".repeat(32));

    // owner 行与 ref_count 恰好 1。
    let raw = open_raw(&temp.config.database_path).await;
    let owners = test_statement(
        "SELECT owner_id, secret_id FROM secret_artifact_owners WHERE owner_kind = 'native_rule_document' AND owner_id = ?",
    )
    .bind("doc-prov")
    .load::<SecretOwnerRow>(&raw)
    .await
    .expect("查询 owner 行");
    assert_eq!(owners.len(), 1);
    let secret_id = &owners[0].secret_id;
    let counts =
        test_statement("SELECT ref_count FROM secret_artifact_projection WHERE secret_id = ?")
            .bind(secret_id)
            .load::<RefCountRow>(&raw)
            .await
            .expect("查询 ref_count");
    assert_eq!(counts.len(), 1);
    assert_eq!(counts[0].ref_count, 1);
    drop(raw);

    // 删除文档 → owner 释放、ref_count 归零。
    storage
        .delete_native_rule_document(DeleteDocumentRequest {
            document_id: "doc-prov".to_string(),
            confirm_linked: false,
            trace_id: "trace".to_string(),
            occurred_at_ms: NOW_MS,
        })
        .await
        .expect("删除文档");
    let raw = open_raw(&temp.config.database_path).await;
    let owners = test_statement(
        "SELECT owner_id, secret_id FROM secret_artifact_owners WHERE owner_kind = 'native_rule_document' AND owner_id = ?",
    )
    .bind("doc-prov")
    .load::<SecretOwnerRow>(&raw)
    .await
    .expect("查询 owner 行");
    assert!(owners.is_empty(), "删除后 owner 行应清空");
    let counts =
        test_statement("SELECT ref_count FROM secret_artifact_projection WHERE secret_id = ?")
            .bind(secret_id)
            .load::<RefCountRow>(&raw)
            .await
            .expect("查询 ref_count");
    assert_eq!(counts[0].ref_count, 0, "ref_count 应归零");
    drop(raw);

    // 重开存储触发 purge + ref-count 校验：无 orphan、无损坏。
    storage.shutdown().await.expect("关闭存储");
    let storage = temp.open().await;
    assert!(
        storage
            .get_native_rule_document("doc-prov")
            .await
            .expect("读取")
            .is_none()
    );
    let raw = open_raw(&temp.config.database_path).await;
    let leftover = test_statement(
        "SELECT COUNT(*) AS count FROM secret_artifact_projection WHERE ref_count = 0",
    )
    .load::<CountRow>(&raw)
    .await
    .expect("查询残留 secret");
    assert_eq!(leftover[0].count, 0, "purge 应清理 ref_count=0 的 secret");
    storage.shutdown().await.expect("关闭存储");
}

async fn ref_count(raw: &DatabaseConnection, secret: &str) -> i64 {
    test_statement("SELECT ref_count FROM secret_artifact_projection WHERE secret_id = ?")
        .bind(secret.to_string())
        .load::<RefCountRow>(raw)
        .await
        .expect("查询 ref_count")[0]
        .ref_count
}

#[tokio::test]
async fn credential_slots_are_independent_replaceable_and_cleared_on_delete() {
    init_mock_keyring();
    let temp = TempStore::new("credentials");
    let storage = temp.open().await;

    storage
        .create_native_rule_document(create_request("doc-cred", "凭证文档"))
        .await
        .expect("创建文档");

    let slot = |node: &str, pointer: &str, value: &str| WriteDocumentCredentialSecretRequest {
        document_id: "doc-cred".to_string(),
        node_id: node.to_string(),
        json_pointer: pointer.to_string(),
        logical_name: "api_key".to_string(),
        value: value.to_string(),
        trace_id: "trace".to_string(),
        occurred_at_ms: NOW_MS,
    };
    let slot_id = |node: &str, pointer: &str| format!("doc-cred::{node}::{pointer}");

    let first = storage
        .write_document_credential_secret(slot("node-1", "/config/credential", "secret-1"))
        .await
        .expect("写槽位 1");
    let second = storage
        .write_document_credential_secret(slot("node-2", "/config/credential", "secret-2"))
        .await
        .expect("写槽位 2");
    assert_ne!(first, second, "不同槽位必须持有不同 secret");

    // 替换槽位 1：旧 secret ref_count 归零、新 secret 持有唯一 owner。
    let replaced = storage
        .write_document_credential_secret(slot("node-1", "/config/credential", "secret-3"))
        .await
        .expect("替换槽位 1");
    assert_ne!(replaced, first);

    let raw = open_raw(&temp.config.database_path).await;
    let owners = test_statement(
        "SELECT owner_id, secret_id FROM secret_artifact_owners WHERE owner_kind = 'native_rule_document_credential' AND owner_id LIKE 'doc-cred::%' ORDER BY owner_id",
    )
    .load::<SecretOwnerRow>(&raw)
    .await
    .expect("查询凭证 owner");
    assert_eq!(owners.len(), 2, "两个槽位各一个 owner 行");
    let by_slot: std::collections::BTreeMap<String, String> = owners
        .iter()
        .map(|row| (row.owner_id.clone(), row.secret_id.clone()))
        .collect();
    assert_eq!(
        by_slot
            .get(&slot_id("node-1", "/config/credential"))
            .unwrap(),
        &replaced.to_string()
    );
    assert_eq!(
        by_slot
            .get(&slot_id("node-2", "/config/credential"))
            .unwrap(),
        &second.to_string()
    );
    assert_eq!(
        ref_count(&raw, &first.to_string()).await,
        0,
        "被替换的旧 secret 归零"
    );
    assert_eq!(ref_count(&raw, &replaced.to_string()).await, 1);
    assert_eq!(ref_count(&raw, &second.to_string()).await, 1);

    // clear 槽位 2 → owner 行消失、ref_count 归零。
    storage
        .clear_document_credential_secret(ClearDocumentCredentialSecretRequest {
            document_id: "doc-cred".to_string(),
            node_id: "node-2".to_string(),
            json_pointer: "/config/credential".to_string(),
            trace_id: "trace".to_string(),
            occurred_at_ms: NOW_MS,
        })
        .await
        .expect("清除槽位 2");
    let owners = test_statement(
        "SELECT owner_id, secret_id FROM secret_artifact_owners WHERE owner_kind = 'native_rule_document_credential' AND owner_id = ?",
    )
    .bind(slot_id("node-2", "/config/credential"))
    .load::<SecretOwnerRow>(&raw)
    .await
    .expect("查询凭证 owner");
    assert!(owners.is_empty());
    assert_eq!(ref_count(&raw, &second.to_string()).await, 0);
    drop(raw);

    // 删除文档 → 剩余凭证 owner 一并释放。
    storage
        .delete_native_rule_document(DeleteDocumentRequest {
            document_id: "doc-cred".to_string(),
            confirm_linked: false,
            trace_id: "trace".to_string(),
            occurred_at_ms: NOW_MS,
        })
        .await
        .expect("删除文档");
    let raw = open_raw(&temp.config.database_path).await;
    let owners = test_statement(
        "SELECT owner_id, secret_id FROM secret_artifact_owners WHERE owner_kind = 'native_rule_document_credential' AND owner_id LIKE 'doc-cred::%'",
    )
    .load::<SecretOwnerRow>(&raw)
    .await
    .expect("查询凭证 owner");
    assert!(owners.is_empty(), "删除文档必须释放全部凭证 owner");
    drop(raw);
    storage.shutdown().await.expect("关闭存储");
}

#[tokio::test]
async fn create_rejects_duplicate_document_and_duplicate_source_identity() {
    init_mock_keyring();
    let temp = TempStore::new("duplicates");
    let storage = temp.open().await;

    storage
        .create_native_rule_document(create_request("doc-dup", "标题"))
        .await
        .expect("首次创建");
    let error = storage
        .create_native_rule_document(create_request("doc-dup", "重复"))
        .await
        .expect_err("重复 document_id 必须拒绝");
    assert!(matches!(error, StorageError::InvalidInput(_)));

    // 相同 source_identity 的第二个文档必须拒绝（UNIQUE）。
    let mut request = create_request("doc-other", "另一文档");
    request.source_identity = "native:doc-dup".to_string();
    let error = storage
        .create_native_rule_document(request)
        .await
        .expect_err("重复 source_identity 必须拒绝");
    assert!(matches!(error, StorageError::InvalidInput(_)));

    // DDL 层 UNIQUE 约束兜底：绕过 API 直接插入也必须失败。
    let raw = open_raw(&temp.config.database_path).await;
    test_statement(
        "INSERT INTO rule_documents (document_id, format, title, source_identity, state, semantic_revision, layout_revision, link_revision, created_at_ms, updated_at_ms) VALUES ('doc-raw', 'native_rule', 'raw', 'native:doc-dup', 'draft', 0, 0, 0, 1, 1)",
    )
    .execute(&raw)
    .await
    .expect_err("DDL UNIQUE 约束必须拒绝重复 source_identity");
    drop(raw);
    storage.shutdown().await.expect("关闭存储");
}

#[tokio::test]
async fn write_paths_on_missing_document_return_document_missing() {
    init_mock_keyring();
    let temp = TempStore::new("missing");
    let storage = temp.open().await;

    let error = storage
        .save_native_rule_document(save_request("ghost"))
        .await
        .expect_err("save 缺失文档必须报错");
    assert!(matches!(error, StorageError::DocumentMissing));

    let error = storage
        .rename_native_rule_document(RenameDocumentRequest {
            document_id: "ghost".to_string(),
            title: "标题".to_string(),
            expected_revision: 0,
            trace_id: "trace".to_string(),
            occurred_at_ms: NOW_MS,
        })
        .await
        .expect_err("rename 缺失文档必须报错");
    assert!(matches!(error, StorageError::DocumentMissing));

    let error = storage
        .delete_native_rule_document(DeleteDocumentRequest {
            document_id: "ghost".to_string(),
            confirm_linked: true,
            trace_id: "trace".to_string(),
            occurred_at_ms: NOW_MS,
        })
        .await
        .expect_err("delete 缺失文档必须报错");
    assert!(matches!(error, StorageError::DocumentMissing));

    let error = storage
        .write_document_credential_secret(WriteDocumentCredentialSecretRequest {
            document_id: "ghost".to_string(),
            node_id: "node-1".to_string(),
            json_pointer: "/config/credential".to_string(),
            logical_name: "api_key".to_string(),
            value: "secret".to_string(),
            trace_id: "trace".to_string(),
            occurred_at_ms: NOW_MS,
        })
        .await
        .expect_err("写凭证到缺失文档必须报错");
    assert!(matches!(error, StorageError::DocumentMissing));

    let error = storage
        .clear_document_credential_secret(ClearDocumentCredentialSecretRequest {
            document_id: "ghost".to_string(),
            node_id: "node-1".to_string(),
            json_pointer: "/config/credential".to_string(),
            trace_id: "trace".to_string(),
            occurred_at_ms: NOW_MS,
        })
        .await
        .expect_err("清凭证到缺失文档必须报错");
    assert!(matches!(error, StorageError::DocumentMissing));
    storage.shutdown().await.expect("关闭存储");
}

#[tokio::test]
async fn create_rejects_unsupported_format() {
    init_mock_keyring();
    let temp = TempStore::new("bad-format");
    let storage = temp.open().await;

    let mut request = create_request("doc-bad", "标题");
    request.format = "legacy".to_string();
    let error = storage
        .create_native_rule_document(request)
        .await
        .expect_err("非 native_rule format 必须拒绝");
    assert!(matches!(error, StorageError::InvalidInput(_)));
    storage.shutdown().await.expect("关闭存储");
}
