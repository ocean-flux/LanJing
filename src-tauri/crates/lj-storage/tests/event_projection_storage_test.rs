//! Event Store、规范化投影、writer 与 durable archive 的真实 `SQLite` 合同测试。
//!
//! 每个测试都创建真实临时 `SQLite` 文件与 artifact 目录；不使用 `:memory:` 或 mock
//! Diesel。keyring 仅使用 keyring-core 官方 mock store，以便在 CI 中
//! 可重复验证主密钥丢失后的 explicit replay failure。

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

use diesel::prelude::*;
use diesel::sql_query;
use keyring_core::{Entry, mock, set_default_store};
use lj_media::{
    MediaAsset, MediaAssetKind, MediaAssetLocator, MediaGraphDelta, MediaItem, MediaKind,
    MediaResourceId, MediaUnit, ResourceCompleteness, SourceProfile,
};
use lj_rule_model::{
    CapabilityManifest, Diagnostic, DiagnosticSeverity, EffectKind, EventType, ExecutionPlan,
    FlowGraph, HttpMethod, PolicyCapabilities, RuleDefinition, RulePackage, SourceIdentity,
    canonical_json, definition_hash,
};
use lj_runtime::{
    ArchivedEffectCapture, CapturedEffectOutput, EffectArchive, EffectCapture, EffectFailure,
    EffectOutput, EffectReplayLookup, EffectWitness, ExecutionMode, HttpDnsTargetKind,
    HttpDnsTargetWitness, HttpEffectErrorKind, HttpEffectWitness, HttpRequestBodyWitness,
    HttpRequestWitness, HttpResponse, effect_bytes_hash, effect_output_hash,
};
use lj_storage::{
    AppendRequest, ArtifactInput, ArtifactKind, CandidateDraft, DEFAULT_CANDIDATE_TTL_MS,
    DeltaCommit, EventProjectionStorage, ExecutionFinish, ExecutionPin, ExecutionStart,
    ExecutionStatus, GcState, InstallCandidateRequest, LibraryEntry, LibraryProgress,
    LibraryUpdate, ProjectionDelta, ProjectionTombstones, ReplayExecutionStart, RetentionPolicy,
    SourceCredentialInput, StorageConfig, StorageError, WRITER_CAPACITY,
};
use uuid::Uuid;

fn init_mock_keyring() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        set_default_store(mock::Store::new().expect("keyring-core mock store"));
    });
}

/// keyring-core mock 跨 Entry 持久；模拟 OS keyring 主密钥丢失时需显式删除。
fn wipe_master_key(keyring_service: &str) {
    let entry = Entry::new(keyring_service, "master-key-v1").expect("master-key entry");
    entry.delete_credential().expect("删除 mock master key");
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

#[derive(diesel::QueryableByName)]
struct ArtifactMetadataTestRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    relative_path: String,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    ref_count: i64,
}

#[derive(diesel::QueryableByName)]
struct ArtifactSecurityTestRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    artifact_kind: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    encryption: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Text)]
    relative_path: String,
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

fn candidate(now_ms: i64) -> CandidateDraft {
    let source_id = "source:test".to_string();
    let definition = RuleDefinition {
        schema_version: 1,
        source_identity: SourceIdentity {
            id: source_id.clone(),
        },
        base_url: "https://example.test".to_string(),
        intent_exports: BTreeMap::new(),
        flow: FlowGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        },
        capability_manifest: CapabilityManifest::default(),
        source_id_rules: vec!["stable-id".to_string()],
    };
    let definition_hash = definition_hash(&definition).expect("canonical Definition hash");
    let package = RulePackage {
        schema_version: 1,
        source_identity: SourceIdentity {
            id: source_id.clone(),
        },
        version: "v1".to_string(),
        definition,
    };
    let mut plan = ExecutionPlan {
        schema_version: 1,
        compiler_version: "storage-test@1".to_string(),
        definition_hash,
        plan_hash: String::new(),
        nodes: Vec::new(),
        edges: Vec::new(),
        intent_entries: BTreeMap::new(),
        effects: Vec::new(),
        capability_requirements: Vec::new(),
    };
    plan.plan_hash = blake3::hash(canonical_json(&plan).expect("canonical Plan").as_bytes())
        .to_hex()
        .to_string();
    CandidateDraft {
        candidate_id: Uuid::new_v4(),
        package,
        plan,
        profile: SourceProfile {
            id: MediaResourceId(source_id),
            title: "测试来源".to_string(),
            icon_url: None,
            version: Some("v1".to_string()),
            supported_intents: Vec::new(),
            risk_notes: Vec::new(),
        },
        required_grant: PolicyCapabilities::default(),
        diagnostics: Vec::new(),
        expires_at_ms: None,
        trace_id: "trace-candidate".to_string(),
        correlation_id: None,
        created_at_ms: now_ms,
    }
}

fn updated_candidate(now_ms: i64) -> CandidateDraft {
    let mut draft = candidate(now_ms);
    draft.package.version = "v2".to_string();
    draft.package.definition.base_url = "https://updated.example.test".to_string();
    draft
        .package
        .definition
        .capability_manifest
        .required
        .network = true;
    draft.profile.title = "更新后的测试来源".to_string();
    draft.profile.version = Some("v2".to_string());
    draft.required_grant.network = true;
    draft.plan.definition_hash =
        definition_hash(&draft.package.definition).expect("updated Definition hash");
    draft.plan.plan_hash.clear();
    draft.plan.plan_hash = blake3::hash(
        canonical_json(&draft.plan)
            .expect("updated canonical Plan")
            .as_bytes(),
    )
    .to_hex()
    .to_string();
    draft
}

