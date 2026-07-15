//! Legado 规则语法解析器集成测试。
//!

use lj_compiler::legado_parser::parse_legado_rule;
use lj_core::extract_rule::{ExtractRule, ExtractType, RegexClean};

// -- 辅助函数 --

fn sel(rule: &ExtractRule) -> &str {
    match rule {
        ExtractRule::CssSelector { selector, .. } => selector,
        ExtractRule::XPath { expression, .. } => expression,
        ExtractRule::JsonPath { path, .. } => path,
        ExtractRule::Regex { pattern, .. } => pattern,
    }
}

fn ext(rule: &ExtractRule) -> ExtractType {
    match rule {
        ExtractRule::CssSelector { extract_type, .. }
        | ExtractRule::XPath { extract_type, .. }
        | ExtractRule::JsonPath { extract_type, .. } => extract_type.clone(),
        ExtractRule::Regex { .. } => ExtractType::Text,
    }
}

fn re(rule: &ExtractRule) -> Option<&RegexClean> {
    match rule {
        ExtractRule::CssSelector { regex_clean, .. }
        | ExtractRule::XPath { regex_clean, .. }
        | ExtractRule::JsonPath { regex_clean, .. }
        | ExtractRule::Regex { regex_clean, .. } => regex_clean.as_ref(),
    }
}

// -- 基础后缀解析 --

#[test]
fn test_empty_input() {
    assert!(parse_legado_rule("").unwrap().is_empty());
    assert!(parse_legado_rule("  ").unwrap().is_empty());
}

#[test]
fn test_pure_css_selector_no_suffix() {
    let r = parse_legado_rule("li[itemprop='mainEntity']").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::CssSelector {
            selector: "li[itemprop='mainEntity']".into(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }
    );
}

#[test]
fn test_css_with_text_suffix() {
    let r = parse_legado_rule("h2[itemprop='name']@text").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::CssSelector {
            selector: "h2[itemprop='name']".into(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }
    );
}

#[test]
fn test_css_with_href_suffix() {
    let r = parse_legado_rule("a[itemprop='url']@href").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::CssSelector {
            selector: "a[itemprop='url']".into(),
            extract_type: ExtractType::Href,
            regex_clean: None,
        }
    );
}

#[test]
fn test_css_with_src_suffix() {
    let r = parse_legado_rule("img[itemprop='image']@src").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::CssSelector {
            selector: "img[itemprop='image']".into(),
            extract_type: ExtractType::Src,
            regex_clean: None,
        }
    );
}

#[test]
fn test_css_with_html_suffix() {
    let r = parse_legado_rule("#article-content@html").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::CssSelector {
            selector: "#article-content".into(),
            extract_type: ExtractType::Html,
            regex_clean: None,
        }
    );
}

// -- CSS 选择器含 @ 属性选择器 --

#[test]
fn test_selector_with_at_in_attribute() {
    let r = parse_legado_rule("a[href*='/author/']@text").unwrap();
    assert_eq!(sel(&r[0]), "a[href*='/author/']");
    assert_eq!(ext(&r[0]), ExtractType::Text);
}

#[test]
fn test_nth_child_selector() {
    let r = parse_legado_rule("ol li:nth-child(3) a@text").unwrap();
    assert_eq!(sel(&r[0]), "ol li:nth-child(3) a");
    assert_eq!(ext(&r[0]), ExtractType::Text);
}

#[test]
fn test_contains_selector() {
    let r = parse_legado_rule("span:contains('万字')@text").unwrap();
    assert_eq!(sel(&r[0]), "span:contains('万字')");
}

// -- 裸属性名 --

#[test]
fn test_bare_href() {
    let r = parse_legado_rule("href").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::CssSelector {
            selector: String::new(),
            extract_type: ExtractType::Href,
            regex_clean: None,
        }
    );
}

#[test]
fn test_bare_text() {
    let r = parse_legado_rule("text").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::CssSelector {
            selector: String::new(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }
    );
}

#[test]
fn test_bare_src() {
    let r = parse_legado_rule("src").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::CssSelector {
            selector: String::new(),
            extract_type: ExtractType::Src,
            regex_clean: None,
        }
    );
}

// -- || 回退链 --

#[test]
fn test_fallback_chain_two() {
    let r = parse_legado_rule("figure img@src||img@src").unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(
        r[0],
        ExtractRule::CssSelector {
            selector: "figure img".into(),
            extract_type: ExtractType::Src,
            regex_clean: None,
        }
    );
    assert_eq!(
        r[1],
        ExtractRule::CssSelector {
            selector: "img".into(),
            extract_type: ExtractType::Src,
            regex_clean: None,
        }
    );
}

