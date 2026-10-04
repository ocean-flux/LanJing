//! Third-party import 的严格 JSON validation 与 RFC 6901 pointer index。
//!
//! parser 直接在原始 UTF-8 bytes 上计数并保留 token span；不会先经 `serde_json::Value` 丢失
//! duplicate key 或把 UTF-8 byte offset 误当 UTF-16 code unit。根深度为 1，node 只计 JSON
//! values，property occurrence 单独计数。

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use lj_rule_model::{DiagnosticSeverity, SensitiveNamePolicy};
use serde::{Deserialize, Serialize};

use crate::{ImportDiagnostic, ImportSupport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ImportLimits {
    utf8_bytes: usize,
    depth: usize,
    nodes: usize,
    properties: usize,
    property_name_utf8_bytes: usize,
    string_utf8_bytes: usize,
}

const IMPORT_LIMITS: ImportLimits = ImportLimits {
    utf8_bytes: 2_097_152,
    depth: 64,
    nodes: 100_000,
    properties: 32_768,
    property_name_utf8_bytes: 1_024,
    string_utf8_bytes: 262_144,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldType {
    String,
    Integer,
    IntegerOrString,
    Boolean,
    ObjectOrString,
}

/// 规则字段的取值语义：决定提取表达式走哪一套白名单。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuleResultType {
    /// 普通规则字段：必须是 current adapter 可执行的提取表达式。
    Rule,
    /// 下一页规则字段：允许空声明，表示来源没有下一页。
    NextPageRule,
}

#[derive(Debug, Clone, Copy)]
struct FieldSpec {
    pointer: &'static str,
    field_type: FieldType,
    required: bool,
    rule_result_type: Option<RuleResultType>,
    support: ImportSupport,
}

macro_rules! field {
    ($pointer:literal, $type:ident, $required:literal, $support:ident) => {
        FieldSpec {
            pointer: $pointer,
            field_type: FieldType::$type,
            required: $required,
            rule_result_type: None,
            support: ImportSupport::$support,
        }
    };
    ($pointer:literal, $type:ident, $required:literal, $support:ident, rule) => {
        FieldSpec {
            pointer: $pointer,
            field_type: FieldType::$type,
            required: $required,
            rule_result_type: Some(RuleResultType::Rule),
            support: ImportSupport::$support,
        }
    };
    ($pointer:literal, $type:ident, $required:literal, $support:ident, next_page_rule) => {
        FieldSpec {
            pointer: $pointer,
            field_type: FieldType::$type,
            required: $required,
            rule_result_type: Some(RuleResultType::NextPageRule),
            support: ImportSupport::$support,
        }
    };
}

// 这是导入支持矩阵，不是可编辑第三方 schema；只记录 current adapter 必须映射、明确忽略或拒绝的字段。
static FIELD_SPECS: &[FieldSpec] = &[
    field!("/bookSourceType", Integer, true, Executable),
    field!("/bookSourceUrl", String, true, Executable),
    field!("/bookSourceName", String, true, Executable),
    field!("/bookSourceGroup", String, false, Executable),
    field!("/bookSourceComment", String, false, Ignored),
    field!("/loginUrl", String, false, Blocked),
    field!("/loginUi", String, false, Blocked),
    field!("/loginCheckJs", String, false, Blocked),
    field!("/coverDecodeJs", String, false, Blocked),
    field!("/bookUrlPattern", String, false, Blocked),
    field!("/header", String, false, Executable),
    field!("/variableComment", String, false, Ignored),
    field!("/concurrentRate", String, false, Blocked),
    field!("/jsLib", String, false, Blocked),
    field!("/customOrder", Integer, false, Ignored),
    field!("/enabled", Boolean, false, Ignored),
    field!("/enabledExplore", Boolean, false, Ignored),
    field!("/enabledCookieJar", Boolean, false, Blocked),
    field!("/lastUpdateTime", IntegerOrString, false, Ignored),
    field!("/respondTime", Integer, false, Ignored),
    field!("/weight", Integer, false, Ignored),
    field!("/searchUrl", String, false, Executable),
    field!("/ruleSearch", ObjectOrString, false, Executable),
    field!("/ruleSearch/checkKeyWord", String, false, Blocked, rule),
    field!("/ruleSearch/bookList", String, false, Executable, rule),
    field!("/ruleSearch/name", String, false, Executable, rule),
    field!("/ruleSearch/author", String, false, Executable, rule),
    field!("/ruleSearch/kind", String, false, Executable, rule),
    field!("/ruleSearch/wordCount", String, false, Blocked, rule),
    field!("/ruleSearch/lastChapter", String, false, Blocked, rule),
    field!("/ruleSearch/updateTime", String, false, Blocked, rule),
    field!("/ruleSearch/intro", String, false, Blocked, rule),
    field!("/ruleSearch/coverUrl", String, false, Executable, rule),
    field!("/ruleSearch/bookUrl", String, false, Executable, rule),
    field!("/exploreUrl", String, false, Executable),
    field!("/exploreScreen", String, false, Blocked),
    field!("/ruleExplore", ObjectOrString, false, Executable),
    field!("/ruleExplore/bookList", String, false, Executable, rule),
    field!("/ruleExplore/name", String, false, Executable, rule),
    field!("/ruleExplore/author", String, false, Executable, rule),
    field!("/ruleExplore/intro", String, false, Blocked, rule),
    field!("/ruleExplore/kind", String, false, Blocked, rule),
    field!("/ruleExplore/lastChapter", String, false, Blocked, rule),
    field!("/ruleExplore/updateTime", String, false, Blocked, rule),
    field!("/ruleExplore/bookUrl", String, false, Executable, rule),
    field!("/ruleExplore/coverUrl", String, false, Executable, rule),
    field!("/ruleExplore/wordCount", String, false, Blocked, rule),
    field!("/ruleBookInfo", ObjectOrString, false, Executable),
    field!("/ruleBookInfo/init", String, false, Blocked, rule),
    field!("/ruleBookInfo/name", String, false, Executable, rule),
    field!("/ruleBookInfo/author", String, false, Blocked, rule),
    field!("/ruleBookInfo/kind", String, false, Blocked, rule),
    field!("/ruleBookInfo/wordCount", String, false, Blocked, rule),
    field!("/ruleBookInfo/lastChapter", String, false, Blocked, rule),
    field!("/ruleBookInfo/updateTime", String, false, Blocked, rule),
    field!("/ruleBookInfo/intro", String, false, Blocked, rule),
    field!("/ruleBookInfo/coverUrl", String, false, Blocked, rule),
    field!("/ruleBookInfo/tocUrl", String, false, Blocked, rule),
    field!("/ruleBookInfo/canReName", String, false, Blocked, rule),
    field!("/ruleBookInfo/downloadUrls", String, false, Blocked, rule),
    field!("/ruleToc", ObjectOrString, false, Executable),
    field!("/ruleToc/preUpdateJs", String, false, Blocked, rule),
    field!("/ruleToc/chapterList", String, false, Executable, rule),
    field!("/ruleToc/chapterName", String, false, Executable, rule),
    field!("/ruleToc/chapterUrl", String, false, Executable, rule),
    field!("/ruleToc/formatJs", String, false, Blocked, rule),
    field!("/ruleToc/isVolume", String, false, Blocked, rule),
    field!("/ruleToc/updateTime", String, false, Blocked, rule),
    field!("/ruleToc/isVip", String, false, Blocked, rule),
    field!("/ruleToc/isPay", String, false, Blocked, rule),
    field!(
        "/ruleToc/nextTocUrl",
        String,
        false,
        Executable,
        next_page_rule
    ),
    field!("/ruleContent", ObjectOrString, false, Executable),
    field!("/ruleContent/content", String, false, Executable, rule),
    field!("/ruleContent/title", String, false, Blocked, rule),
    field!(
        "/ruleContent/nextContentUrl",
        String,
        false,
        Executable,
        next_page_rule
    ),
    field!("/ruleContent/webJs", String, false, Blocked, rule),
    field!("/ruleContent/sourceRegex", String, false, Blocked, rule),
    field!("/ruleContent/replaceRegex", String, false, Blocked, rule),
    field!("/ruleContent/imageStyle", String, false, Ignored),
    field!("/ruleContent/imageDecode", String, false, Blocked, rule),
    field!("/ruleContent/payAction", String, false, Blocked, rule),
    field!("/ruleReview", ObjectOrString, false, Blocked),
];

fn field_for_pointer(pointer: &str) -> Option<&'static FieldSpec> {
    FIELD_SPECS.iter().find(|field| field.pointer == pointer)
}

