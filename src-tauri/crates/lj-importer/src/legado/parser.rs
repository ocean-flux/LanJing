//! Legado 规则字符串的来源内解析。
//!
//! 本模块只把 Legado 作者格式转换为共享 `ExtractRule` IR；不会生成旧 Graph、执行计划，
//! 或让运行时了解 Legado 选择器语法。

use lj_rule_model::{ExtractRule, ExtractType, RegexClean};

/// 已知提取后缀，按长度降序排列避免短后缀抢先匹配。
const KNOWN_SUFFIXES: &[(&str, ExtractType)] = &[
    ("ownText", ExtractType::OwnText),
    ("text", ExtractType::Text),
    ("href", ExtractType::Href),
    ("src", ExtractType::Src),
    ("html", ExtractType::Html),
];

/// 无选择器时允许直接从当前元素读取的属性。
const BARE_ATTRIBUTES: &[(&str, ExtractType)] = &[
    ("text", ExtractType::Text),
    ("href", ExtractType::Href),
    ("src", ExtractType::Src),
    ("html", ExtractType::Html),
    ("ownText", ExtractType::OwnText),
];

/// 将一个 Legado 规则字符串转换为按优先级排序的提取回退链。
#[must_use]
pub(crate) fn parse_legado_rule(input: &str) -> Vec<ExtractRule> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    trimmed
        .split("||")
        .map(str::trim)
        .filter(|candidate| !candidate.is_empty())
        .map(parse_single)
        .collect()
}

fn parse_single(input: &str) -> ExtractRule {
    let (selector_part, regex_clean) = split_regex_clean(input);
    let (body, kind) = strip_prefix(selector_part);
    let (selector, extract_type) = selector_and_type(body);
    build_rule(kind, selector, extract_type, regex_clean)
}

fn split_regex_clean(input: &str) -> (&str, Option<RegexClean>) {
    let Some(position) = input.find("##") else {
        return (input, None);
    };
    let selector_part = &input[..position];
    let remainder = &input[position + 2..];
    let (pattern, replacement) = remainder.find("##").map_or((remainder, ""), |separator| {
        (&remainder[..separator], &remainder[separator + 2..])
    });
    (
        selector_part,
        Some(RegexClean {
            pattern: pattern.to_string(),
            replacement: replacement.to_string(),
        }),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuleKind {
    Css,
    XPath,
    JsonPath,
    Regex,
}

fn strip_prefix(input: &str) -> (&str, RuleKind) {
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

fn selector_and_type(body: &str) -> (String, ExtractType) {
    for (suffix, extract_type) in KNOWN_SUFFIXES {
        let marker = format!("@{suffix}");
        if let Some(selector) = body.strip_suffix(&marker) {
            return (selector.to_string(), extract_type.clone());
        }
    }
    for (name, extract_type) in BARE_ATTRIBUTES {
        if body == *name {
            return (String::new(), extract_type.clone());
        }
    }
    (body.to_string(), ExtractType::Text)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fallback_and_regex_clean_inside_importer() {
        let rules = parse_legado_rule("figure img@src||img@src##unused##");
        assert_eq!(rules.len(), 2);
        assert!(matches!(
            &rules[0],
            ExtractRule::CssSelector {
                selector,
                extract_type: ExtractType::Src,
                regex_clean: None,
            } if selector == "figure img"
        ));
        assert!(matches!(
            &rules[1],
            ExtractRule::CssSelector {
                selector,
                extract_type: ExtractType::Src,
                regex_clean: Some(RegexClean { pattern, replacement }),
            } if pattern == "unused" && replacement.is_empty()
        ));
    }

    #[test]
    fn preserves_selector_at_sign_and_explicit_prefixes() {
        let css = parse_legado_rule("a[href*='/author/']@text");
        assert!(matches!(
            &css[0],
            ExtractRule::CssSelector { selector, .. } if selector == "a[href*='/author/']"
        ));
        let xpath = parse_legado_rule("@xpath://a/@href");
        assert!(matches!(
            &xpath[0],
            ExtractRule::XPath {
                expression,
                extract_type: ExtractType::Href,
                ..
            } if expression == "//a/"
        ));
    }
}
