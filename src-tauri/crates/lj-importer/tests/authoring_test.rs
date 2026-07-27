//! Legado authoring catalog、strict validation、pointer 与 credential codec 合同测试。

use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use lj_importer::authoring::catalog::{
    AUTHORING_LIMITS, check_generated_artifacts, project_root_from_manifest,
    write_generated_artifacts,
};
use lj_importer::authoring::{
    CredentialRebaseResolution, CredentialSecret, CredentialSlotCodec, Utf8ByteSpan,
    locate_json_pointer, validate_legado_document,
};
use lj_importer::legado::LegadoImporter;
use lj_rule_model::{
    AuthoringDiagnostic, CREDENTIAL_SCHEMA_VERSION, CredentialSlotId, CredentialTargetIdentity,
    DiagnosticSeverity, SourceDocumentFormat, canonical_json, credential_sentinel,
};
use serde::Deserialize;
use uuid::Uuid;

const REQUIRED_PREFIX: &str =
    r#"{"bookSourceType":0,"bookSourceUrl":"https://example.test","bookSourceName":"源""#;

#[derive(Deserialize)]
struct DiagnosticFixture {
    fixture_version: u32,
    cases: Vec<DiagnosticFixtureCase>,
}

#[derive(Deserialize)]
struct DiagnosticFixtureCase {
    name: String,
    scope: String,
    text: String,
    diagnostics: Vec<AuthoringDiagnostic>,
}

fn project_root() -> PathBuf {
    project_root_from_manifest(Path::new(env!("CARGO_MANIFEST_DIR")))
        .expect("importer manifest must have a project root")
}

fn target(format: SourceDocumentFormat) -> CredentialTargetIdentity {
    CredentialTargetIdentity {
        format,
        document_id: "document-opaque-id".to_string(),
        revision: 7,
    }
}

fn has_error(diagnostics: &[AuthoringDiagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
}

fn has_code(diagnostics: &[AuthoringDiagnostic], code: &str) -> bool {
    diagnostics.iter().any(|diagnostic| diagnostic.code == code)
}

#[test]
fn generated_bundle_is_deterministic_and_drift_check_detects_manual_edit() {
    check_generated_artifacts(&project_root())
        .expect("committed authoring artifacts must match the Rust generator");
    let root = std::env::temp_dir().join(format!(
        "lanjing-legado-authoring-generator-{}",
        Uuid::new_v4()
    ));
    write_generated_artifacts(&root).expect("generator must write complete bundle");
    check_generated_artifacts(&root).expect("fresh bundle must match generator");

    let catalog = root.join("schemas/sources/legado/field-catalog.v1.json");
    let mut edited = fs::read_to_string(&catalog).expect("catalog must exist");
    edited.push('\n');
    fs::write(&catalog, edited).expect("test must be able to simulate a manual edit");
    assert!(check_generated_artifacts(&root).is_err());
    fs::remove_dir_all(root).expect("temporary generator root must be removed");
}

#[test]
fn shared_diagnostic_fixture_matches_document_and_credential_owners() {
    let fixture_text = fs::read_to_string(
        project_root().join("schemas/sources/legado/fixtures/diagnostic-parity.v1.json"),
    )
    .expect("committed parity fixture must exist");
    let fixture: DiagnosticFixture =
        serde_json::from_str(&fixture_text).expect("fixture must deserialize");
    assert_eq!(fixture.fixture_version, 1);

    for case in fixture.cases {
        let actual = match case.scope.as_str() {
            "document" => validate_legado_document(&case.text),
            "credential_adapter" => vec![
                CredentialSlotCodec::split(&case.text, target(SourceDocumentFormat::Legado))
                    .expect_err("credential fixture must be rejected")
                    .diagnostic,
            ],
            other => panic!("unknown fixture scope: {other}"),
        };
        assert_eq!(actual, case.diagnostics, "fixture case {}", case.name);
    }
}

#[test]
fn document_utf8_byte_limit_accepts_exact_boundary_and_rejects_one_more_byte() {
    let base = exact_document_bytes(AUTHORING_LIMITS.max_utf8_bytes);
    assert_eq!(base.len(), AUTHORING_LIMITS.max_utf8_bytes);
    let diagnostics = validate_legado_document(&base);
    assert!(!has_error(&diagnostics));
    assert!(!has_code(&diagnostics, "document_bytes_exceeded"));

    let mut bad = base;
    bad.push(' ');
    let diagnostics = validate_legado_document(&bad);
    assert!(has_code(&diagnostics, "document_bytes_exceeded"));
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "document_bytes_exceeded")
        .expect("byte limit diagnostic");
    assert_eq!(diagnostic.byte_offset, AUTHORING_LIMITS.max_utf8_bytes);
    assert_eq!(diagnostic.byte_length, 1);
}

