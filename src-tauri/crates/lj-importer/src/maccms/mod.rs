//! Maccms10 视频源 importer：采集 API URL → immutable Definition。
//!
//! Maccms 专属协议字段只停留在本模块的 `vocab` 与提取规则中。调用方只会取得
//! `RuleDefinition`；不会取得旧 Graph、节点处理器或执行器装配。

pub mod translator;
pub mod types;
pub(crate) mod vocab;

pub use types::{MaccmsFormat, MaccmsSourceUrl};

use lj_rule_model::{Error, RuleDefinition};

use vocab::API_PATH;

/// Maccms10 视频源 importer。
pub struct MaccmsImporter;

impl MaccmsImporter {
    /// 将 Maccms 采集端点转换为可交给 compiler 的 Definition。
    ///
    /// # Errors
    ///
    /// 输入不是 Maccms 采集 API URL 时返回 [`Error::Import`]。
    pub fn definition(&self, input: &MaccmsSourceUrl) -> Result<RuleDefinition, Error> {
        let endpoint = normalize_endpoint(&input.url)?;
        Ok(translator::definition(endpoint, input.at))
    }
}

fn normalize_endpoint(raw: &str) -> Result<String, Error> {
    let trimmed = raw.trim();
    if !trimmed.contains(API_PATH) {
        return Err(Error::Import(format!(
            "非 Maccms 采集 API URL（缺少 {API_PATH} 路径）"
        )));
    }
    let (path, query) = trimmed
        .split_once('?')
        .map_or((trimmed, None), |(path, query)| (path, Some(query)));
    let path = path.trim_end_matches('/');
    if path.is_empty() {
        return Err(Error::Import("Maccms 采集 API URL 不能为空".to_string()));
    }
    let normalized = format!("{path}/");
    Ok(query.map_or(normalized.clone(), |query| format!("{normalized}?{query}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lj_capability::StandardIntent;
    use lj_rule_model::{ExtractRule, FlowNodeKind};

    fn definition(format: MaccmsFormat, url: &str) -> RuleDefinition {
        MaccmsImporter
            .definition(&MaccmsSourceUrl {
                url: url.to_string(),
                at: format,
            })
            .expect("Maccms Definition 应生成")
    }

    #[test]
    fn maccms_json_definition_exports_four_standard_intents_without_graph() {
        let definition = definition(MaccmsFormat::Json, "https://hnyun.com/api.php/provide/vod/");
        assert_eq!(definition.flow.nodes.len(), 8);
        assert_eq!(definition.flow.edges.len(), 6);
        assert!(definition.capability_manifest.required.network);
        for intent in [
            StandardIntent::Discover,
            StandardIntent::ResolveItem,
            StandardIntent::ListUnits,
            StandardIntent::ResolveAsset,
        ] {
            assert!(
                definition.intent_exports.contains_key(&intent),
                "Maccms Definition 应导出 {intent:?}"
            );
        }
        assert!(definition.source_identity.id.starts_with("source:maccms:"));
        assert_eq!(definition.source_id_rules, vec!["vod_id"]);
    }

    #[test]
    fn maccms_definition_uses_stable_identity_and_node_ids() {
        let first = definition(MaccmsFormat::Json, "https://hnyun.com/api.php/provide/vod");
        let second = definition(MaccmsFormat::Json, "https://hnyun.com/api.php/provide/vod/");
        assert_eq!(first.source_identity, second.source_identity);
        assert_eq!(first.flow.nodes, second.flow.nodes);
        assert_eq!(first.flow.edges, second.flow.edges);
    }

    #[test]
    fn maccms_discover_and_detail_urls_keep_protocol_parameters() {
        let definition = definition(MaccmsFormat::Json, "https://hnyun.com/api.php/provide/vod/");
        let http_urls = definition
            .flow
            .nodes
            .iter()
            .filter(|node| node.kind == FlowNodeKind::Http)
            .filter_map(|node| node.http.as_ref().map(|spec| spec.url.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(http_urls.len(), 2);
        assert!(http_urls.iter().any(|url| {
            url.contains("ac=list") && url.contains("t={{type}}") && url.contains("pg={{page}}")
        }));
        assert!(
            http_urls
                .iter()
                .any(|url| url.contains("ac=detail") && url.contains("ids={{vod_id}}"))
        );
    }

    #[test]
    fn maccms_xml_definition_uses_xpath_field_rules() {
        let definition = definition(MaccmsFormat::Xml, "https://hnyun.com/api.php/provide/vod/");
        let extract = definition
            .flow
            .nodes
            .iter()
            .find(|node| node.kind == FlowNodeKind::Extract)
            .and_then(|node| node.extract.as_ref())
            .expect("Maccms Definition 应包含 Extract");
        let name_rules = extract
            .field_rules
            .get("name")
            .expect("缺少 name field rule");
        assert!(
            name_rules
                .iter()
                .any(|rule| matches!(rule, ExtractRule::XPath { .. })),
            "XML format 的 name field rule 应为 XPath"
        );
    }

    #[test]
    fn invalid_url_is_rejected_before_definition_creation() {
        let error = MaccmsImporter
            .definition(&MaccmsSourceUrl {
                url: "https://example.com/not-maccms".to_string(),
                at: MaccmsFormat::Json,
            })
            .expect_err("非法 Maccms URL 必须失败");
        assert!(matches!(error, Error::Import(_)));
    }
}
