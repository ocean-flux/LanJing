//! 来源文档 vault/candidate-v2/revision pin 的 focused durable contracts。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

use diesel::prelude::*;
use diesel::sql_query;
use keyring_core::{Entry, mock, set_default_store};
use lj_media::{MediaResourceId, SourceProfile};
use lj_rule_model::{
    CREDENTIAL_SCHEMA_VERSION, CapabilityManifest, CredentialSlot, CredentialSlotId,
    CredentialSlotManifest, CredentialTargetIdentity, ExecutionPlan, ExecutionPlanParts, FlowGraph,
    PolicyCapabilities, RuleDefinition, RulePackage, SourceDocumentFormat, SourceIdentity,
    definition_hash,
};
use lj_storage::{
    CandidateDocumentInput, CandidateDraft, CreateSourceDocumentInput, CredentialSlotMaterial,
    DocumentMutationOutcome, DocumentRef, EditSourceDocumentCredentialInput,
    EventProjectionStorage, ExecutionFinish, ExecutionStart, ExecutionStatus,
    InstallCandidateRequest, LoadSourceDocumentRebaseMaterialInput, MAX_SOURCE_DOCUMENT_BYTES,
    PinSourceDocumentRevisionInput, RebaseSourceDocumentInput, RetentionPolicy,
    RuntimeCredentialMaterial, SaveSourceDocumentInput, SourceDocumentCredentialTarget,
    SourceDocumentId, SourceDocumentRebaseCommitMode, SourceDocumentRebaseInvalidReason,
    SourceDocumentRebaseMaterialOutcome, SourceDocumentRevisionInput, StorageConfig, StorageError,
    TransientSourceDocumentInput,
};
use uuid::Uuid;

fn init_mock_keyring() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        set_default_store(mock::Store::new().expect("keyring-core mock store"));
    });
}

struct TempVault {
    root: PathBuf,
    config: StorageConfig,
}

impl TempVault {
    fn new(label: &str) -> Self {
        init_mock_keyring();
        let root = std::env::temp_dir().join(format!("lj-vault-{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create vault test root");
        let mut config = StorageConfig::desktop(root.join("vault.db"), root.join("artifacts"));
        config.keyring_service = format!("lanjing.vault.test.{}", Uuid::new_v4());
        Self { root, config }
    }

    async fn open(&self) -> EventProjectionStorage {
        EventProjectionStorage::open(self.config.clone())
            .await
            .expect("open source document vault")
    }

    fn connection(&self) -> SqliteConnection {
        SqliteConnection::establish(
            self.config
                .database_path
                .to_str()
                .expect("test database path is UTF-8"),
        )
        .expect("open assertion connection")
    }
}

impl Drop for TempVault {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct TextRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    value: String,
}

fn count(conn: &mut SqliteConnection, table: &str) -> i64 {
    sql_query(format!("SELECT COUNT(*) AS value FROM {table}"))
        .get_result::<CountRow>(conn)
        .expect("count durable rows")
        .value
}

fn sum(conn: &mut SqliteConnection, table: &str, column: &str) -> i64 {
    sql_query(format!(
        "SELECT COALESCE(SUM({column}), 0) AS value FROM {table}"
    ))
    .get_result::<CountRow>(conn)
    .expect("sum durable rows")
    .value
}

fn assert_rebase_invalid_reason(
    outcome: &SourceDocumentRebaseMaterialOutcome,
    expected: SourceDocumentRebaseInvalidReason,
) {
    let SourceDocumentRebaseMaterialOutcome::Invalid(actual) = outcome else {
        panic!("expected invalid rebase material outcome");
    };
    assert_eq!(*actual, expected);
}

fn artifact_file_count(path: &Path) -> usize {
    fs::read_dir(path)
        .expect("read artifact directory")
        .map(|entry| {
            let entry = entry.expect("read artifact entry");
            if entry.file_type().expect("read artifact file type").is_dir() {
                artifact_file_count(&entry.path())
            } else {
                1
            }
        })
        .sum()
}

fn target(
    document_id: SourceDocumentId,
    revision: u64,
    format: SourceDocumentFormat,
) -> CredentialTargetIdentity {
    CredentialTargetIdentity {
        format,
        document_id: document_id.to_string(),
        revision,
    }
}

fn revision_without_slots(
    document_id: SourceDocumentId,
    revision: u64,
    raw_text: String,
) -> SourceDocumentRevisionInput {
    let target = target(document_id, revision, SourceDocumentFormat::Legado);
    SourceDocumentRevisionInput {
        format: SourceDocumentFormat::Legado,
        masked_text: raw_text.clone(),
        raw_text,
        manifest: CredentialSlotManifest {
            schema_version: CREDENTIAL_SCHEMA_VERSION,
            target,
            slots: Vec::new(),
        },
        credentials: Vec::new(),
        issues: Vec::new(),
    }
}

fn revision_with_slot(
    document_id: SourceDocumentId,
    revision: u64,
    slot_id: CredentialSlotId,
    value: &str,
) -> SourceDocumentRevisionInput {
    let target = target(document_id, revision, SourceDocumentFormat::Legado);
    let slot = CredentialSlot {
        schema_version: CREDENTIAL_SCHEMA_VERSION,
        slot_id,
        target: target.clone(),
        path: "/login/token".to_string(),
        name: "token".to_string(),
    };
    SourceDocumentRevisionInput {
        format: SourceDocumentFormat::Legado,
        masked_text: format!(
            "{{\"login\":{{\"token\":\"__LANJING_CREDENTIAL_SLOT_V1__:{slot_id}\"}}}}"
        ),
        raw_text: format!("{{\"login\":{{\"token\":\"{value}\"}}}}"),
        manifest: CredentialSlotManifest {
            schema_version: CREDENTIAL_SCHEMA_VERSION,
            target,
            slots: vec![slot],
        },
        credentials: vec![CredentialSlotMaterial::new(slot_id, value.to_string())],
        issues: Vec::new(),
    }
}

fn package_plan(source_identity: &str, version: &str) -> (RulePackage, ExecutionPlan) {
    let definition = RuleDefinition::new(
        SourceIdentity {
            id: source_identity.to_string(),
        },
        "https://vault.example.test",
        BTreeMap::new(),
        FlowGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        },
        CapabilityManifest::default(),
        vec!["stable-id".to_string()],
    );
    let plan = ExecutionPlan::new(
        "vault-test@1",
        definition_hash(&definition).expect("definition hash"),
        ExecutionPlanParts {
            nodes: Vec::new(),
            edges: Vec::new(),
            intent_entries: BTreeMap::new(),
            effects: Vec::new(),
            capability_requirements: Vec::new(),
            control_regions: Vec::new(),
        },
    )
    .expect("seal vault test Plan");
    let package = RulePackage::new(definition.source_identity().clone(), version, definition);
    (package, plan)
}

