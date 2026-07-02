//! HTML 提取 — 用 `scraper` 引擎按 CSS 选择器提取字段。

use std::collections::HashMap;

use lj_core::extract_rule::{ExtractType, RegexClean};
use scraper::Selector;

use crate::regex_extract::{RegexCache, apply_regex_clean};

/// 解析 HTML 文档(供 `extract_from_doc` 复用)。
#[must_use]
pub fn parse_html(html: &str) -> scraper::Html {
    scraper::Html::parse_document(html)
}

/// 从已解析的 HTML 文档按 CSS 选择器提取单个元素的值。
///
/// `selector_str` 为空时从根元素提取(纯属性名场景)。
/// 支持 `:contains('text')` 伪类(非标准，手动实现)。
/// 使用 `regex_cache`(预编译缓存)避免重复解析和编译。
///
/// # Errors
///
/// 返回 `SelectorParse` 当 CSS 选择器语法错误，`NoMatch` 当未匹配。
pub fn extract_from_doc(
    doc: &scraper::Html,
    selector_str: &str,
    extract_type: &ExtractType,
    regex_clean: Option<&RegexClean>,
    regex_cache: &RegexCache,
) -> Result<String, crate::error::ExtractError> {
    if selector_str.is_empty() {
        // 空选择器：从根元素提取
        return extract_by_type_on_root(doc, extract_type, regex_clean, regex_cache);
    }

    // scraper 的 Selector::parse 不原生支持 :contains()
    if selector_str.contains(":contains(") {
        return extract_with_contains(doc, selector_str, extract_type, regex_clean, regex_cache);
    }

    let selector = parse_selector(selector_str)?;

    match doc.select(&selector).next() {
        Some(element) => {
            let text = extract_value(&element, extract_type);
            Ok(apply_clean(&text, regex_clean, regex_cache))
        }
        None => Err(crate::error::ExtractError::NoMatch(format!(
            "选择器 '{selector_str}' 未匹配任何元素"
        ))),
    }
}

// ponytail: extract_html/extract_html_list 仅为测试使用，非公开 API
/// 从 HTML 文档字符串按 CSS 选择器提取(便捷包装)。
///
/// 生产代码请使用 `extract_from_doc`。
#[doc(hidden)]
pub fn extract_html(
    html: &str,
    selector_str: &str,
    extract_type: &ExtractType,
    regex_clean: Option<&RegexClean>,
) -> Result<String, crate::error::ExtractError> {
    let doc = parse_html(html);
    let cache = HashMap::new();
    extract_from_doc(&doc, selector_str, extract_type, regex_clean, &cache)
}

/// 从已解析文档提取多个元素(bookList 场景)，返回每元素的 innerHTML。
///
/// 用 `el.html()`(outerHTML)返回含元素自身标签的序列化，
/// 保留外层属性(如 `<a href="...">`)供下游选择器匹配。
///
/// # Errors
///
/// 返回 `SelectorParse` 当 CSS 选择器语法错误。
pub fn extract_html_list_from_doc(
    doc: &scraper::Html,
    selector_str: &str,
) -> Result<Vec<String>, crate::error::ExtractError> {
    let selector = parse_selector(selector_str)?;

    let results: Vec<String> = doc.select(&selector).map(|el| el.html()).collect();
    Ok(results)
}

/// 从 HTML 文档字符串提取多个元素(便捷包装)。
///
/// 生产代码请使用 `extract_elements_from_doc`。
#[doc(hidden)]
pub fn extract_html_list(
    html: &str,
    selector_str: &str,
) -> Result<Vec<String>, crate::error::ExtractError> {
    let doc = parse_html(html);
    extract_html_list_from_doc(&doc, selector_str)
}

/// 从已解析文档提取多个 `ElementRef`(避免 serialize→reparse round-trip)。
///
/// 列表提取场景(bookList/chapterList)首选此函数，processor 可直接对 `ElementRef`
/// 跑字段选择器，无需重复解析 HTML。
///
/// # Errors
///
/// 返回 `SelectorParse` 当 CSS 选择器语法错误。
pub fn extract_elements_from_doc<'a>(
    doc: &'a scraper::Html,
    selector_str: &str,
) -> Result<Vec<scraper::ElementRef<'a>>, crate::error::ExtractError> {
    let selector = parse_selector(selector_str)?;
    Ok(doc.select(&selector).collect())
}

