//! Html+XPath 提取 — 用 `xmloxide` html5 解析后按 `XPath` 提取字段。
//!
//! 与 `html_css`(scraper+CSS)并列,处理 `ExpectedDataType::Html` + `ExtractRule::XPath`
//! 组合。xmloxide html5 解析器默认不解析外部实体(XXE 防护),与 `xml` 模块共用
//! `evaluate_xpath` 求值逻辑,仅解析入口换为 `parse_html5`。

use lj_core::extract_rule::ExtractRule;
use lj_core::media::{BookChapter, BookMedia, Media};
use lj_core::node_data::NodeData;
use xmloxide::Document;
use xmloxide::NodeId;
use xmloxide::html5::parse_html5;
use xmloxide::xpath::{self, XPathValue};

use crate::processor::resolve_url;
use crate::regex_extract::RegexCache;
use crate::xml::{apply_clean, evaluate_xpath};

/// 单值模式:按 `XPath` 回退链提取,首个非空结果产 1 个 `BookMedia`,否则产 `Error`。
#[must_use]
pub fn extract_single(
    body_str: &str,
    rules: &[ExtractRule],
    regex_cache: &RegexCache,
) -> Vec<NodeData> {
    let doc = match parse_html5(body_str) {
        Ok(d) => d,
        Err(e) => return vec![NodeData::Error(format!("HTML 解析失败: {e}"))],
    };
    let Some(root) = doc.root_element() else {
        return vec![NodeData::Error("HTML 无根元素".to_string())];
    };
    for rule in rules {
        if let ExtractRule::XPath {
            expression,
            extract_type,
            regex_clean,
        } = rule
        {
            match evaluate_xpath(&doc, root, expression, extract_type) {
                Ok(s) if !s.is_empty() => {
                    let cleaned = apply_clean(&s, regex_clean.as_ref(), regex_cache);
                    return vec![NodeData::Media(Media::Book(BookMedia {
                        title: cleaned,
                        author: None,
                        cover_url: None,
                        description: None,
                        kind: None,
                        last_chapter: None,
                        book_url: None,
                        chapters: vec![],
                    }))];
                }
                _ => {}
            }
        }
    }
    vec![NodeData::Error("提取未匹配".to_string())]
}

/// 列表模式:按首条 `XPath` 取 N 个 item,对每 item 逐字段提取产 N 个 `BookMedia`。
#[must_use]
pub fn extract_list(
    body_str: &str,
    rules: &[ExtractRule],
    field_rules: &crate::FieldRules,
    regex_cache: &RegexCache,
    base_url: &str,
) -> Vec<NodeData> {
    let doc = match parse_html5(body_str) {
        Ok(d) => d,
        Err(e) => return vec![NodeData::Error(format!("HTML 解析失败: {e}"))],
    };
    let Some(root) = doc.root_element() else {
        return vec![NodeData::Error("HTML 无根元素".to_string())];
    };
    let items = match collect_list_nodes(&doc, root, rules, "列表模式") {
        Ok(items) => items,
        Err(e) => return e,
    };
    items
        .iter()
        .map(|&item| {
            NodeData::Media(Media::Book(BookMedia {
                title: field_str(&doc, item, field_rules, "name", regex_cache),
                author: field_opt(&doc, item, field_rules, "author", regex_cache),
                book_url: field_opt(&doc, item, field_rules, "bookUrl", regex_cache)
                    .map(|url| resolve_url(&url, base_url)),
                cover_url: field_opt(&doc, item, field_rules, "coverUrl", regex_cache)
                    .map(|url| resolve_url(&url, base_url)),
                kind: field_opt(&doc, item, field_rules, "kind", regex_cache),
                description: None,
                last_chapter: None,
                chapters: vec![],
            }))
        })
        .collect()
}

/// Toc 端点列表提取:按首条 `XPath` 取 N 个 item,逐字段提取章节,产 1 个含 N 章 `BookChapter` 的 `BookMedia`。
#[must_use]
pub fn extract_toc_list(
    body_str: &str,
    rules: &[ExtractRule],
    field_rules: &crate::FieldRules,
    regex_cache: &RegexCache,
    base_url: &str,
) -> Vec<NodeData> {
    let doc = match parse_html5(body_str) {
        Ok(d) => d,
        Err(e) => return vec![NodeData::Error(format!("HTML 解析失败: {e}"))],
    };
    let Some(root) = doc.root_element() else {
        return vec![NodeData::Error("HTML 无根元素".to_string())];
    };
    let items = match collect_list_nodes(&doc, root, rules, "Toc 列表模式") {
        Ok(items) => items,
        Err(e) => return e,
    };
    let chapters: Vec<BookChapter> = items
        .iter()
        .filter_map(|&item| {
            let title = field_str(&doc, item, field_rules, "chapterName", regex_cache);
            let chapter_url =
                if let Some(url) = field_opt(&doc, item, field_rules, "chapterUrl", regex_cache) {
                    resolve_url(&url, base_url)
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

/// 按首条 `XPath` 表达式求值产列表节点集。
fn collect_list_nodes(
    doc: &Document,
    root: NodeId,
    rules: &[ExtractRule],
    err_label: &str,
) -> Result<Vec<NodeId>, Vec<NodeData>> {
    let list_xpath = match rules.first() {
        Some(ExtractRule::XPath { expression, .. }) => expression.as_str(),
        _ => {
            return Err(vec![NodeData::Error(format!(
                "{err_label} 缺少 XPath 表达式"
            ))]);
        }
    };
    match xpath::evaluate(doc, root, list_xpath) {
        Ok(XPathValue::NodeSet(ids)) if !ids.is_empty() => Ok(ids),
        _ => Err(vec![NodeData::Error(format!(
            "{err_label} XPath '{list_xpath}' 未匹配任何节点"
        ))]),
    }
}

/// 从节点按 `field_rules` 取必须字段(空时返 "未知")。
fn field_str(
    doc: &Document,
    node: NodeId,
    field_rules: &crate::FieldRules,
    field: &str,
    regex_cache: &RegexCache,
) -> String {
    field_opt(doc, node, field_rules, field, regex_cache)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "未知".to_string())
}

/// 从节点按 `field_rules` 取可选字段(XPath 回退链,首个非空胜出)。
fn field_opt(
    doc: &Document,
    node: NodeId,
    field_rules: &crate::FieldRules,
    field: &str,
    regex_cache: &RegexCache,
) -> Option<String> {
    let rules = field_rules.get(field)?;
    for rule in rules {
        if let ExtractRule::XPath {
            expression,
            extract_type,
            regex_clean,
        } = rule
        {
            match evaluate_xpath(doc, node, expression, extract_type) {
                Ok(s) if !s.is_empty() => {
                    return Some(apply_clean(&s, regex_clean.as_ref(), regex_cache));
                }
                _ => {}
            }
        }
    }
    None
}