fn transient_candidate(
    source_identity: &str,
    version: &str,
    expected_installed_revision: u64,
    now_ms: i64,
    runtime_secret: Option<&[u8]>,
) -> CandidateDraft {
    let (package, plan) = package_plan(source_identity, version);
    let transient_id = SourceDocumentId::new();
    CandidateDraft {
        candidate_id: Uuid::new_v4(),
        package,
        plan,
        profile: SourceProfile {
            id: MediaResourceId(source_identity.to_string()),
            title: "Vault source".to_string(),
            icon_url: None,
            version: Some(version.to_string()),
            group: None,
            supported_intents: Vec::new(),
            risk_notes: Vec::new(),
        },
        required_grant: PolicyCapabilities::default(),
        diagnostics: Vec::new(),
        document: CandidateDocumentInput::Transient(TransientSourceDocumentInput {
            format: SourceDocumentFormat::Legado,
            masked_text: "{\"bookSourceName\":\"Vault source\"}".to_string(),
            raw_text: "{\"bookSourceName\":\"Vault source\"}".to_string(),
            manifest: CredentialSlotManifest {
                schema_version: CREDENTIAL_SCHEMA_VERSION,
                target: target(transient_id, 1, SourceDocumentFormat::Legado),
                slots: Vec::new(),
            },
        }),
        runtime_credentials: runtime_secret
            .map(|bytes| RuntimeCredentialMaterial::new(bytes.to_vec())),
        expected_installed_revision,
        expires_at_ms: None,
        trace_id: "vault-candidate".to_string(),
        correlation_id: None,
        created_at_ms: now_ms,
    }
}

async fn install_candidate(
    storage: &EventProjectionStorage,
    draft: CandidateDraft,
    now_ms: i64,
) -> u64 {
    let candidate_id = draft.candidate_id;
    storage
        .stage_candidate(draft)
        .await
        .expect("composite candidate publish");
    storage
        .install_candidate(InstallCandidateRequest {
            candidate_id,
            grant: PolicyCapabilities::default(),
            event_id: Uuid::new_v4(),
            trace_id: "vault-install".to_string(),
            occurred_at_ms: now_ms,
            correlation_id: None,
        })
        .await
        .expect("atomic candidate install")
        .source_revision
}

