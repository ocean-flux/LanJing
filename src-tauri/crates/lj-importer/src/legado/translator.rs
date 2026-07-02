//! Legado 规则翻译器 — 将 Legado 规则翻译为节点图。
//!
//! 包含 5 端点翻译函数、端点间边连接、`import_hash` 计算、JSON 解析等辅助。

use std::collections::HashMap;

use sha2::{Digest, Sha256};
use uuid::Uuid;

use lj_compiler::legado_parser::parse_legado_rule;
use lj_core::endpoint::{EndpointKind, HttpMethod, HttpSpec};
use lj_core::error::CoreError;
use lj_core::extract_rule::{ExpectedDataType, ExtractRule, ExtractSpec, FieldRules};
use lj_core::node::{Edge, JsSpec, Node, NodeId, NodeKind, NodeSpec};

use super::types::{RuleBookInfo, RuleContent, RuleExplore, RuleSearch, RuleToc};

/// 端点翻译状态(避免 `too_many_lines`)。
pub(crate) struct EndpointState {
    /// 生成的节点。
    pub(crate) nodes: Vec<Node>,
    /// 生成的边。
    pub(crate) edges: Vec<Edge>,
    /// HTTP 目标 URL。
    pub(crate) http_target_urls: Vec<String>,
    /// JS 源码块。
    pub(crate) js_sources: Vec<String>,
    /// search/discover Extract 节点 ID(→ detail Http)。
    pub(crate) search_extract_id: Option<NodeId>,
    /// detail Http 节点 ID。
    pub(crate) detail_http_id: Option<NodeId>,
    /// detail Extract 节点 ID。
    pub(crate) detail_extract_id: Option<NodeId>,
    /// toc Http 节点 ID。
    pub(crate) toc_http_id: Option<NodeId>,
    /// toc Extract 节点 ID。
    pub(crate) toc_extract_id: Option<NodeId>,
    /// content Http 节点 ID。
    pub(crate) content_http_id: Option<NodeId>,
}

/// 翻译 discover 端点(Js → Http → Extract)。
///
/// discover 段的 Http 节点 URL 使用 `{{discoverUrl}}` 模板变量，
/// 实际 URL 由 executor 在运行时从 Js 节点产出中解析。
/// executor 需要特殊处理:Js 产出(URL 列表 JSON)→ Http 消费(逐 URL 请求)。
pub(crate) fn translate_discover(
    explore_url: &str,
    headers: &HashMap<String, String>,
    rule: Option<&RuleExplore>,
    st: &mut EndpointState,
) -> Result<(), CoreError> {
    let js_code = extract_js_code(explore_url);
    st.js_sources.push(js_code.clone());

    let node_js = create_node(NodeSpec {
        kind: NodeKind::Js,
        http: None,
        js: Some(JsSpec {
            code: js_code,
            endpoint_kind: Some(EndpointKind::Discover),
        }),
        extract: None,
    });

    // URL 用 {{key}} 模板变量，executor 将分类 URL 作为 query 注入。
    let (explore_rules, explore_field_rules) = collect_explore_rules(rule)?;
    let (http_node, extract_node) = build_http_extract_pair(
        EndpointKind::Discover,
        "{{key}}",
        headers,
        ExpectedDataType::Html,
        &explore_rules,
        explore_field_rules,
    );

    // Js(output=Raw) → Http(input=None):源头节点,输入不限制
    st.edges.push(Edge {
        from: node_js.node_id.clone(),
        to: http_node.node_id.clone(),
        condition_branch: None,
    });
    // Http(output=HttpResponse) → Extract(input=HttpResponse)
    st.edges.push(Edge {
        from: http_node.node_id.clone(),
        to: extract_node.node_id.clone(),
        condition_branch: None,
    });
    st.search_extract_id = st
        .search_extract_id
        .clone()
        .or(Some(extract_node.node_id.clone()));

    st.nodes.push(node_js);
    st.nodes.push(http_node);
    st.nodes.push(extract_node);
    Ok(())
}

/// 翻译 detail 端点(Http → Extract)。
pub(crate) fn translate_detail(
    rule: &RuleBookInfo,
    headers: &HashMap<String, String>,
    st: &mut EndpointState,
) -> Result<(), CoreError> {
    let (detail_rules, detail_field_rules) = collect_book_info_rules(Some(rule))?;
    let (http_node, extract_node) = build_http_extract_pair(
        EndpointKind::Detail,
        "{{bookUrl}}",
        headers,
        ExpectedDataType::Html,
        &detail_rules,
        detail_field_rules,
    );
    st.edges.push(Edge {
        from: http_node.node_id.clone(),
        to: extract_node.node_id.clone(),
        condition_branch: None,
    });
    st.detail_http_id = Some(http_node.node_id.clone());
    st.detail_extract_id = Some(extract_node.node_id.clone());
    st.nodes.push(http_node);
    st.nodes.push(extract_node);
    Ok(())
}

