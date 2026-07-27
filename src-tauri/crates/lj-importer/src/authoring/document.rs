//! 严格 JSON authoring validation 与 RFC 6901 pointer index。
//!
//! parser 直接在原始 UTF-8 bytes 上计数并保留 token span；不会先经 `serde_json::Value` 丢失
//! duplicate key 或把 UTF-8 byte offset 误当 UTF-16 code unit。根深度为 1，node 只计 JSON
//! values，property occurrence 单独计数。

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use lj_rule_model::{
    AuthoringDiagnostic, CREDENTIAL_SENTINEL_PREFIX, DiagnosticSeverity, SensitiveNamePolicy,
    SupportClass, parse_credential_sentinel,
};
use serde::{Deserialize, Serialize};

use super::catalog::{
    AUTHORING_LIMITS, CatalogFieldType, FIELD_SPECS, FieldSpec, field_for_pointer,
};

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
    Array(Vec<usize>),
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
    pub(crate) diagnostics: Vec<AuthoringDiagnostic>,
}

/// 严格验证 Legado authoring 文档，并返回稳定、无 secret 文案的 diagnostics。
#[must_use]
pub fn validate_legado_document(text: &str) -> Vec<AuthoringDiagnostic> {
    analyze_legado_document(text).diagnostics
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

/// 在严格单对象 JSON 中唯一定位 RFC 6901 pointer 的 value token span。
///
/// duplicate target 返回 `pointer_ambiguous`；缺失 target 返回 `pointer_missing`。文档其他位置
/// 的 duplicate 或 limits/syntax/root 失败同样阻止定位。
///
/// # Errors
///
/// JSON 非法、非对象、超限、包含 duplicate key，或 pointer 非法/缺失/歧义时返回稳定
/// [`AuthoringDiagnostic`]。
pub fn locate_json_pointer(text: &str, pointer: &str) -> Result<Utf8ByteSpan, AuthoringDiagnostic> {
    let ParseOutcome {
        document,
        diagnostics,
    } = parse_strict_document(text);
    let Some(document) = document else {
        return Err(diagnostics
            .into_iter()
            .next()
            .unwrap_or_else(|| blocked_diagnostic("invalid_json", "", 0, text.len().min(1))));
    };
    let node = resolve_pointer(&document, pointer)?;
    if let Some(diagnostic) = diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    {
        return Err(diagnostic);
    }
    Ok(document.nodes[node].span)
}

pub(crate) fn parse_strict_document(text: &str) -> ParseOutcome {
    if text.len() > AUTHORING_LIMITS.max_utf8_bytes {
        return ParseOutcome {
            document: None,
            diagnostics: vec![blocked_diagnostic(
                "document_bytes_exceeded",
                "",
                AUTHORING_LIMITS.max_utf8_bytes,
                text.len() - AUTHORING_LIMITS.max_utf8_bytes,
            )],
        };
    }
    Parser::new(text).parse()
}

pub(crate) fn resolve_pointer(
    document: &ParsedDocument,
    pointer: &str,
) -> Result<usize, AuthoringDiagnostic> {
    let segments = decode_pointer(pointer).map_err(|()| {
        blocked_diagnostic(
            "pointer_missing",
            pointer,
            document.nodes[document.root].span.byte_offset,
            1,
        )
    })?;
    let mut node_index = document.root;
    for segment in segments {
        let node = &document.nodes[node_index];
        match &node.kind {
            JsonKind::Object(properties) => {
                let matching = properties
                    .iter()
                    .filter(|property| property.key == segment)
                    .collect::<Vec<_>>();
                match matching.as_slice() {
                    [] => {
                        return Err(blocked_diagnostic(
                            "pointer_missing",
                            pointer,
                            node.span.byte_offset,
                            1,
                        ));
                    }
                    [property] => node_index = property.value,
                    [_, second, ..] => {
                        return Err(blocked_diagnostic(
                            "pointer_ambiguous",
                            pointer,
                            second.key_span.byte_offset,
                            second.key_span.byte_length,
                        ));
                    }
                }
            }
            JsonKind::Array(children) => {
                let Some(index) = canonical_array_index(&segment) else {
                    return Err(blocked_diagnostic(
                        "pointer_missing",
                        pointer,
                        node.span.byte_offset,
                        1,
                    ));
                };
                node_index = *children.get(index).ok_or_else(|| {
                    blocked_diagnostic("pointer_missing", pointer, node.span.byte_offset, 1)
                })?;
            }
            _ => {
                return Err(blocked_diagnostic(
                    "pointer_missing",
                    pointer,
                    node.span.byte_offset,
                    node.span.byte_length,
                ));
            }
        }
    }
    Ok(node_index)
}

fn classify_legado(document: &ParsedDocument, diagnostics: &mut Vec<AuthoringDiagnostic>) {
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
}

fn classify_object(
    document: &ParsedDocument,
    properties: &[ObjectProperty],
    path: &mut Vec<String>,
    diagnostics: &mut Vec<AuthoringDiagnostic>,
) {
    for property in properties {
        path.push(property.key.clone());
        let pointer = encode_pointer(path);
        let value = &document.nodes[property.value];
        if let JsonKind::String(string) = &value.kind
            && string.starts_with(CREDENTIAL_SENTINEL_PREFIX)
            && parse_credential_sentinel(string).is_err()
        {
            diagnostics.push(AuthoringDiagnostic::new(
                DiagnosticSeverity::Error,
                "credential_sentinel_invalid",
                &pointer,
                value.span.byte_offset,
                value.span.byte_length,
                SupportClass::Blocked,
            ));
            path.pop();
            continue;
        }
        let Some(field) = field_for_pointer(&pointer) else {
            diagnostics.push(AuthoringDiagnostic::new(
                DiagnosticSeverity::Warning,
                "unknown_field",
                &pointer,
                property.key_span.byte_offset,
                property.key_span.byte_length,
                SupportClass::Unknown,
            ));
            path.pop();
            continue;
        };
        if !field.required && matches!(&value.kind, JsonKind::Null) {
            path.pop();
            continue;
        }
        if !value_matches(field, value) {
            diagnostics.push(AuthoringDiagnostic::new(
                DiagnosticSeverity::Error,
                "invalid_field_type",
                &pointer,
                value.span.byte_offset,
                value.span.byte_length,
                SupportClass::Blocked,
            ));
            path.pop();
            continue;
        }
        if field.support == SupportClass::Blocked {
            if pointer == "/enabledCookieJar" && matches!(&value.kind, JsonKind::Boolean(false)) {
                path.pop();
                continue;
            }
            diagnostics.push(AuthoringDiagnostic::new(
                DiagnosticSeverity::Error,
                "known_field_blocked",
                &pointer,
                property.key_span.byte_offset,
                property.key_span.byte_length,
                SupportClass::Blocked,
            ));
            path.pop();
            continue;
        }
        if field.support == SupportClass::Preserved {
            diagnostics.push(AuthoringDiagnostic::new(
                DiagnosticSeverity::Info,
                "known_field_preserved",
                &pointer,
                property.key_span.byte_offset,
                property.key_span.byte_length,
                SupportClass::Preserved,
            ));
        }
        if let JsonKind::String(expression) = &value.kind
            && ((pointer == "/exploreUrl" && !expression.trim_start().starts_with("@js:"))
                || (field.rule_result_type.is_some()
                    && field.support == SupportClass::Executable
                    && !rule_expression_is_supported(expression)))
        {
            diagnostics.push(AuthoringDiagnostic::new(
                DiagnosticSeverity::Error,
                "known_field_blocked",
                &pointer,
                value.span.byte_offset,
                value.span.byte_length,
                SupportClass::Blocked,
            ));
            path.pop();
            continue;
        }
        if pointer == "/bookSourceType"
            && !matches!(&value.kind, JsonKind::Number(number) if number.parse::<i64>() == Ok(0))
        {
            diagnostics.push(AuthoringDiagnostic::new(
                DiagnosticSeverity::Error,
                "known_field_blocked",
                &pointer,
                value.span.byte_offset,
                value.span.byte_length,
                SupportClass::Blocked,
            ));
        }
        if is_executable_url_pointer(&pointer)
            && let JsonKind::String(url) = &value.kind
            && SensitiveNamePolicy::url_contains_sensitive_query_name(url)
        {
            diagnostics.push(AuthoringDiagnostic::new(
                DiagnosticSeverity::Error,
                "credential_query_blocked",
                &pointer,
                value.span.byte_offset,
                value.span.byte_length,
                SupportClass::Blocked,
            ));
        }
        if matches!(field.field_type, CatalogFieldType::ObjectOrString) {
            match &value.kind {
                JsonKind::Object(children) => {
                    classify_object(document, children, path, diagnostics);
                }
                JsonKind::String(_) => {
                    diagnostics.push(AuthoringDiagnostic::new(
                        DiagnosticSeverity::Error,
                        "known_field_blocked",
                        &pointer,
                        value.span.byte_offset,
                        value.span.byte_length,
                        SupportClass::Blocked,
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
        CatalogFieldType::String => matches!(&value.kind, JsonKind::String(_)),
        CatalogFieldType::Integer => {
            matches!(&value.kind, JsonKind::Number(number) if !number.bytes().any(|byte| matches!(byte, b'.' | b'e' | b'E')))
        }
        CatalogFieldType::IntegerOrString => {
            matches!(
                &value.kind,
                JsonKind::Number(number)
                    if !number.bytes().any(|byte| matches!(byte, b'.' | b'e' | b'E'))
            ) || matches!(&value.kind, JsonKind::String(string) if is_integer_string(string))
        }
        CatalogFieldType::Boolean => matches!(&value.kind, JsonKind::Boolean(_)),
        CatalogFieldType::ObjectOrString => {
            matches!(&value.kind, JsonKind::Object(_) | JsonKind::String(_))
        }
    }
}

fn is_integer_string(value: &str) -> bool {
    let digits = value.strip_prefix('-').unwrap_or(value);
    !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
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
    diagnostics: Vec<AuthoringDiagnostic>,
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
    ) -> Result<usize, AuthoringDiagnostic> {
        self.skip_whitespace();
        if depth > AUTHORING_LIMITS.max_depth {
            return Err(blocked_diagnostic(
                "depth_exceeded",
                &encode_pointer(path),
                self.cursor,
                self.current_char_len(),
            ));
        }
        self.node_count += 1;
        if self.node_count > AUTHORING_LIMITS.max_nodes {
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
    ) -> Result<usize, AuthoringDiagnostic> {
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
            if self.property_count > AUTHORING_LIMITS.max_properties {
                return Err(blocked_diagnostic(
                    "property_count_exceeded",
                    &pointer,
                    key_span.byte_offset,
                    key_span.byte_length,
                ));
            }
            if key.len() > AUTHORING_LIMITS.max_property_name_utf8_bytes {
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
    ) -> Result<usize, AuthoringDiagnostic> {
        let start = self.cursor;
        self.cursor += 1;
        self.skip_whitespace();
        let mut children = Vec::new();
        if self.consume(b']') {
            return Ok(self.push_node(
                Utf8ByteSpan::new(start, self.cursor - start),
                JsonKind::Array(children),
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
            JsonKind::Array(children),
        ))
    }

    fn parse_string(
        &mut self,
        path: &[String],
    ) -> Result<(String, Utf8ByteSpan), AuthoringDiagnostic> {
        let (value, span) = self.parse_string_token(path)?;
        if value.len() > AUTHORING_LIMITS.max_string_utf8_bytes {
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
    ) -> Result<(String, Utf8ByteSpan), AuthoringDiagnostic> {
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

    fn parse_unicode_escape(&mut self, path: &[String]) -> Result<char, AuthoringDiagnostic> {
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

    fn parse_hex_quad(&mut self, path: &[String]) -> Result<u16, AuthoringDiagnostic> {
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

    fn parse_number(&mut self, path: &[String]) -> Result<usize, AuthoringDiagnostic> {
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
    ) -> Result<usize, AuthoringDiagnostic> {
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

    fn syntax_error(&self, path: &[String]) -> AuthoringDiagnostic {
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
) -> AuthoringDiagnostic {
    AuthoringDiagnostic::new(
        DiagnosticSeverity::Error,
        code,
        path,
        byte_offset,
        byte_length,
        SupportClass::Blocked,
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

fn decode_pointer(pointer: &str) -> Result<Vec<String>, ()> {
    if pointer.is_empty() {
        return Ok(Vec::new());
    }
    let Some(rest) = pointer.strip_prefix('/') else {
        return Err(());
    };
    rest.split('/').map(decode_pointer_segment).collect()
}

fn decode_pointer_segment(segment: &str) -> Result<String, ()> {
    let mut decoded = String::with_capacity(segment.len());
    let mut characters = segment.chars();
    while let Some(character) = characters.next() {
        if character != '~' {
            decoded.push(character);
            continue;
        }
        match characters.next() {
            Some('0') => decoded.push('~'),
            Some('1') => decoded.push('/'),
            _ => return Err(()),
        }
    }
    Ok(decoded)
}

fn canonical_array_index(segment: &str) -> Option<usize> {
    if segment.is_empty() || (segment.len() > 1 && segment.starts_with('0')) {
        return None;
    }
    if !segment.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    segment.parse().ok()
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