#[tokio::test]
async fn limits_and_injected_owner_failure_never_publish_secret_metadata() {
    let temp = TempVault::new("limits-failure");
    let storage = temp.open().await;
    let document_id = SourceDocumentId::new();
    let oversized = "x".repeat(MAX_SOURCE_DOCUMENT_BYTES + 1);
    let outcome = storage
        .create_source_document(CreateSourceDocumentInput {
            document_id,
            title: "oversized".to_string(),
            revision: revision_without_slots(document_id, 1, oversized.clone()),
            created_at_ms: 1,
        })
        .await
        .expect("limit is a typed mutation outcome");
    assert!(matches!(outcome, DocumentMutationOutcome::Invalid { .. }));
    let mut conn = temp.connection();
    assert_eq!(count(&mut conn, "secret_artifact_projection"), 0);

    let mut oversized_candidate = transient_candidate("source:oversized", "1", 0, 2, None);
    let CandidateDocumentInput::Transient(input) = &mut oversized_candidate.document else {
        panic!("fixture must remain transient");
    };
    input.masked_text.clone_from(&oversized);
    input.raw_text = oversized;
    let error = storage
        .stage_candidate(oversized_candidate)
        .await
        .expect_err("oversized transient document must fail before every artifact write");
    assert!(matches!(error, StorageError::InvalidInput(_)));
    assert_eq!(count(&mut conn, "artifact_metadata"), 0);
    assert_eq!(count(&mut conn, "secret_artifact_projection"), 0);
    assert_eq!(artifact_file_count(&temp.config.artifact_root), 0);

    sql_query(
        "CREATE TRIGGER inject_secret_owner_failure BEFORE INSERT ON secret_artifact_owners BEGIN SELECT RAISE(ABORT, 'injected owner failure'); END",
    )
    .execute(&mut conn)
    .expect("install failure trigger");
    let failed_id = SourceDocumentId::new();
    let error = storage
        .create_source_document(CreateSourceDocumentInput {
            document_id: failed_id,
            title: "failure injection".to_string(),
            revision: revision_without_slots(failed_id, 1, "{}".to_string()),
            created_at_ms: 2,
        })
        .await
        .expect_err("owner transaction must fail");
    assert!(matches!(error, StorageError::Database(_)));
    assert_eq!(count(&mut conn, "secret_artifact_projection"), 0);
    sql_query("DROP TRIGGER inject_secret_owner_failure")
        .execute(&mut conn)
        .expect("remove failure trigger");
    let recovery = storage
        .recover_orphans()
        .await
        .expect("remove pre-transaction secret files");
    assert!(recovery.removed_files >= 3);
    storage.shutdown().await.expect("shutdown writer");
}

#[tokio::test]
async fn conflict_rotation_clear_and_gc_preserve_only_current_revision() {
    let temp = TempVault::new("document-rotation");
    let storage = temp.open().await;
    let document_id = SourceDocumentId::new();
    let slot_id = CredentialSlotId::new();
    let created = storage
        .create_source_document(CreateSourceDocumentInput {
            document_id,
            title: "credential document".to_string(),
            revision: revision_with_slot(document_id, 1, slot_id, "first-secret"),
            created_at_ms: 10,
        })
        .await
        .expect("create credential document");
    assert!(matches!(created, DocumentMutationOutcome::Saved { .. }));

    let conflict = storage
        .save_source_document(SaveSourceDocumentInput {
            document_id,
            expected_revision: 0,
            revision: revision_with_slot(document_id, 1, slot_id, "must-not-write"),
            saved_at_ms: 11,
        })
        .await
        .expect("conflict is structured");
    assert!(matches!(
        conflict,
        DocumentMutationOutcome::Conflict {
            expected_revision: 0,
            actual_revision: 1,
            ..
        }
    ));

    let target_v1 = SourceDocumentCredentialTarget {
        document_id,
        document_revision: 1,
        slot_id,
    };
    let revealed = storage
        .reveal_source_document_credential(target_v1)
        .await
        .expect("reveal credential")
        .expect("slot exists");
    assert_eq!(revealed.expose_value(), "first-secret");

    let rotated = storage
        .replace_source_document_credential(EditSourceDocumentCredentialInput {
            target: target_v1,
            next_revision: revision_with_slot(document_id, 2, slot_id, "second-secret"),
            saved_at_ms: 12,
        })
        .await
        .expect("rotate credential");
    assert!(matches!(rotated, DocumentMutationOutcome::Saved { .. }));
    assert!(matches!(
        storage.reveal_source_document_credential(target_v1).await,
        Err(StorageError::CredentialOwnershipMismatch)
    ));
    let target_v2 = SourceDocumentCredentialTarget {
        document_id,
        document_revision: 2,
        slot_id,
    };
    assert_eq!(
        storage
            .reveal_source_document_credential(target_v2)
            .await
            .expect("new reveal lookup")
            .expect("new slot exists")
            .expose_value(),
        "second-secret"
    );

    let cleared = storage
        .clear_source_document_credential(EditSourceDocumentCredentialInput {
            target: target_v2,
            next_revision: revision_without_slots(document_id, 3, "{}".to_string()),
            saved_at_ms: 13,
        })
        .await
        .expect("clear credential");
    assert!(matches!(cleared, DocumentMutationOutcome::Saved { .. }));
    let current = storage
        .get_source_document(document_id)
        .await
        .expect("read masked current")
        .expect("document exists");
    assert_eq!(current.summary.revision, 3);
    assert!(current.credential_slots.is_empty());

    storage
        .run_gc(
            RetentionPolicy {
                quota_bytes: u64::MAX,
                archive_ttl_ms: None,
            },
            20,
        )
        .await
        .expect("idempotent secret GC");
    let mut conn = temp.connection();
    assert_eq!(count(&mut conn, "secret_artifact_projection"), 3);
    assert_eq!(count(&mut conn, "secret_artifact_owners"), 3);
    storage.shutdown().await.expect("shutdown writer");
}

