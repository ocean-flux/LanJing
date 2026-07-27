//! Maccms10 视频源 importer：采集 API URL → immutable Definition。
//!
//! Maccms 专属协议字段只停留在本模块的 `vocab` 与提取规则中。调用方只会取得
//! `RuleDefinition`；不会取得旧 Graph、节点处理器或执行器装配。

pub mod translator;
pub mod types;
pub(crate) mod vocab;

pub use types::{MaccmsFormat, MaccmsSourceUrl};

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use lj_rule_model::{Error, RequestHeaderDisposition, RuleDefinition, SensitiveNamePolicy};
use url::Url;

use vocab::API_PATH;

/// Maccms10 视频源 importer。
pub struct MaccmsImporter;

/// 已保存 Maccms10 JSON endpoint 文档进入既有 Definition adapter 的结果。
///
/// credential headers 只通过 [`Self::credential_snapshot_bytes`] 显式移交给加密 candidate；
/// 此类型不实现可泄漏 plaintext 的派生 `Debug`/`Clone`/serde。
pub struct MaccmsDocumentImport {
    /// 仅含 non-sensitive request headers 的 Definition。
    pub definition: RuleDefinition,
    credential_headers: BTreeMap<String, String>,
}

impl MaccmsDocumentImport {
    /// 将 runtime credential headers 序列化为 opaque secret bytes。
    ///
    /// # Errors
    ///
    /// credential map 无法序列化时返回不含 plaintext 的 [`Error::Import`]。
    pub fn credential_snapshot_bytes(&self) -> Result<Option<Vec<u8>>, Error> {
        if self.credential_headers.is_empty() {
            return Ok(None);
        }
        serde_json::to_vec(&self.credential_headers)
            .map(Some)
            .map_err(|_| Error::Import("Maccms credential snapshot 无法序列化".to_string()))
    }
}

impl fmt::Debug for MaccmsDocumentImport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MaccmsDocumentImport")
            .field("source_identity", &self.definition.source_identity().id)
            .field("has_credentials", &!self.credential_headers.is_empty())
            .finish()
    }
}

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

    /// 将已通过 authoring codec 的 Maccms10 endpoint JSON 转为 Definition 与 credential snapshot。
    ///
    /// 当前 vault 路径只接受 `{ base_url, format: "json", headers? }`；XML/query builder 与完整
    /// endpoint workspace 仍由后续 Maccms 任务拥有。未知字段保留在 vault 原文，不进入 Definition。
    ///
    /// # Errors
    ///
    /// JSON/root/base URL/format/header 类型无效，URL 含 userinfo/query/fragment，header 被共享
    /// sensitive policy 阻断，或 endpoint 不是 Maccms `Provide::vod` 路径时返回 [`Error::Import`]。
    pub fn import_document(&self, text: &str) -> Result<MaccmsDocumentImport, Error> {
        let value = serde_json::from_str::<serde_json::Value>(text)
            .map_err(|_| Error::Import("Maccms endpoint 文档 JSON 无效".to_string()))?;
        let object = value
            .as_object()
            .ok_or_else(|| Error::Import("Maccms endpoint 文档根必须是 object".to_string()))?;
        let base_url = object
            .get("base_url")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| Error::Import("Maccms endpoint base_url 无效".to_string()))?;
        match object.get("format").and_then(serde_json::Value::as_str) {
            Some("json") => {}
            _ => {
                return Err(Error::Import(
                    "当前 Maccms endpoint 文档仅支持 JSON format".to_string(),
                ));
            }
        }
        let parsed_url = Url::parse(base_url)
            .map_err(|_| Error::Import("Maccms endpoint base_url 无效".to_string()))?;
        if !matches!(parsed_url.scheme(), "http" | "https")
            || !parsed_url.username().is_empty()
            || parsed_url.password().is_some()
            || parsed_url.query().is_some()
            || parsed_url.fragment().is_some()
        {
            return Err(Error::Import(
                "Maccms endpoint base_url 不能包含 userinfo/query/fragment".to_string(),
            ));
        }

        let mut public_headers = HashMap::new();
        let mut credential_headers = BTreeMap::new();
        let mut credential_names: Vec<String> = Vec::new();
        if let Some(headers) = object.get("headers") {
            let headers = headers
                .as_object()
                .ok_or_else(|| Error::Import("Maccms endpoint headers 无效".to_string()))?;
            for (name, value) in headers {
                let value = value.as_str().ok_or_else(|| {
                    Error::Import("Maccms endpoint header value 无效".to_string())
                })?;
                match SensitiveNamePolicy::request_header_disposition(name) {
                    RequestHeaderDisposition::Public => {
                        public_headers.insert(name.clone(), value.to_string());
                    }
                    RequestHeaderDisposition::Credential => {
                        if credential_names
                            .iter()
                            .any(|existing| SensitiveNamePolicy::equivalent(existing, name))
                        {
                            return Err(Error::Import(
                                "Maccms endpoint credential header 重复".to_string(),
                            ));
                        }
                        credential_names.push(name.clone());
                        credential_headers.insert(name.clone(), value.to_string());
                    }
                    RequestHeaderDisposition::Blocked => {
                        return Err(Error::Import(
                            "Maccms endpoint request header 被安全策略阻断".to_string(),
                        ));
                    }
                }
            }
        }
        let endpoint = normalize_endpoint(base_url)?;
        Ok(MaccmsDocumentImport {
            definition: translator::definition_with_headers(
                endpoint,
                MaccmsFormat::Json,
                public_headers,
            ),
            credential_headers,
        })
    }
}

