//! Html+CSS 提取 — processor 中的 CSS 选择器提取逻辑。
//!
//! 使用 `scraper` 引擎按 CSS 选择器提取字段，支持回退链和列表模式。

use lj_core::extract_rule::ExtractRule;
use lj_core::media::{BookChapter, BookMedia, Media};
use lj_core::node_data::NodeData;

use crate::html;
use crate::regex_extract::RegexCache;

/// 从已解析文档上按回退链尝试提取（第一个非空结果胜出）。
///
/// # Errors
///
/// 返回 `NoMatch` 当所有回退规则均未匹配。
pub fn try_extract_on_doc(
    doc: &scraper::Html,
    rules: &[ExtractRule],
    regex_cache: &RegexCache,
) -> Result<String, crate::error::ExtractError> {
    for rule in rules {
        match rule {
            ExtractRule::CssSelector {
                selector,
                extract_type,
                regex_clean,
            } => {
                match html::extract_from_doc(
                    doc,
                    selector,
                    extract_type,
                    regex_clean.as_ref(),
                    regex_cache,
                ) {
                    Ok(result) if !result.is_empty() => return Ok(result),
                    Ok(_) | Err(crate::error::ExtractError::NoMatch(_)) => {}
                    Err(e) => return Err(e),
                }
            }
            // XPath/JsonPath/Regex 非 CSS 路径，此模块不处理
            ExtractRule::XPath { .. }
            | ExtractRule::JsonPath { .. }
            | ExtractRule::Regex { .. } => {}
        }
    }
    Err(crate::error::ExtractError::NoMatch(
        "所有回退规则均未匹配".to_string(),
    ))
}

/// 从 HTML 元素按回退链尝试提取（元素级）。
///
/// # Errors
///
/// 返回 `NoMatch` 当所有回退规则均未匹配。
fn try_extract_on_element(
    element: &scraper::ElementRef<'_>,
    rules: &[ExtractRule],
    regex_cache: &RegexCache,
) -> Result<String, crate::error::ExtractError> {
    for rule in rules {
        match rule {
            ExtractRule::CssSelector {
                selector,
                extract_type,
                regex_clean,
            } => {
                match html::extract_from_element(
                    element,
                    selector,
                    extract_type,
                    regex_clean.as_ref(),
                    regex_cache,
                ) {
                    Ok(result) if !result.is_empty() => return Ok(result),
                    Ok(_) | Err(crate::error::ExtractError::NoMatch(_)) => {}
                    Err(e) => return Err(e),
                }
            }
            ExtractRule::XPath { .. }
            | ExtractRule::JsonPath { .. }
            | ExtractRule::Regex { .. } => {}
        }
    }
    Err(crate::error::ExtractError::NoMatch(
        "所有回退规则均未匹配".to_string(),
    ))
}

/// 单值模式：按回退链提取，产 0~1 个 `Media`。
#[must_use]
pub fn extract_single(
    body_str: &str,
    rules: &[ExtractRule],
    regex_cache: &RegexCache,
) -> Vec<NodeData> {
    let doc = html::parse_html(body_str);
    match try_extract_on_doc(&doc, rules, regex_cache) {
        Ok(text) if !text.is_empty() => {
            vec![NodeData::Media(Media::Book(BookMedia {
                title: text,
                author: None,
                cover_url: None,
                description: None,
                kind: None,
                last_chapter: None,
                book_url: None,
                chapters: vec![],
            }))]
        }
        Ok(_) | Err(_) => {
            vec![NodeData::Error("提取未匹配".to_string())]
        }
    }
}

/// 列表模式：先用 CSS 选择器取 N 个 item，再对每个 item 逐字段提取。
#[must_use]
pub fn extract_list(
    body_str: &str,
    rules: &[ExtractRule],
    field_rules: &crate::FieldRules,
    regex_cache: &RegexCache,
    base_url: &str,
) -> Vec<NodeData> {
    let doc = html::parse_html(body_str);
    let items = match collect_list_elements(&doc, rules, "列表模式") {
        Ok(items) => items,
        Err(e) => return e,
    };

    items
        .iter()
        .map(|item_el| {
            let title = extract_field_element(item_el, field_rules, "name", regex_cache);
            let author = extract_field_opt_element(item_el, field_rules, "author", regex_cache);
            let book_url = extract_field_opt_element(item_el, field_rules, "bookUrl", regex_cache)
                .map(|url| crate::processor::resolve_url(&url, base_url));
            let cover_url =
                extract_field_opt_element(item_el, field_rules, "coverUrl", regex_cache)
                    .map(|url| crate::processor::resolve_url(&url, base_url));
            let kind = extract_field_opt_element(item_el, field_rules, "kind", regex_cache);

            NodeData::Media(Media::Book(BookMedia {
                title,
                author,
                cover_url,
                description: None,
                kind,
                last_chapter: None,
                book_url,
                chapters: vec![],
            }))
        })
        .collect()
}

