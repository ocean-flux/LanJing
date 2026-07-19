//! Maccms10 JSON/XML 采集 API → `RuleDefinition` 翻译。
//!
//! 本模块只构造作者 Definition；不会生成旧 Graph、执行计划或运行时节点。节点 ID 由
//! 稳定来源身份与语义角色派生，使同一采集端点的 compiler 产物保持可复现。

use std::collections::{BTreeMap, HashMap};

use lj_capability::{IntentExport, StandardIntent};
use lj_rule_model::definition::MapperOutputKind;
use lj_rule_model::{
    CapabilityManifest, ControlledMapper, ExpectedDataType, ExtractRule, ExtractSpec, ExtractType,
    FlowEdge, FlowGraph, FlowNode, FlowNodeKind, HttpMethod, HttpSpec, PolicyCapabilities,
    RuleDefinition, SourceIdentity, SystemCapabilities,
};
use uuid::Uuid;

use super::types::MaccmsFormat;
use super::vocab::{
    ASSET_IDENTITY_FIELDS, COVER_FIELD, DESCRIPTION_FIELD, DISCOVERY_IDENTITY_FIELDS,
    ITEM_IDENTITY_FIELDS, KIND_FIELD, NAME_FIELD, PLAY_FROM_FIELD, PLAY_URL_FIELD, REMARKS_FIELD,
    TYPE_NAME_FIELD, UNIT_IDENTITY_FIELDS, VOD_CONTENT_FIELD, VOD_ID_EXPORT_FIELD, VOD_ID_FIELD,
    VOD_NAME_FIELD, VOD_PIC_FIELD, VOD_PLAY_FROM_FIELD, VOD_PLAY_URL_FIELD, VOD_REMARKS_FIELD,
};

/// 从已验证的 Maccms 端点构造只含标准意图的 Definition。
#[must_use]
pub(crate) fn definition(base_url: String, format: MaccmsFormat) -> RuleDefinition {
    let source_identity = source_identity(&base_url, format);
    let expected_type = match format {
        MaccmsFormat::Json => ExpectedDataType::Json,
        MaccmsFormat::Xml => ExpectedDataType::Xml,
    };
    let query_separator = if base_url.contains('?') { '&' } else { '?' };

    let discover_http = node_id(&source_identity, "discover-http");
    let discover_extract = node_id(&source_identity, "discover-extract");
    let discover_mapper = node_id(&source_identity, "discover-mapper");
    let detail_http = node_id(&source_identity, "detail-http");
    let detail_extract = node_id(&source_identity, "detail-extract");
    let item_mapper = node_id(&source_identity, "item-mapper");
    let unit_mapper = node_id(&source_identity, "unit-mapper");
    let asset_mapper = node_id(&source_identity, "asset-mapper");

    let discover_url = format!("{base_url}{query_separator}ac=list&t={{{{type}}}}&pg={{{{page}}}}");
    let detail_url = format!("{base_url}{query_separator}ac=detail&ids={{{{vod_id}}}}");
    let nodes = vec![
        http_node(discover_http, discover_url, expected_type),
        extract_node(
            discover_extract,
            discover_rules(format),
            discover_field_rules(format),
            expected_type,
        ),
        mapper_node(
            discover_mapper,
            MapperOutputKind::Discovery,
            DISCOVERY_IDENTITY_FIELDS,
        ),
        http_node(detail_http, detail_url, expected_type),
        extract_node(
            detail_extract,
            detail_rules(format),
            detail_field_rules(format),
            expected_type,
        ),
        mapper_node(item_mapper, MapperOutputKind::Items, ITEM_IDENTITY_FIELDS),
        mapper_node(unit_mapper, MapperOutputKind::Units, UNIT_IDENTITY_FIELDS),
        mapper_node(
            asset_mapper,
            MapperOutputKind::Assets,
            ASSET_IDENTITY_FIELDS,
        ),
    ];
    let edges = vec![
        edge(discover_http, discover_extract),
        edge(discover_extract, discover_mapper),
        edge(detail_http, detail_extract),
        edge(detail_extract, item_mapper),
        edge(detail_extract, unit_mapper),
        edge(detail_extract, asset_mapper),
    ];
    let intent_exports = BTreeMap::from([
        (
            StandardIntent::Discover,
            IntentExport::new(discover_http, discover_mapper),
        ),
        (
            StandardIntent::ResolveItem,
            IntentExport::new(detail_http, item_mapper),
        ),
        (
            StandardIntent::ListUnits,
            IntentExport::new(detail_http, unit_mapper),
        ),
        (
            StandardIntent::ResolveAsset,
            IntentExport::new(detail_http, asset_mapper),
        ),
    ]);

    RuleDefinition {
        schema_version: 1,
        source_identity: SourceIdentity {
            id: source_identity,
        },
        base_url,
        intent_exports,
        flow: FlowGraph { nodes, edges },
        capability_manifest: CapabilityManifest {
            required: PolicyCapabilities {
                network: true,
                system: SystemCapabilities::default(),
            },
        },
        source_id_rules: vec![VOD_ID_FIELD.to_string()],
    }
}

fn source_identity(base_url: &str, format: MaccmsFormat) -> String {
    let format_label = match format {
        MaccmsFormat::Json => "json",
        MaccmsFormat::Xml => "xml",
    };
    let digest = blake3::hash(format!("maccms:{format_label}:{base_url}").as_bytes());
    format!("source:maccms:{}", digest.to_hex())
}

fn node_id(source_identity: &str, role: &str) -> Uuid {
    let bytes = *blake3::hash(format!("{source_identity}:{role}").as_bytes()).as_bytes();
    let mut uuid_bytes = [0_u8; 16];
    uuid_bytes.copy_from_slice(&bytes[..16]);
    Uuid::from_bytes(uuid_bytes)
}

