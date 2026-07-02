//! `LegadoImporter` 集成测试。

use std::fs;

use lj_core::node::NodeKind;
use lj_core::traits::Importer;
use lj_importer::legado::{LegadoImporter, LegadoSourceJson};

#[test]
fn import_synthetic_source() {
    let json = fs::read_to_string("fixtures/legado_synthetic_source.json")
        .expect("fixture file should exist");
    let source: LegadoSourceJson =
        serde_json::from_str(&json).expect("fixture JSON should deserialize");

    let importer = LegadoImporter;
    let preview = importer.import(source).expect("import should succeed");

    assert_eq!(preview.source_url, "https://example.com");
    assert!(preview.node_count > 0, "应该有节点");
    assert!(preview.edge_count > 0, "应该有边");

    let graph = &preview.graph;
    assert!(!graph.nodes.is_empty(), "节点列表非空");
    assert!(!graph.edges.is_empty(), "边列表非空");

    // 验证有 Http 和 Extract 节点
    let has_http = graph.nodes.iter().any(|n| n.spec.kind == NodeKind::Http);
    let has_extract = graph.nodes.iter().any(|n| n.spec.kind == NodeKind::Extract);
    assert!(has_http, "应包含 Http 节点");
    assert!(has_extract, "应包含 Extract 节点");

    // 验证 import_hash 是 64 字符 hex
    for node in &graph.nodes {
        assert_eq!(
            node.import_hash.len(),
            64,
            "import_hash 应为 64 字符, 得到 {}",
            node.import_hash.len()
        );
        assert!(
            node.import_hash.chars().all(|c| c.is_ascii_hexdigit()),
            "import_hash 应全为 hex 字符: {}",
            node.import_hash
        );
    }

    // 验证 HTTP target URLs 非空
    assert!(!preview.http_target_urls.is_empty(), "应有 HTTP 目标 URL");

    // 验证搜索 URL 包含 base URL
    let has_search_url = preview
        .http_target_urls
        .iter()
        .any(|u| u.contains("example.com"));
    assert!(has_search_url, "HTTP 目标 URL 应包含书源域名");
}