#[test]
fn depth_limit_counts_root_as_one_and_json_values_only() {
    let base = nested_array_document(AUTHORING_LIMITS.max_depth - 2);
    let diagnostics = validate_legado_document(&base);
    assert!(!has_code(&diagnostics, "depth_exceeded"));
    assert!(!has_error(&diagnostics));

    let bad = nested_array_document(AUTHORING_LIMITS.max_depth - 1);
    assert!(has_code(&validate_legado_document(&bad), "depth_exceeded"));
}

#[test]
fn node_limit_counts_root_containers_and_scalars_but_not_property_keys() {
    let fixed_nodes = 5; // root + three required values + padding array
    let base = array_node_document(AUTHORING_LIMITS.max_nodes - fixed_nodes);
    let diagnostics = validate_legado_document(&base);
    assert!(!has_code(&diagnostics, "node_count_exceeded"));
    assert!(!has_error(&diagnostics));

    let bad = array_node_document(AUTHORING_LIMITS.max_nodes - fixed_nodes + 1);
    assert!(has_code(
        &validate_legado_document(&bad),
        "node_count_exceeded"
    ));
}

#[test]
fn property_limit_counts_all_member_occurrences_across_document() {
    let required_properties = 3;
    let base = property_count_document(AUTHORING_LIMITS.max_properties - required_properties);
    let diagnostics = validate_legado_document(&base);
    assert!(!has_code(&diagnostics, "property_count_exceeded"));
    assert!(!has_error(&diagnostics));

    let bad = property_count_document(AUTHORING_LIMITS.max_properties - required_properties + 1);
    assert!(has_code(
        &validate_legado_document(&bad),
        "property_count_exceeded"
    ));
}

#[test]
fn decoded_property_name_and_string_utf8_limits_have_base_and_bad_cases() {
    let base_name = "名".repeat(AUTHORING_LIMITS.max_property_name_utf8_bytes / 3);
    let base_name_bytes = base_name.len();
    let base_name = format!(
        "{base_name}{}",
        "x".repeat(AUTHORING_LIMITS.max_property_name_utf8_bytes - base_name_bytes)
    );
    let base = format!("{REQUIRED_PREFIX},\"{base_name}\":null}}");
    let diagnostics = validate_legado_document(&base);
    assert!(!has_code(&diagnostics, "property_name_utf8_bytes_exceeded"));
    assert!(!has_error(&diagnostics));

    let bad_name = format!("{base_name}x");
    let bad = format!("{REQUIRED_PREFIX},\"{bad_name}\":null}}");
    assert!(has_code(
        &validate_legado_document(&bad),
        "property_name_utf8_bytes_exceeded"
    ));

    let base_string = "文".repeat(AUTHORING_LIMITS.max_string_utf8_bytes / 3);
    let base_string_bytes = base_string.len();
    let base_string = format!(
        "{base_string}{}",
        "x".repeat(AUTHORING_LIMITS.max_string_utf8_bytes - base_string_bytes)
    );
    let base = format!("{REQUIRED_PREFIX},\"bookSourceComment\":\"{base_string}\"}}");
    assert!(!has_code(
        &validate_legado_document(&base),
        "string_utf8_bytes_exceeded"
    ));
    let bad = format!("{REQUIRED_PREFIX},\"bookSourceComment\":\"{base_string}x\"}}");
    assert!(has_code(
        &validate_legado_document(&bad),
        "string_utf8_bytes_exceeded"
    ));
}