#[tokio::test]
async fn stale_and_schema_invalid_candidate_are_rejected_and_recovered() {
    let temp = TempVault::new("candidate-recovery");
    let storage = temp.open().await;
    let document_id = SourceDocumentId::new();
    storage
        .create_source_document(CreateSourceDocumentInput {
            document_id,
            title: "saved candidate".to_string(),
            revision: revision_without_slots(document_id, 1, "{}".to_string()),
            created_at_ms: 30,
        })
        .await
        .expect("create saved source document");
    let (package, plan) = package_plan("source:saved", "v1");
    let candidate_id = Uuid::new_v4();
    storage
        .stage_candidate(CandidateDraft {
            candidate_id,
            package,
            plan,
            profile: SourceProfile {
                id: MediaResourceId("source:saved".to_string()),
                title: "Saved source".to_string(),
                icon_url: None,
                version: Some("v1".to_string()),
                group: None,
                supported_intents: Vec::new(),
                risk_notes: Vec::new(),
            },
            required_grant: PolicyCapabilities::default(),
            diagnostics: Vec::new(),
            document: CandidateDocumentInput::Saved(DocumentRef {
                document_id,
                document_revision: 1,
            }),
            runtime_credentials: None,
            expected_installed_revision: 0,
            expires_at_ms: None,
            trace_id: "saved-candidate".to_string(),
            correlation_id: None,
            created_at_ms: 31,
        })
        .await
        .expect("stage saved candidate");
    storage
        .save_source_document(SaveSourceDocumentInput {
            document_id,
            expected_revision: 1,
            revision: revision_without_slots(document_id, 2, "{\"changed\":true}".to_string()),
            saved_at_ms: 32,
        })
        .await
        .expect("advance document revision");
    let error = storage
        .install_candidate(InstallCandidateRequest {
            candidate_id,
            grant: PolicyCapabilities::default(),
            event_id: Uuid::new_v4(),
            trace_id: "stale-install".to_string(),
            occurred_at_ms: 33,
            correlation_id: None,
        })
        .await
        .expect_err("stale document baseline must fail");
    assert!(matches!(error, StorageError::CandidateStale));

    let mut conn = temp.connection();
    sql_query(
        "UPDATE candidate_projection SET candidate_schema_version = 999 WHERE candidate_id = ?",
    )
    .bind::<diesel::sql_types::Text, _>(candidate_id.to_string())
    .execute(&mut conn)
    .expect("simulate pre-upgrade staged candidate");
    storage.shutdown().await.expect("shutdown before recovery");
    drop(storage);
    let reopened = temp.open().await;
    assert!(
        reopened
            .get_candidate_summary(candidate_id)
            .await
            .expect("candidate recovery query")
            .is_none()
    );
    assert_eq!(
        reopened
            .get_source_document(document_id)
            .await
            .expect("document survives candidate recovery")
            .expect("document remains")
            .summary
            .revision,
        2
    );
    reopened.shutdown().await.expect("shutdown reopened writer");
}

