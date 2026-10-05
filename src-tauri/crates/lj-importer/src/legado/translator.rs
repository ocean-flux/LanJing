//! Legado 作者规则到 `RuleDefinition` 的来源内翻译。
//!
//! 所有 Legado 字段名、选择器和 `@js:` 约定都在本模块结束。输出只包含标准 intent、
//! HTTP/Extract/QuickJS Flow 与受控 Mapper，运行时不需要知道书源格式。

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use lj_capability::{IntentExport, StandardIntent};
use lj_rule_model::definition::MapperOutputKind;
use lj_rule_model::mapper_vocab::{
    ASSET_IDENTITY_FIELDS, BOOK_URL_TEMPLATE_VAR, CHAPTER_URL_TEMPLATE_VAR,
    DISCOVERY_ACTION_IDENTITY_FIELDS, DISCOVERY_SECTION_IDENTITY_FIELDS, ITEM_IDENTITY_FIELDS,
    UNIT_IDENTITY_FIELDS,
};
use lj_rule_model::{
    CapabilityManifest, ControlledMapper, Error, ExpectedDataType, ExtractRule, ExtractSpec,
    FieldRules, FlowEdge, FlowGraph, FlowNode, FlowNodeConfig, FlowPortRef, HttpMethod, HttpSpec,
    JsConfig, JsOutputKind, LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE, OutputTarget,
    RequestHeaderDisposition, RuleDefinition, SensitiveNamePolicy, SourceIdentity,
    SystemCapabilities,
};
use serde::de::{Deserializer, MapAccess, Visitor};
use uuid::Uuid;

use super::parser::parse_legado_rule;
use super::types::{LegadoSourceJson, RuleBookInfo, RuleContent, RuleExplore, RuleSearch, RuleToc};

/// 将来源声明的非敏感请求头与加密前凭证头分开。
pub(crate) struct ParsedHeaders {
    pub(crate) safe: HashMap<String, String>,
    pub(crate) credentials: BTreeMap<String, String>,
}