#[test]
fn duplicate_key_and_pointer_diagnostics_use_second_raw_utf8_key_token() {
    let text =
        format!("{REQUIRED_PREFIX},\"ruleSearch\":{{\"名称/键~\":\"甲\",\"名称/键~\":\"乙\"}}}}");
    let diagnostics = validate_legado_document(&text);
    let duplicate = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "duplicate_key")
        .expect("duplicate diagnostic");
    assert_eq!(duplicate.path, "/ruleSearch/名称~1键~0");
    let key_token = "\"名称/键~\"";
    let second_offset = text
        .match_indices(key_token)
        .nth(1)
        .expect("second key token")
        .0;
    assert_eq!(duplicate.byte_offset, second_offset);
    assert_eq!(duplicate.byte_length, key_token.len());

    let ambiguous = locate_json_pointer(&text, "/ruleSearch/名称~1键~0")
        .expect_err("duplicate target must be ambiguous");
    assert_eq!(ambiguous.code, "pointer_ambiguous");
    assert_eq!(ambiguous.byte_offset, second_offset);
    let missing =
        locate_json_pointer(&text, "/ruleSearch/missing").expect_err("missing target must fail");
    assert_eq!(missing.code, "pointer_missing");
}

#[test]
fn support_classes_are_observable_without_silently_projecting_fields() {
    let text = format!(
        "{REQUIRED_PREFIX},\"bookSourceComment\":\"保留\",\"loginUrl\":\"https://example.test/login\",\"未知\":42}}"
    );
    let diagnostics = validate_legado_document(&text);
    let preserved = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.path == "/bookSourceComment")
        .expect("preserved field diagnostic");
    assert_eq!(preserved.code, "known_field_preserved");
    assert_eq!(preserved.severity, DiagnosticSeverity::Info);
    let blocked = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.path == "/loginUrl")
        .expect("blocked field diagnostic");
    assert_eq!(blocked.code, "known_field_blocked");
    assert_eq!(blocked.severity, DiagnosticSeverity::Error);
    let unknown = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.path == "/未知")
        .expect("unknown field diagnostic");
    assert_eq!(unknown.code, "unknown_field");
    assert_eq!(unknown.severity, DiagnosticSeverity::Warning);

    let disabled_cookie_jar = format!("{REQUIRED_PREFIX},\"enabledCookieJar\":false}}");
    assert!(
        validate_legado_document(&disabled_cookie_jar)
            .iter()
            .all(|diagnostic| diagnostic.path != "/enabledCookieJar"),
        "显式关闭 CookieJar 不请求未支持行为"
    );
    let enabled_cookie_jar = format!("{REQUIRED_PREFIX},\"enabledCookieJar\":true}}");
    assert!(
        validate_legado_document(&enabled_cookie_jar)
            .iter()
            .any(|diagnostic| diagnostic.path == "/enabledCookieJar"
                && diagnostic.code == "known_field_blocked"),
        "启用 CookieJar 必须阻断安装"
    );
    let silently_dropped_behavior = format!(
        "{REQUIRED_PREFIX},\"ruleBookInfo\":{{\"author\":\".author@text\"}},\"ruleContent\":{{\"content\":\"#content@html\",\"replaceRegex\":\"secret replacement\"}}}}"
    );
    let diagnostics = validate_legado_document(&silently_dropped_behavior);
    for path in ["/ruleBookInfo/author", "/ruleContent/replaceRegex"] {
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.path == path
                    && diagnostic.code == "known_field_blocked"),
            "未实现的行为字段必须阻断而不是进入 Definition: {path}"
        );
    }
    for (text, path) in [
        (
            r#"{"bookSourceType":1,"bookSourceUrl":"https://example.test","bookSourceName":"源"}"#
                .to_string(),
            "/bookSourceType",
        ),
        (
            format!("{REQUIRED_PREFIX},\"exploreUrl\":\"/discover?page={{{{page}}}}\"}}"),
            "/exploreUrl",
        ),
        (
            format!("{REQUIRED_PREFIX},\"ruleSearch\":{{\"name\":\"@xpath://h1/text()\"}}}}"),
            "/ruleSearch/name",
        ),
    ] {
        assert!(
            validate_legado_document(&text).iter().any(|diagnostic| {
                diagnostic.path == path && diagnostic.code == "known_field_blocked"
            }),
            "当前 runtime 无法执行的 Legado 行为必须阻断: {path}"
        );
    }
    let optional_nulls = format!(
        "{REQUIRED_PREFIX},\"loginUrl\":null,\"ruleSearch\":null,\"lastUpdateTime\":null}}"
    );
    assert!(
        validate_legado_document(&optional_nulls)
            .iter()
            .all(|diagnostic| !matches!(
                diagnostic.path.as_str(),
                "/loginUrl" | "/ruleSearch" | "/lastUpdateTime"
            )),
        "可选上游字段的显式 null 必须作为空值保留，而不是触发类型或支持级别错误"
    );
}

