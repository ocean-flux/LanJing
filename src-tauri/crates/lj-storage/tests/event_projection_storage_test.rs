//! Event Store、规范化投影、writer 与 durable archive 的真实 `SQLite` 合同测试。
//!
//! 每个测试都创建真实临时 `SQLite` 文件与 artifact 目录；不使用 `:memory:` 或 mock
//! ORM mock。keyring 仅使用 keyring-core 官方 mock store，以便在 CI 中
//! 可重复验证主密钥丢失后的 explicit replay failure。

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

use keyring_core::{Entry, mock, set_default_store};
use lj_media::{
    MediaAsset, MediaAssetKind, MediaAssetLocator, MediaGraphDelta, MediaItem, MediaKind,
    MediaResourceId, MediaUnit, ResourceCompleteness, SourceProfile,
};
use lj_rule_model::{
    CapabilityManifest, ControlTrace, Diagnostic, DiagnosticSeverity, EffectKind, EventType,
    ExecutionPlan, ExecutionPlanParts, FlowGraph, HttpMethod, InvocationPath,
    LoopInvocationSegment, PolicyCapabilities, RuleDefinition, RulePackage, SourceIdentity,
    SystemCapabilities, definition_hash, read_execution_plan,
};
use lj_runtime::{
    ArchivedEffectCapture, CapturedEffectOutput, ControlReplayLookup, ControlTraceCapture,
    EffectArchive, EffectArchiveErrorCode, EffectCapture, EffectFailure, EffectOutput,
    EffectReplayLookup, EffectWitness, ExecutionMode, HttpDnsTargetKind, HttpDnsTargetWitness,
    HttpEffectErrorKind, HttpEffectWitness, HttpRequestBodyWitness, HttpRequestWitness,
    HttpResponse, ReplayCompletionLookup, effect_bytes_hash, effect_output_hash,
};
use lj_storage::{
    AppendRequest, ArtifactInput, CandidateDraft, DEFAULT_CANDIDATE_TTL_MS, DeltaCommit,
    EventProjectionStorage, ExecutionFinish, ExecutionPin, ExecutionStart, ExecutionStatus,
    GcState, InstallCandidateRequest, LibraryEntry, LibraryProgress, LibraryProjectionEntry,
    LibraryUpdate, ProjectionDelta, ProjectionTombstones, ReplayExecutionStart, RetentionPolicy,
    RuntimeCredentialMaterial, StorageConfig, StorageError, WRITER_CAPACITY,
};
use sea_orm::{
    ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, FromQueryResult, Statement,
    Value,
};
use uuid::Uuid;

struct TestStatement {
    sql: String,
    values: Vec<Value>,
}

fn library_entry<'a>(
    entries: &'a [LibraryProjectionEntry],
    resource_id: &str,
) -> &'a LibraryProjectionEntry {
    entries
        .iter()
        .find(|entry| entry.resource_id.0 == resource_id)
        .expect("library entry exists")
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

    async fn get_result<T: FromQueryResult>(
        self,
        connection: &DatabaseConnection,
    ) -> Result<T, sea_orm::DbErr> {
        connection
            .query_one_raw(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                self.sql,
                self.values,
            ))
            .await?
            .ok_or_else(|| sea_orm::DbErr::RecordNotFound("test query 未返回记录".to_string()))
            .and_then(|row| T::from_query_result(&row, ""))
    }
}

async fn open_test_connection(path: &Path) -> DatabaseConnection {
    let normalized = path.to_string_lossy().replace('\\', "/");
    Database::connect(format!("sqlite://{normalized}?mode=rw"))
        .await
        .expect("open real SQLite database")
}

fn init_mock_keyring() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        set_default_store(mock::Store::new().expect("keyring-core mock store"));
    });
}

/// keyring-core mock 跨 Entry 持久；模拟 OS secure-store key 丢失时需显式删除。
async fn wipe_master_key(temp: &TempStore) {
    #[derive(FromQueryResult)]
    struct VaultKeyIdRow {
        value: String,
    }

    let conn = open_test_connection(&temp.config.database_path).await;
    let keys = test_statement("SELECT key_id AS value FROM vault_key_metadata")
        .load::<VaultKeyIdRow>(&conn)
        .await
        .expect("read vault key metadata");
    assert!(!keys.is_empty(), "vault key metadata must exist");
    for key in keys {
        let account = format!("vault-key-v1/{}", key.value);
        Entry::new(&temp.config.keyring_service, &account)
            .expect("vault key entry")
            .delete_credential()
            .expect("删除 mock vault key");
    }
}