#[tokio::test]
async fn transient_install_and_execution_receipt_pin_exact_source_revision() {
    let temp = TempVault::new("execution-pin");
    let storage = temp.open().await;
    let first_revision = install_candidate(
        &storage,
        transient_candidate("source:pin", "v1", 0, 100, Some(b"first-runtime")),
        101,
    )
    .await;
    assert_eq!(first_revision, 1);
    assert!(
        storage
            .list_source_documents()
            .await
            .expect("list drafts")
            .is_empty()
    );

    let execution_id = Uuid::new_v4();
    let start = storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:pin".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "live-start".to_string(),
            started_at_ms: 102,
            correlation_id: None,
        })
        .await
        .expect("atomic execution receipt");
    assert_eq!(start.record.source_revision, Some(1));
    assert_eq!(start.installed_snapshot.source_revision, 1);
    assert_eq!(
        start.installed_snapshot.runtime_credentials.secret_bytes(),
        Some(b"first-runtime".as_slice())
    );

    let second_revision = install_candidate(
        &storage,
        transient_candidate("source:pin", "v1", 1, 103, Some(b"second-runtime")),
        104,
    )
    .await;
    assert_eq!(second_revision, 2);
    let pinned_credentials = storage
        .load_execution_source_credentials(execution_id)
        .await
        .expect("load credential from pinned revision");
    assert_eq!(
        pinned_credentials.secret_bytes(),
        Some(b"first-runtime".as_slice())
    );

    let finished = storage
        .finish_execution(ExecutionFinish {
            execution_id,
            expected_version: start.record.revision,
            event_id: Uuid::new_v4(),
            status: ExecutionStatus::Completed,
            finished_at_ms: 105,
            trace_id: "finish".to_string(),
        })
        .await
        .expect("finish execution");
    assert_eq!(finished.source_revision, Some(1));
    let replay_pin = storage
        .load_execution_replay_pin(execution_id)
        .await
        .expect("load exact historical revision pin");
    assert_eq!(replay_pin.source_revision, 1);
    assert_eq!(replay_pin.source_version, "v1");

    let mut conn = temp.connection();
    sql_query("UPDATE execution_projection SET source_revision = NULL, replay_unavailable_reason = 'legacy_evidence_missing', archive_available = 0 WHERE execution_id = ?")
        .bind::<diesel::sql_types::Text, _>(execution_id.to_string())
        .execute(&mut conn)
        .expect("simulate legacy archive without unique evidence");
    let error = storage
        .load_execution_replay_pin(execution_id)
        .await
        .expect_err("legacy unavailable archive cannot use current source");
    assert!(matches!(error, StorageError::ReplayUnavailable(_)));
    storage
        .run_gc(
            RetentionPolicy {
                quota_bytes: u64::MAX,
                archive_ttl_ms: None,
            },
            106,
        )
        .await
        .expect("collect source revision after archive loses its pin");
    assert_eq!(count(&mut conn, "source_versions"), 1);
    storage.shutdown().await.expect("shutdown writer");
}

#[tokio::test]
async fn vault_key_loss_is_typed_and_ciphertext_has_random_public_identity() {
    let temp = TempVault::new("key-loss");
    let storage = temp.open().await;
    install_candidate(
        &storage,
        transient_candidate("source:key", "v1", 0, 200, Some(b"runtime-secret")),
        201,
    )
    .await;
    let execution_id = Uuid::new_v4();
    storage
        .start_execution(ExecutionStart {
            execution_id,
            source_identity: "source:key".to_string(),
            event_id: Uuid::new_v4(),
            trace_id: "key-loss-start".to_string(),
            started_at_ms: 202,
            correlation_id: None,
        })
        .await
        .expect("start execution before key loss");

    let mut conn = temp.connection();
    let key_id = sql_query("SELECT key_id AS value FROM vault_key_metadata WHERE id = 1")
        .get_result::<TextRow>(&mut conn)
        .expect("read random vault key id")
        .value;
    let public_secret =
        sql_query("SELECT secret_id AS value FROM secret_artifact_projection LIMIT 1")
            .get_result::<TextRow>(&mut conn)
            .expect("read random secret id")
            .value;
    assert!(Uuid::parse_str(&public_secret).is_ok());
    assert_ne!(
        public_secret,
        blake3::hash(b"runtime-secret").to_hex().as_str()
    );
    let runtime_locator = sql_query(
        "SELECT secret.blob_locator AS value FROM secret_artifact_projection AS secret JOIN source_projection AS source ON source.runtime_credential_secret_id = secret.secret_id WHERE source.source_identity = 'source:key'",
    )
    .get_result::<TextRow>(&mut conn)
    .expect("read random runtime blob locator")
    .value;
    assert!(!runtime_locator.contains(blake3::hash(b"runtime-secret").to_hex().as_str()));
    let runtime_path = temp.config.artifact_root.join(&runtime_locator);
    let encrypted = fs::read(&runtime_path).expect("read encrypted runtime blob");
    fs::write(&runtime_path, b"corrupt ciphertext").expect("tamper ciphertext");
    let Err(corrupt) = storage
        .load_execution_source_credentials(execution_id)
        .await
    else {
        panic!("tampered ciphertext must be rejected");
    };
    assert!(matches!(corrupt, StorageError::ArtifactCorrupt));
    fs::write(&runtime_path, encrypted).expect("restore encrypted runtime blob");

    Entry::new(
        &temp.config.keyring_service,
        &format!("vault-key-v1/{key_id}"),
    )
    .expect("vault key entry")
    .delete_credential()
    .expect("delete vault key");
    let Err(error) = storage
        .load_execution_source_credentials(execution_id)
        .await
    else {
        panic!("missing key must be typed");
    };
    assert!(matches!(error, StorageError::KeyLost));
    storage.shutdown().await.expect("shutdown writer");
}