/// 从 HTML 元素按 CSS 选择器提取子元素的值。
///
/// `selector_str` 为空时直接从当前元素提取(与文档级 `extract_from_doc` 的
/// root 提取逻辑不同:元素不需要 body > * 包裹处理)。
///
/// # Errors
///
/// 返回 `SelectorParse` 当选择器语法错误，`NoMatch` 当未匹配。
pub fn extract_from_element(
    element: &scraper::ElementRef<'_>,
    selector_str: &str,
    extract_type: &ExtractType,
    regex_clean: Option<&RegexClean>,
    regex_cache: &RegexCache,
) -> Result<String, crate::error::ExtractError> {
    if selector_str.is_empty() {
        // 从当前元素自身提取
        let text = extract_value(element, extract_type);
        return Ok(apply_clean(&text, regex_clean, regex_cache));
    }

    if selector_str.contains(":contains(") {
        return extract_with_contains_on_element(
            element,
            selector_str,
            extract_type,
            regex_clean,
            regex_cache,
        );
    }

    let selector = parse_selector(selector_str)?;
    match element.select(&selector).next() {
        Some(child) => {
            let text = extract_value(&child, extract_type);
            Ok(apply_clean(&text, regex_clean, regex_cache))
        }
        None => Err(crate::error::ExtractError::NoMatch(format!(
            "选择器 '{selector_str}' 未匹配任何元素"
        ))),
    }
}

// ---------------------------------------------------------------------------
// 内部辅助
// ---------------------------------------------------------------------------

/// 解析 CSS 选择器，返回 `Selector`，失败时映射为 `SelectorParse`。
fn parse_selector(s: &str) -> Result<Selector, crate::error::ExtractError> {
    Selector::parse(s).map_err(|e| crate::error::ExtractError::SelectorParse(e.to_string()))
}

/// 从元素按提取类型取值。
fn extract_value(element: &scraper::ElementRef<'_>, extract_type: &ExtractType) -> String {
    match extract_type {
        ExtractType::Text => element.text().collect::<String>(),
        ExtractType::Href => element.value().attr("href").unwrap_or("").to_string(),
        ExtractType::Src => element.value().attr("src").unwrap_or("").to_string(),
        ExtractType::Html => element.inner_html(),
        ExtractType::OwnText => extract_own_text(element),
        ExtractType::Attr(name) => element.value().attr(name).unwrap_or("").to_string(),
    }
}

/// 提取直接文本子节点(不含后代元素文本)。
fn extract_own_text(element: &scraper::ElementRef<'_>) -> String {
    element
        .children()
        .filter_map(|child| match child.value() {
            // t.text 是 Tendril<UTF8>，转 String 后收集
            scraper::node::Node::Text(t) => Some(t.text.to_string()),
            _ => None,
        })
        .collect::<String>()
}

/// 可选应用 `regex_clean`。
fn apply_clean(text: &str, clean: Option<&RegexClean>, regex_cache: &RegexCache) -> String {
    match clean {
        Some(c) => apply_regex_clean(text, c, regex_cache),
        None => text.to_string(),
    }
}

/// 空选择器时从根元素提取。
fn extract_by_type_on_root(
    document: &scraper::Html,
    extract_type: &ExtractType,
    regex_clean: Option<&RegexClean>,
    regex_cache: &RegexCache,
) -> Result<String, crate::error::ExtractError> {
    // item HTML 片段场景:scraper 自动补全 <html><body>,目标元素在 body 下。
    // 优先选 body > *(跳过 html/head/body 包裹元素),回退到 html(完整文档场景)。
    if let Ok(sel) = parse_selector("body > *")
        && let Some(el) = document.select(&sel).next()
    {
        let text = extract_value(&el, extract_type);
        return Ok(apply_clean(&text, regex_clean, regex_cache));
    }
    if let Ok(sel) = parse_selector("html")
        && let Some(el) = document.select(&sel).next()
    {
        let text = extract_value(&el, extract_type);
        return Ok(apply_clean(&text, regex_clean, regex_cache));
    }
    Err(crate::error::ExtractError::NoMatch(
        "根元素未找到".to_string(),
    ))
}

/// `:contains('text')` 伪类手动实现。
///
/// 不支持嵌套选择器 + contains 的复杂组合；仅支持形如
/// `tag:contains('text')` 或 `tag.class:contains("text")`。
fn extract_with_contains(
    document: &scraper::Html,
    selector_str: &str,
    extract_type: &ExtractType,
    regex_clean: Option<&RegexClean>,
    regex_cache: &RegexCache,
) -> Result<String, crate::error::ExtractError> {
    let (base, needle) = split_contains_selector(selector_str)?;
    let selector = parse_selector(&base)?;

    for element in document.select(&selector) {
        let full_text: String = element.text().collect();
        if full_text.contains(&needle) {
            let value = extract_value(&element, extract_type);
            return Ok(apply_clean(&value, regex_clean, regex_cache));
        }
    }

    Err(crate::error::ExtractError::NoMatch(format!(
        "选择器 '{selector_str}' 未匹配 :contains() 条件"
    )))
}

