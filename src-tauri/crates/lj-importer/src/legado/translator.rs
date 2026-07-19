//! Legado 作者规则到 `RuleDefinition` 的来源内翻译。
//!
//! 所有 Legado 字段名、选择器和 `@js:` 约定都在本模块结束。输出只包含标准 intent、
//! HTTP/Extract/QuickJS Flow 与受控 Mapper，运行时不需要知道书源格式。

use std::collections::{BTreeMap, HashMap};

use lj_capability::{IntentExport, StandardIntent};
use lj_rule_model::definition::MapperOutputKind;
use lj_rule_model::mapper_vocab::{
    ASSET_IDENTITY_FIELDS, BOOK_URL_TEMPLATE_VAR, CHAPTER_URL_TEMPLATE_VAR,
    DISCOVERY_ACTION_IDENTITY_FIELDS, DISCOVERY_SECTION_IDENTITY_FIELDS, ITEM_IDENTITY_FIELDS,
    UNIT_IDENTITY_FIELDS,
};
use lj_rule_model::{
    CapabilityManifest, ControlledMapper, Error, ExpectedDataType, ExtractRule, ExtractSpec,
    FieldRules, FlowEdge, FlowGraph, FlowNode, FlowNodeKind, HttpMethod, HttpSpec, OutputTarget,
    PolicyCapabilities, RuleDefinition, SourceIdentity, SystemCapabilities,
};
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

    if let Some(search_url) = source
        .search_url
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let (rules, fields) = collect_search_rules(source.rule_search.as_ref());
        builder.add_http_extract_mapper(HttpIntentFlow {
            intent: StandardIntent::Search,
            role: "search",
            url: join_base_url(&base_url, search_url),
            rules,
            field_rules: fields,
            output_target: OutputTarget::Media,
            mapper_output: MapperOutputKind::Items,
            identity_fields: ITEM_IDENTITY_FIELDS,
        });
    }

    if let Some(explore_url) = source
        .explore_url
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
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

    Ok(RuleDefinition {
        schema_version: 1,
        source_identity: SourceIdentity {
            id: source_identity,
        },
        base_url,
        intent_exports: builder.intent_exports,
        flow: FlowGraph {
            nodes: builder.nodes,
            edges: builder.edges,
        },
        capability_manifest: CapabilityManifest {
            required: PolicyCapabilities {
                network: true,
                system: SystemCapabilities::default(),
            },
        },
        source_id_rules: vec!["bookUrl".to_string(), "chapterUrl".to_string()],
    })
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
    let parsed = serde_json::from_str::<HashMap<String, String>>(header)
        .map_err(|_| Error::Import("Legado header 不是字符串键值 JSON".to_string()))?;
    let mut safe = HashMap::new();
    let mut credentials = BTreeMap::new();
    for (name, value) in parsed {
        if name.trim().is_empty() {
            return Err(Error::Import("Legado header 包含空字段名".to_string()));
        }
        if is_sensitive_header(&name) {
            credentials.insert(name, value);
        } else {
            safe.insert(name, value);
        }
    }
    Ok(ParsedHeaders { safe, credentials })
}

/// 返回来源稳定身份；同一规范化基础 URL 的书源始终得到相同值。
#[must_use]
pub(crate) fn source_identity(base_url: &str) -> String {
    let digest = blake3::hash(format!("legado:{base_url}").as_bytes());
    format!("source:legado:{}", digest.to_hex())
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
        self.nodes.push(FlowNode {
            id: entry,
            kind: FlowNodeKind::Js,
            http: None,
            js_code: Some(code),
            extract: None,
            mapper: None,
            span: None,
        });
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
    if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
        return Err(Error::Import(
            "Legado 书源 URL 必须使用 HTTP 或 HTTPS".to_string(),
        ));
    }
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

fn extract_js_code(explore_url: &str) -> Result<String, Error> {
    let code = explore_url
        .strip_prefix("@js:")
        .unwrap_or(explore_url)
        .trim();
    if code.is_empty() {
        return Err(Error::Import("Legado 探索脚本不能为空".to_string()));
    }
    Ok(code.to_string())
}

fn is_sensitive_header(header: &str) -> bool {
    let normalized = header.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "authorization" | "cookie" | "set-cookie"
    ) || normalized.contains("token")
}

fn edge(from: Uuid, to: Uuid) -> FlowEdge {
    FlowEdge {
        from,
        to,
        condition_branch: None,
    }
}

fn http_node(id: Uuid, url: String, headers: &HashMap<String, String>) -> FlowNode {
    FlowNode {
        id,
        kind: FlowNodeKind::Http,
        http: Some(HttpSpec {
            method: HttpMethod::Get,
            url,
            headers: headers.clone(),
            body: None,
            charset: None,
            expected_type: ExpectedDataType::Html,
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
    field_rules: FieldRules,
    output_target: OutputTarget,
) -> FlowNode {
    FlowNode {
        id,
        kind: FlowNodeKind::Extract,
        http: None,
        js_code: None,
        extract: Some(ExtractSpec {
            rules,
            field_rules,
            expected_type: ExpectedDataType::Html,
            output_target,
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
    let mut rules = Vec::new();
    for field in [
        rule.name.as_ref(),
        rule.author.as_ref(),
        rule.cover_url.as_ref(),
        rule.intro.as_ref(),
        rule.kind.as_ref(),
        rule.word_count.as_ref(),
    ] {
        rules.extend(parse_rule_field(field));
    }
    (rules, FieldRules::new())
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
    let mut rules = parse_rule_field(rule.content.as_ref());
    rules.extend(parse_rule_field(rule.replace_regex.as_ref()));
    (rules, FieldRules::new())
}

fn insert_field(fields: &mut FieldRules, name: &str, rules: Vec<ExtractRule>) {
    if !rules.is_empty() {
        fields.insert(name.to_string(), rules);
    }
}