struct TempStore {
    root: PathBuf,
    config: StorageConfig,
}

impl TempStore {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("lj-storage-{name}-{}", Uuid::new_v4()));
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

#[derive(FromQueryResult)]
struct ArtifactMetadataTestRow {
    relative_path: String,
    ref_count: i64,
}

#[derive(FromQueryResult)]
struct ArtifactSecurityTestRow {
    key_id: String,
    blob_locator: String,
    ciphertext_hash: String,
}

fn hash(label: &str) -> String {
    blake3::hash(label.as_bytes()).to_hex().to_string()
}

fn http_witness(method: HttpMethod, body: Option<HttpRequestBodyWitness>) -> EffectWitness {
    EffectWitness::Http(HttpEffectWitness {
        request: HttpRequestWitness {
            method,
            safe_url: "https://example.test/effect".to_string(),
            headers: Vec::new(),
            body,
        },
        redirects: Vec::new(),
        dns_targets: vec![HttpDnsTargetWitness {
            host: "example.test".to_string(),
            addresses: vec!["203.0.113.42".to_string()],
            kind: HttpDnsTargetKind::DirectHost,
        }],
        error: None,
        duration_ms: 1,
    })
}
fn current_time_ms() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_millis(),
    )
    .expect("test wall clock fits i64")
}

fn package_plan(
    source_identity: &str,
    version: &str,
    base_url: &str,
    network: bool,
) -> (RulePackage, ExecutionPlan) {
    let definition = RuleDefinition::new(
        SourceIdentity {
            id: source_identity.to_string(),
        },
        base_url,
        BTreeMap::new(),
        FlowGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        },
        CapabilityManifest {
            required: PolicyCapabilities {
                network,
                system: SystemCapabilities::default(),
            },
        },
        vec!["stable-id".to_string()],
    );
    let plan = ExecutionPlan::new(
        "storage-test@1",
        definition_hash(&definition).expect("canonical Definition hash"),
        ExecutionPlanParts {
            nodes: Vec::new(),
            edges: Vec::new(),
            intent_entries: BTreeMap::new(),
            effects: Vec::new(),
            capability_requirements: Vec::new(),
            control_regions: Vec::new(),
        },
    )
    .expect("seal storage test Plan");
    let package = RulePackage::new(definition.source_identity().clone(), version, definition);
    (package, plan)
}

fn candidate(now_ms: i64) -> CandidateDraft {
    let source_id = "source:test".to_string();
    let (package, plan) = package_plan(&source_id, "v1", "https://example.test", false);
    CandidateDraft {
        candidate_id: Uuid::new_v4(),
        package,
        plan,
        profile: SourceProfile {
            id: MediaResourceId(source_id),
            title: "测试来源".to_string(),
            icon_url: None,
            version: Some("v1".to_string()),
            group: None,
            supported_intents: Vec::new(),
            risk_notes: Vec::new(),
        },
        required_grant: PolicyCapabilities::default(),
        diagnostics: Vec::new(),
        runtime_credentials: None,
        expected_installed_revision: 0,
        expires_at_ms: None,
        trace_id: "trace-candidate".to_string(),
        correlation_id: None,
        created_at_ms: now_ms,
    }
}

fn updated_candidate(now_ms: i64) -> CandidateDraft {
    let mut draft = candidate(now_ms);
    (draft.package, draft.plan) =
        package_plan("source:test", "v2", "https://updated.example.test", true);
    draft.profile.title = "更新后的测试来源".to_string();
    draft.profile.version = Some("v2".to_string());
    draft.required_grant.network = true;
    draft
}

fn candidate_for_source(now_ms: i64, source_identity: &str) -> CandidateDraft {
    let mut draft = candidate(now_ms);
    (draft.package, draft.plan) =
        package_plan(source_identity, "v1", "https://example.test", false);
    draft.profile.id = MediaResourceId(source_identity.to_string());
    draft.profile.title = format!("测试来源 {source_identity}");
    draft
}

fn require_network_capability(draft: &mut CandidateDraft) {
    let source_identity = draft.package.source_identity().id.clone();
    let version = draft.package.version().to_string();
    let base_url = draft.package.definition().base_url().to_string();
    (draft.package, draft.plan) = package_plan(&source_identity, &version, &base_url, true);
    draft.required_grant.network = true;
}

