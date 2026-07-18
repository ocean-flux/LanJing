//! Maccms10 视频源 translator — 采集 API URL → 标准意图 Graph。
//!
//! 参考 `legado::translator` 的 `build_http_extract_pair` 模式,详情 Flow 带播放 URL 解析字段。

use std::collections::HashMap;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use lj_rule_model::{ExpectedDataType, ExtractRule, ExtractSpec, ExtractType};
use lj_rule_model::{HttpMethod, HttpSpec};
use lj_runtime::{Edge, JsSpec, MapperOutputKind, MapperSpec, Node, NodeId, NodeKind, NodeSpec};

use super::types::MaccmsFormat;
use super::vocab::{
    COVER_FIELD, DESCRIPTION_FIELD, KIND_FIELD, NAME_FIELD, PLAY_FROM_FIELD, PLAY_URL_FIELD,
    REMARKS_FIELD, TYPE_NAME_FIELD, VOD_CONTENT_FIELD, VOD_ID_EXPORT_FIELD, VOD_ID_FIELD,
    VOD_NAME_FIELD, VOD_PIC_FIELD, VOD_PLAY_FROM_FIELD, VOD_PLAY_URL_FIELD, VOD_REMARKS_FIELD,
};

/// 构建一对 Http + Extract 节点。
pub(crate) fn build_pair(
    url: &str,
    expected_type: ExpectedDataType,
    rules: Vec<ExtractRule>,
    field_rules: HashMap<String, Vec<ExtractRule>>,
) -> (Node, Node) {
    let http_spec = HttpSpec {
        method: HttpMethod::Get,
        url: url.to_string(),
        headers: HashMap::new(),
        body: None,
        charset: None,
        expected_type,
    };
    let http_node = create_node(NodeSpec {
        kind: NodeKind::Http,
        http: Some(http_spec),
        js: None,
        extract: None,
        mapper: None,
    });

    let extract_spec = ExtractSpec {
        rules,
        field_rules,
        expected_type,
        output_target: lj_rule_model::OutputTarget::default(),
    };
    let extract_node = create_node(NodeSpec {
        kind: NodeKind::Extract,
        http: None,
        js: None,
        extract: Some(extract_spec),
        mapper: None,
    });

    (http_node, extract_node)
}

/// 创建 Js 节点(Maccms 发现用空 code 满足首个 Flow 入口,无 @js: 块)。
#[must_use]
pub(crate) fn create_js_node(code: &str) -> Node {
    create_node(NodeSpec {
        kind: NodeKind::Js,
        http: None,
        js: Some(JsSpec {
            code: code.to_string(),
        }),
        extract: None,
        mapper: None,
    })
}

/// 创建受控 Mapper 节点。
#[must_use]
pub(crate) fn create_mapper_node(output: MapperOutputKind, identity_fields: &[&str]) -> Node {
    create_node(NodeSpec {
        kind: NodeKind::Mapper,
        http: None,
        js: None,
        extract: None,
        mapper: Some(MapperSpec {
            output,
            identity_fields: identity_fields
                .iter()
                .map(|field| (*field).to_string())
                .collect(),
        }),
    })
}

/// 创建带 `import_hash` 的节点。
fn create_node(spec: NodeSpec) -> Node {
    use std::fmt::Write;
    let node_id = NodeId(Uuid::new_v4());
    let json = serde_json::to_string(&spec).unwrap_or_default();
    let hash = Sha256::digest(json.as_bytes());
    let mut hex = String::with_capacity(64);
    for b in hash {
        let _ = write!(hex, "{b:02x}");
    }
    Node {
        node_id,
        import_hash: hex,
        spec,
    }
}