/// 将已解析的 Legado 来源转换为稳定的 `RuleDefinition`。
///
/// # Errors
///
/// 书源基础 URL 为空或格式不受支持时返回 [`Error::Import`]。
pub(crate) fn definition(
    source: &LegadoSourceJson,
    headers: HashMap<String, String>,
) -> Result<RuleDefinition, Error> {
    let base_url = normalize_base_url(&source.book_source_url)?;
    let source_identity = source_identity(&base_url);
    let mut builder = DefinitionBuilder::new(source_identity.clone(), headers);
    // Legado 一个来源只有一套 ContinueAction: 发现入口的继续动作不能与分页链共存,
    // 否则来源自身的继续动作会被分页链顶掉。冲突由 `strict_json` 报出 warning。
    let next_page = next_page_declaration(source);
    let explore_url = source
        .explore_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(search_url) = source
        .search_url
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let request_url = join_base_url(&base_url, search_url);
        validate_request_url(&request_url)?;
        let (rules, fields) = collect_search_rules(source.rule_search.as_ref());
        builder.add_http_extract_mapper(HttpIntentFlow {
            intent: StandardIntent::Search,
            role: "search",
            url: request_url,
            rules,
            field_rules: fields,
            output_target: OutputTarget::Media,
            mapper_output: MapperOutputKind::Items,
            identity_fields: ITEM_IDENTITY_FIELDS,
        });
    }

    if let Some(explore_url) = explore_url {
        builder.add_discover(extract_js_code(explore_url)?);
        let (rules, fields) = collect_explore_rules(source.rule_explore.as_ref());
        builder.add_http_extract_mapper(HttpIntentFlow {
            intent: StandardIntent::ContinueAction,
            role: "continue-action",
            url: format!("{{{{{BOOK_URL_TEMPLATE_VAR}}}}}"),
            rules,
            field_rules: fields,
            output_target: OutputTarget::Media,
            mapper_output: MapperOutputKind::Discovery,
            identity_fields: DISCOVERY_ACTION_IDENTITY_FIELDS,
        });
    }

    if let Some(rule) = source.rule_book_info.as_ref() {
        let (rules, fields) = collect_book_info_rules(rule);
        builder.add_http_extract_mapper(HttpIntentFlow {
            intent: StandardIntent::ResolveItem,
            role: "resolve-item",
            url: format!("{{{{{BOOK_URL_TEMPLATE_VAR}}}}}"),
            rules,
            field_rules: fields,
            output_target: OutputTarget::Media,
            mapper_output: MapperOutputKind::Items,
            identity_fields: ITEM_IDENTITY_FIELDS,
        });
    }

    if let Some(rule) = source.rule_toc.as_ref() {
        let (rules, fields) = collect_toc_rules(rule);
        builder.add_http_extract_mapper(HttpIntentFlow {
            intent: StandardIntent::ListUnits,
            role: "list-units",
            url: format!("{{{{{BOOK_URL_TEMPLATE_VAR}}}}}"),
            rules,
            field_rules: fields,
            output_target: OutputTarget::Units,
            mapper_output: MapperOutputKind::Units,
            identity_fields: UNIT_IDENTITY_FIELDS,
        });
    }

    if let Some(rule) = source.rule_content.as_ref() {
        let (rules, fields) = collect_content_rules(rule);
        builder.add_http_extract_mapper(HttpIntentFlow {
            intent: StandardIntent::ResolveAsset,
            role: "resolve-asset",
            url: format!("{{{{{CHAPTER_URL_TEMPLATE_VAR}}}}}"),
            rules,
            field_rules: fields,
            output_target: OutputTarget::Asset,
            mapper_output: MapperOutputKind::Assets,
            identity_fields: ASSET_IDENTITY_FIELDS,
        });
    }

    if let Some(declaration) = next_page.filter(|_| explore_url.is_none()) {
        // 下一页规则产出的是一个继续动作: HTTP 请求页游标 URL, Extract 把下一页 URL 取出,
        // Discovery Mapper 把它变成携带 `url` 的 MediaAction, 调用方据此继续读取下一页。
        let field_rules = next_page_field_rules(&declaration.rules);
        builder.add_http_extract_mapper(HttpIntentFlow {
            intent: StandardIntent::ContinueAction,
            role: declaration.role,
            url: format!("{{{{{}}}}}", declaration.cursor_variable),
            rules: declaration.rules,
            field_rules,
            output_target: OutputTarget::Media,
            mapper_output: MapperOutputKind::Discovery,
            identity_fields: DISCOVERY_ACTION_IDENTITY_FIELDS,
        });
    }

    Ok(RuleDefinition::new(
        SourceIdentity {
            id: source_identity,
        },
        base_url,
        builder.intent_exports,
        FlowGraph {
            nodes: builder.nodes,
            edges: builder.edges,
        },
        CapabilityManifest {
            required: SystemCapabilities::default(),
        },
        vec!["bookUrl".to_string(), "chapterUrl".to_string()],
    ))
}

/// 解析书源 header JSON，并把凭证字段与可公开进入 Definition 的字段分开。
///
/// # Errors
///
/// header 不是字符串键值 JSON，或含有空字段名时返回 [`Error::Import`]。
pub(crate) fn parse_headers(header: Option<&str>) -> Result<ParsedHeaders, Error> {
    let Some(header) = header.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(ParsedHeaders {
            safe: HashMap::new(),
            credentials: BTreeMap::new(),
        });
    };
    let parsed = parse_header_entries(header)?;
    let mut safe = HashMap::new();
    let mut credentials = BTreeMap::new();
    let mut seen_names: Vec<String> = Vec::with_capacity(parsed.len());
    for (name, value) in parsed {
        if seen_names
            .iter()
            .any(|seen| SensitiveNamePolicy::equivalent(seen, &name))
        {
            let code = if SensitiveNamePolicy::is_sensitive(&name) {
                "Legado header 包含重复敏感字段"
            } else {
                "Legado header 包含重复字段"
            };
            return Err(Error::Import(code.to_string()));
        }
        seen_names.push(name.clone());
        if name.trim().is_empty() {
            return Err(Error::Import("Legado header 包含空字段名".to_string()));
        }
        match SensitiveNamePolicy::request_header_disposition(&name) {
            RequestHeaderDisposition::Public => {
                safe.insert(name, value);
            }
            RequestHeaderDisposition::Credential => {
                credentials.insert(name, value);
            }
            RequestHeaderDisposition::Blocked => {
                return Err(Error::Import(
                    "Legado header 包含不可安全注入 request 的字段".to_string(),
                ));
            }
        }
    }
    Ok(ParsedHeaders { safe, credentials })
}