fn normalize_endpoint(raw: &str) -> Result<String, Error> {
    let mut parsed = Url::parse(raw.trim())
        .map_err(|_| Error::Import("Maccms 采集 API URL 无效".to_string()))?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(Error::Import(
            "Maccms 采集 API URL 不能包含 userinfo/query/fragment".to_string(),
        ));
    }
    let path = parsed.path().trim_end_matches('/').to_string();
    if !path.ends_with(API_PATH) {
        return Err(Error::Import(format!(
            "非 Maccms 采集 API URL（缺少 {API_PATH} 路径）"
        )));
    }
    parsed.set_path(&format!("{path}/"));
    Ok(parsed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lj_capability::StandardIntent;
    use lj_rule_model::{ContractSchemaVersion, ExtractRule, FlowNodeConfig, FlowNodeKind};

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
        assert_eq!(definition.schema_version(), ContractSchemaVersion::V2);
        assert_eq!(definition.flow().nodes.len(), 8);
        assert_eq!(definition.flow().edges.len(), 6);
        assert!(definition.capability_manifest().required.network);
        for intent in [
            StandardIntent::Discover,
            StandardIntent::ResolveItem,
            StandardIntent::ListUnits,
            StandardIntent::ResolveAsset,
        ] {
            assert!(
                definition.intent_exports().contains_key(&intent),
                "Maccms Definition 应导出 {intent:?}"
            );
        }
        assert!(
            definition
                .source_identity()
                .id
                .starts_with("source:maccms:")
        );
        assert_eq!(definition.source_id_rules(), ["vod_id"]);
    }

    #[test]
    fn maccms_definition_uses_stable_identity_and_node_ids() {
        let first = definition(MaccmsFormat::Json, "https://hnyun.com/api.php/provide/vod");
        let second = definition(MaccmsFormat::Json, "https://hnyun.com/api.php/provide/vod/");
        assert_eq!(first.source_identity(), second.source_identity());
        assert_eq!(first.flow().nodes, second.flow().nodes);
        assert_eq!(first.flow().edges, second.flow().edges);
    }

    #[test]
    fn maccms_discover_and_detail_urls_keep_protocol_parameters() {
        let definition = definition(MaccmsFormat::Json, "https://hnyun.com/api.php/provide/vod/");
        let http_urls = definition
            .flow()
            .nodes
            .iter()
            .filter(|node| node.kind() == FlowNodeKind::Http)
            .filter_map(|node| match &node.config {
                FlowNodeConfig::Http(spec) => Some(spec.url.as_str()),
                _ => None,
            })
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
            .flow()
            .nodes
            .iter()
            .find_map(|node| match &node.config {
                FlowNodeConfig::Extract(spec) => Some(spec),
                _ => None,
            })
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
    fn endpoint_document_separates_runtime_credentials_from_definition() {
        let secret = "maccms-document-secret";
        let text = serde_json::json!({
            "base_url": "https://hnyun.com/api.php/provide/vod/",
            "format": "json",
            "headers": {
                "User-Agent": "lanjing-maccms",
                "Authorization": format!("Bearer {secret}")
            },
            "preserved": { "future": true }
        })
        .to_string();
        let imported = MaccmsImporter
            .import_document(&text)
            .expect("saved Maccms JSON document must import");
        let definition = lj_rule_model::canonical_json(&imported.definition)
            .expect("Maccms Definition canonical JSON");
        assert!(!definition.contains(secret));
        for http in imported
            .definition
            .flow()
            .nodes
            .iter()
            .filter_map(|node| match &node.config {
                FlowNodeConfig::Http(spec) => Some(spec),
                _ => None,
            })
        {
            assert_eq!(
                http.headers.get("User-Agent").map(String::as_str),
                Some("lanjing-maccms")
            );
            assert!(!http.headers.contains_key("Authorization"));
        }
        let snapshot = imported
            .credential_snapshot_bytes()
            .expect("credential snapshot serialization")
            .expect("sensitive header must create a runtime credential snapshot");
        assert!(
            String::from_utf8(snapshot)
                .expect("snapshot JSON")
                .contains(secret)
        );
        assert!(!format!("{imported:?}").contains(secret));

        for invalid in [
            r#"{"base_url":"https://user@example.com/api.php/provide/vod/","format":"json"}"#,
            r#"{"base_url":"https://example.com/api.php/provide/vod/?token=secret","format":"json"}"#,
            r#"{"base_url":"https://example.com/api.php/provide/vod/","format":"xml"}"#,
        ] {
            assert!(MaccmsImporter.import_document(invalid).is_err());
        }
    }

    #[test]
    fn invalid_url_is_rejected_before_definition_creation() {
        for url in [
            "https://example.com/not-maccms",
            "https://user@example.com/api.php/provide/vod/",
            "https://example.com/api.php/provide/vod/?token=secret",
            "https://example.com/api.php/provide/vodcast/",
        ] {
            let error = MaccmsImporter
                .definition(&MaccmsSourceUrl {
                    url: url.to_string(),
                    at: MaccmsFormat::Json,
                })
                .expect_err("非法 Maccms URL 必须失败");
            assert!(matches!(error, Error::Import(_)));
        }
    }
}