/// Toc 端点列表提取：产出 1 个 `BookMedia` 含 N 个 `BookChapter`。
#[must_use]
pub fn extract_toc_list(
    body_str: &str,
    rules: &[ExtractRule],
    field_rules: &crate::FieldRules,
    regex_cache: &RegexCache,
    base_url: &str,
) -> Vec<NodeData> {
    let doc = html::parse_html(body_str);
    let items = match collect_list_elements(&doc, rules, "Toc 列表模式") {
        Ok(items) => items,
        Err(e) => return e,
    };

    let chapters: Vec<BookChapter> = items
        .iter()
        .filter_map(|item_el| {
            let title = extract_field_element(item_el, field_rules, "chapterName", regex_cache);
            let chapter_url = if let Some(url) =
                extract_field_opt_element(item_el, field_rules, "chapterUrl", regex_cache)
            {
                crate::processor::resolve_url(&url, base_url)
            } else {
                tracing::warn!(title = %title, "chapterUrl 提取失败,跳过该章节");
                return None;
            };
            Some(BookChapter {
                title,
                chapter_url,
                content: None,
            })
        })
        .collect();

    if chapters.is_empty() {
        return vec![NodeData::Error("Toc 提取未得到任何有效章节".to_string())];
    }

    vec![NodeData::Media(Media::Book(BookMedia {
        title: String::new(),
        author: None,
        cover_url: None,
        description: None,
        kind: None,
        last_chapter: None,
        book_url: None,
        chapters,
    }))]
}

/// 从文档中提取列表元素（CSS 选择器）。
fn collect_list_elements<'a>(
    doc: &'a scraper::Html,
    rules: &[ExtractRule],
    err_label: &str,
) -> Result<Vec<scraper::ElementRef<'a>>, Vec<NodeData>> {
    let list_selector = match rules.first() {
        Some(ExtractRule::CssSelector { selector, .. }) => selector.as_str(),
        _ => {
            return Err(vec![NodeData::Error(format!(
                "{err_label} 缺少 CSS 选择器"
            ))]);
        }
    };

    let items = match html::extract_elements_from_doc(doc, list_selector) {
        Ok(items) => items,
        Err(e) => {
            return Err(vec![NodeData::Error(format!("{err_label} 提取失败: {e}"))]);
        }
    };

    if items.is_empty() {
        return Err(vec![NodeData::Error(format!(
            "{err_label} 选择器未匹配任何元素"
        ))]);
    }

    Ok(items)
}

/// 从元素提取必须字段。
fn extract_field_element(
    element: &scraper::ElementRef<'_>,
    field_rules: &crate::FieldRules,
    field: &str,
    regex_cache: &RegexCache,
) -> String {
    field_rules
        .get(field)
        .and_then(|rules| try_extract_on_element(element, rules, regex_cache).ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "未知".to_string())
}

/// 从元素提取可选字段。
fn extract_field_opt_element(
    element: &scraper::ElementRef<'_>,
    field_rules: &crate::FieldRules,
    field: &str,
    regex_cache: &RegexCache,
) -> Option<String> {
    field_rules
        .get(field)
        .and_then(|rules| try_extract_on_element(element, rules, regex_cache).ok())
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lj_core::extract_rule::ExtractType;

    #[test]
    fn test_try_extract_fallback_first_wins() {
        let html = r"<img src='/fallback.jpg' />";
        let doc = html::parse_html(html);
        let rules = vec![
            ExtractRule::CssSelector {
                selector: "figure img".to_string(),
                extract_type: ExtractType::Src,
                regex_clean: None,
            },
            ExtractRule::CssSelector {
                selector: "img".to_string(),
                extract_type: ExtractType::Src,
                regex_clean: None,
            },
        ];
        let cache = RegexCache::new();
        let result = try_extract_on_doc(&doc, &rules, &cache).unwrap();
        assert_eq!(result, "/fallback.jpg");
    }

    #[test]
    fn test_try_extract_fallback_all_empty() {
        let html = "<div></div>";
        let doc = html::parse_html(html);
        let rules = vec![ExtractRule::CssSelector {
            selector: "span".to_string(),
            extract_type: ExtractType::Text,
            regex_clean: None,
        }];
        let cache = RegexCache::new();
        assert!(try_extract_on_doc(&doc, &rules, &cache).is_err());
    }
}