#[test]
fn credential_split_mask_reconstitute_import_never_leaks_secret_to_debug_or_definition() {
    let secret = "credential-plain-secret";
    let header = serde_json::json!({
        "Authorization": format!("Bearer {secret}"),
        "Cookie": format!("sid={secret}"),
        "User-Agent": "authoring-fixture"
    })
    .to_string();
    let text = format!(
        "{REQUIRED_PREFIX},  \"customOpaque\" : {{\"keep\":true}}, \"header\":{}}}",
        serde_json::to_string(&header).expect("header JSON string")
    );
    let expected_target = target(SourceDocumentFormat::Legado);
    let split = CredentialSlotCodec::split(&text, expected_target.clone())
        .expect("Legado credential must split");
    assert_eq!(split.manifest.schema_version, CREDENTIAL_SCHEMA_VERSION);
    assert_eq!(split.manifest.slots.len(), 1);
    assert_eq!(split.manifest.slots[0].path, "/header");
    assert!(!split.masked_text.contains(secret));
    assert!(split.masked_text.contains("  \"customOpaque\" :"));
    assert!(!format!("{split:?}").contains(secret));
    assert!(!format!("{split:?}").contains("customOpaque"));
    assert!(!format!("{:?}", split.secrets[0]).contains(secret));

    let restored = CredentialSlotCodec::reconstitute(
        &split.masked_text,
        &split.manifest,
        &split.secrets,
        &expected_target,
    )
    .expect("target-bound document must restore");
    assert_eq!(restored, text);

    let imported = LegadoImporter
        .import_masked_document(
            &split.masked_text,
            &split.manifest,
            &split.secrets,
            &expected_target,
        )
        .expect("restored document must enter importer");
    let definition = canonical_json(&imported.adapted.definition).expect("Definition JSON");
    assert!(!definition.contains(secret));
    assert!(!definition.contains("customOpaque"));
    let snapshot = imported
        .adapted
        .credential_snapshot_bytes()
        .expect("credential snapshot serialization")
        .expect("credential snapshot must exist");
    assert!(
        String::from_utf8(snapshot)
            .expect("snapshot is JSON")
            .contains(secret)
    );

    let masked_error = LegadoImporter
        .import_document(&split.masked_text)
        .expect_err("masked sentinel cannot enter importer");
    assert_eq!(
        masked_error.diagnostics()[0].code,
        "credential_sentinel_invalid"
    );
}

