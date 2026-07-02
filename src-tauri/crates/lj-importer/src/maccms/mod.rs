//! Maccms10 视频源 importer — 采集 API URL → Graph（ADR-0003 协议适配）。
//!
//! 与 Legado 书源 JSON 翻译器并列的第二类第三方 importer（内置协议模板,无第三方文件格式）。
//! 输入采集 API URL（`/api.php/provide/vod/`），按协议推断生成 Discover/Detail Graph。

pub mod translator;
pub mod types;

pub use types::{MaccmsFormat, MaccmsSourceUrl};

use std::collections::HashMap;

use lj_core::endpoint::EndpointKind;
use lj_core::error::CoreError;
use lj_core::extract_rule::ExpectedDataType;
use lj_core::graph_schema::GraphSchema;
use lj_core::node::{Edge, Graph, Node, SourceId};
use lj_core::sandbox::Sandbox;
use lj_core::traits::{ImportPreview, Importer};

use translator::{
    build_pair, connect_endpoint_edges, create_js_node, detail_field_rules, detail_rules,
    discover_field_rules, discover_rules, play_url_parser_defaults,
};

/// Maccms10 视频源导入器。
pub struct MaccmsImporter;

impl Importer<MaccmsSourceUrl> for MaccmsImporter {
    fn import(&self, opts: MaccmsSourceUrl) -> Result<ImportPreview, CoreError> {
        // 保留 URL 尾斜杠(`/api.php/provide/vod/`),避免与采集端点路径精确匹配失败;
        // 追加查询参数时 `base_url?ac=list&...` 路径含尾斜杠,符合 Maccms 协议约定。
        let base_url = opts.url.clone();
        if !base_url.contains("/api.php/provide/vod") {
            return Err(CoreError::Import(format!(
                "非 Maccms 采集 API URL(缺少 /api.php/provide/vod/ 路径): {base_url}"
            )));
        }
        // base_url 已含 query 时用 & 连接,否则用 ? (#12 防 double-? 畸形 URL)。
        let query_sep = if base_url.contains('?') { '&' } else { '?' };

        let expected_type = match opts.at {
            MaccmsFormat::Json => ExpectedDataType::Json,
            MaccmsFormat::Xml => ExpectedDataType::Xml,
        };

        let source_id = SourceId(uuid::Uuid::new_v4());
        let mut nodes: Vec<Node> = Vec::new();
        let mut edges: Vec<Edge> = Vec::new();
        let mut http_target_urls: Vec<String> = Vec::new();

        // -- Discover: Js(空,满足 schema) → Http ?ac=list → Extract 列表模式 --
        // ponytail: 空 Js 节点满足 Discover schema(Maccms 无 @js: 块,Legado explore_url 才需)
        let disc_js = create_js_node(EndpointKind::Discover, "");
        let discover_url = format!("{base_url}{query_sep}ac=list&t={{{{type}}}}&pg={{{{page}}}}");
        http_target_urls.push(discover_url.clone());
        let (disc_http, disc_ext) = build_pair(
            EndpointKind::Discover,
            &discover_url,
            expected_type,
            discover_rules(opts.at),
            discover_field_rules(opts.at),
            None,
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

        // -- Detail: Http ?ac=detail&ids={{vod_id}} → Extract detail + play_url_parser --
        let detail_url = format!("{base_url}{query_sep}ac=detail&ids={{{{vod_id}}}}");
        http_target_urls.push(detail_url.clone());
        let (det_http, det_ext) = build_pair(
            EndpointKind::Detail,
            &detail_url,
            expected_type,
            detail_rules(opts.at),
            detail_field_rules(opts.at),
            Some(play_url_parser_defaults()),
        );
        edges.push(Edge {
            from: det_http.node_id.clone(),
            to: det_ext.node_id.clone(),
            condition_branch: None,
        });
        let detail_http_id_placeholder = det_http.node_id.clone();
        nodes.push(det_http);
        nodes.push(det_ext);

        // -- 端点间边: Discover Extract → Detail Http --
        connect_endpoint_edges(&mut edges, &disc_ext_id, &detail_http_id_placeholder);

        let graph = Graph {
            nodes,
            edges,
            subroutines: HashMap::new(),
            source_id,
            base_url: base_url.clone(),
        };

        let schema = GraphSchema::default_schema();
        crate::validate::validate_graph(&graph, &schema)?;

        Ok(ImportPreview {
            source_url: base_url,
            node_count: graph.nodes.len(),
            edge_count: graph.edges.len(),
            js_block_count: 0,
            sandbox: Sandbox {
                network: true,
                system: lj_core::sandbox::SystemCapabilities::default(),
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
    use lj_core::endpoint::EndpointKind;
    use lj_core::node::NodeKind;

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
        assert_eq!(preview.node_count, 5); // 1 Js + 2 Http + 2 Extract
        assert_eq!(preview.edge_count, 4); // Js→Http, Http→Extract, 端点间, Detail Http→Extract
        assert_eq!(preview.js_block_count, 0); // Discover Js 节点无实际代码

        let kinds: Vec<NodeKind> = preview
            .graph
            .nodes
            .iter()
            .map(|n| n.spec.kind.clone())
            .collect();
        assert_eq!(kinds.iter().filter(|k| **k == NodeKind::Http).count(), 2);
        assert_eq!(kinds.iter().filter(|k| **k == NodeKind::Extract).count(), 2);
        assert_eq!(kinds.iter().filter(|k| **k == NodeKind::Js).count(), 1);

        let endpoints: Vec<EndpointKind> = preview
            .graph
            .nodes
            .iter()
            .filter_map(|n| n.spec.http.as_ref().map(|h| h.endpoint_kind.clone()))
            .collect();
        assert!(endpoints.contains(&EndpointKind::Discover));
        assert!(endpoints.contains(&EndpointKind::Detail));
    }

    #[test]
    fn maccms_discover_url_contains_ac_list_type_pg() {
        let preview = import_json("https://hnyun.com/api.php/provide/vod/");
        let disc_http = preview
            .graph
            .nodes
            .iter()
            .find(|n| {
                n.spec
                    .http
                    .as_ref()
                    .is_some_and(|h| h.endpoint_kind == EndpointKind::Discover)
            })
            .expect("无 Discover Http 节点");
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
        let det_http = preview
            .graph
            .nodes
            .iter()
            .find(|n| {
                n.spec
                    .http
                    .as_ref()
                    .is_some_and(|h| h.endpoint_kind == EndpointKind::Detail)
            })
            .expect("无 Detail Http 节点");
        let url = det_http.spec.http.as_ref().expect("Http spec").url.as_str();
        assert!(url.contains("ac=detail"));
        assert!(url.contains("ids={{vod_id}}"));
    }

    #[test]
    fn maccms_detail_extract_has_play_url_parser() {
        let preview = import_json("https://hnyun.com/api.php/provide/vod/");
        let det_ext = preview
            .graph
            .nodes
            .iter()
            .find(|n| {
                n.spec
                    .extract
                    .as_ref()
                    .is_some_and(|e| e.endpoint_kind == Some(EndpointKind::Detail))
            })
            .expect("无 Detail Extract 节点");
        let ext = det_ext.spec.extract.as_ref().expect("Extract spec");
        assert!(
            ext.play_url_parser.is_some(),
            "Detail Extract 应含 play_url_parser"
        );
        let parser = ext.play_url_parser.as_ref().unwrap();
        assert_eq!(parser.line_sep, "###");
        assert_eq!(parser.episode_sep, "#");
        assert_eq!(parser.name_url_sep, "$");
        assert_eq!(parser.play_from_sep, ",");
    }

    #[test]
    fn maccms_xml_format_uses_xpath_field_rules() {
        let preview = MaccmsImporter
            .import(MaccmsSourceUrl {
                url: "https://hnyun.com/api.php/provide/vod/".to_string(),
                at: MaccmsFormat::Xml,
            })
            .expect("import 失败");
        let disc_ext = preview
            .graph
            .nodes
            .iter()
            .find(|n| {
                n.spec
                    .extract
                    .as_ref()
                    .is_some_and(|e| e.endpoint_kind == Some(EndpointKind::Discover))
            })
            .expect("无 Discover Extract 节点");
        let ext = disc_ext.spec.extract.as_ref().expect("Extract spec");
        let name_rules = ext.field_rules.get("name").expect("缺 name field rule");
        assert!(
            name_rules
                .iter()
                .any(|r| matches!(r, lj_core::extract_rule::ExtractRule::XPath { .. })),
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