/// 原始 UTF-8 文档中的半开区间。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Utf8ByteSpan {
    /// 起始 UTF-8 byte offset。
    pub byte_offset: usize,
    /// UTF-8 byte length。
    pub byte_length: usize,
}

impl Utf8ByteSpan {
    const fn new(byte_offset: usize, byte_length: usize) -> Self {
        Self {
            byte_offset,
            byte_length,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ParsedDocument {
    pub(crate) nodes: Vec<JsonNode>,
    pub(crate) root: usize,
}

#[derive(Debug)]
pub(crate) struct JsonNode {
    pub(crate) span: Utf8ByteSpan,
    pub(crate) kind: JsonKind,
}

#[derive(Debug)]
pub(crate) enum JsonKind {
    Object(Vec<ObjectProperty>),
    Array,
    String(String),
    Number(String),
    Boolean(bool),
    Null,
}

#[derive(Debug)]
pub(crate) struct ObjectProperty {
    pub(crate) key: String,
    pub(crate) key_span: Utf8ByteSpan,
    pub(crate) value: usize,
}

pub(crate) struct ParseOutcome {
    pub(crate) document: Option<ParsedDocument>,
    pub(crate) diagnostics: Vec<ImportDiagnostic>,
}

#[must_use]
pub(crate) fn validate_json_document(text: &str) -> Vec<ImportDiagnostic> {
    parse_strict_document(text).diagnostics
}

pub(crate) fn analyze_legado_document(text: &str) -> ParseOutcome {
    let mut outcome = parse_strict_document(text);
    if outcome
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != DiagnosticSeverity::Error)
        && let Some(document) = outcome.document.as_ref()
    {
        classify_legado(document, &mut outcome.diagnostics);
    }
    outcome.diagnostics.sort_by(|left, right| {
        left.byte_offset
            .cmp(&right.byte_offset)
            .then_with(|| severity_rank(left.severity).cmp(&severity_rank(right.severity)))
            .then_with(|| left.code.cmp(&right.code))
            .then_with(|| left.path.cmp(&right.path))
    });
    outcome
}

pub(crate) fn parse_strict_document(text: &str) -> ParseOutcome {
    if text.len() > IMPORT_LIMITS.utf8_bytes {
        return ParseOutcome {
            document: None,
            diagnostics: vec![blocked_diagnostic(
                "document_bytes_exceeded",
                "",
                IMPORT_LIMITS.utf8_bytes,
                text.len() - IMPORT_LIMITS.utf8_bytes,
            )],
        };
    }
    Parser::new(text).parse()
}

fn classify_legado(document: &ParsedDocument, diagnostics: &mut Vec<ImportDiagnostic>) {
    let root = &document.nodes[document.root];
    let JsonKind::Object(properties) = &root.kind else {
        return;
    };
    for required in FIELD_SPECS.iter().filter(|field| field.required) {
        let name = required.pointer.trim_start_matches('/');
        if !name.contains('/') && !properties.iter().any(|property| property.key == name) {
            diagnostics.push(blocked_diagnostic(
                "required_field_missing",
                required.pointer,
                root.span.byte_offset,
                1,
            ));
        }
    }
    let mut path = Vec::new();
    classify_object(document, properties, &mut path, diagnostics);
    classify_shadowed_next_page(document, properties, diagnostics);
}

/// 报告被发现入口顶掉的下一页声明: 两者共用同一个 `ContinueAction` 导出。
fn classify_shadowed_next_page(
    document: &ParsedDocument,
    properties: &[ObjectProperty],
    diagnostics: &mut Vec<ImportDiagnostic>,
) {
    let explore_url = properties
        .iter()
        .find(|property| property.key == "exploreUrl")
        .map(|property| &document.nodes[property.value]);
    if !matches!(explore_url.map(|node| &node.kind), Some(JsonKind::String(value)) if !value.trim().is_empty())
    {
        return;
    }
    for pointer in ["/ruleToc/nextTocUrl", "/ruleContent/nextContentUrl"] {
        let Some(node) = string_node_at(document, properties, pointer) else {
            continue;
        };
        let JsonKind::String(expression) = &node.kind else {
            continue;
        };
        if expression.trim().is_empty() {
            continue;
        }
        diagnostics.push(ImportDiagnostic::new(
            DiagnosticSeverity::Warning,
            "next_page_rule_shadowed",
            pointer,
            node.span.byte_offset,
            node.span.byte_length,
            ImportSupport::Blocked,
        ));
    }
}

/// 沿 RFC 6901 pointer 在根对象下找字符串节点, 用于跨字段诊断的真实 span。
fn string_node_at<'a>(
    document: &'a ParsedDocument,
    properties: &[ObjectProperty],
    pointer: &str,
) -> Option<&'a JsonNode> {
    let mut segments = pointer.split('/').filter(|segment| !segment.is_empty());
    let first = segments.next()?;
    let mut node = document.nodes.get(
        properties
            .iter()
            .find(|property| property.key == first)?
            .value,
    )?;
    for segment in segments {
        let JsonKind::Object(children) = &node.kind else {
            return None;
        };
        node = document.nodes.get(
            children
                .iter()
                .find(|property| property.key == segment)?
                .value,
        )?;
    }
    matches!(&node.kind, JsonKind::String(_)).then_some(node)
}