#[test]
fn credential_codec_rejects_cross_format_owner_revision_path_and_occurrence_mismatch() {
    let secret = "credential-plain-secret";
    let header = serde_json::json!({ "Authorization": secret }).to_string();
    let text = format!(
        "{REQUIRED_PREFIX},\"header\":{}}}",
        serde_json::to_string(&header).expect("header JSON string")
    );
    let expected_target = target(SourceDocumentFormat::Legado);
    let split =
        CredentialSlotCodec::split(&text, expected_target.clone()).expect("credential must split");

    let mut other_format = expected_target.clone();
    other_format.format = SourceDocumentFormat::Maccms10Endpoint;
    assert_eq!(
        CredentialSlotCodec::reconstitute(
            &split.masked_text,
            &split.manifest,
            &split.secrets,
            &other_format,
        )
        .expect_err("cross format must fail")
        .diagnostic
        .code,
        "credential_format_mismatch"
    );
    let mut other_owner = expected_target.clone();
    other_owner.document_id = "another-document".to_string();
    assert_eq!(
        CredentialSlotCodec::reconstitute(
            &split.masked_text,
            &split.manifest,
            &split.secrets,
            &other_owner,
        )
        .expect_err("cross owner must fail")
        .diagnostic
        .code,
        "credential_owner_mismatch"
    );
    let mut other_revision = expected_target.clone();
    other_revision.revision += 1;
    assert_eq!(
        CredentialSlotCodec::reconstitute(
            &split.masked_text,
            &split.manifest,
            &split.secrets,
            &other_revision,
        )
        .expect_err("cross revision must fail")
        .diagnostic
        .code,
        "credential_revision_mismatch"
    );

    let moved = split.masked_text.replacen("\"header\"", "\"other\"", 1);
    assert_eq!(
        CredentialSlotCodec::reconstitute(
            &moved,
            &split.manifest,
            &split.secrets,
            &expected_target,
        )
        .expect_err("sentinel moved to another path must fail")
        .diagnostic
        .code,
        "credential_path_mismatch"
    );
    let slot_id = split.manifest.slots[0].slot_id;
    let sentinel = credential_sentinel(slot_id);
    let duplicated = split
        .masked_text
        .strip_suffix('}')
        .map(|prefix| format!("{prefix},\"copy\":\"{sentinel}\"}}"))
        .expect("object suffix");
    assert_eq!(
        CredentialSlotCodec::reconstitute(
            &duplicated,
            &split.manifest,
            &split.secrets,
            &expected_target,
        )
        .expect_err("duplicate sentinel must fail")
        .diagnostic
        .code,
        "credential_sentinel_duplicate"
    );
    let missing = split.masked_text.replace(&sentinel, "missing");
    assert_eq!(
        CredentialSlotCodec::reconstitute(
            &missing,
            &split.manifest,
            &split.secrets,
            &expected_target,
        )
        .expect_err("missing sentinel must fail")
        .diagnostic
        .code,
        "credential_sentinel_missing"
    );

    let mut duplicate_manifest = split.manifest.clone();
    duplicate_manifest
        .slots
        .push(duplicate_manifest.slots[0].clone());
    assert_eq!(
        CredentialSlotCodec::reconstitute(
            &split.masked_text,
            &duplicate_manifest,
            &split.secrets,
            &expected_target,
        )
        .expect_err("duplicate slot must fail")
        .diagnostic
        .code,
        "credential_slot_duplicate"
    );
    let duplicate_secrets = vec![
        CredentialSecret::new(slot_id, header.clone()),
        CredentialSecret::new(slot_id, header),
    ];
    assert_eq!(
        CredentialSlotCodec::reconstitute(
            &split.masked_text,
            &split.manifest,
            &duplicate_secrets,
            &expected_target,
        )
        .expect_err("duplicate secret material must fail")
        .diagnostic
        .code,
        "credential_slot_duplicate"
    );
}

