//! Maccms10 视频源 translator — 采集 API URL → Graph（Discover/Detail endpoint subgraph）。
//!
//! 参考 `legado::translator` 的 `build_http_extract_pair` 模式,但 Detail 端点带 `play_url_parser`。

use std::collections::HashMap;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use lj_core::endpoint::{EndpointKind, HttpMethod, HttpSpec};
use lj_core::extract_rule::{
    ExpectedDataType, ExtractRule, ExtractSpec, ExtractType, PlayUrlParserSpec,
};
use lj_core::node::{Edge, JsSpec, Node, NodeId, NodeKind, NodeSpec};

use super::types::MaccmsFormat;

/// 构建一对 Http + Extract 节点(Extract 含可选 `play_url_parser`,Maccms Detail 用)。
pub(crate) fn build_pair(
    kind: EndpointKind,
    url: &str,
    expected_type: ExpectedDataType,
    rules: Vec<ExtractRule>,
    field_rules: HashMap<String, Vec<ExtractRule>>,
    play_url_parser: Option<PlayUrlParserSpec>,
) -> (Node, Node) {
    let http_spec = HttpSpec {
        endpoint_kind: kind.clone(),
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
    });

    let extract_spec = ExtractSpec {
        rules,
        field_rules,
        endpoint_kind: Some(kind),
        expected_type,
        play_url_parser,
    };
    let extract_node = create_node(NodeSpec {
        kind: NodeKind::Extract,
        http: None,
        js: None,
        extract: Some(extract_spec),
    });

    (http_node, extract_node)
}

/// 创建 Js 节点(Maccms Discover 用空 code 满足 schema,无 @js: 块)。
#[must_use]
pub(crate) fn create_js_node(endpoint_kind: EndpointKind, code: &str) -> Node {
    create_node(NodeSpec {
        kind: NodeKind::Js,
        http: None,
        js: Some(JsSpec {
            code: code.to_string(),
            endpoint_kind: Some(endpoint_kind),
        }),
        extract: None,
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

/// Discover 列表 rules(取 item 列表的 XPath/JSONPath)。
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

/// Detail item rules(取首 item 的 XPath/JSONPath)。
pub(crate) fn detail_rules(at: MaccmsFormat) -> Vec<ExtractRule> {
    // detail 响应同 list 结构(<rss><list><video> 或 {list:[...]}),取首 item
    discover_rules(at)
}

/// Discover 字段 rules(列表项元数据,KTD4 按 format 分别生成)。
pub(crate) fn discover_field_rules(at: MaccmsFormat) -> HashMap<String, Vec<ExtractRule>> {
    let mut map = HashMap::new();
    match at {
        MaccmsFormat::Json => {
            insert_json(&mut map, "name", "$.vod_name");
            insert_json(&mut map, "cover", "$.vod_pic");
            insert_json(&mut map, "vodId", "$.vod_id");
            insert_json(&mut map, "kind", "$.type_name");
            insert_json(&mut map, "remarks", "$.vod_remarks");
        }
        MaccmsFormat::Xml => {
            insert_xml(&mut map, "name", "name/text()");
            insert_xml(&mut map, "cover", "pic/text()");
            insert_xml(&mut map, "vodId", "id/text()");
            insert_xml(&mut map, "kind", "type/text()");
            insert_xml(&mut map, "remarks", "note/text()");
        }
    }
    map
}

/// Detail 字段 rules(元数据 + `vod_play_url/vod_play_from,KTD4`)。
pub(crate) fn detail_field_rules(at: MaccmsFormat) -> HashMap<String, Vec<ExtractRule>> {
    let mut map = discover_field_rules(at);
    match at {
        MaccmsFormat::Json => {
            insert_json(&mut map, "description", "$.vod_content");
            insert_json(&mut map, "playUrl", "$.vod_play_url");
            insert_json(&mut map, "playFrom", "$.vod_play_from");
        }
        MaccmsFormat::Xml => {
            // Maccms XML detail: <des><![CDATA[...]]></des>
            insert_xml(&mut map, "description", "des");
            // Maccms XML detail: <dl><dd flag="hnyun"><![CDATA[...]]></dd>...</dl>
            // playUrl: 取所有 dd 的 CDATA 内容，$$$ 拼接
            insert_xml(&mut map, "playUrl", "dl/dd");
            // playFrom: 取所有 dd 的 flag 属性，$$$ 拼接
            insert_xml(&mut map, "playFrom", "dl/dd/@flag");
        }
    }
    map
}

/// `play_url_parser` defaults(Maccms 协议约定,KTD3)。
#[must_use]
pub(crate) fn play_url_parser_defaults() -> PlayUrlParserSpec {
    PlayUrlParserSpec {
        line_sep: "###".to_string(),
        episode_sep: "#".to_string(),
        name_url_sep: "$".to_string(),
        play_from_sep: ",".to_string(),
    }
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

/// 连接 Discover Extract → Detail Http 端点间边(执行流延续,ADR-0025 段边界在 IPC 层)。
pub(crate) fn connect_endpoint_edges(
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