fn classify_object(
    document: &ParsedDocument,
    properties: &[ObjectProperty],
    path: &mut Vec<String>,
    diagnostics: &mut Vec<ImportDiagnostic>,
) {
    for property in properties {
        path.push(property.key.clone());
        let pointer = encode_pointer(path);
        let value = &document.nodes[property.value];
        let Some(field) = field_for_pointer(&pointer) else {
            diagnostics.push(ImportDiagnostic::new(
                DiagnosticSeverity::Warning,
                "unknown_field",
                &pointer,
                property.key_span.byte_offset,
                property.key_span.byte_length,
                ImportSupport::Unknown,
            ));
            path.pop();
            continue;
        };
        if !field.required && matches!(&value.kind, JsonKind::Null) {
            path.pop();
            continue;
        }
        if !value_matches(field, value) {
            diagnostics.push(ImportDiagnostic::new(
                DiagnosticSeverity::Error,
                "invalid_field_type",
                &pointer,
                value.span.byte_offset,
                value.span.byte_length,
                ImportSupport::Blocked,
            ));
            path.pop();
            continue;
        }
        if field.support == ImportSupport::Blocked {
            if pointer == "/enabledCookieJar" && matches!(&value.kind, JsonKind::Boolean(false)) {
                path.pop();
                continue;
            }
            diagnostics.push(ImportDiagnostic::new(
                DiagnosticSeverity::Error,
                "known_field_blocked",
                &pointer,
                property.key_span.byte_offset,
                property.key_span.byte_length,
                ImportSupport::Blocked,
            ));
            path.pop();
            continue;
        }
        if field.support == ImportSupport::Ignored {
            diagnostics.push(ImportDiagnostic::new(
                DiagnosticSeverity::Info,
                "known_field_ignored",
                &pointer,
                property.key_span.byte_offset,
                property.key_span.byte_length,
                ImportSupport::Ignored,
            ));
        }
        if let JsonKind::String(expression) = &value.kind
            && ((pointer == "/exploreUrl" && !expression.trim_start().starts_with("@js:"))
                || (field.rule_result_type.is_some()
                    && field.support == ImportSupport::Executable
                    && !rule_expression_is_accepted(field, expression)))
        {
            diagnostics.push(ImportDiagnostic::new(
                DiagnosticSeverity::Error,
                "known_field_blocked",
                &pointer,
                value.span.byte_offset,
                value.span.byte_length,
                ImportSupport::Blocked,
            ));
            path.pop();
            continue;
        }
        if pointer == "/bookSourceType"
            && !matches!(&value.kind, JsonKind::Number(number) if number.parse::<i64>() == Ok(0))
        {
            diagnostics.push(ImportDiagnostic::new(
                DiagnosticSeverity::Error,
                "known_field_blocked",
                &pointer,
                value.span.byte_offset,
                value.span.byte_length,
                ImportSupport::Blocked,
            ));
        }
        if is_executable_url_pointer(&pointer)
            && let JsonKind::String(url) = &value.kind
            && SensitiveNamePolicy::url_contains_sensitive_query_name(url)
        {
            diagnostics.push(ImportDiagnostic::new(
                DiagnosticSeverity::Error,
                "credential_query_blocked",
                &pointer,
                value.span.byte_offset,
                value.span.byte_length,
                ImportSupport::Blocked,
            ));
        }
        if matches!(field.field_type, FieldType::ObjectOrString) {
            match &value.kind {
                JsonKind::Object(children) => {
                    classify_object(document, children, path, diagnostics);
                }
                JsonKind::String(_) => {
                    diagnostics.push(ImportDiagnostic::new(
                        DiagnosticSeverity::Error,
                        "known_field_blocked",
                        &pointer,
                        value.span.byte_offset,
                        value.span.byte_length,
                        ImportSupport::Blocked,
                    ));
                }
                _ => {}
            }
        }
        path.pop();
    }
}