#[test]
fn credential_mutations_reissue_revision_bound_slots_and_clear_only_owned_material() {
    let old_secret = "old-revision-secret";
    let old_header = serde_json::json!({
        "Authorization": format!("Bearer {old_secret}"),
        "User-Agent": "authoring-fixture"
    })
    .to_string();
    let legado = format!(
        "{REQUIRED_PREFIX},\"header\":{}}}",
        serde_json::to_string(&old_header).expect("header JSON string")
    );
    let current = target(SourceDocumentFormat::Legado);
    let split = CredentialSlotCodec::split(&legado, current.clone())
        .expect("current Legado credential must split");
    let old_slot = split.manifest.slots[0].slot_id;
    let mut next = current.clone();
    next.revision += 1;

    let resigned = CredentialSlotCodec::resign(
        &split.masked_text,
        &split.manifest,
        &split.secrets,
        &current,
        next.clone(),
    )
    .expect("save must re-sign unchanged credential material");
    assert_ne!(resigned.manifest.slots[0].slot_id, old_slot);
    assert_eq!(
        CredentialSlotCodec::reconstitute(
            &resigned.masked_text,
            &resigned.manifest,
            &resigned.secrets,
            &next,
        )
        .expect("re-signed document must restore"),
        legado
    );

    let replacement_header = serde_json::json!({
        "Authorization": "Bearer rotated-secret",
        "User-Agent": "authoring-fixture"
    })
    .to_string();
    let replaced = CredentialSlotCodec::replace_credential(
        &split.masked_text,
        &split.manifest,
        &split.secrets,
        &current,
        old_slot,
        &replacement_header,
        next.clone(),
    )
    .expect("owned slot replacement must create the next revision");
    let replaced_raw = CredentialSlotCodec::reconstitute(
        &replaced.masked_text,
        &replaced.manifest,
        &replaced.secrets,
        &next,
    )
    .expect("replacement revision must restore");
    let replaced_json = serde_json::from_str::<serde_json::Value>(&replaced_raw)
        .expect("replacement document JSON");
    assert_eq!(
        replaced_json["header"].as_str(),
        Some(replacement_header.as_str())
    );
    assert!(!replaced_raw.contains(old_secret));

    let cleared = CredentialSlotCodec::clear_credential(
        &split.masked_text,
        &split.manifest,
        &split.secrets,
        &current,
        old_slot,
        next.clone(),
    )
    .expect("owned Legado header slot must clear");
    assert!(cleared.manifest.slots.is_empty());
    let cleared_raw = CredentialSlotCodec::reconstitute(
        &cleared.masked_text,
        &cleared.manifest,
        &cleared.secrets,
        &next,
    )
    .expect("cleared Legado revision must restore");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&cleared_raw).expect("cleared Legado JSON")["header"],
        "{}"
    );

    let maccms = r#"{
  "headers": {
    "X_API_KEY": "first-secret",
    "Authorization": "second-secret",
    "User-Agent": "safe"
  },
  "extra": true
}"#;
    let maccms_current = target(SourceDocumentFormat::Maccms10Endpoint);
    let maccms_split = CredentialSlotCodec::split(maccms, maccms_current.clone())
        .expect("Maccms credentials must split");
    let api_key_slot = maccms_split
        .manifest
        .slots
        .iter()
        .find(|slot| slot.name == "X_API_KEY")
        .expect("API key slot")
        .slot_id;
    let mut maccms_next = maccms_current.clone();
    maccms_next.revision += 1;
    let maccms_cleared = CredentialSlotCodec::clear_credential(
        &maccms_split.masked_text,
        &maccms_split.manifest,
        &maccms_split.secrets,
        &maccms_current,
        api_key_slot,
        maccms_next.clone(),
    )
    .expect("Maccms clear must remove only the selected property");
    assert_eq!(maccms_cleared.manifest.slots.len(), 1);
    assert_eq!(maccms_cleared.manifest.slots[0].name, "Authorization");
    let maccms_raw = CredentialSlotCodec::reconstitute(
        &maccms_cleared.masked_text,
        &maccms_cleared.manifest,
        &maccms_cleared.secrets,
        &maccms_next,
    )
    .expect("cleared Maccms revision must restore");
    let maccms_json =
        serde_json::from_str::<serde_json::Value>(&maccms_raw).expect("cleared Maccms JSON");
    assert!(maccms_json["headers"].get("X_API_KEY").is_none());
    assert_eq!(maccms_json["headers"]["Authorization"], "second-secret");
    assert_eq!(maccms_json["headers"]["User-Agent"], "safe");
    assert_eq!(maccms_json["extra"], true);
}