fn parse_header_entries(header: &str) -> Result<Vec<(String, String)>, Error> {
    let mut deserializer = serde_json::Deserializer::from_str(header);
    let entries = deserializer
        .deserialize_map(HeaderEntriesVisitor)
        .map_err(|_| Error::Import("Legado header 不是字符串键值 JSON".to_string()))?;
    deserializer
        .end()
        .map_err(|_| Error::Import("Legado header 不是字符串键值 JSON".to_string()))?;
    Ok(entries)
}

struct HeaderEntriesVisitor;

impl<'de> Visitor<'de> for HeaderEntriesVisitor {
    type Value = Vec<(String, String)>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object containing string header values")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut entries = Vec::new();
        while let Some(entry) = map.next_entry::<String, String>()? {
            entries.push(entry);
        }
        Ok(entries)
    }
}

/// 返回来源稳定身份；同一规范化基础 URL 的书源始终得到相同值。
#[must_use]
pub(crate) fn source_identity(base_url: &str) -> String {
    let digest = blake3::hash(format!("legado:{base_url}").as_bytes());
    format!("source:legado:{}", digest.to_hex())
}

/// 来源声明的下一页: 继续动作请求的游标模板变量与整页规则。
struct NextPageDeclaration {
    role: &'static str,
    cursor_variable: &'static str,
    rules: Vec<ExtractRule>,
}

/// 按 `nextTocUrl` → `nextContentUrl` 顺序取第一个非空声明；空字符串/null 代表该列表单页。
fn next_page_declaration(source: &LegadoSourceJson) -> Option<NextPageDeclaration> {
    let toc = source
        .rule_toc
        .as_ref()
        .and_then(|rule| rule.next_toc_url.as_deref());
    let content = source
        .rule_content
        .as_ref()
        .and_then(|rule| rule.next_content_url.as_deref());
    [
        ("next-toc-page", BOOK_URL_TEMPLATE_VAR, toc),
        ("next-content-page", CHAPTER_URL_TEMPLATE_VAR, content),
    ]
    .into_iter()
    .find_map(|(role, cursor_variable, expression)| {
        let rules = parse_legado_rule(expression?.trim());
        (!rules.is_empty()).then_some(NextPageDeclaration {
            role,
            cursor_variable,
            rules,
        })
    })
}

/// 下一页 URL 必须落到列表记录的可继续动作目标字段, Discovery Mapper 才能产出携带 `url` 的动作。
fn next_page_field_rules(rules: &[ExtractRule]) -> FieldRules {
    let mut fields = FieldRules::new();
    insert_field(
        &mut fields,
        "bookUrl",
        rules.iter().map(element_relative_rule).collect(),
    );
    fields
}

/// 整页规则选中元素后，取值要从该元素自身读；非 CSS 规则已在 `strict_json` 字段矩阵被拒。
fn element_relative_rule(rule: &ExtractRule) -> ExtractRule {
    match rule {
        ExtractRule::CssSelector {
            extract_type,
            regex_clean,
            ..
        } => ExtractRule::CssSelector {
            selector: String::new(),
            extract_type: extract_type.clone(),
            regex_clean: regex_clean.clone(),
        },
        other => other.clone(),
    }
}

struct HttpIntentFlow {
    intent: StandardIntent,
    role: &'static str,
    url: String,
    rules: Vec<ExtractRule>,
    field_rules: FieldRules,
    output_target: OutputTarget,
    mapper_output: MapperOutputKind,
    identity_fields: &'static [&'static str],
}