fn value_matches(field: &FieldSpec, value: &JsonNode) -> bool {
    match field.field_type {
        FieldType::String => matches!(&value.kind, JsonKind::String(_)),
        FieldType::Integer => {
            matches!(&value.kind, JsonKind::Number(number) if !number.bytes().any(|byte| matches!(byte, b'.' | b'e' | b'E')))
        }
        FieldType::IntegerOrString => {
            matches!(
                &value.kind,
                JsonKind::Number(number)
                    if !number.bytes().any(|byte| matches!(byte, b'.' | b'e' | b'E'))
            ) || matches!(&value.kind, JsonKind::String(string) if is_integer_string(string))
        }
        FieldType::Boolean => matches!(&value.kind, JsonKind::Boolean(_)),
        FieldType::ObjectOrString => {
            matches!(&value.kind, JsonKind::Object(_) | JsonKind::String(_))
        }
    }
}

fn is_integer_string(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
}

/// 下一页规则字段允许空字符串/null 声明, 表示该来源只有一页; 其余仍走通用规则白名单。
fn rule_expression_is_accepted(field: &FieldSpec, value: &str) -> bool {
    if field.rule_result_type == Some(RuleResultType::NextPageRule) && value.trim().is_empty() {
        return true;
    }
    rule_expression_is_supported(value)
}