/// 翻译 toc 端点(Http → Extract)。
pub(crate) fn translate_toc(
    rule: &RuleToc,
    headers: &HashMap<String, String>,
    st: &mut EndpointState,
) -> Result<(), CoreError> {
    let (toc_rules, toc_field_rules) = collect_toc_rules(Some(rule))?;
    let (http_node, extract_node) = build_http_extract_pair(
        EndpointKind::Toc,
        "{{bookUrl}}",
        headers,
        ExpectedDataType::Html,
        &toc_rules,
        toc_field_rules,
    );
    st.edges.push(Edge {
        from: http_node.node_id.clone(),
        to: extract_node.node_id.clone(),
        condition_branch: None,
    });
    st.toc_http_id = Some(http_node.node_id.clone());
    st.toc_extract_id = Some(extract_node.node_id.clone());
    st.nodes.push(http_node);
    st.nodes.push(extract_node);
    Ok(())
}

/// 翻译 content 端点(Http → Extract)。
pub(crate) fn translate_content(
    rule: &RuleContent,
    headers: &HashMap<String, String>,
    st: &mut EndpointState,
) -> Result<(), CoreError> {
    let (content_rules, content_field_rules) = collect_content_rules(Some(rule))?;
    let (http_node, extract_node) = build_http_extract_pair(
        EndpointKind::Content,
        "{{chapterUrl}}",
        headers,
        ExpectedDataType::Html,
        &content_rules,
        content_field_rules,
    );
    st.edges.push(Edge {
        from: http_node.node_id.clone(),
        to: extract_node.node_id.clone(),
        condition_branch: None,
    });
    st.content_http_id = Some(http_node.node_id.clone());
    st.nodes.push(http_node);
    st.nodes.push(extract_node);
    Ok(())
}

/// 构建端点间边。
///
/// 连接顺序: search/discover Extract → detail Http → toc Http → content Http。
pub(crate) fn connect_endpoint_edges(st: &mut EndpointState) {
    // search/discover Extract → detail Http
    if let Some(ref from) = st.search_extract_id
        && let Some(ref to) = st.detail_http_id
    {
        st.edges.push(Edge {
            from: from.clone(),
            to: to.clone(),
            condition_branch: None,
        });
    }
    // detail Extract → toc Http
    if let Some(ref from) = st.detail_extract_id
        && let Some(ref to) = st.toc_http_id
    {
        st.edges.push(Edge {
            from: from.clone(),
            to: to.clone(),
            condition_branch: None,
        });
    }
    // toc Extract → content Http
    if let Some(ref from) = st.toc_extract_id
        && let Some(ref to) = st.content_http_id
    {
        st.edges.push(Edge {
            from: from.clone(),
            to: to.clone(),
            condition_branch: None,
        });
    }
}

// ===== 内部辅助 =====

/// 构建一对 Http + Extract 节点。
pub(crate) fn build_http_extract_pair(
    kind: EndpointKind,
    url: &str,
    headers: &HashMap<String, String>,
    expected_type: ExpectedDataType,
    rules: &[ExtractRule],
    field_rules: FieldRules,
) -> (Node, Node) {
    let http_spec = HttpSpec {
        endpoint_kind: kind.clone(),
        method: HttpMethod::Get,
        url: url.to_string(),
        headers: headers.clone(),
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
        rules: rules.to_vec(),
        field_rules,
        endpoint_kind: Some(kind),
        expected_type,
        play_url_parser: None,
    };
    let extract_node = create_node(NodeSpec {
        kind: NodeKind::Extract,
        http: None,
        js: None,
        extract: Some(extract_spec),
    });

    (http_node, extract_node)
}

/// 创建带 `import_hash` 的节点。
fn create_node(spec: NodeSpec) -> Node {
    let node_id = NodeId(Uuid::new_v4());
    let import_hash = compute_import_hash(&spec);
    Node {
        node_id,
        import_hash,
        spec,
    }
}

/// 计算节点 `import_hash`(64 字符 hex `sha256` canonical json spec)。
#[must_use]
pub fn compute_import_hash(spec: &NodeSpec) -> String {
    let json = serde_json::to_string(spec).unwrap_or_default();
    let hash = Sha256::digest(json.as_bytes());
    let mut hex = String::with_capacity(64);
    for b in hash {
        use std::fmt::Write;
        let _ = write!(hex, "{b:02x}");
    }
    hex
}

/// 从 `@js:` 前缀字符串中提取 JS 代码。
fn extract_js_code(explore_url: &str) -> String {
    if let Some(rest) = explore_url.strip_prefix("@js:") {
        rest.trim().to_string()
    } else {
        explore_url.to_string()
    }
}

/// 解析 `header` JSON 字符串为键值对。
///
/// # Errors
///
/// 返回 `CoreError::Import` 当 header JSON 格式无效。
pub(crate) fn parse_headers(header: Option<&str>) -> Result<HashMap<String, String>, CoreError> {
    match header {
        None => Ok(HashMap::new()),
        Some(s) if s.trim().is_empty() => Ok(HashMap::new()),
        Some(s) => {
            serde_json::from_str(s).map_err(|e| CoreError::Import(format!("解析 header 失败: {e}")))
        }
    }
}

