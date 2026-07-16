//! Legado 规则导入器 — `Legado` JSON → `Graph`。
//!
//! 把 Legado 书源 JSON 的搜索、发现、详情、目录、正文来源规则
//! 翻译成标准意图节点图：每个意图入口接 Flow，再由 Mapper 收口。

pub mod translator;
pub mod types;

use std::collections::HashMap;

use lj_capability::{IntentExport, StandardIntent};
use lj_core::error::CoreError;
use lj_core::extract_rule::ExpectedDataType;
use lj_core::node::{Edge, Graph, MapperOutputKind, SourceId};
use lj_core::sandbox::Sandbox;

use lj_core::mapper_vocab::ITEM_IDENTITY_FIELDS;
#[cfg(test)]
use lj_core::node::{NodeKind, NodeSpec};
use lj_core::traits::{ImportPreview, Importer};

pub use translator::compute_import_hash;
pub use types::LegadoSourceJson;

use translator::{
    IntentGraphState, attach_mapper, build_http_extract_pair, collect_search_rules,
    connect_flow_edges, parse_headers, translate_content, translate_detail, translate_discover,
    translate_toc,
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

        let mut st = IntentGraphState {
            nodes: Vec::new(),
            edges: Vec::new(),
            http_target_urls: Vec::new(),
            js_sources: Vec::new(),
            search_http_id: None,
            search_extract_id: None,
            search_mapper_id: None,
            discover_entry_id: None,
            discover_extract_id: None,
            discover_mapper_id: None,
            continue_entry_id: None,
            continue_extract_id: None,
            continue_mapper_id: None,
            detail_http_id: None,
            detail_extract_id: None,
            detail_mapper_id: None,
            toc_http_id: None,
            toc_extract_id: None,
            toc_mapper_id: None,
            content_http_id: None,
            content_extract_id: None,
            content_mapper_id: None,
        };

        // -- 搜索 --
        if let Some(search_url_tpl) = &source.search_url {
            let full_search_url = format!("{base_url}{search_url_tpl}");
            st.http_target_urls.push(full_search_url.clone());
            let (search_rules, search_field_rules) =
                collect_search_rules(source.rule_search.as_ref())?;
            let (http_node, extract_node) = build_http_extract_pair(
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
            let http_node_id = http_node.node_id.clone();
            let extract_node_id = extract_node.node_id.clone();
            st.nodes.push(http_node);
            st.nodes.push(extract_node);
            let mapper_id = attach_mapper(
                &mut st,
                &extract_node_id,
                MapperOutputKind::Items,
                ITEM_IDENTITY_FIELDS,
            );
            st.search_http_id = Some(http_node_id);
            st.search_extract_id = Some(extract_node_id);
            st.search_mapper_id = Some(mapper_id);
        }

        // -- 发现 --
        if let Some(explore_url) = &source.explore_url {
            translate_discover(explore_url, &headers, source.rule_explore.as_ref(), &mut st)?;
        }

        // -- 详情 --
        if let Some(ref rule) = source.rule_book_info {
            translate_detail(rule, &headers, &mut st)?;
        }

        // -- 目录 --
        if let Some(ref rule) = source.rule_toc {
            translate_toc(rule, &headers, &mut st)?;
        }

        // -- 正文 --
        if let Some(ref rule) = source.rule_content {
            translate_content(rule, &headers, &mut st)?;
        }

        // -- 意图续接边 --
        connect_flow_edges(&mut st);

        let mut intent_exports = HashMap::new();
        if let (Some(flow_entry), Some(mapper_output)) = (&st.search_http_id, &st.search_mapper_id)
        {
            intent_exports.insert(
                StandardIntent::Search,
                IntentExport::new(flow_entry.0, mapper_output.0),
            );
        }
        if let (Some(flow_entry), Some(mapper_output)) =
            (&st.discover_entry_id, &st.discover_mapper_id)
        {
            intent_exports.insert(
                StandardIntent::Discover,
                IntentExport::new(flow_entry.0, mapper_output.0),
            );
        }
        if let (Some(flow_entry), Some(mapper_output)) = (&st.detail_http_id, &st.detail_mapper_id)
        {
            intent_exports.insert(
                StandardIntent::ResolveItem,
                IntentExport::new(flow_entry.0, mapper_output.0),
            );
        }
        if let (Some(flow_entry), Some(mapper_output)) = (&st.toc_http_id, &st.toc_mapper_id) {
            intent_exports.insert(
                StandardIntent::ListUnits,
                IntentExport::new(flow_entry.0, mapper_output.0),
            );
        }
        if let (Some(flow_entry), Some(mapper_output)) =
            (&st.content_http_id, &st.content_mapper_id)
        {
            intent_exports.insert(
                StandardIntent::ResolveAsset,
                IntentExport::new(flow_entry.0, mapper_output.0),
            );
        }
        if let (Some(flow_entry), Some(mapper_output)) =
            (&st.continue_entry_id, &st.continue_mapper_id)
        {
            intent_exports.insert(
                StandardIntent::ContinueAction,
                IntentExport::new(flow_entry.0, mapper_output.0),
            );
        }

        let graph = Graph {
            nodes: st.nodes.clone(),
            edges: st.edges.clone(),
            subroutines: HashMap::new(),
            source_id,
            base_url: base_url.clone(),
            intent_exports,
        };

        crate::validate::validate_graph(&graph)?;

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
            mapper: None,
        };
        let hash = compute_import_hash(&spec);
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