/// `:contains('text')` 伪类手动实现(元素级，搜索子元素)。
///
/// 同 `extract_with_contains`，但从 `ElementRef` 的子元素树中搜索。
fn extract_with_contains_on_element(
    element: &scraper::ElementRef<'_>,
    selector_str: &str,
    extract_type: &ExtractType,
    regex_clean: Option<&RegexClean>,
    regex_cache: &RegexCache,
) -> Result<String, crate::error::ExtractError> {
    let (base, needle) = split_contains_selector(selector_str)?;
    let selector = parse_selector(&base)?;

    for child in element.select(&selector) {
        let full_text: String = child.text().collect();
        if full_text.contains(&needle) {
            let value = extract_value(&child, extract_type);
            return Ok(apply_clean(&value, regex_clean, regex_cache));
        }
    }

    Err(crate::error::ExtractError::NoMatch(format!(
        "选择器 '{selector_str}' 未匹配 :contains() 条件"
    )))
}

/// 拆分 `:contains('xxx')` 为基础选择器和查找文本。
///
/// 支持单引号和双引号两种参数形式。
fn split_contains_selector(s: &str) -> Result<(String, String), crate::error::ExtractError> {
    let start = s.find(":contains(").ok_or_else(|| {
        crate::error::ExtractError::SelectorParse("未找到 :contains()".to_string())
    })?;

    let base = s[..start].to_string();
    let after = &s[start + ":contains(".len()..];

    let quote = after.chars().next().ok_or_else(|| {
        crate::error::ExtractError::SelectorParse(":contains() 参数不完整".to_string())
    })?;

    if quote != '\'' && quote != '"' {
        return Err(crate::error::ExtractError::SelectorParse(
            ":contains() 需要一个引号参数".to_string(),
        ));
    }

    // 查找闭合引号
    if let Some(end) = after[1..].find(quote) {
        let needle = after[1..=end].to_string();
        Ok((base, needle))
    } else {
        Err(crate::error::ExtractError::SelectorParse(
            ":contains() 引号未闭合".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lj_core::extract_rule::ExtractType;

    #[test]
    fn test_extract_text() {
        let html = r"<h2 itemprop='name'>修罗武神</h2>";
        assert_eq!(
            extract_html(html, "h2[itemprop='name']", &ExtractType::Text, None).unwrap(),
            "修罗武神"
        );
    }

    #[test]
    fn test_extract_href() {
        let html = r"<a itemprop='url' href='/book/123'>链接</a>";
        assert_eq!(
            extract_html(html, "a[itemprop='url']", &ExtractType::Href, None).unwrap(),
            "/book/123"
        );
    }

    #[test]
    fn test_extract_src() {
        let html = r"<img itemprop='image' src='/cover/123.jpg' />";
        assert_eq!(
            extract_html(html, "img[itemprop='image']", &ExtractType::Src, None).unwrap(),
            "/cover/123.jpg"
        );
    }

    #[test]
    fn test_extract_html_content() {
        let html = r"<div id='article-content'><p>正文内容</p></div>";
        let result = extract_html(html, "#article-content", &ExtractType::Html, None).unwrap();
        assert!(result.contains("正文内容"));
    }

    #[test]
    fn test_extract_no_match() {
        let html = "<div>hello</div>";
        let result = extract_html(html, "span", &ExtractType::Text, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_list() {
        let html = r"<ul><li>A</li><li>B</li></ul>";
        let list = extract_html_list(html, "li").unwrap();
        assert_eq!(list.len(), 2);
        assert!(list[0].contains('A'));
        assert!(list[1].contains('B'));
    }

    #[test]
    fn test_nth_child() {
        let html = r"<ol><li>1</li><li>2</li><li><a>玄幻</a></li></ol>";
        assert_eq!(
            extract_html(html, "ol li:nth-child(3) a", &ExtractType::Text, None).unwrap(),
            "玄幻"
        );
    }

    #[test]
    fn test_contains_pseudo() {
        let html = r"<span>10万字</span>";
        assert_eq!(
            extract_html(html, "span:contains('万字')", &ExtractType::Text, None).unwrap(),
            "10万字"
        );
    }

    #[test]
    fn test_attr_extract() {
        let html = r#"<div data-id="42">内容</div>"#;
        assert_eq!(
            extract_html(html, "div", &ExtractType::Attr("data-id".to_string()), None).unwrap(),
            "42"
        );
    }
}