/// 解析单个规则字段。
///
/// # Errors
///
/// 规则字符串语法错误时返回 `CoreError::Import`。
fn parse_rule_field(field: Option<&String>) -> Result<Vec<ExtractRule>, CoreError> {
    match field {
        None => Ok(Vec::new()),
        Some(s) if s.trim().is_empty() => Ok(Vec::new()),
        Some(s) => {
            parse_legado_rule(s).map_err(|e| CoreError::Import(format!("规则解析失败: {e}")))
        }
    }
}

macro_rules! collect_rules {
    ($rule:expr, $($field:ident),+ $(,)?) => {{
        let mut rules = Vec::new();
        if let Some(r) = $rule {
            $( rules.extend(parse_rule_field(r.$field.as_ref())?); )+
        }
        Ok::<Vec<ExtractRule>, CoreError>(rules)
    }};
}

/// 收集列表端点(search/explore)的 bookList + 字段提取规则。
///
/// `kind` 为 `None` 时跳过 "kind" 字段(explore 端点无此字段)。
fn collect_list_field_rules(
    book_list: Option<&String>,
    name: Option<&String>,
    author: Option<&String>,
    book_url: Option<&String>,
    cover_url: Option<&String>,
    kind: Option<&String>,
) -> Result<(Vec<ExtractRule>, FieldRules), CoreError> {
    let mut field_rules: FieldRules = FieldRules::new();
    let mut book_list_rules: Vec<ExtractRule> = Vec::new();
    if let Some(f) = book_list {
        book_list_rules = parse_rule_field(Some(f))?;
    }
    insert_field(&mut field_rules, "name", parse_rule_field(name)?);
    insert_field(&mut field_rules, "author", parse_rule_field(author)?);
    insert_field(&mut field_rules, "bookUrl", parse_rule_field(book_url)?);
    insert_field(&mut field_rules, "coverUrl", parse_rule_field(cover_url)?);
    if let Some(k) = kind {
        insert_field(&mut field_rules, "kind", parse_rule_field(Some(k))?);
    }
    Ok((book_list_rules, field_rules))
}

/// 收集 search 端点规则，返回 (`bookList_rules`, `field_rules`)。
pub(crate) fn collect_search_rules(
    rule: Option<&RuleSearch>,
) -> Result<(Vec<ExtractRule>, FieldRules), CoreError> {
    match rule {
        None => Ok((Vec::new(), HashMap::new())),
        Some(r) => collect_list_field_rules(
            r.book_list.as_ref(),
            r.name.as_ref(),
            r.author.as_ref(),
            r.book_url.as_ref(),
            r.cover_url.as_ref(),
            r.kind.as_ref(),
        ),
    }
}

/// 收集 explore 端点规则，返回 (`bookList_rules`, `field_rules`)。
pub(crate) fn collect_explore_rules(
    rule: Option<&RuleExplore>,
) -> Result<(Vec<ExtractRule>, FieldRules), CoreError> {
    match rule {
        None => Ok((Vec::new(), HashMap::new())),
        Some(r) => collect_list_field_rules(
            r.book_list.as_ref(),
            r.name.as_ref(),
            r.author.as_ref(),
            r.book_url.as_ref(),
            r.cover_url.as_ref(),
            None, // explore 无 kind 字段
        ),
    }
}

/// 收集 `bookInfo` 端点规则(单值模式，`field_rules` 为空)。
pub(crate) fn collect_book_info_rules(
    rule: Option<&RuleBookInfo>,
) -> Result<(Vec<ExtractRule>, FieldRules), CoreError> {
    let rules = collect_rules!(rule, name, author, cover_url, intro, kind, word_count)?;
    Ok((rules, HashMap::new()))
}

/// 收集 toc 端点规则(单值模式，`field_rules` 为空)。
pub(crate) fn collect_toc_rules(
    rule: Option<&RuleToc>,
) -> Result<(Vec<ExtractRule>, FieldRules), CoreError> {
    let mut field_rules: FieldRules = FieldRules::new();
    let mut chapter_list: Vec<ExtractRule> = Vec::new();
    if let Some(r) = rule {
        if let Some(f) = &r.chapter_list {
            chapter_list = parse_rule_field(Some(f))?;
        }
        insert_field(
            &mut field_rules,
            "chapterName",
            parse_rule_field(r.chapter_name.as_ref())?,
        );
        insert_field(
            &mut field_rules,
            "chapterUrl",
            parse_rule_field(r.chapter_url.as_ref())?,
        );
    }
    Ok((chapter_list, field_rules))
}

/// 收集 content 端点规则(单值模式，`field_rules` 为空)。
pub(crate) fn collect_content_rules(
    rule: Option<&RuleContent>,
) -> Result<(Vec<ExtractRule>, FieldRules), CoreError> {
    let rules = collect_rules!(rule, content, replace_regex)?;
    Ok((rules, HashMap::new()))
}

/// 非空时插入字段规则映射。
fn insert_field(map: &mut FieldRules, name: &str, rules: Vec<ExtractRule>) {
    if !rules.is_empty() {
        map.insert(name.to_string(), rules);
    }
}
