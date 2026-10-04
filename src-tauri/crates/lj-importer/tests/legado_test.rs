//! Legado Definition adapter 合同测试。

use std::collections::BTreeMap;
use std::fs;

use lj_capability::{IntentInput, StandardIntent};
use lj_importer::legado::{
    CONTINUE_ACTION_SCHEMA_VERSION, CONTINUE_ACTION_TTL_MS, ContinueActionError, LegadoImporter,
};
use lj_rule_model::{
    ExtractRule, ExtractSpec, ExtractType, FlowNodeConfig, FlowNodeKind, HttpSpec,
    MapperOutputKind, OutputTarget, RuleDefinition, canonical_json,
};
use serde_json::json;

fn fixture_source() -> String {
    fs::read_to_string("fixtures/legado_synthetic_source.json").expect("fixture file should exist")
}

fn import_error(input: &str) -> lj_importer::ImportError {
    match LegadoImporter.import(input) {
        Ok(_) => panic!("input must be rejected"),
        Err(error) => error,
    }
}

#[test]
fn adapter_exports_six_standard_intents_as_stable_definition() {
    let importer = LegadoImporter;
    let adapted = importer
        .import(&fixture_source())
        .expect("synthetic source should adapt");
    let repeated = importer
        .import(&fixture_source())
        .expect("same source should adapt repeatedly");

    assert_eq!(adapted.definition, repeated.definition);
    assert_eq!(adapted.provenance, repeated.provenance);
    assert!(LegadoImporter::owns_source(
        &adapted.definition.source_identity().id
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
            adapted.definition.intent_exports().contains_key(&intent),
            "Legado Definition should export {intent:?}"
        );
    }
    assert!(
        adapted
            .definition
            .flow()
            .nodes
            .iter()
            .any(|node| node.kind() == Some(FlowNodeKind::Http))
    );
    assert!(
        adapted
            .definition
            .flow()
            .nodes
            .iter()
            .any(|node| node.kind() == Some(FlowNodeKind::Js))
    );
    assert!(
        adapted
            .definition
            .flow()
            .nodes
            .iter()
            .all(|node| node.kind() != Some(FlowNodeKind::Merge))
    );
}

#[test]
fn sensitive_headers_are_removed_from_definition_before_staging() {
    let source = json!({
        "bookSourceName": "credential fixture",
        "bookSourceType": 0,
        "bookSourceUrl": "https://example.test",
        "searchUrl": "/search?q={{key}}",
        "ruleSearch": { "bookList": "li", "name": "a@text", "bookUrl": "a@href" },
        "header": "{\"Authorization\":\"Bearer credential-do-not-store\",\"Cookie\":\"sid=credential-do-not-store\",\"User-Agent\":\"fixture\"}"
    })
    .to_string();
    let mut adapted = LegadoImporter
        .import(&source)
        .expect("adapter should separate credential headers");

    let credential_bytes = adapted
        .take_credentials()
        .expect("sensitive headers should require encrypted staging")
        .into_bytes();
    let credential_headers = serde_json::from_slice::<BTreeMap<String, String>>(&credential_bytes)
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
fn adapter_rejects_credentials_in_base_and_search_urls_without_echoing_them() {
    let secret = "plain-secret-must-not-leak";
    for (base_url, search_url) in [
        (format!("https://user:{secret}@example.test"), None),
        (format!("https://example.test?api_key={secret}"), None),
        (
            "https://example.test".to_string(),
            Some(format!("/search?access_token={secret}")),
        ),
    ] {
        let source = json!({
            "bookSourceName": "credential URL fixture",
            "bookSourceType": 0,
            "bookSourceUrl": base_url,
            "searchUrl": search_url,
        })
        .to_string();
        let Err(error) = LegadoImporter.import(&source) else {
            panic!("credential-bearing URL must be rejected");
        };
        assert!(!format!("{error:?}").contains(secret));
    }
}

#[test]
fn import_rejects_duplicate_keys_limits_and_known_unsupported_behavior() {
    let duplicate = r#"{
        "bookSourceType": 0,
        "bookSourceUrl": "https://example.test",
        "bookSourceName": "first",
        "bookSourceName": "second"
    }"#;
    let duplicate_error = import_error(duplicate);
    assert!(
        duplicate_error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "duplicate_key")
    );

    let oversized = format!(
        r#"{{"bookSourceType":0,"bookSourceUrl":"https://example.test","bookSourceName":"fixture","bookSourceComment":"{}"}}"#,
        "x".repeat(2_097_152)
    );
    let size_error = import_error(&oversized);
    assert_eq!(size_error.diagnostics()[0].code, "document_bytes_exceeded");

    let blocked = json!({
        "bookSourceType": 0,
        "bookSourceUrl": "https://example.test",
        "bookSourceName": "fixture",
        "loginUrl": "https://example.test/login"
    })
    .to_string();
    let blocked_error = import_error(&blocked);
    assert!(
        blocked_error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == "known_field_blocked")
    );
}