#[tokio::test]
async fn revision_pin_retains_old_material_and_release_is_idempotent() {
    let temp = TempVault::new("revision-pin-retention");
    let storage = temp.open().await;
    let document_id = SourceDocumentId::new();
    let slot_id = CredentialSlotId::new();
    storage
        .create_source_document(CreateSourceDocumentInput {
            document_id,
            title: "pinned revision".to_string(),
            revision: revision_with_slot(document_id, 1, slot_id, "base-secret"),
            created_at_ms: 10,
        })
        .await
        .expect("create pinned base");
    let pin_id = Uuid::new_v4();
    let pin = storage
        .pin_source_document_revision(PinSourceDocumentRevisionInput {
            pin_id,
            document_ref: DocumentRef {
                document_id,
                document_revision: 1,
            },
            created_at_ms: 11,
            expires_at_ms: 100,
        })
        .await
        .expect("pin current revision");
    assert_eq!(pin.pin_id, pin_id);
    assert_eq!(pin.document_revision, 1);
    let mut conn = temp.connection();
    assert_eq!(count(&mut conn, "source_document_revision_pins"), 1);
    assert_eq!(count(&mut conn, "source_document_revision_pin_slots"), 1);
    assert_eq!(count(&mut conn, "secret_artifact_owners"), 8);
    assert_eq!(sum(&mut conn, "secret_artifact_projection", "ref_count"), 8);

    storage
        .save_source_document(SaveSourceDocumentInput {
            document_id,
            expected_revision: 1,
            revision: revision_without_slots(document_id, 2, "{}".to_string()),
            saved_at_ms: 12,
        })
        .await
        .expect("advance current while base stays pinned");
    let materials = storage
        .load_source_document_rebase_material(LoadSourceDocumentRebaseMaterialInput {
            pin_id,
            document_ref: DocumentRef {
                document_id,
                document_revision: 1,
            },
            current_revision: 2,
            now_ms: 20,
        })
        .await
        .expect("load pinned base and current");
    let base = match materials {
        SourceDocumentRebaseMaterialOutcome::Ready(materials) => {
            let (base, current) = (*materials).into_parts();
            assert_eq!(current.document_ref().document_revision, 2);
            base
        }
        _ => panic!("active pin must load trusted materials"),
    };
    assert_eq!(base.document_ref().document_revision, 1);
    assert_eq!(base.credentials().len(), 1);
    assert_eq!(base.credentials()[0].expose_value(), "base-secret");
    assert!(
        base.masked_text()
            .contains("__LANJING_CREDENTIAL_SLOT_V1__:")
    );
    assert!(base.expose_raw_text().contains("base-secret"));
    assert_eq!(base.manifest().slots[0].path, "/login/token");
    assert_eq!(count(&mut conn, "secret_artifact_owners"), 7);

    storage
        .release_source_document_revision_pin(pin_id)
        .await
        .expect("release pin");
    storage
        .release_source_document_revision_pin(pin_id)
        .await
        .expect("repeat release is idempotent");
    assert_eq!(count(&mut conn, "source_document_revision_pins"), 0);
    assert_eq!(count(&mut conn, "secret_artifact_owners"), 3);
    storage
        .run_gc(
            RetentionPolicy {
                quota_bytes: u64::MAX,
                archive_ttl_ms: None,
            },
            30,
        )
        .await
        .expect("collect released old revision");
    assert_eq!(count(&mut conn, "secret_artifact_projection"), 3);
    storage.shutdown().await.expect("shutdown writer");
}

