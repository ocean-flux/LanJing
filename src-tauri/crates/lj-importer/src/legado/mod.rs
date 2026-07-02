//! Legado 规则导入器 — `Legado` JSON → `Graph`。
//!
//! 把 Legado 书源 JSON 的 5 端点(search/discover/detail/toc/content)
//! 翻译成节点图:每端点 = `Http` 节点 → `Extract` 节点 + 端点间 `Edge`。

pub mod translator;
pub mod types;

use std::collections::HashMap;

use lj_core::endpoint::EndpointKind;
use lj_core::error::CoreError;
use lj_core::extract_rule::ExpectedDataType;
use lj_core::graph_schema::GraphSchema;
use lj_core::node::{Edge, Graph, SourceId};
use lj_core::sandbox::Sandbox;

#[cfg(test)]
use lj_core::node::{NodeKind, NodeSpec};
use lj_core::traits::{ImportPreview, Importer};

pub use translator::compute_import_hash;
pub use types::LegadoSourceJson;

use translator::{
    EndpointState, build_http_extract_pair, collect_search_rules, connect_endpoint_edges,
    parse_headers, translate_content, translate_detail, translate_discover, translate_toc,
};

/// Legado 书源导入器。
///
/// 将 `Legado` 书源 JSON 翻译为 `LanJing` 节点图。
pub struct LegadoImporter;

impl Importer<LegadoSourceJson> for LegadoImporter {
    fn import(&self, source: LegadoSourceJson) -> Result<ImportPreview, CoreError> {
        let source_id = SourceId(uuid::Uuid::new_v4());
        let headers = parse_headers(source.header.as_deref())?;
        let base_url = source.book_source_url.trim_end_matches('/').to_string();

        let mut st = EndpointState {
            nodes: Vec::new(),
            edges: Vec::new(),
            http_target_urls: Vec::new(),
            js_sources: Vec::new(),
            search_extract_id: None,
            detail_http_id: None,
            detail_extract_id: None,
            toc_http_id: None,
            toc_extract_id: None,
            content_http_id: None,
        };

        // -- Search --
        if let Some(search_url_tpl) = &source.search_url {
            let full_search_url = format!("{base_url}{search_url_tpl}");
            st.http_target_urls.push(full_search_url.clone());
            let (search_rules, search_field_rules) =
                collect_search_rules(source.rule_search.as_ref())?;
            let (http_node, extract_node) = build_http_extract_pair(
                EndpointKind::Search,
                &full_search_url,
                &headers,
                ExpectedDataType::Html,
                &search_rules,
                search_field_rules,
            );
            st.edges.push(Edge {
                from: http_node.node_id.clone(),
                to: extract_node.node_id.clone(),
                condition_branch: None,
            });
            st.search_extract_id = Some(extract_node.node_id.clone());
            st.nodes.push(http_node);
            st.nodes.push(extract_node);
        }

        // -- Discover --
        if let Some(explore_url) = &source.explore_url {
            translate_discover(explore_url, &headers, source.rule_explore.as_ref(), &mut st)?;
        }

        // -- Detail --
        if let Some(ref rule) = source.rule_book_info {
            translate_detail(rule, &headers, &mut st)?;
        }

        // -- Toc --
        if let Some(ref rule) = source.rule_toc {
            translate_toc(rule, &headers, &mut st)?;
        }

        // -- Content --
        if let Some(ref rule) = source.rule_content {
            translate_content(rule, &headers, &mut st)?;
        }

        // -- 端点间边 --
        connect_endpoint_edges(&mut st);

        let graph = Graph {
            nodes: st.nodes.clone(),
            edges: st.edges.clone(),
            subroutines: HashMap::new(),
            source_id,
            base_url: base_url.clone(),
        };

        // 使用 `GraphSchema` 默认模板验证
        let schema = GraphSchema::default_schema();
        crate::validate::validate_graph(&graph, &schema)?;

        Ok(ImportPreview {
            source_url: source.book_source_url,
            node_count: graph.nodes.len(),
            edge_count: graph.edges.len(),
            js_block_count: st.js_sources.len(),
            sandbox: Sandbox {
                network: true,
                system: lj_core::sandbox::SystemCapabilities::default(),
            },
            http_target_urls: st.http_target_urls,
            js_sources: st.js_sources,
            graph,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_import_hash_produces_64_hex_chars() {
        let spec = NodeSpec {
            kind: NodeKind::Http,
            http: None,
            js: None,
            extract: None,
        };
        let hash = compute_import_hash(&spec);
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