#[test]
fn import_diagnostics_and_provenance_do_not_echo_credentials() {
    let secret = "diagnostic-secret-must-not-leak";
    let input = json!({
        "bookSourceType": 0,
        "bookSourceUrl": "https://example.test",
        "bookSourceName": "fixture",
        "header": format!(r#"{{"Authorization":"Bearer {secret}","authorization":"Bearer {secret}"}}"#)
    })
    .to_string();
    let error = import_error(&input);
    assert!(!format!("{error:?}").contains(secret));
    assert!(!error.to_string().contains(secret));
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

    assert_eq!(
        LegadoImporter::seal_continue_action_payload(
            &json!({"url": "/next?%61pi_key=plain-secret"}),
            source_identity,
            now,
        ),
        Err(ContinueActionError::StateInvalid)
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

fn pagination_fixture(name: &str) -> String {
    fs::read_to_string(format!("fixtures/{name}")).expect("pagination fixture should exist")
}

/// 返回某个 intent 导出链的 HTTP spec。
fn intent_http(definition: &RuleDefinition, intent: StandardIntent) -> Option<&HttpSpec> {
    let export = definition.intent_exports().get(&intent)?;
    let nodes = &definition.flow().nodes;
    let entry = nodes
        .iter()
        .find(|node| node.id == export.flow_entry)
        .map(|node| &node.config);
    let Some(FlowNodeConfig::Http(http)) = entry else {
        return None;
    };
    Some(http)
}

#[test]
fn legado_page_number_pagination_stays_in_request_template() {
    let adapted = LegadoImporter
        .import(&pagination_fixture("legado_next_page_source.json"))
        .expect("页码分页书源应可导入");
    let http = intent_http(&adapted.definition, StandardIntent::Search)
        .expect("Search 链应以 HTTP 节点为入口");
    assert!(
        http.url.ends_with("page={{page}}"),
        "页码分页必须留在 HTTP url 模板里: {}",
        http.url
    );
}

/// 返回 `ContinueAction` 链的 (HTTP spec, Extract spec, Mapper 输出) 三层配置。
fn continue_action_flow(
    definition: &RuleDefinition,
) -> Option<(&HttpSpec, &ExtractSpec, &MapperOutputKind)> {
    let export = definition
        .intent_exports()
        .get(&StandardIntent::ContinueAction)?;
    let nodes = &definition.flow().nodes;
    let edges = &definition.flow().edges;
    let extract_id = edges
        .iter()
        .find(|edge| edge.to.node_id == export.mapper_output)?
        .from
        .node_id;
    let http_id = edges
        .iter()
        .find(|edge| edge.to.node_id == extract_id)?
        .from
        .node_id;
    let config = |id| {
        nodes
            .iter()
            .find(|node| node.id == id)
            .map(|node| &node.config)
    };
    match (
        config(http_id),
        config(extract_id),
        config(export.mapper_output),
    ) {
        (
            Some(FlowNodeConfig::Http(http)),
            Some(FlowNodeConfig::Extract(extract)),
            Some(FlowNodeConfig::Mapper(mapper)),
        ) => Some((http, extract, &mapper.output)),
        _ => None,
    }
}

#[test]
fn legado_next_page_rule_becomes_continue_action_cursor_request() {
    let adapted = LegadoImporter
        .import(&pagination_fixture("legado_next_page_source.json"))
        .expect("声明下一页规则的书源应可导入");

    let (http, extract, mapper_output) =
        continue_action_flow(&adapted.definition).expect("下一页规则应产出 ContinueAction 请求链");
    assert_eq!(http.url, "{{bookUrl}}");
    assert_eq!(extract.output_target, OutputTarget::Media);
    assert_eq!(*mapper_output, MapperOutputKind::Discovery);
    assert!(matches!(
        extract.rules.as_slice(),
        [ExtractRule::CssSelector {
            selector,
            extract_type: ExtractType::Href,
            ..
        }] if selector == ".pager .next"
    ));
    let url_rules = extract
        .field_rules
        .get("bookUrl")
        .expect("下一页 URL 必须落到 bookUrl 字段以便继续动作携带 url");
    assert!(matches!(
        url_rules.as_slice(),
        [ExtractRule::CssSelector {
            selector,
            extract_type: ExtractType::Href,
            ..
        }] if selector.is_empty()
    ));
}

#[test]
fn legado_last_page_and_absent_declarations_emit_no_pagination() {
    for fixture in [
        "legado_last_page_source.json",
        "legado_no_pagination_source.json",
    ] {
        let adapted = LegadoImporter
            .import(&pagination_fixture(fixture))
            .expect("空值或缺失的下一页声明不得让导入失败");
        assert!(
            adapted
                .definition
                .intent_exports()
                .get(&StandardIntent::ContinueAction)
                .is_none(),
            "{fixture} 不应产出 ContinueAction 分页链"
        );
        assert!(
            adapted
                .definition
                .intent_exports()
                .contains_key(&StandardIntent::ListUnits),
            "{fixture} 仍应导出 ListUnits"
        );
    }
}

#[test]
fn legado_explore_continuation_keeps_continue_action_and_reports_shadowed_pagination() {
    let source = json!({
        "bookSourceName": "分页与发现并存",
        "bookSourceType": 0,
        "bookSourceUrl": "https://example.test",
        "searchUrl": "/search?q={{key}}",
        "exploreUrl": "@js:JSON.stringify([{title:'分类',url:'/cat?page={{page}}'}])",
        "ruleExplore": { "bookList": "li", "name": "a@text", "bookUrl": "a@href" },
        "ruleToc": {
            "chapterList": "#catalog a",
            "chapterName": "text",
            "chapterUrl": "href",
            "nextTocUrl": ".pager .next@href"
        }
    })
    .to_string();
    let adapted = LegadoImporter.import(&source).expect("并存来源应可导入");

    assert!(
        adapted
            .definition
            .intent_exports()
            .contains_key(&StandardIntent::Discover),
        "发现入口必须保留"
    );
    let (_, extract, _) = continue_action_flow(&adapted.definition)
        .expect("并存来源的 ContinueAction 必须保留发现入口的列表继续动作");
    assert!(
        matches!(
            extract.rules.as_slice(),
            [ExtractRule::CssSelector { selector, .. }] if selector == "li"
        ),
        "发现入口的列表规则不能被分页规则顶掉: {:?}",
        extract.rules
    );
    assert!(
        adapted.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "next_page_rule_shadowed" && diagnostic.path == "/ruleToc/nextTocUrl"
        }),
        "被顶掉的下一页声明必须显式报告: {:?}",
        adapted.diagnostics
    );
}

#[test]
fn unsupported_next_page_rule_is_rejected_before_definition() {
    let source = json!({
        "bookSourceName": "脚本分页",
        "bookSourceType": 0,
        "bookSourceUrl": "https://example.test",
        "ruleToc": {
            "chapterList": "#catalog a",
            "chapterName": "text",
            "chapterUrl": "href",
            "nextTocUrl": "@js:nextUrl"
        }
    })
    .to_string();
    let error = import_error(&source);
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.path == "/ruleToc/nextTocUrl"),
        "越界下一页规则应被字段矩阵拒绝: {:?}",
        error.diagnostics()
    );
}