fn rule_expression_is_supported(value: &str) -> bool {
    value.split("||").all(|candidate| {
        let selector = candidate
            .split_once("##")
            .map_or(candidate, |(selector, _)| selector);
        let selector = selector.trim();
        if selector.is_empty() {
            return false;
        }
        let normalized = selector.to_ascii_lowercase();
        !(normalized.starts_with("@js:")
            || normalized.starts_with("@json:")
            || normalized.starts_with("@regex:")
            || normalized.starts_with("@xpath:")
            || normalized.starts_with('/')
            || normalized.starts_with('$')
            || normalized.contains("<js>")
            || normalized.contains("{{"))
    })
}

fn is_executable_url_pointer(pointer: &str) -> bool {
    matches!(pointer, "/bookSourceUrl" | "/searchUrl" | "/exploreUrl")
}

struct Parser<'a> {
    text: &'a str,
    bytes: &'a [u8],
    cursor: usize,
    node_count: usize,
    property_count: usize,
    nodes: Vec<JsonNode>,
    diagnostics: Vec<ImportDiagnostic>,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            bytes: text.as_bytes(),
            cursor: 0,
            node_count: 0,
            property_count: 0,
            nodes: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn parse(mut self) -> ParseOutcome {
        self.skip_whitespace();
        let mut path = Vec::new();
        let root = match self.parse_value(1, &mut path) {
            Ok(root) => root,
            Err(diagnostic) => {
                self.diagnostics.push(diagnostic);
                return ParseOutcome {
                    document: None,
                    diagnostics: self.diagnostics,
                };
            }
        };
        self.skip_whitespace();
        if self.cursor != self.bytes.len() {
            self.diagnostics.push(self.syntax_error(&path));
            return ParseOutcome {
                document: None,
                diagnostics: self.diagnostics,
            };
        }
        if !matches!(self.nodes[root].kind, JsonKind::Object(_)) {
            let span = self.nodes[root].span;
            self.diagnostics.push(blocked_diagnostic(
                "invalid_root",
                "",
                span.byte_offset,
                span.byte_length,
            ));
        }
        ParseOutcome {
            document: Some(ParsedDocument {
                nodes: self.nodes,
                root,
            }),
            diagnostics: self.diagnostics,
        }
    }

    fn parse_value(
        &mut self,
        depth: usize,
        path: &mut Vec<String>,
    ) -> Result<usize, ImportDiagnostic> {
        self.skip_whitespace();
        if depth > IMPORT_LIMITS.depth {
            return Err(blocked_diagnostic(
                "depth_exceeded",
                &encode_pointer(path),
                self.cursor,
                self.current_char_len(),
            ));
        }
        self.node_count += 1;
        if self.node_count > IMPORT_LIMITS.nodes {
            return Err(blocked_diagnostic(
                "node_count_exceeded",
                &encode_pointer(path),
                self.cursor,
                self.current_char_len(),
            ));
        }
        match self.bytes.get(self.cursor).copied() {
            Some(b'{') => self.parse_object(depth, path),
            Some(b'[') => self.parse_array(depth, path),
            Some(b'"') => {
                let (value, span) = self.parse_string(path)?;
                Ok(self.push_node(span, JsonKind::String(value)))
            }
            Some(b't') => self.parse_literal(b"true", JsonKind::Boolean(true), path),
            Some(b'f') => self.parse_literal(b"false", JsonKind::Boolean(false), path),
            Some(b'n') => self.parse_literal(b"null", JsonKind::Null, path),
            Some(b'-' | b'0'..=b'9') => self.parse_number(path),
            _ => Err(self.syntax_error(path)),
        }
    }

    fn parse_object(
        &mut self,
        depth: usize,
        path: &mut Vec<String>,
    ) -> Result<usize, ImportDiagnostic> {
        let start = self.cursor;
        self.cursor += 1;
        self.skip_whitespace();
        let mut properties = Vec::new();
        let mut seen: HashMap<u64, Vec<usize>> = HashMap::new();
        if self.consume(b'}') {
            return Ok(self.push_node(
                Utf8ByteSpan::new(start, self.cursor - start),
                JsonKind::Object(properties),
            ));
        }
        loop {
            if self.bytes.get(self.cursor) != Some(&b'"') {
                return Err(self.syntax_error(path));
            }
            let (key, key_span) = self.parse_string_token(path)?;
            self.property_count += 1;
            path.push(key.clone());
            let pointer = encode_pointer(path);
            if self.property_count > IMPORT_LIMITS.properties {
                return Err(blocked_diagnostic(
                    "property_count_exceeded",
                    &pointer,
                    key_span.byte_offset,
                    key_span.byte_length,
                ));
            }
            if key.len() > IMPORT_LIMITS.property_name_utf8_bytes {
                self.diagnostics.push(blocked_diagnostic(
                    "property_name_utf8_bytes_exceeded",
                    &pointer,
                    key_span.byte_offset,
                    key_span.byte_length,
                ));
            }
            let hash = key_hash(&key);
            if let Some(indices) = seen.get(&hash)
                && indices.iter().any(|index| properties[*index].key == key)
            {
                self.diagnostics.push(blocked_diagnostic(
                    "duplicate_key",
                    &pointer,
                    key_span.byte_offset,
                    key_span.byte_length,
                ));
            }
            self.skip_whitespace();
            if !self.consume(b':') {
                return Err(self.syntax_error(path));
            }
            let value = self.parse_value(depth + 1, path)?;
            path.pop();
            seen.entry(hash).or_default().push(properties.len());
            properties.push(ObjectProperty {
                key,
                key_span,
                value,
            });
            self.skip_whitespace();
            if self.consume(b'}') {
                break;
            }
            if !self.consume(b',') {
                return Err(self.syntax_error(path));
            }
            self.skip_whitespace();
            if self.bytes.get(self.cursor) == Some(&b'}') {
                return Err(self.syntax_error(path));
            }
        }
        Ok(self.push_node(
            Utf8ByteSpan::new(start, self.cursor - start),
            JsonKind::Object(properties),
        ))
    }

    fn parse_array(
        &mut self,
        depth: usize,
        path: &mut Vec<String>,
    ) -> Result<usize, ImportDiagnostic> {
        let start = self.cursor;
        self.cursor += 1;
        self.skip_whitespace();
        let mut children = Vec::new();
        if self.consume(b']') {
            return Ok(self.push_node(
                Utf8ByteSpan::new(start, self.cursor - start),
                JsonKind::Array,
            ));
        }
        loop {
            path.push(children.len().to_string());
            let child = self.parse_value(depth + 1, path)?;
            path.pop();
            children.push(child);
            self.skip_whitespace();
            if self.consume(b']') {
                break;
            }
            if !self.consume(b',') {
                return Err(self.syntax_error(path));
            }
            self.skip_whitespace();
            if self.bytes.get(self.cursor) == Some(&b']') {
                return Err(self.syntax_error(path));
            }
        }
        Ok(self.push_node(
            Utf8ByteSpan::new(start, self.cursor - start),
            JsonKind::Array,
        ))
    }

    fn parse_string(
        &mut self,
        path: &[String],
    ) -> Result<(String, Utf8ByteSpan), ImportDiagnostic> {
        let (value, span) = self.parse_string_token(path)?;
        if value.len() > IMPORT_LIMITS.string_utf8_bytes {
            self.diagnostics.push(blocked_diagnostic(
                "string_utf8_bytes_exceeded",
                &encode_pointer(path),
                span.byte_offset,
                span.byte_length,
            ));
        }
        Ok((value, span))
    }

    fn parse_string_token(
        &mut self,
        path: &[String],
    ) -> Result<(String, Utf8ByteSpan), ImportDiagnostic> {
        let start = self.cursor;
        if !self.consume(b'"') {
            return Err(self.syntax_error(path));
        }
        let mut value = String::new();
        while self.cursor < self.bytes.len() {
            match self.bytes[self.cursor] {
                b'"' => {
                    self.cursor += 1;
                    return Ok((value, Utf8ByteSpan::new(start, self.cursor - start)));
                }
                b'\\' => {
                    self.cursor += 1;
                    let Some(escape) = self.bytes.get(self.cursor).copied() else {
                        return Err(self.syntax_error(path));
                    };
                    self.cursor += 1;
                    match escape {
                        b'"' => value.push('"'),
                        b'\\' => value.push('\\'),
                        b'/' => value.push('/'),
                        b'b' => value.push('\u{0008}'),
                        b'f' => value.push('\u{000c}'),
                        b'n' => value.push('\n'),
                        b'r' => value.push('\r'),
                        b't' => value.push('\t'),
                        b'u' => value.push(self.parse_unicode_escape(path)?),
                        _ => return Err(self.syntax_error(path)),
                    }
                }
                0x00..=0x1f => return Err(self.syntax_error(path)),
                byte if byte.is_ascii() => {
                    value.push(char::from(byte));
                    self.cursor += 1;
                }
                _ => {
                    let character = self.text[self.cursor..]
                        .chars()
                        .next()
                        .ok_or_else(|| self.syntax_error(path))?;
                    value.push(character);
                    self.cursor += character.len_utf8();
                }
            }
        }
        Err(self.syntax_error(path))
    }

    fn parse_unicode_escape(&mut self, path: &[String]) -> Result<char, ImportDiagnostic> {
        let first = self.parse_hex_quad(path)?;
        let scalar = if (0xd800..=0xdbff).contains(&first) {
            if self.bytes.get(self.cursor..self.cursor + 2) != Some(b"\\u") {
                return Err(self.syntax_error(path));
            }
            self.cursor += 2;
            let second = self.parse_hex_quad(path)?;
            if !(0xdc00..=0xdfff).contains(&second) {
                return Err(self.syntax_error(path));
            }
            0x1_0000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00)
        } else if (0xdc00..=0xdfff).contains(&first) {
            return Err(self.syntax_error(path));
        } else {
            u32::from(first)
        };
        char::from_u32(scalar).ok_or_else(|| self.syntax_error(path))
    }

    fn parse_hex_quad(&mut self, path: &[String]) -> Result<u16, ImportDiagnostic> {
        if self.cursor + 4 > self.bytes.len() {
            return Err(self.syntax_error(path));
        }
        let mut value = 0_u16;
        for _ in 0..4 {
            let digit =
                hex_value(self.bytes[self.cursor]).ok_or_else(|| self.syntax_error(path))?;
            value = value * 16 + u16::from(digit);
            self.cursor += 1;
        }
        Ok(value)
    }

    fn parse_number(&mut self, path: &[String]) -> Result<usize, ImportDiagnostic> {
        let start = self.cursor;
        self.consume(b'-');
        match self.bytes.get(self.cursor).copied() {
            Some(b'0') => {
                self.cursor += 1;
                if matches!(self.bytes.get(self.cursor), Some(b'0'..=b'9')) {
                    return Err(self.syntax_error(path));
                }
            }
            Some(b'1'..=b'9') => {
                self.cursor += 1;
                while matches!(self.bytes.get(self.cursor), Some(b'0'..=b'9')) {
                    self.cursor += 1;
                }
            }
            _ => return Err(self.syntax_error(path)),
        }
        if self.consume(b'.') {
            if !matches!(self.bytes.get(self.cursor), Some(b'0'..=b'9')) {
                return Err(self.syntax_error(path));
            }
            while matches!(self.bytes.get(self.cursor), Some(b'0'..=b'9')) {
                self.cursor += 1;
            }
        }
        if matches!(self.bytes.get(self.cursor), Some(b'e' | b'E')) {
            self.cursor += 1;
            if matches!(self.bytes.get(self.cursor), Some(b'+' | b'-')) {
                self.cursor += 1;
            }
            if !matches!(self.bytes.get(self.cursor), Some(b'0'..=b'9')) {
                return Err(self.syntax_error(path));
            }
            while matches!(self.bytes.get(self.cursor), Some(b'0'..=b'9')) {
                self.cursor += 1;
            }
        }
        let number = self.text[start..self.cursor].to_string();
        Ok(self.push_node(
            Utf8ByteSpan::new(start, self.cursor - start),
            JsonKind::Number(number),
        ))
    }

    fn parse_literal(
        &mut self,
        literal: &[u8],
        kind: JsonKind,
        path: &[String],
    ) -> Result<usize, ImportDiagnostic> {
        let start = self.cursor;
        if self.bytes.get(start..start + literal.len()) != Some(literal) {
            return Err(self.syntax_error(path));
        }
        self.cursor += literal.len();
        Ok(self.push_node(Utf8ByteSpan::new(start, literal.len()), kind))
    }

    fn push_node(&mut self, span: Utf8ByteSpan, kind: JsonKind) -> usize {
        let index = self.nodes.len();
        self.nodes.push(JsonNode { span, kind });
        index
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.bytes.get(self.cursor) == Some(&expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(
            self.bytes.get(self.cursor),
            Some(b' ' | b'\n' | b'\r' | b'\t')
        ) {
            self.cursor += 1;
        }
    }

    fn syntax_error(&self, path: &[String]) -> ImportDiagnostic {
        blocked_diagnostic(
            "invalid_json",
            &encode_pointer(path),
            self.cursor,
            self.current_char_len(),
        )
    }

    fn current_char_len(&self) -> usize {
        if self.cursor >= self.bytes.len() {
            return 0;
        }
        self.text[self.cursor..]
            .chars()
            .next()
            .map_or(0, char::len_utf8)
    }
}

fn blocked_diagnostic(
    code: &str,
    path: &str,
    byte_offset: usize,
    byte_length: usize,
) -> ImportDiagnostic {
    ImportDiagnostic::new(
        DiagnosticSeverity::Error,
        code,
        path,
        byte_offset,
        byte_length,
        ImportSupport::Blocked,
    )
}

fn encode_pointer(path: &[String]) -> String {
    if path.is_empty() {
        return String::new();
    }
    let mut pointer = String::new();
    for segment in path {
        pointer.push('/');
        for character in segment.chars() {
            match character {
                '~' => pointer.push_str("~0"),
                '/' => pointer.push_str("~1"),
                character => pointer.push(character),
            }
        }
    }
    pointer
}

fn key_hash(key: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

const fn severity_rank(severity: DiagnosticSeverity) -> u8 {
    match severity {
        DiagnosticSeverity::Error => 0,
        DiagnosticSeverity::Warning => 1,
        DiagnosticSeverity::Info => 2,
    }
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