/// 发现列表 rules(取 item 列表的 XPath/JSONPath)。
pub(crate) fn discover_rules(at: MaccmsFormat) -> Vec<ExtractRule> {
    match at {
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

/// 详情 item rules(取首 item 的 XPath/JSONPath)。
pub(crate) fn detail_rules(at: MaccmsFormat) -> Vec<ExtractRule> {
    // detail 响应同 list 结构(<rss><list><video> 或 {list:[...]}),取首 item
    discover_rules(at)
}

/// 发现字段 rules(列表项元数据,KTD4 按 format 分别生成)。
pub(crate) fn discover_field_rules(at: MaccmsFormat) -> HashMap<String, Vec<ExtractRule>> {
    let mut map = HashMap::new();
    match at {
        MaccmsFormat::Json => {
            insert_json(&mut map, NAME_FIELD, &format!("$.{VOD_NAME_FIELD}"));
            insert_json(&mut map, COVER_FIELD, &format!("$.{VOD_PIC_FIELD}"));
            insert_json(&mut map, VOD_ID_EXPORT_FIELD, &format!("$.{VOD_ID_FIELD}"));
            insert_json(&mut map, KIND_FIELD, &format!("$.{TYPE_NAME_FIELD}"));
            insert_json(&mut map, REMARKS_FIELD, &format!("$.{VOD_REMARKS_FIELD}"));
        }
        MaccmsFormat::Xml => {
            insert_xml(&mut map, NAME_FIELD, "name/text()");
            insert_xml(&mut map, COVER_FIELD, "pic/text()");
            insert_xml(&mut map, VOD_ID_EXPORT_FIELD, "id/text()");
            insert_xml(&mut map, KIND_FIELD, "type/text()");
            insert_xml(&mut map, REMARKS_FIELD, "note/text()");
        }
    }
    map
}

/// 详情字段 rules(元数据 + `vod_play_url/vod_play_from,KTD4`)。
pub(crate) fn detail_field_rules(at: MaccmsFormat) -> HashMap<String, Vec<ExtractRule>> {
    let mut map = discover_field_rules(at);
    match at {
        MaccmsFormat::Json => {
            insert_json(
                &mut map,
                DESCRIPTION_FIELD,
                &format!("$.{VOD_CONTENT_FIELD}"),
            );
            insert_json(&mut map, PLAY_URL_FIELD, &format!("$.{VOD_PLAY_URL_FIELD}"));
            insert_json(
                &mut map,
                PLAY_FROM_FIELD,
                &format!("$.{VOD_PLAY_FROM_FIELD}"),
            );
        }
        MaccmsFormat::Xml => {
            // Maccms XML 详情：<des><![CDATA[...]]></des>
            insert_xml(&mut map, DESCRIPTION_FIELD, "des");
            // Maccms XML 详情：<dl><dd flag="hnyun"><![CDATA[...]]></dd>...</dl>
            // playUrl：取所有 dd 的 CDATA，$$$ 拼接
            insert_xml(&mut map, PLAY_URL_FIELD, "dl/dd");
            // playFrom：取所有 dd 的 flag 属性，$$$ 拼接
            insert_xml_attr(&mut map, PLAY_FROM_FIELD, "dl/dd", "flag");
        }
    }
    map
}

/// 插入 `JSONPath` 字段规则。
fn insert_json(map: &mut HashMap<String, Vec<ExtractRule>>, name: &str, path: &str) {
    map.insert(
        name.to_string(),
        vec![ExtractRule::JsonPath {
            path: path.to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }],
    );
}

/// 插入 `XPath` 字段规则。
fn insert_xml(map: &mut HashMap<String, Vec<ExtractRule>>, name: &str, expr: &str) {
    map.insert(
        name.to_string(),
        vec![ExtractRule::XPath {
            expression: expr.to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }],
    );
}

/// 插入 `XPath` 属性字段规则。
fn insert_xml_attr(
    map: &mut HashMap<String, Vec<ExtractRule>>,
    name: &str,
    expr: &str,
    attr: &str,
) {
    map.insert(
        name.to_string(),
        vec![ExtractRule::XPath {
            expression: expr.to_string(),
            extract_type: ExtractType::Attr(attr.to_string()),
            regex_clean: None,
        }],
    );
}

/// 连接发现 Extract → 详情 Http 的 Flow 延续边。
pub(crate) fn connect_flow_edges(
    edges: &mut Vec<Edge>,
    discover_extract_id: &NodeId,
    detail_http_id: &NodeId,
) {
    edges.push(Edge {
        from: discover_extract_id.clone(),
        to: detail_http_id.clone(),
        condition_branch: None,
    });
}