#[tokio::test]
async fn revision_pin_owner_revision_and_expiry_rejections_release_ref_counts() {
    let temp = TempVault::new("revision-pin-expiry");
    let storage = temp.open().await;
    let document_id = SourceDocumentId::new();
    storage
        .create_source_document(CreateSourceDocumentInput {
            document_id,
            title: "expiring pin".to_string(),
            revision: revision_without_slots(document_id, 1, "{}".to_string()),
            created_at_ms: 40,
        })
        .await
        .expect("create expiry document");
    let pin_id = Uuid::new_v4();
    storage
        .pin_source_document_revision(PinSourceDocumentRevisionInput {
            pin_id,
            document_ref: DocumentRef {
                document_id,
                document_revision: 1,
            },
            created_at_ms: 41,
            expires_at_ms: 50,
        })
        .await
        .expect("pin current for expiry test");
    let wrong_owner = storage
        .load_source_document_rebase_material(LoadSourceDocumentRebaseMaterialInput {
            pin_id,
            document_ref: DocumentRef {
                document_id: SourceDocumentId::new(),
                document_revision: 1,
            },
            current_revision: 1,
            now_ms: 45,
        })
        .await
        .expect("owner mismatch is typed");
    assert_rebase_invalid_reason(
        &wrong_owner,
        SourceDocumentRebaseInvalidReason::PinOwnerMismatch,
    );
    let wrong_revision = storage
        .load_source_document_rebase_material(LoadSourceDocumentRebaseMaterialInput {
            pin_id,
            document_ref: DocumentRef {
                document_id,
                document_revision: 2,
            },
            current_revision: 1,
            now_ms: 45,
        })
        .await
        .expect("base mismatch is typed");
    assert_rebase_invalid_reason(
        &wrong_revision,
        SourceDocumentRebaseInvalidReason::PinRevisionMismatch,
    );
    let expired = storage
        .load_source_document_rebase_material(LoadSourceDocumentRebaseMaterialInput {
            pin_id,
            document_ref: DocumentRef {
                document_id,
                document_revision: 1,
            },
            current_revision: 1,
            now_ms: 50,
        })
        .await
        .expect("expired pin rejection is typed");
    assert_rebase_invalid_reason(&expired, SourceDocumentRebaseInvalidReason::PinExpired);
    let missing = storage
        .load_source_document_rebase_material(LoadSourceDocumentRebaseMaterialInput {
            pin_id,
            document_ref: DocumentRef {
                document_id,
                document_revision: 1,
            },
            current_revision: 1,
            now_ms: 51,
        })
        .await
        .expect("released expiry becomes missing");
    assert_rebase_invalid_reason(&missing, SourceDocumentRebaseInvalidReason::PinNotFound);
    let mut conn = temp.connection();
    assert_eq!(count(&mut conn, "source_document_revision_pins"), 0);
    assert_eq!(count(&mut conn, "secret_artifact_owners"), 3);
    assert_eq!(sum(&mut conn, "secret_artifact_projection", "ref_count"), 3);
    storage
        .release_source_document_revision_pin(pin_id)
        .await
        .expect("release after expiry remains idempotent");
    storage.shutdown().await.expect("shutdown writer");
}

#[tokio::test]
async fn rebase_commit_rechecks_current_after_material_load() {
    let temp = TempVault::new("rebase-current-recheck");
    let storage = temp.open().await;
    let document_id = SourceDocumentId::new();
    storage
        .create_source_document(CreateSourceDocumentInput {
            document_id,
            title: "rebase current recheck".to_string(),
            revision: revision_without_slots(document_id, 1, "{}".to_string()),
            created_at_ms: 10,
        })
        .await
        .expect("create rebase current fixture");
    let pin_id = Uuid::new_v4();
    storage
        .pin_source_document_revision(PinSourceDocumentRevisionInput {
            pin_id,
            document_ref: DocumentRef {
                document_id,
                document_revision: 1,
            },
            created_at_ms: 20,
            expires_at_ms: 1_000,
        })
        .await
        .expect("pin rebase current fixture");
    let loaded = storage
        .load_source_document_rebase_material(LoadSourceDocumentRebaseMaterialInput {
            pin_id,
            document_ref: DocumentRef {
                document_id,
                document_revision: 1,
            },
            current_revision: 1,
            now_ms: 30,
        })
        .await
        .expect("load pre-race materials");
    assert!(matches!(
        loaded,
        SourceDocumentRebaseMaterialOutcome::Ready(_)
    ));
    let saved = storage
        .save_source_document(SaveSourceDocumentInput {
            document_id,
            expected_revision: 1,
            revision: revision_without_slots(document_id, 2, "{\"current\":true}".to_string()),
            saved_at_ms: 40,
        })
        .await
        .expect("advance current after material load");
    let DocumentMutationOutcome::Saved {
        document: Some(saved),
    } = saved
    else {
        panic!("concurrent save must advance current");
    };
    let stale = storage
        .rebase_source_document(RebaseSourceDocumentInput {
            pin_id,
            document_id,
            base_revision: 1,
            current_revision: 1,
            mode: SourceDocumentRebaseCommitMode::Merge,
            revision: revision_without_slots(document_id, 2, "{\"stale\":true}".to_string()),
            saved_at_ms: 50,
        })
        .await
        .expect("writer rebase conflict outcome");
    let DocumentMutationOutcome::Conflict {
        expected_revision,
        actual_revision,
        current,
    } = stale
    else {
        panic!("writer must recheck current after material load");
    };
    assert_eq!(expected_revision, 1);
    assert_eq!(actual_revision, 2);
    assert_eq!(current, saved);
    assert_eq!(
        storage
            .get_source_document(document_id)
            .await
            .expect("read current after stale rebase")
            .expect("current remains"),
        saved
    );
    storage
        .release_source_document_revision_pin(pin_id)
        .await
        .expect("release current-recheck pin");
    storage.shutdown().await.expect("shutdown writer");
}