fn edge(from: Uuid, to: Uuid) -> FlowEdge {
    FlowEdge {
        from,
        to,
        condition_branch: None,
    }
}

fn http_node(id: Uuid, url: String, expected_type: ExpectedDataType) -> FlowNode {
    FlowNode {
        id,
        kind: FlowNodeKind::Http,
        http: Some(HttpSpec {
            method: HttpMethod::Get,
            url,
            headers: HashMap::new(),
            body: None,
            charset: None,
            expected_type,
        }),
        js_code: None,
        extract: None,
        mapper: None,
        span: None,
    }
}

fn extract_node(
    id: Uuid,
    rules: Vec<ExtractRule>,
    field_rules: HashMap<String, Vec<ExtractRule>>,
    expected_type: ExpectedDataType,
) -> FlowNode {
    FlowNode {
        id,
        kind: FlowNodeKind::Extract,
        http: None,
        js_code: None,
        extract: Some(ExtractSpec {
            rules,
            field_rules,
            expected_type,
            output_target: lj_rule_model::OutputTarget::default(),
        }),
        mapper: None,
        span: None,
    }
}

fn mapper_node(id: Uuid, output: MapperOutputKind, identity_fields: &[&str]) -> FlowNode {
    FlowNode {
        id,
        kind: FlowNodeKind::Mapper,
        http: None,
        js_code: None,
        extract: None,
        mapper: Some(ControlledMapper {
            output,
            identity_fields: identity_fields
                .iter()
                .map(|field| (*field).to_string())
                .collect(),
        }),
        span: None,
    }
}

/// 发现列表规则（取得 Maccms 的视频数组）。
pub(crate) fn discover_rules(format: MaccmsFormat) -> Vec<ExtractRule> {
    match format {
        MaccmsFormat::Json => vec![ExtractRule::JsonPath {
            path: "$.list[*]".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }],
        MaccmsFormat::Xml => vec![ExtractRule::XPath {
            expression: "/rss/list/video".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }],
    }
}

/// 详情规则（Maccms JSON/XML 与列表共享外层视频数组）。
pub(crate) fn detail_rules(format: MaccmsFormat) -> Vec<ExtractRule> {
    discover_rules(format)
}

/// 发现记录的字段规则。
pub(crate) fn discover_field_rules(format: MaccmsFormat) -> HashMap<String, Vec<ExtractRule>> {
    let mut fields = HashMap::new();
    match format {
        MaccmsFormat::Json => {
            insert_json(&mut fields, NAME_FIELD, &format!("$.{VOD_NAME_FIELD}"));
            insert_json(&mut fields, COVER_FIELD, &format!("$.{VOD_PIC_FIELD}"));
            insert_json(
                &mut fields,
                VOD_ID_EXPORT_FIELD,
                &format!("$.{VOD_ID_FIELD}"),
            );
            insert_json(&mut fields, KIND_FIELD, &format!("$.{TYPE_NAME_FIELD}"));
            insert_json(
                &mut fields,
                REMARKS_FIELD,
                &format!("$.{VOD_REMARKS_FIELD}"),
            );
        }
        MaccmsFormat::Xml => {
            insert_xml(&mut fields, NAME_FIELD, "name/text()");
            insert_xml(&mut fields, COVER_FIELD, "pic/text()");
            insert_xml(&mut fields, VOD_ID_EXPORT_FIELD, "id/text()");
            insert_xml(&mut fields, KIND_FIELD, "type/text()");
            insert_xml(&mut fields, REMARKS_FIELD, "note/text()");
        }
    }
    fields
}

/// 详情记录的字段规则，包括播放线路与播放地址。
pub(crate) fn detail_field_rules(format: MaccmsFormat) -> HashMap<String, Vec<ExtractRule>> {
    let mut fields = discover_field_rules(format);
    match format {
        MaccmsFormat::Json => {
            insert_json(
                &mut fields,
                DESCRIPTION_FIELD,
                &format!("$.{VOD_CONTENT_FIELD}"),
            );
            insert_json(
                &mut fields,
                PLAY_URL_FIELD,
                &format!("$.{VOD_PLAY_URL_FIELD}"),
            );
            insert_json(
                &mut fields,
                PLAY_FROM_FIELD,
                &format!("$.{VOD_PLAY_FROM_FIELD}"),
            );
        }
        MaccmsFormat::Xml => {
            insert_xml(&mut fields, DESCRIPTION_FIELD, "des");
            insert_xml(&mut fields, PLAY_URL_FIELD, "dl/dd");
            insert_xml_attr(&mut fields, PLAY_FROM_FIELD, "dl/dd", "flag");
        }
    }
    fields
}

fn insert_json(fields: &mut HashMap<String, Vec<ExtractRule>>, name: &str, path: &str) {
    fields.insert(
        name.to_string(),
        vec![ExtractRule::JsonPath {
            path: path.to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }],
    );
}

fn insert_xml(fields: &mut HashMap<String, Vec<ExtractRule>>, name: &str, expression: &str) {
    fields.insert(
        name.to_string(),
        vec![ExtractRule::XPath {
            expression: expression.to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }],
    );
}

fn insert_xml_attr(
    fields: &mut HashMap<String, Vec<ExtractRule>>,
    name: &str,
    expression: &str,
    attribute: &str,
) {
    fields.insert(
        name.to_string(),
        vec![ExtractRule::XPath {
            expression: expression.to_string(),
            extract_type: ExtractType::Attr(attribute.to_string()),
            regex_clean: None,
        }],
    );
}