fn plan_with_hash(plan: &ExecutionPlan, plan_hash: String) -> ExecutionPlan {
    let mut value = serde_json::to_value(plan).expect("serialize Plan fixture");
    value["plan_hash"] = serde_json::Value::String(plan_hash);
    read_execution_plan(&serde_json::to_vec(&value).expect("serialize rewritten Plan fixture"))
        .expect("read rewritten Plan fixture")
}

async fn install_source(storage: &EventProjectionStorage, now_ms: i64) {
    let draft = candidate(now_ms);
    let candidate_id = draft.candidate_id;
    storage
        .stage_candidate(draft)
        .await
        .expect("candidate durable staging");
    storage
        .install_candidate(InstallCandidateRequest {
            candidate_id,
            grant: PolicyCapabilities::default(),
            event_id: Uuid::new_v4(),
            trace_id: "trace-install".to_string(),
            occurred_at_ms: now_ms + 1,
            correlation_id: None,
        })
        .await
        .expect("candidate atomically install");
}

async fn install_draft(
    storage: &EventProjectionStorage,
    mut draft: CandidateDraft,
    expected_installed_revision: u64,
    grant: PolicyCapabilities,
    occurred_at_ms: i64,
) {
    draft.expected_installed_revision = expected_installed_revision;
    let candidate_id = draft.candidate_id;
    storage
        .stage_candidate(draft)
        .await
        .expect("candidate durable staging");
    storage
        .install_candidate(InstallCandidateRequest {
            candidate_id,
            grant,
            event_id: Uuid::new_v4(),
            trace_id: "trace-install-draft".to_string(),
            occurred_at_ms,
            correlation_id: None,
        })
        .await
        .expect("candidate atomically install");
}

async fn install_draft_with_source_credentials(
    storage: &EventProjectionStorage,
    mut draft: CandidateDraft,
    expected_installed_revision: u64,
    secret_bytes: Vec<u8>,
    occurred_at_ms: i64,
) {
    draft.expected_installed_revision = expected_installed_revision;
    draft.runtime_credentials = Some(RuntimeCredentialMaterial::new(secret_bytes));
    let candidate_id = draft.candidate_id;
    let grant = draft.required_grant.clone();
    storage
        .stage_candidate(draft)
        .await
        .expect("candidate durable staging");
    storage
        .install_candidate(InstallCandidateRequest {
            candidate_id,
            grant,
            event_id: Uuid::new_v4(),
            trace_id: "trace-install-source-credential-draft".to_string(),
            occurred_at_ms,
            correlation_id: None,
        })
        .await
        .expect("candidate installs with its source credential");
}

#[tokio::test]
async fn candidate_hashes_are_verified_before_staging_and_installation() {
    let temp = TempStore::new("candidate-hash");
    let storage = temp.open().await;
    let now = 1_750_000_000_000;

    let mut malformed = candidate(now);
    malformed.plan = plan_with_hash(&malformed.plan, hash("tampered-plan-hash"));
    assert!(matches!(
        storage.stage_candidate(malformed).await,
        Err(StorageError::InvalidInput(_))
    ));

    let draft = candidate(now);
    let candidate_id = draft.candidate_id;
    let plan_bytes = serde_json::to_vec(&draft.plan).expect("serialize staged Plan");
    let artifact_hash = blake3::hash(&plan_bytes).to_hex().to_string();
    let mut tampered_plan = serde_json::to_value(&draft.plan).expect("serialize staged Plan value");
    storage
        .stage_candidate(draft)
        .await
        .expect("stage valid candidate");

    tampered_plan["compiler_version"] = serde_json::json!("tampered-compiler@1");
    let tampered_bytes = serde_json::to_vec(&tampered_plan).expect("serialize tampered Plan");
    let artifact_path = temp
        .config
        .artifact_root
        .join("body")
        .join(&artifact_hash[..2])
        .join(&artifact_hash[2..4])
        .join(format!("{artifact_hash}.zst"));
    fs::write(
        artifact_path,
        zstd::stream::encode_all(std::io::Cursor::new(tampered_bytes), 3)
            .expect("compress tampered Plan"),
    )
    .expect("overwrite staged Plan artifact");

    assert!(matches!(
        storage
            .install_candidate(InstallCandidateRequest {
                candidate_id,
                grant: PolicyCapabilities::default(),
                event_id: Uuid::new_v4(),
                trace_id: "trace-tampered-candidate".to_string(),
                occurred_at_ms: now + 1,
                correlation_id: None,
            })
            .await,
        Err(StorageError::CandidateTampered)
    ));
    storage.shutdown().await.expect("writer shutdown");
}

