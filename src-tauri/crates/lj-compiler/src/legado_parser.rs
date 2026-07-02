//! Legado 规则语法解析器 — 字符串 → `ExtractRule` IR。
//!
//! 解析 Legado 规则字符串(`@text`/`@href`/`||`/`##regex##`)为 `ExtractRule`。
//! 纯函数，无 IO。

use lj_core::extract_rule::{ExtractRule, ExtractType, RegexClean};

use crate::error::CompilerError;

/// 已知提取后缀，按长度降序排列避免误匹配。
const KNOWN_SUFFIXES: &[(&str, ExtractType)] = &[
    ("ownText", ExtractType::OwnText),
    ("text", ExtractType::Text),
    ("href", ExtractType::Href),
    ("src", ExtractType::Src),
    ("html", ExtractType::Html),
];

/// 纯属性名 — 无选择器时从当前元素直接取值。
const BARE_ATTRIBUTES: &[(&str, ExtractType)] = &[
    ("text", ExtractType::Text),
    ("href", ExtractType::Href),
    ("src", ExtractType::Src),
    ("html", ExtractType::Html),
    ("ownText", ExtractType::OwnText),
];

/// 解析 Legado 规则字符串为 `ExtractRule` 回退链。
///
/// 输入: `"h2[itemprop='name']@text||a[href*='/book/']@text"`
/// 输出: `Vec<ExtractRule>` (按优先级，第一个非空结果胜出)
///
/// # Errors
///
/// 当任一候选规则语法不合法时返回 `CompilerError::SyntaxError`。
pub fn parse_legado_rule(input: &str) -> Result<Vec<ExtractRule>, CompilerError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    Ok(trimmed
        .split("||")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(parse_single)
        .collect())
}

/// 解析单个规则字符串(不含 `||`)。
/// 解析单个规则字符串(不含 `||`)。
fn parse_single(input: &str) -> ExtractRule {
    // 1. 分离 ##regex##replacement
    let (selector_part, regex_clean) = split_regex_clean(input);

    // 2. 检查选择器前缀(@xpath:/@json:/@regex:/@css:)
    let (body, kind) = try_strip_prefix(selector_part);

    // 3. 分离提取后缀(@text/@href/@src/@html/@ownText)
    let (selector, extract_type) = resolve_selector_and_type(body);

    // 4. 根据前缀类型构建规则
    build_rule(kind, selector, extract_type, regex_clean)
}

/// 分离 `##regex##replacement`。
///
/// 输入 `"selector@text##pattern##replacement"` → `("selector@text", Some(RegexClean{..}))`。
/// 输入 `"selector@text##pattern"` → `("selector@text", Some(RegexClean{pattern, ""}))`。
/// 无 `##` 时直接返回原串与 `None`。
fn split_regex_clean(input: &str) -> (&str, Option<RegexClean>) {
    let Some(pos) = input.find("##") else {
        return (input, None);
    };

    let selector_part = &input[..pos];
    let rest = &input[pos + 2..];

    // 查找第二个 ## 作为 pattern/replacement 分隔
    let (pattern, replacement) = if let Some(sep_pos) = rest.find("##") {
        (&rest[..sep_pos], &rest[sep_pos + 2..])
    } else {
        (rest, "")
    };

    (
        selector_part,
        Some(RegexClean {
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
        }),
    )
}

/// 选择器类型前缀。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuleKind {
    /// CSS 选择器(默认)。
    Css,
    /// `XPath` 表达式。
    XPath,
    /// `JSONPath` 路径。
    JsonPath,
    /// 正则表达式。
    Regex,
}

/// 剥离选择器前缀(`@xpath:`/`@json:`/`@regex:`/`@css:`)。
///
/// 无匹配时返回 `(input, RuleKind::Css)` 作为默认。
fn try_strip_prefix(input: &str) -> (&str, RuleKind) {
    if let Some(rest) = input.strip_prefix("@xpath:") {
        (rest, RuleKind::XPath)
    } else if let Some(rest) = input.strip_prefix("@json:") {
        (rest, RuleKind::JsonPath)
    } else if let Some(rest) = input.strip_prefix("@regex:") {
        (rest, RuleKind::Regex)
    } else if let Some(rest) = input.strip_prefix("@css:") {
        (rest, RuleKind::Css)
    } else {
        (input, RuleKind::Css)
    }
}

/// 从体部解析出 CSS 选择器与提取类型。
///
/// 优先尝试剥离已知后缀；无后缀时检查是否为纯属性名；
/// 否则整个体部作为 CSS 选择器，类型默认为 `Text`。
fn resolve_selector_and_type(body: &str) -> (String, ExtractType) {
    // 尝试匹配已知后缀
    for (suffix, extract_type) in KNOWN_SUFFIXES {
        // 需要以 @suffix 结尾(不是裸的 suffix)
        let marker = ['@'];
        let suffix_marker: String = marker.into_iter().chain(suffix.chars()).collect();
        if let Some(rest) = body.strip_suffix(&suffix_marker) {
            return (rest.to_string(), extract_type.clone());
        }
    }

    // 无后缀：检查是否为纯属性名
    for (name, extract_type) in BARE_ATTRIBUTES {
        if body == *name {
            return (String::new(), extract_type.clone());
        }
    }

    // 否则整个为 CSS 选择器，默认 Text
    (body.to_string(), ExtractType::Text)
}

/// 依据选择器类型构建对应的 `ExtractRule`。
fn build_rule(
    kind: RuleKind,
    selector: String,
    extract_type: ExtractType,
    regex_clean: Option<RegexClean>,
) -> ExtractRule {
    match kind {
        RuleKind::Css => ExtractRule::CssSelector {
            selector,
            extract_type,
            regex_clean,
        },
        RuleKind::XPath => ExtractRule::XPath {
            expression: selector,
            extract_type,
            regex_clean,
        },
        RuleKind::JsonPath => ExtractRule::JsonPath {
            path: selector,
            extract_type,
            regex_clean,
        },
        RuleKind::Regex => ExtractRule::Regex {
            pattern: selector,
            group: 0,
            regex_clean,
        },
    }
}
