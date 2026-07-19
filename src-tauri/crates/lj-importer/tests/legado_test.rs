//! Legado Definition adapter 合同测试。

use std::collections::BTreeMap;
use std::fs;

use lj_capability::{IntentInput, StandardIntent};
use lj_importer::legado::{
    CONTINUE_ACTION_SCHEMA_VERSION, CONTINUE_ACTION_TTL_MS, ContinueActionError, LegadoImporter,
    LegadoSourceJson,
};
use lj_rule_model::{FlowNodeKind, canonical_json};
use serde_json::json;

fn fixture_source() -> LegadoSourceJson {
    let json = fs::read_to_string("fixtures/legado_synthetic_source.json")
        .expect("fixture file should exist");
    serde_json::from_str(&json).expect("fixture JSON should deserialize")
}

#[test]
fn adapter_exports_six_standard_intents_as_stable_definition() {
    let importer = LegadoImporter;
    let adapted = importer
        .adapt(&fixture_source())
        .expect("synthetic source should adapt");
    let repeated = importer
        .adapt(&fixture_source())
        .expect("same source should adapt repeatedly");

    assert_eq!(adapted.definition, repeated.definition);
    assert!(!adapted.has_credentials());
    assert!(LegadoImporter::owns_source(
        &adapted.definition.source_identity.id
    ));
    for intent in [
        StandardIntent::Search,
        StandardIntent::Discover,
        StandardIntent::ResolveItem,
        StandardIntent::ListUnits,
        StandardIntent::ResolveAsset,
        StandardIntent::ContinueAction,
    ] {
        assert!(
            adapted.definition.intent_exports.contains_key(&intent),
            "Legado Definition should export {intent:?}"
        );
    }
    assert!(
        adapted
            .definition
            .flow
            .nodes
            .iter()
            .any(|node| node.kind == FlowNodeKind::Http)
    );
    assert!(
        adapted
            .definition
            .flow
            .nodes
            .iter()
            .any(|node| node.kind == FlowNodeKind::Js)
    );
    assert!(
        adapted
            .definition
            .flow
            .nodes
            .iter()
            .all(|node| node.kind != FlowNodeKind::Merge)
    );
}

#[test]
fn sensitive_headers_are_removed_from_definition_before_staging() {
    let source: LegadoSourceJson = serde_json::from_value(json!({
        "bookSourceName": "credential fixture",
        "bookSourceUrl": "https://example.test",
        "searchUrl": "/search?q={{key}}",
        "ruleSearch": { "bookList": "li", "name": "a@text", "bookUrl": "a@href" },
        "header": "{\"Authorization\":\"Bearer credential-do-not-store\",\"Cookie\":\"sid=credential-do-not-store\",\"User-Agent\":\"fixture\"}"
    }))
    .expect("source should deserialize");
    let adapted = LegadoImporter
        .adapt(&source)
        .expect("adapter should separate credential headers");

    let credential_headers = serde_json::from_slice::<BTreeMap<String, String>>(
        &adapted
            .credential_snapshot_bytes()
            .expect("credential snapshot should serialize")
            .expect("sensitive headers should require encrypted staging"),
    )
    .expect("credential snapshot should be a header map");
    assert_eq!(
        credential_headers.get("Authorization"),
        Some(&"Bearer credential-do-not-store".to_string())
    );
    assert_eq!(
        credential_headers.get("Cookie"),
        Some(&"sid=credential-do-not-store".to_string())
    );
    let definition = canonical_json(&adapted.definition).expect("Definition canonical JSON");
    assert!(!definition.contains("credential-do-not-store"));
    assert!(definition.contains("fixture"));
}

#[test]
fn continue_action_is_versioned_source_owned_and_expiring() {
    let source_identity = "source:legado:fixture";
    let now = 1_750_000_000_000_i64;
    let sealed = LegadoImporter::seal_continue_action_payload(
        &json!({"title": "分类", "url": "/discover?page=1"}),
        source_identity,
        now,
    )
    .expect("Legado action should seal");

    assert_eq!(sealed["schema_version"], CONTINUE_ACTION_SCHEMA_VERSION);
    assert_eq!(sealed["source_identity"], source_identity);
    assert!(sealed["action_identity"].as_str().is_some());
    assert!(sealed["integrity"].as_str().is_some());
    assert_eq!(
        LegadoImporter::consume_continue_action(
            &IntentInput::Opaque(sealed.clone()),
            source_identity,
            now + 1,
        ),
        Ok(IntentInput::Opaque(json!({"url": "/discover?page=1"})))
    );
    assert_eq!(
        LegadoImporter::consume_continue_action(
            &IntentInput::Opaque(sealed.clone()),
            "source:legado:other",
            now + 1,
        ),
        Err(ContinueActionError::SourceMismatch)
    );

    let expired = LegadoImporter::seal_continue_action_payload(
        &json!({"title": "分类", "url": "/discover?page=1"}),
        source_identity,
        now - CONTINUE_ACTION_TTL_MS - 1,
    )
    .expect("expired action can be constructed for validation test");
    assert_eq!(
        LegadoImporter::consume_continue_action(
            &IntentInput::Opaque(expired),
            source_identity,
            now,
        ),
        Err(ContinueActionError::Expired)
    );
}