#[test]
fn test_fallback_chain_three() {
    let r = parse_legado_rule("a@href||b@href||c@href").unwrap();
    assert_eq!(r.len(), 3);
    assert_eq!(sel(&r[0]), "a");
    assert_eq!(sel(&r[1]), "b");
    assert_eq!(sel(&r[2]), "c");
}

// -- ##regex clean --

#[test]
fn test_regex_clean_no_replacement() {
    let r = parse_legado_rule("p[itemprop='author']@text##\\|.*").unwrap();
    assert_eq!(
        re(&r[0]),
        Some(&RegexClean {
            pattern: "\\|.*".into(),
            replacement: String::new(),
        })
    );
}

#[test]
fn test_text_with_regex_clean() {
    let r = parse_legado_rule("text##上次阅读").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::CssSelector {
            selector: String::new(),
            extract_type: ExtractType::Text,
            regex_clean: Some(RegexClean {
                pattern: "上次阅读".into(),
                replacement: String::new(),
            }),
        }
    );
}

#[test]
fn test_regex_clean_with_replacement() {
    let r = parse_legado_rule("div@text##bad##good").unwrap();
    assert_eq!(
        re(&r[0]),
        Some(&RegexClean {
            pattern: "bad".into(),
            replacement: "good".into(),
        })
    );
}

#[test]
fn test_empty_selector_with_regex_clean() {
    let r = parse_legado_rule("##remove_this").unwrap();
    assert_eq!(sel(&r[0]), "");
    assert_eq!(
        re(&r[0]),
        Some(&RegexClean {
            pattern: "remove_this".into(),
            replacement: String::new(),
        })
    );
}

// -- || + ## 复合 --

#[test]
fn test_fallback_with_regex_in_first_arm() {
    let r =
        parse_legado_rule("p[itemprop='author']@text##\\|.*||a[href*='/author/']@text").unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(
        re(&r[0]),
        Some(&RegexClean {
            pattern: "\\|.*".into(),
            replacement: String::new(),
        })
    );
    assert_eq!(re(&r[1]), None);
}

// -- 前缀 --

#[test]
fn test_xpath_prefix() {
    let r = parse_legado_rule("@xpath://div[@class='foo']/text()@text").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::XPath {
            expression: "//div[@class='foo']/text()".into(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }
    );
}

#[test]
fn test_jsonpath_prefix() {
    let r = parse_legado_rule("@json:$.store.book[*].author@href").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::JsonPath {
            path: "$.store.book[*].author".into(),
            extract_type: ExtractType::Href,
            regex_clean: None,
        }
    );
}

#[test]
fn test_regex_prefix() {
    let r = parse_legado_rule("@regex:(\\d+)@text").unwrap();
    assert_eq!(sel(&r[0]), "(\\d+)");
    assert!(re(&r[0]).is_none());
    if let ExtractRule::Regex {
        pattern: _,
        group,
        regex_clean: _,
    } = &r[0]
    {
        assert_eq!(*group, 0_usize);
    } else {
        panic!("期望 Regex 变体");
    }
}

#[test]
fn test_css_explicit_prefix() {
    let r = parse_legado_rule("@css:div.title@text").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::CssSelector {
            selector: "div.title".into(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }
    );
}

// -- 属性选择器中有 @ 但无后缀 --

#[test]
fn test_css_selector_with_at_in_attribute_and_no_suffix() {
    let r = parse_legado_rule("a[href*='/author/']").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::CssSelector {
            selector: "a[href*='/author/']".into(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }
    );
}

// -- ownText 后缀与裸属性 --

#[test]
fn test_own_text_suffix() {
    let r = parse_legado_rule("div.item@ownText").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::CssSelector {
            selector: "div.item".into(),
            extract_type: ExtractType::OwnText,
            regex_clean: None,
        }
    );
}

#[test]
fn test_bare_own_text() {
    let r = parse_legado_rule("ownText").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::CssSelector {
            selector: String::new(),
            extract_type: ExtractType::OwnText,
            regex_clean: None,
        }
    );
}

#[test]
fn test_bare_html() {
    let r = parse_legado_rule("html").unwrap();
    assert_eq!(
        r[0],
        ExtractRule::CssSelector {
            selector: String::new(),
            extract_type: ExtractType::Html,
            regex_clean: None,
        }
    );
}

// -- 空白与空段 --

#[test]
fn test_trimmed_whitespace() {
    let r = parse_legado_rule("  h1@text  ").unwrap();
    assert_eq!(sel(&r[0]), "h1");
}

#[test]
fn test_trailing_empty_segment_skipped() {
    let r = parse_legado_rule("a@text||").unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(sel(&r[0]), "a");
}

#[test]
fn test_leading_empty_segment_skipped() {
    let r = parse_legado_rule("||a@text").unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(sel(&r[0]), "a");
}