#[tokio::test]
async fn updates_cannot_clear_or_replace_a_linked_document_relation() {
    let temp = TempVault::new("linked-transient-update");
    let storage = temp.open().await;
    let document_id = SourceDocumentId::new();
    storage
        .create_source_document(CreateSourceDocumentInput {
            document_id,
            title: "linked source".to_string(),
            revision: revision_without_slots(document_id, 1, "{}".to_string()),
            created_at_ms: 100,
        })
        .await
        .expect("create editable source document");
    let document_ref = DocumentRef {
        document_id,
        document_revision: 1,
    };
    let mut saved_candidate = transient_candidate("source:linked", "v1", 0, 101, None);
    saved_candidate.document = CandidateDocumentInput::Saved(document_ref);
    let saved_candidate_id = saved_candidate.candidate_id;
    storage
        .stage_candidate(saved_candidate)
        .await
        .expect("stage saved-document candidate");
    let installed = storage
        .install_candidate(InstallCandidateRequest {
            candidate_id: saved_candidate_id,
            grant: PolicyCapabilities::default(),
            event_id: Uuid::new_v4(),
            trace_id: "linked-install".to_string(),
            occurred_at_ms: 102,
            correlation_id: None,
        })
        .await
        .expect("install linked source");
    assert_eq!(installed.source_revision, 1);
    assert_eq!(installed.document_id, Some(document_id));
    assert_eq!(installed.document_revision, Some(1));

    let transient_update = transient_candidate("source:linked", "v2", 1, 103, None);
    let transient_candidate_id = transient_update.candidate_id;
    storage
        .stage_candidate(transient_update)
        .await
        .expect("stage transient update");
    let error = storage
        .install_candidate(InstallCandidateRequest {
            candidate_id: transient_candidate_id,
            grant: PolicyCapabilities::default(),
            event_id: Uuid::new_v4(),
            trace_id: "linked-transient-update".to_string(),
            occurred_at_ms: 104,
            correlation_id: None,
        })
        .await
        .expect_err("transient update must not clear editable relation");
    assert!(matches!(error, StorageError::CandidateStale));

    let replacement_document_id = SourceDocumentId::new();
    storage
        .create_source_document(CreateSourceDocumentInput {
            document_id: replacement_document_id,
            title: "replacement draft".to_string(),
            revision: revision_without_slots(replacement_document_id, 1, "{}".to_string()),
            created_at_ms: 105,
        })
        .await
        .expect("create a different editable document");
    let mut replacement_update = transient_candidate("source:linked", "v2", 1, 106, None);
    replacement_update.document = CandidateDocumentInput::Saved(DocumentRef {
        document_id: replacement_document_id,
        document_revision: 1,
    });
    let replacement_candidate_id = replacement_update.candidate_id;
    storage
        .stage_candidate(replacement_update)
        .await
        .expect("stage update from a different document");
    let error = storage
        .install_candidate(InstallCandidateRequest {
            candidate_id: replacement_candidate_id,
            grant: PolicyCapabilities::default(),
            event_id: Uuid::new_v4(),
            trace_id: "linked-replacement-update".to_string(),
            occurred_at_ms: 107,
            correlation_id: None,
        })
        .await
        .expect_err("a different draft must not replace the linked document");
    assert!(matches!(error, StorageError::CandidateStale));
    let replacement_document = storage
        .get_source_document(replacement_document_id)
        .await
        .expect("read rejected replacement draft")
        .expect("replacement draft remains");
    assert_eq!(replacement_document.summary.source_identity, None);
    assert_eq!(replacement_document.summary.installed_revision, None);

    let listed = storage
        .list_installed_sources()
        .await
        .expect("list installed source after rollback");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].source_revision, 1);
    assert!(listed[0].has_editable_source);
    assert_eq!(listed[0].document_id, Some(document_id));
    assert_eq!(listed[0].document_revision, Some(1));
    let current = storage
        .get_installed_source("source:linked")
        .await
        .expect("read installed source")
        .expect("linked source remains installed");
    assert_eq!(current.version, "v1");
    assert_eq!(current.source_revision, 1);
    assert_eq!(current.document_id, Some(document_id));
    assert!(
        storage
            .get_candidate_summary(transient_candidate_id)
            .await
            .expect("failed update candidate remains readable")
            .is_some()
    );
    let mut conn = temp.connection();
    assert_eq!(count(&mut conn, "source_versions"), 1);
    storage.shutdown().await.expect("shutdown writer");
}
