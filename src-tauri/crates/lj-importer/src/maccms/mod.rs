//! Maccms10 视频源 importer — 采集 API URL → 标准意图 Graph（ADR-0003 协议适配）。
//!
//! 与 Legado 书源 JSON 翻译器并列的第二类第三方 importer（内置协议模板,无第三方文件格式）。
//! 输入采集 API URL（`/api.php/provide/vod/`），按协议推断生成发现与详情 Flow。

pub mod translator;
pub mod types;
pub mod vocab;

pub use types::{MaccmsFormat, MaccmsSourceUrl};

use std::collections::HashMap;

use crate::preview::ImportPreview;
use lj_capability::{IntentExport, StandardIntent};
use lj_rule_model::Error;
use lj_rule_model::ExpectedDataType;
use lj_rule_model::PolicyCapabilities;
use lj_runtime::{Edge, Graph, MapperOutputKind, Node, SourceId};

use translator::{
    build_pair, connect_flow_edges, create_js_node, create_mapper_node, detail_field_rules,
    detail_rules, discover_field_rules, discover_rules,
};
use vocab::{
    ACTION_QUERY_KEY, API_PATH, ASSET_IDENTITY_FIELDS, DETAIL_ACTION, DISCOVERY_IDENTITY_FIELDS,
    IDS_QUERY_KEY, ITEM_IDENTITY_FIELDS, LIST_ACTION, UNIT_IDENTITY_FIELDS, VOD_ID_PLACEHOLDER,
};

/// Maccms10 视频源导入器。
pub struct MaccmsImporter;