fn item(source_id: &str, title: &str) -> MediaItem {
    MediaItem {
        id: MediaResourceId("item:test:1".to_string()),
        source_id: MediaResourceId(source_id.to_string()),
        media_kind: MediaKind::Text,
        title: title.to_string(),
        subtitle: None,
        creators: Vec::new(),
        description: None,
        cover_asset_id: None,
        metadata: BTreeMap::new(),
        completeness: ResourceCompleteness::Complete,
        updated_at: None,
    }
}

fn delta_with_item(source_id: &str, title: &str) -> ProjectionDelta {
    let media = item(source_id, title);
    let unit = MediaUnit {
        id: MediaResourceId("unit:test:1".to_string()),
        source_id: MediaResourceId(source_id.to_string()),
        item_id: media.id.clone(),
        title: "第一单元".to_string(),
        position: Some(1),
        metadata: BTreeMap::new(),
        completeness: ResourceCompleteness::Complete,
    };
    let asset = MediaAsset {
        id: MediaResourceId("asset:test:1".to_string()),
        source_id: MediaResourceId(source_id.to_string()),
        unit_id: Some(unit.id.clone()),
        asset_kind: MediaAssetKind::Text,
        locator: MediaAssetLocator::Text("正文".to_string()),
        metadata: BTreeMap::new(),
        completeness: ResourceCompleteness::Complete,
    };
    ProjectionDelta {
        upserts: MediaGraphDelta {
            items: vec![media],
            units: vec![unit],
            assets: vec![asset],
            ..MediaGraphDelta::default()
        },
        tombstones: ProjectionTombstones::default(),
    }
}

#[path = "event_projection_storage_test/archive_contract.rs"]
mod archive_contract;
#[path = "event_projection_storage_test/credential_writer_contract.rs"]
mod credential_writer_contract;
#[path = "event_projection_storage_test/media_query_contract.rs"]
mod media_query_contract;
#[path = "event_projection_storage_test/projection_retention_contract.rs"]
mod projection_retention_contract;
#[path = "event_projection_storage_test/replay_contract.rs"]
mod replay_contract;
fn collect_file_bytes(root: &Path) -> Vec<Vec<u8>> {
    let mut files = Vec::new();
    collect_files(root, &mut files);
    files
        .into_iter()
        .map(|path| fs::read(path).expect("读取 artifact 文件"))
        .collect()
}

fn collect_files(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, files);
        } else if path.is_file() {
            files.push(path);
        }
    }
}

#[tokio::test]
async fn candidate_summary_round_trips_safe_preview_and_rejects_insufficient_grant() {
    let temp = TempStore::new("candidate-grant");
    let storage = temp.open().await;
    let now = 1_750_000_700_000;
    let mut draft = candidate(now);
    require_network_capability(&mut draft);
    draft.diagnostics = vec![Diagnostic {
        code: "CAPABILITY_NETWORK".to_string(),
        severity: DiagnosticSeverity::Warning,
        message: "安装需要 network capability".to_string(),
        span: None,
    }];
    let candidate_id = draft.candidate_id;
    let expected_profile = draft.profile.clone();
    let expected_grant = draft.required_grant.clone();
    let expected_diagnostics = draft.diagnostics.clone();

    let staged = storage
        .stage_candidate(draft)
        .await
        .expect("stage candidate with required grant");
    let preview = storage
        .get_candidate_summary(candidate_id)
        .await
        .expect("read safe candidate preview")
        .expect("candidate preview exists");
    assert_eq!(preview, staged);
    assert_eq!(preview.profile, expected_profile);
    assert_eq!(preview.required_grant, expected_grant);
    assert_eq!(preview.diagnostics, expected_diagnostics);

    assert!(matches!(
        storage
            .install_candidate(InstallCandidateRequest {
                candidate_id,
                grant: PolicyCapabilities::default(),
                event_id: Uuid::new_v4(),
                trace_id: "trace-insufficient-grant".to_string(),
                occurred_at_ms: now + 1,
                correlation_id: None,
            })
            .await,
        Err(StorageError::GrantInsufficient)
    ));

    let installed = storage
        .install_candidate(InstallCandidateRequest {
            candidate_id,
            grant: expected_grant,
            event_id: Uuid::new_v4(),
            trace_id: "trace-sufficient-grant".to_string(),
            occurred_at_ms: now + 2,
            correlation_id: None,
        })
        .await
        .expect("install candidate with covering grant");
    assert!(installed.grant.network);
    storage.shutdown().await.expect("writer shutdown");
}