#[test]
fn current_only_legado_header_rebase_is_path_limited_and_byte_preserving() {
    let prefix = concat!(
        "{\r\n",
        "  \"customOpaque\" : {\"keep\":\"原样\"},\r\n",
        "  \"bookSourceType\" : 0,\r\n",
        "  \"bookSourceUrl\" : \"https://example.test\",\r\n",
        "  \"bookSourceName\" : \"源\""
    );
    let local = format!("{prefix}\r\n}}\r\n");
    let current_header = r#"{"Authorization":"Bearer current-only"}"#;
    let header_token = serde_json::to_string(current_header).expect("header string token");
    let expected = format!("{prefix},\r\n  \"header\" : {header_token}\r\n}}\r\n");
    let mut next = target(SourceDocumentFormat::Legado);
    next.revision += 1;

    let inserted = CredentialSlotCodec::rebase_resolved_credentials(
        &local,
        SourceDocumentFormat::Legado,
        &[CredentialRebaseResolution::replace(
            "/header",
            current_header,
        )],
        next.clone(),
    )
    .expect("trusted current-only Legado header must be inserted");
    let (inserted_raw, inserted_split) = inserted.into_parts();
    assert_eq!(inserted_raw, expected);
    assert_eq!(inserted_split.manifest.slots.len(), 1);
    assert_eq!(inserted_split.manifest.slots[0].path, "/header");
    assert!(!inserted_split.masked_text.contains("Bearer current-only"));
    assert_eq!(
        CredentialSlotCodec::reconstitute(
            &inserted_split.masked_text,
            &inserted_split.manifest,
            &inserted_split.secrets,
            &next,
        )
        .expect("inserted header must round-trip through the real codec"),
        expected
    );

    let cleared = CredentialSlotCodec::rebase_resolved_credentials(
        &local,
        SourceDocumentFormat::Legado,
        &[CredentialRebaseResolution::clear("/header")],
        next.clone(),
    )
    .expect("clearing a missing trusted header must be a no-op");
    let (cleared_raw, cleared_split) = cleared.into_parts();
    assert_eq!(cleared_raw, local);
    assert_eq!(cleared_split.masked_text, local);
    assert!(cleared_split.manifest.slots.is_empty());

    let illegal = match CredentialSlotCodec::rebase_resolved_credentials(
        &local,
        SourceDocumentFormat::Legado,
        &[CredentialRebaseResolution::replace(
            "/cookie",
            "must-not-be-written",
        )],
        next.clone(),
    ) {
        Ok(_) => panic!("an arbitrary missing JSON pointer must not become a credential path"),
        Err(error) => error,
    };
    assert_eq!(illegal.diagnostic.code, "credential_path_mismatch");
    assert_eq!(illegal.diagnostic.path, "/cookie");

    let duplicate_sensitive = r#"{"Authorization":"one","authorization":"two"}"#;
    let duplicate = match CredentialSlotCodec::rebase_resolved_credentials(
        &local,
        SourceDocumentFormat::Legado,
        &[CredentialRebaseResolution::replace(
            "/header",
            duplicate_sensitive,
        )],
        next.clone(),
    ) {
        Ok(_) => panic!("current-only insertion must re-run sensitive-name policy"),
        Err(error) => error,
    };
    assert_eq!(
        duplicate.diagnostic.code,
        "credential_duplicate_sensitive_key"
    );

    let sentinel = credential_sentinel(CredentialSlotId::new());
    let sentinel_token = serde_json::to_string(&sentinel).expect("sentinel string token");
    let tainted = local.replacen(
        "{\"keep\":\"原样\"}",
        &format!("{{\"keep\":\"原样\",\"marker\":{sentinel_token}}}"),
        1,
    );
    let replay = match CredentialSlotCodec::rebase_resolved_credentials(
        &tainted,
        SourceDocumentFormat::Legado,
        &[CredentialRebaseResolution::replace(
            "/header",
            current_header,
        )],
        next,
    ) {
        Ok(_) => panic!("a local sentinel must not be erased by current-only insertion"),
        Err(error) => error,
    };
    assert_eq!(replay.diagnostic.code, "credential_sentinel_invalid");
}

#[test]
fn legado_and_maccms_adapters_share_duplicate_and_unsafe_header_policy() {
    for header in [
        r#"{"Authorization":"one","authorization":"two"}"#,
        r#"{"Proxy-Authorization":"secret"}"#,
        r#"{"Set-Cookie":"secret"}"#,
    ] {
        let text = format!(
            "{REQUIRED_PREFIX},\"header\":{}}}",
            serde_json::to_string(header).expect("header JSON string")
        );
        let error = CredentialSlotCodec::split(&text, target(SourceDocumentFormat::Legado))
            .expect_err("unsafe Legado header must fail");
        assert!(matches!(
            error.diagnostic.code.as_str(),
            "credential_duplicate_sensitive_key" | "credential_request_header_blocked"
        ));
    }

    let maccms = r#"{"headers":{"X_API_KEY":"secret","User-Agent":"safe"},"extra":true}"#;
    let maccms_target = target(SourceDocumentFormat::Maccms10Endpoint);
    let split = CredentialSlotCodec::split(maccms, maccms_target.clone())
        .expect("Maccms sensitive header must split through shared policy");
    assert_eq!(split.manifest.slots[0].path, "/headers/X_API_KEY");
    assert!(!split.masked_text.contains("secret"));
    assert_eq!(
        CredentialSlotCodec::reconstitute(
            &split.masked_text,
            &split.manifest,
            &split.secrets,
            &maccms_target,
        )
        .expect("Maccms target-bound restore"),
        maccms
    );

    let duplicate = r#"{"headers":{"X_API_KEY":"one","x-api-key":"two"}}"#;
    assert_eq!(
        CredentialSlotCodec::split(duplicate, maccms_target)
            .expect_err("normalized duplicate sensitive name must fail")
            .diagnostic
            .code,
        "credential_duplicate_sensitive_key"
    );
}