#[derive(Debug)]
struct DefinitionBuilder {
    source_identity: String,
    headers: HashMap<String, String>,
    nodes: Vec<FlowNode>,
    edges: Vec<FlowEdge>,
    intent_exports: BTreeMap<StandardIntent, IntentExport>,
}

impl DefinitionBuilder {
    fn new(source_identity: String, headers: HashMap<String, String>) -> Self {
        Self {
            source_identity,
            headers,
            nodes: Vec::new(),
            edges: Vec::new(),
            intent_exports: BTreeMap::new(),
        }
    }

    fn add_discover(&mut self, code: String) {
        let entry = self.node_id("discover-js");
        let mapper = self.node_id("discover-mapper");
        self.nodes.push(FlowNode::new(
            entry,
            FlowNodeConfig::Js(JsConfig::new(code, JsOutputKind::Json)),
        ));
        self.nodes.push(mapper_node(
            mapper,
            MapperOutputKind::Discovery,
            DISCOVERY_SECTION_IDENTITY_FIELDS,
        ));
        self.edges.push(edge(entry, mapper));
        self.intent_exports
            .insert(StandardIntent::Discover, IntentExport::new(entry, mapper));
    }

    fn add_http_extract_mapper(&mut self, flow: HttpIntentFlow) {
        let http = self.node_id(&format!("{}-http", flow.role));
        let extract = self.node_id(&format!("{}-extract", flow.role));
        let mapper = self.node_id(&format!("{}-mapper", flow.role));
        let http_node = http_node(http, flow.url, &self.headers);
        let extract_node = extract_node(extract, flow.rules, flow.field_rules, flow.output_target);
        let mapper_node = mapper_node(mapper, flow.mapper_output, flow.identity_fields);
        self.nodes.extend([http_node, extract_node, mapper_node]);
        self.edges
            .extend([edge(http, extract), edge(extract, mapper)]);
        self.intent_exports
            .insert(flow.intent, IntentExport::new(http, mapper));
    }

    fn node_id(&self, role: &str) -> Uuid {
        let bytes = *blake3::hash(format!("{}:{role}", self.source_identity).as_bytes()).as_bytes();
        let mut uuid_bytes = [0_u8; 16];
        uuid_bytes.copy_from_slice(&bytes[..16]);
        Uuid::from_bytes(uuid_bytes)
    }
}

fn normalize_base_url(raw: &str) -> Result<String, Error> {
    let base_url = raw.trim().trim_end_matches('/');
    if base_url.is_empty() {
        return Err(Error::Import("Legado 书源 URL 不能为空".to_string()));
    }
    validate_request_url(base_url)?;
    Ok(base_url.to_string())
}

fn join_base_url(base_url: &str, path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("{base_url}{trimmed}")
    }
}

fn validate_request_url(raw: &str) -> Result<(), Error> {
    let raw = raw.trim();
    let authority = raw
        .strip_prefix("http://")
        .or_else(|| raw.strip_prefix("https://"))
        .and_then(|tail| tail.split(['/', '?', '#']).next())
        .filter(|authority| !authority.is_empty())
        .ok_or_else(|| Error::Import("Legado 请求 URL 无效".to_string()))?;
    if authority.contains('@')
        || raw.chars().any(char::is_whitespace)
        || SensitiveNamePolicy::url_contains_sensitive_query_name(raw)
    {
        return Err(Error::Import(
            "Legado 请求 URL 不能携带 credential".to_string(),
        ));
    }
    Ok(())
}

fn extract_js_code(explore_url: &str) -> Result<String, Error> {
    let code = explore_url
        .trim_start()
        .strip_prefix("@js:")
        .ok_or_else(|| Error::Import("Legado 发现入口需要受限 @js: 脚本".to_string()))?
        .trim();
    if code.is_empty() {
        return Err(Error::Import("Legado 探索脚本不能为空".to_string()));
    }
    Ok(code.to_string())
}

fn edge(from: Uuid, to: Uuid) -> FlowEdge {
    FlowEdge::new(
        FlowPortRef::new(from, LINEAR_OUTPUT_HANDLE),
        FlowPortRef::new(to, LINEAR_INPUT_HANDLE),
    )
}