impl MaccmsImporter {
    /// 导入 Maccms 采集源。
    ///
    /// # Errors
    ///
    /// URL 非法或图验证失败时返回错误。
    pub fn import(&self, opts: MaccmsSourceUrl) -> Result<ImportPreview, Error> {
        // 保留 URL 尾斜杠(`/api.php/provide/vod/`),避免与采集端点路径精确匹配失败;
        // 追加查询参数时 `base_url?ac=list&...` 路径含尾斜杠,符合 Maccms 协议约定。
        let MaccmsSourceUrl { url: base_url, at } = opts;
        if !base_url.contains(API_PATH) {
            return Err(Error::Import(format!(
                "非 Maccms 采集 API URL(缺少 /api.php/provide/vod/ 路径): {base_url}"
            )));
        }
        // base_url 已含 query 时用 & 连接,否则用 ? (#12 防 double-? 畸形 URL)。
        let query_sep = if base_url.contains('?') { '&' } else { '?' };

        let expected_type = match at {
            MaccmsFormat::Json => ExpectedDataType::Json,
            MaccmsFormat::Xml => ExpectedDataType::Xml,
        };

        let source_id = SourceId(uuid::Uuid::new_v4());
        let mut nodes: Vec<Node> = Vec::new();
        let mut edges: Vec<Edge> = Vec::new();
        let mut http_target_urls: Vec<String> = Vec::new();

        // -- 发现: Js(空) → Http ?ac=list → Extract 列表模式 --
        let disc_js = create_js_node("");
        let disc_js_id = disc_js.node_id.clone();
        let discover_url = format!(
            "{base_url}{query_sep}{ACTION_QUERY_KEY}={LIST_ACTION}&t={{{{type}}}}&pg={{{{page}}}}"
        );
        http_target_urls.push(discover_url.clone());
        let (disc_http, disc_ext) = build_pair(
            &discover_url,
            expected_type,
            discover_rules(at),
            discover_field_rules(at),
        );
        edges.push(Edge {
            from: disc_js.node_id.clone(),
            to: disc_http.node_id.clone(),
            condition_branch: None,
        });
        edges.push(Edge {
            from: disc_http.node_id.clone(),
            to: disc_ext.node_id.clone(),
            condition_branch: None,
        });
        let disc_ext_id = disc_ext.node_id.clone();
        nodes.push(disc_js);
        nodes.push(disc_http);
        nodes.push(disc_ext);
        let disc_mapper =
            create_mapper_node(MapperOutputKind::Discovery, DISCOVERY_IDENTITY_FIELDS);
        let disc_mapper_id = disc_mapper.node_id.clone();
        edges.push(Edge {
            from: disc_ext_id.clone(),
            to: disc_mapper_id.clone(),
            condition_branch: None,
        });
        nodes.push(disc_mapper);

        // -- 详情: Http ?ac=detail&ids={{vod_id}} → Extract detail --
        let detail_url = format!(
            "{base_url}{query_sep}{ACTION_QUERY_KEY}={DETAIL_ACTION}&{IDS_QUERY_KEY}={VOD_ID_PLACEHOLDER}"
        );
        http_target_urls.push(detail_url.clone());
        let (det_http, det_ext) = build_pair(
            &detail_url,
            expected_type,
            detail_rules(at),
            detail_field_rules(at),
        );
        edges.push(Edge {
            from: det_http.node_id.clone(),
            to: det_ext.node_id.clone(),
            condition_branch: None,
        });
        let detail_http_id_placeholder = det_http.node_id.clone();
        let detail_extract_id = det_ext.node_id.clone();
        nodes.push(det_http);
        nodes.push(det_ext);
        let item_mapper = create_mapper_node(MapperOutputKind::Items, ITEM_IDENTITY_FIELDS);
        let item_mapper_id = item_mapper.node_id.clone();
        let unit_mapper = create_mapper_node(MapperOutputKind::Units, UNIT_IDENTITY_FIELDS);
        let unit_mapper_id = unit_mapper.node_id.clone();
        let asset_mapper = create_mapper_node(MapperOutputKind::Assets, ASSET_IDENTITY_FIELDS);
        let asset_mapper_id = asset_mapper.node_id.clone();
        for mapper_id in [&item_mapper_id, &unit_mapper_id, &asset_mapper_id] {
            edges.push(Edge {
                from: detail_extract_id.clone(),
                to: mapper_id.clone(),
                condition_branch: None,
            });
        }
        nodes.push(item_mapper);
        nodes.push(unit_mapper);
        nodes.push(asset_mapper);

        // -- 意图间边: 发现 Extract → 详情 Http --
        connect_flow_edges(&mut edges, &disc_ext_id, &detail_http_id_placeholder);
        let mut intent_exports = HashMap::new();
        intent_exports.insert(
            StandardIntent::Discover,
            IntentExport::new(disc_js_id.0, disc_mapper_id.0),
        );
        intent_exports.insert(
            StandardIntent::ResolveItem,
            IntentExport::new(detail_http_id_placeholder.0, item_mapper_id.0),
        );
        intent_exports.insert(
            StandardIntent::ListUnits,
            IntentExport::new(detail_http_id_placeholder.0, unit_mapper_id.0),
        );
        intent_exports.insert(
            StandardIntent::ResolveAsset,
            IntentExport::new(detail_http_id_placeholder.0, asset_mapper_id.0),
        );

        let graph = Graph {
            nodes,
            edges,
            subroutines: HashMap::new(),
            source_id,
            base_url: base_url.clone(),
            intent_exports,
        };

        crate::validate::validate_graph(&graph)?;

        Ok(ImportPreview {
            source_url: base_url,
            node_count: graph.nodes.len(),
            edge_count: graph.edges.len(),
            js_block_count: 0,
            sandbox: PolicyCapabilities {
                network: true,
                system: lj_rule_model::SystemCapabilities::default(),
            },
            http_target_urls,
            js_sources: Vec::new(),
            graph,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lj_runtime::NodeKind;

    fn import_json(url: &str) -> ImportPreview {
        MaccmsImporter
            .import(MaccmsSourceUrl {
                url: url.to_string(),
                at: MaccmsFormat::Json,
            })
            .expect("import 失败")
    }

    #[test]
    fn maccms_url_imports_discover_detail_subgraph() {
        let preview = import_json("https://hnyun.com/api.php/provide/vod/");
        // ponytail: 空 Js 节点保留（Maccms 无 @js: 块），后续调度器可直接以 Http 为入口。
        assert_eq!(preview.node_count, 9); // 1 Js + 2 Http + 2 Extract + 4 Mapper
        assert_eq!(preview.edge_count, 8); // 发现 3 边 + 详情 4 边 + 意图间 1 边
        assert_eq!(preview.js_block_count, 0); // 发现 Js 节点无实际代码
        assert!(
            preview
                .graph
                .intent_exports
                .contains_key(&StandardIntent::Discover),
            "应声明 Discover 标准意图"
        );
        assert!(
            preview
                .graph
                .intent_exports
                .contains_key(&StandardIntent::ResolveItem),
            "应声明 ResolveItem 标准意图"
        );

        let kinds: Vec<NodeKind> = preview
            .graph
            .nodes
            .iter()
            .map(|n| n.spec.kind.clone())
            .collect();
        assert_eq!(kinds.iter().filter(|k| **k == NodeKind::Http).count(), 2);
        assert_eq!(kinds.iter().filter(|k| **k == NodeKind::Extract).count(), 2);
        assert_eq!(kinds.iter().filter(|k| **k == NodeKind::Js).count(), 1);
        assert_eq!(kinds.iter().filter(|k| **k == NodeKind::Mapper).count(), 4);
    }

    #[test]
    fn maccms_discover_url_contains_ac_list_type_pg() {
        let preview = import_json("https://hnyun.com/api.php/provide/vod/");
        // 发现 Http 是第一个 Http 节点（Js 后面那个）
        let disc_http = preview
            .graph
            .nodes
            .iter()
            .find(|n| n.spec.kind == NodeKind::Http)
            .expect("无 Http 节点");
        let url = disc_http
            .spec
            .http
            .as_ref()
            .expect("Http spec")
            .url
            .as_str();
        assert!(url.contains("ac=list"));
        assert!(url.contains("t={{type}}"));
        assert!(url.contains("pg={{page}}"));
    }

    #[test]
    fn maccms_detail_url_contains_ac_detail_ids_vod_id() {
        let preview = import_json("https://hnyun.com/api.php/provide/vod/");
        // 详情 Http 是第二个 Http 节点
        let http_nodes: Vec<_> = preview
            .graph
            .nodes
            .iter()
            .filter(|n| n.spec.kind == NodeKind::Http)
            .collect();
        let det_http = http_nodes.get(1).expect("需要至少 2 个 Http 节点");
        let url = det_http.spec.http.as_ref().expect("Http spec").url.as_str();
        assert!(url.contains("ac=detail"));
        assert!(url.contains("ids={{vod_id}}"));
    }

    #[test]
    fn maccms_xml_format_uses_xpath_field_rules() {
        let preview = MaccmsImporter
            .import(MaccmsSourceUrl {
                url: "https://hnyun.com/api.php/provide/vod/".to_string(),
                at: MaccmsFormat::Xml,
            })
            .expect("import 失败");
        // 发现 Extract 是第一个 Extract 节点
        let ext_nodes: Vec<_> = preview
            .graph
            .nodes
            .iter()
            .filter(|n| n.spec.kind == NodeKind::Extract)
            .collect();
        let disc_ext = ext_nodes.first().expect("无 Extract 节点");
        let ext = disc_ext.spec.extract.as_ref().expect("Extract spec");
        let name_rules = ext.field_rules.get("name").expect("缺 name field rule");
        assert!(
            name_rules
                .iter()
                .any(|r| matches!(r, lj_rule_model::ExtractRule::XPath { .. })),
            "XML format 的 name field rule 应为 XPath"
        );
    }

    #[test]
    fn maccms_invalid_url_returns_error() {
        let result = MaccmsImporter.import(MaccmsSourceUrl {
            url: "https://example.com/not-maccms".to_string(),
            at: MaccmsFormat::Json,
        });
        assert!(result.is_err());
    }
}