fn candidate_for_source(now_ms: i64, source_identity: &str) -> CandidateDraft {
    let mut draft = candidate(now_ms);
    draft.package.source_identity.id = source_identity.to_string();
    draft.package.definition.source_identity.id = source_identity.to_string();
    draft.profile.id = MediaResourceId(source_identity.to_string());
    draft.profile.title = format!("测试来源 {source_identity}");
    draft.plan.definition_hash =
        definition_hash(&draft.package.definition).expect("source-specific Definition hash");
    draft.plan.plan_hash.clear();
    draft.plan.plan_hash = blake3::hash(
        canonical_json(&draft.plan)
            .expect("source-specific canonical Plan")
            .as_bytes(),
    )
    .to_hex()
    .to_string();
    draft
}

fn require_network_capability(draft: &mut CandidateDraft) {
    draft
        .package
        .definition
        .capability_manifest
        .required
        .network = true;
    draft.required_grant.network = true;
    draft.plan.definition_hash =
        definition_hash(&draft.package.definition).expect("network Definition hash");
    draft.plan.plan_hash.clear();
    draft.plan.plan_hash = blake3::hash(
        canonical_json(&draft.plan)
            .expect("network canonical Plan")
            .as_bytes(),
    )
    .to_hex()
    .to_string();
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
            expected_source_version: 0,
            event_id: Uuid::new_v4(),
            trace_id: "trace-install".to_string(),
            occurred_at_ms: now_ms + 1,
            correlation_id: None,
            source_credentials: None,
        })
        .await
        .expect("candidate atomically install");
}

async fn install_draft(
    storage: &EventProjectionStorage,
    draft: CandidateDraft,
    expected_source_version: u64,
    grant: PolicyCapabilities,
    occurred_at_ms: i64,
) {
    let candidate_id = draft.candidate_id;
    storage
        .stage_candidate(draft)
        .await
        .expect("candidate durable staging");
    storage
        .install_candidate(InstallCandidateRequest {
            candidate_id,
            grant,
            expected_source_version,
            event_id: Uuid::new_v4(),
            trace_id: "trace-install-draft".to_string(),
            occurred_at_ms,
            correlation_id: None,
            source_credentials: None,
        })
        .await
        .expect("candidate atomically install");
}

async fn install_draft_with_source_credentials(
    storage: &EventProjectionStorage,
    draft: CandidateDraft,
    expected_source_version: u64,
    secret_bytes: Vec<u8>,
    occurred_at_ms: i64,
) -> String {
    let candidate_id = draft.candidate_id;
    let source_identity = draft.package.source_identity.id.clone();
    let grant = draft.required_grant.clone();
    storage
        .stage_candidate(draft)
        .await
        .expect("candidate durable staging");
    let snapshot = storage
        .stage_source_credentials(SourceCredentialInput {
            candidate_id,
            source_identity,
            secret_bytes,
            created_at_ms: occurred_at_ms.saturating_sub(1),
        })
        .await
        .expect("source credential durable staging");
    let secret_hash = snapshot.secret_ref.hash.clone();
    storage
        .install_candidate(InstallCandidateRequest {
            candidate_id,
            grant,
            expected_source_version,
            event_id: Uuid::new_v4(),
            trace_id: "trace-install-source-credential-draft".to_string(),
            occurred_at_ms,
            correlation_id: None,
            source_credentials: Some(snapshot),
        })
        .await
        .expect("candidate installs with its source credential");
    secret_hash
}

#[tokio::test]
async fn candidate_hashes_are_verified_before_staging_and_installation() {
    let temp = TempStore::new("candidate-hash");
    let storage = temp.open().await;
    let now = 1_750_000_000_000;

    let mut malformed = candidate(now);
    malformed.plan.plan_hash = hash("tampered-plan-hash");
    assert!(matches!(
        storage.stage_candidate(malformed).await,
        Err(StorageError::InvalidInput(_))
    ));

    let draft = candidate(now);
    let candidate_id = draft.candidate_id;
    let plan_bytes = serde_json::to_vec(&draft.plan).expect("serialize staged Plan");
    let artifact_hash = blake3::hash(&plan_bytes).to_hex().to_string();
    storage
        .stage_candidate(draft.clone())
        .await
        .expect("stage valid candidate");

    let mut tampered_plan = draft.plan;
    tampered_plan.compiler_version = "tampered-compiler@1".to_string();
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
                expected_source_version: 0,
                event_id: Uuid::new_v4(),
                trace_id: "trace-tampered-candidate".to_string(),
                occurred_at_ms: now + 1,
                correlation_id: None,
                source_credentials: None,
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
                expected_source_version: 0,
                event_id: Uuid::new_v4(),
                trace_id: "trace-insufficient-grant".to_string(),
                occurred_at_ms: now + 1,
                correlation_id: None,
                source_credentials: None,
            })
            .await,
        Err(StorageError::GrantInsufficient)
    ));

    let installed = storage
        .install_candidate(InstallCandidateRequest {
            candidate_id,
            grant: expected_grant,
            expected_source_version: 0,
            event_id: Uuid::new_v4(),
            trace_id: "trace-sufficient-grant".to_string(),
            occurred_at_ms: now + 2,
            correlation_id: None,
            source_credentials: None,
        })
        .await
        .expect("install candidate with covering grant");
    assert!(installed.grant.network);
    storage.shutdown().await.expect("writer shutdown");
}