fn http_node(id: Uuid, url: String, headers: &HashMap<String, String>) -> FlowNode {
    FlowNode::new(
        id,
        FlowNodeConfig::Http(HttpSpec {
            method: HttpMethod::Get,
            url,
            headers: headers.clone(),
            body: None,
            charset: None,
            expected_type: ExpectedDataType::Html,
        }),
    )
}

fn extract_node(
    id: Uuid,
    rules: Vec<ExtractRule>,
    field_rules: FieldRules,
    output_target: OutputTarget,
) -> FlowNode {
    FlowNode::new(
        id,
        FlowNodeConfig::Extract(ExtractSpec {
            rules,
            field_rules,
            expected_type: ExpectedDataType::Html,
            output_target,
        }),
    )
}

fn mapper_node(id: Uuid, output: MapperOutputKind, identity_fields: &[&str]) -> FlowNode {
    FlowNode::new(
        id,
        FlowNodeConfig::Mapper(ControlledMapper {
            output,
            identity_fields: identity_fields
                .iter()
                .map(|field| (*field).to_string())
                .collect(),
        }),
    )
}

fn parse_rule_field(field: Option<&String>) -> Vec<ExtractRule> {
    field.map_or_else(Vec::new, |value| parse_legado_rule(value))
}

fn collect_list_field_rules(
    book_list: Option<&String>,
    name: Option<&String>,
    author: Option<&String>,
    book_url: Option<&String>,
    cover_url: Option<&String>,
    kind: Option<&String>,
) -> (Vec<ExtractRule>, FieldRules) {
    let mut fields = FieldRules::new();
    insert_field(&mut fields, "name", parse_rule_field(name));
    insert_field(&mut fields, "author", parse_rule_field(author));
    insert_field(&mut fields, "bookUrl", parse_rule_field(book_url));
    insert_field(&mut fields, "coverUrl", parse_rule_field(cover_url));
    if kind.is_some() {
        insert_field(&mut fields, "kind", parse_rule_field(kind));
    }
    (parse_rule_field(book_list), fields)
}

fn collect_search_rules(rule: Option<&RuleSearch>) -> (Vec<ExtractRule>, FieldRules) {
    rule.map_or_else(
        || (Vec::new(), FieldRules::new()),
        |rule| {
            collect_list_field_rules(
                rule.book_list.as_ref(),
                rule.name.as_ref(),
                rule.author.as_ref(),
                rule.book_url.as_ref(),
                rule.cover_url.as_ref(),
                rule.kind.as_ref(),
            )
        },
    )
}

fn collect_explore_rules(rule: Option<&RuleExplore>) -> (Vec<ExtractRule>, FieldRules) {
    rule.map_or_else(
        || (Vec::new(), FieldRules::new()),
        |rule| {
            collect_list_field_rules(
                rule.book_list.as_ref(),
                rule.name.as_ref(),
                rule.author.as_ref(),
                rule.book_url.as_ref(),
                rule.cover_url.as_ref(),
                None,
            )
        },
    )
}

fn collect_book_info_rules(rule: &RuleBookInfo) -> (Vec<ExtractRule>, FieldRules) {
    (parse_rule_field(rule.name.as_ref()), FieldRules::new())
}

fn collect_toc_rules(rule: &RuleToc) -> (Vec<ExtractRule>, FieldRules) {
    let mut fields = FieldRules::new();
    insert_field(
        &mut fields,
        "chapterName",
        parse_rule_field(rule.chapter_name.as_ref()),
    );
    insert_field(
        &mut fields,
        "chapterUrl",
        parse_rule_field(rule.chapter_url.as_ref()),
    );
    (parse_rule_field(rule.chapter_list.as_ref()), fields)
}

fn collect_content_rules(rule: &RuleContent) -> (Vec<ExtractRule>, FieldRules) {
    (parse_rule_field(rule.content.as_ref()), FieldRules::new())
}

fn insert_field(fields: &mut FieldRules, name: &str, rules: Vec<ExtractRule>) {
    if !rules.is_empty() {
        fields.insert(name.to_string(), rules);
    }
}