#[test]
fn sensitive_query_is_blocked_before_import_or_credential_split() {
    let text = r#"{"bookSourceType":0,"bookSourceUrl":"https://example.test?api_key=plain-secret","bookSourceName":"源"}"#;
    let diagnostics = validate_legado_document(text);
    assert!(has_code(&diagnostics, "credential_query_blocked"));
    let error = LegadoImporter
        .import_document(text)
        .expect_err("sensitive URL query cannot enter importer");
    assert_eq!(error.diagnostics()[0].code, "credential_query_blocked");
    assert_eq!(
        CredentialSlotCodec::split(text, target(SourceDocumentFormat::Legado))
            .expect_err("sensitive query cannot be moved to a header slot")
            .diagnostic
            .code,
        "credential_query_blocked"
    );
}

fn exact_document_bytes(target_bytes: usize) -> String {
    let prefix = format!("{REQUIRED_PREFIX},\"padding\":[");
    let suffix = "]}";
    let available = target_bytes - prefix.len() - suffix.len();
    let mut string_count = 1;
    while available < 3 * string_count - 1
        || available - (3 * string_count - 1)
            > string_count * AUTHORING_LIMITS.max_string_utf8_bytes
    {
        string_count += 1;
    }
    let mut content_bytes = available - (3 * string_count - 1);
    let mut output = String::with_capacity(target_bytes);
    output.push_str(&prefix);
    for index in 0..string_count {
        if index > 0 {
            output.push(',');
        }
        output.push('"');
        let chunk = content_bytes.min(AUTHORING_LIMITS.max_string_utf8_bytes);
        output.push_str(&"x".repeat(chunk));
        content_bytes -= chunk;
        output.push('"');
    }
    output.push_str(suffix);
    assert_eq!(content_bytes, 0);
    assert_eq!(output.len(), target_bytes);
    output
}

fn nested_array_document(array_count: usize) -> String {
    let mut output = format!("{REQUIRED_PREFIX},\"padding\":");
    output.push_str(&"[".repeat(array_count));
    output.push('0');
    output.push_str(&"]".repeat(array_count));
    output.push('}');
    output
}

fn array_node_document(element_count: usize) -> String {
    let mut output = format!("{REQUIRED_PREFIX},\"padding\":[");
    for index in 0..element_count {
        if index > 0 {
            output.push(',');
        }
        output.push('0');
    }
    output.push_str("]}");
    output
}

fn property_count_document(extra_properties: usize) -> String {
    let mut output = REQUIRED_PREFIX.to_string();
    for index in 0..extra_properties {
        write!(output, ",\"p{index}\":null").expect("write property fixture");
    }
    output.push('}');
    output
}

#[test]
fn root_and_middle_pointer_spans_remain_raw_utf8_bytes() {
    let text = r#"{"bookSourceType":0,"bookSourceUrl":"https://example.test","bookSourceName":"源","nested":{"数组":["甲","乙"]}}"#;
    assert_eq!(
        locate_json_pointer(text, "").expect("root pointer"),
        Utf8ByteSpan {
            byte_offset: 0,
            byte_length: text.len()
        }
    );
    let span = locate_json_pointer(text, "/nested/数组/1").expect("nested array pointer");
    assert_eq!(
        &text.as_bytes()[span.byte_offset..span.byte_offset + span.byte_length],
        "\"乙\"".as_bytes()
    );
}
