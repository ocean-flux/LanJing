//! 提取节点处理器 — 消费 `HttpResponse`，按 `ExtractRule` IR 提取产出。
//!
//! 两种模式:
//! - **列表模式**(`field_rules` 非空):先用 list 规则取 N 个 item,
//!   再对每个 item 提取多字段，产 N 条 JSON 中间记录。
//! - **单值模式**(`field_rules` 为空):直接用 fallback 链取单值，产 1 条 JSON 中间记录。
//!
//! 分发策略:
//! - `ExpectedDataType::Html` + `ExtractRule::XPath` → `html_xpath` 模块(html5+XPath)
//! - `ExpectedDataType::Html` + `ExtractRule::CssSelector` → `html_css` 模块(scraper+CSS)
//! - `ExpectedDataType::Xml` → `xml` 模块
//! - `ExpectedDataType::Json` → `json` 模块

use encoding_rs::GBK;
use futures::stream::{BoxStream, StreamExt};
use regex::Regex;

use lj_rule_model::{ExpectedDataType, ExtractRule, OutputTarget};
use lj_runtime::{ExecutionContext, NodeProcessor};
use lj_runtime::{NodeData, NodeDataVariant};
use lj_runtime::{NodeKind, NodeSpec};

use crate::regex_extract::RegexCache;
use crate::{html_css, html_xpath, json, xml};

/// 提取节点处理器。
pub struct ExtractNodeProcessor;

impl NodeProcessor for ExtractNodeProcessor {
    fn kind(&self) -> NodeKind {
        NodeKind::Extract
    }

    fn input_type(&self) -> Option<NodeDataVariant> {
        Some(NodeDataVariant::HttpResponse)
    }

    fn output_type(&self) -> NodeDataVariant {
        NodeDataVariant::Json
    }

    /// 消费 input stream(每个 HttpResponse)，按 `ExtractSpec` 提取并产出。
    ///
    /// `field_rules` 非空 → 列表模式，否则 → 单值模式。
    /// `output_target` 决定提取意图：媒体条目、媒体单元或媒体载荷中间记录。
    fn process<'a>(
        &'a self,
        ctx: &'a ExecutionContext,
        spec: &'a NodeSpec,
        input: BoxStream<'a, NodeData>,
    ) -> BoxStream<'a, NodeData> {
        let Some(extract_spec) = spec.extract.clone() else {
            return futures::stream::empty().boxed();
        };
        let expected_type = extract_spec.expected_type;
        let rules = extract_spec.rules;
        let field_rules = extract_spec.field_rules;
        let output_target = extract_spec.output_target;

        // 列表/单值模式：field_rules 非空 = 列表模式
        let is_list = !field_rules.is_empty();

        // 预编译 regex 缓存
        let regex_cache = build_regex_cache(&rules, &field_rules);

        // base_url 用于相对路径→绝对路径拼接
        let base_url = ctx.base_url.clone();

        input
            .flat_map(move |data| {
                let rules = rules.clone();
                let field_rules = field_rules.clone();
                let regex_cache = regex_cache.clone();
                let base_url = base_url.clone();

                match data {
                    NodeData::HttpResponse(resp) => {
                        let body_str = decode_body(&resp.body, resp.charset.as_deref());
                        let results = match expected_type {
                            ExpectedDataType::Html => {
                                let has_xpath =
                                    rules.iter().any(|r| matches!(r, ExtractRule::XPath { .. }));
                                if has_xpath {
                                    // Html+XPath 路径 (xmloxide html5)
                                    process_html_xpath(
                                        &body_str,
                                        &rules,
                                        &field_rules,
                                        &regex_cache,
                                        &base_url,
                                        is_list,
                                        output_target,
                                    )
                                } else {
                                    // Html+CSS 路径 (scraper)
                                    process_html_css(
                                        &body_str,
                                        &rules,
                                        &field_rules,
                                        &regex_cache,
                                        &base_url,
                                        is_list,
                                        output_target,
                                    )
                                }
                            }
                            // XML/JSON 真实分发
                            ExpectedDataType::Xml | ExpectedDataType::Json => extract_xml_or_json(
                                expected_type,
                                &body_str,
                                &rules,
                                &field_rules,
                                &regex_cache,
                                is_list,
                            ),
                        };
                        futures::stream::iter(results).boxed()
                    }
                    // Error 透传,不静默吞没
                    NodeData::Error(_) => futures::stream::once(async move { data }).boxed(),
                    _ => futures::stream::empty().boxed(),
                }
            })
            .boxed()
    }
}

/// HTML+XPath 路径分发，按 `output_target` 和 `is_list` 选择提取函数。
fn process_html_xpath(
    body_str: &str,
    rules: &[ExtractRule],
    field_rules: &crate::FieldRules,
    regex_cache: &RegexCache,
    base_url: &str,
    is_list: bool,
    output_target: OutputTarget,
) -> Vec<NodeData> {
    match output_target {
        OutputTarget::Units if is_list => {
            // 目录列表 → 产出媒体单元中间记录
            let chapters =
                html_xpath::extract_toc_list(body_str, rules, field_rules, regex_cache, base_url);
            if chapters.is_empty() {
                vec![NodeData::Error(
                    "媒体单元提取未得到任何有效记录".to_string(),
                )]
            } else {
                vec![NodeData::Json(serde_json::Value::Array(chapters))]
            }
        }
        OutputTarget::Asset => {
            // 文本资产 → 单值文本中间记录
            html_xpath::extract_single(body_str, rules, regex_cache)
        }
        _ if is_list => {
            // 媒体列表模式 → 多条来源中间记录
            html_xpath::extract_list(body_str, rules, field_rules, regex_cache, base_url)
        }
        _ => {
            // 媒体单值模式 → 1 条来源中间记录
            html_xpath::extract_single(body_str, rules, regex_cache)
        }
    }
}

/// HTML+CSS 路径分发，按 `output_target` 和 `is_list` 选择提取函数。
fn process_html_css(
    body_str: &str,
    rules: &[ExtractRule],
    field_rules: &crate::FieldRules,
    regex_cache: &RegexCache,
    base_url: &str,
    is_list: bool,
    output_target: OutputTarget,
) -> Vec<NodeData> {
    match output_target {
        OutputTarget::Units if is_list => {
            // 目录列表 → 产出媒体单元中间记录
            let chapters =
                html_css::extract_toc_list(body_str, rules, field_rules, regex_cache, base_url);
            if chapters.is_empty() {
                vec![NodeData::Error(
                    "媒体单元提取未得到任何有效记录".to_string(),
                )]
            } else {
                vec![NodeData::Json(serde_json::Value::Array(chapters))]
            }
        }
        OutputTarget::Asset => {
            // 文本资产 → 单值文本中间记录
            html_css::extract_single(body_str, rules, regex_cache)
        }
        _ if is_list => {
            // Media 列表模式 → 多个 Media
            html_css::extract_list(body_str, rules, field_rules, regex_cache, base_url)
        }
        _ => {
            // Media 单值模式 → 1 个 Media
            html_css::extract_single(body_str, rules, regex_cache)
        }
    }
}

/// charset 解码: `Vec<u8>` + charset → `String`。
///
/// 支持 `gbk`/`GBK`/`gb2312`，其余回退 `String::from_utf8_lossy`。
#[must_use]
pub fn decode_body(body: &[u8], charset: Option<&str>) -> String {
    match charset {
        Some("gbk" | "GBK" | "gb2312") => {
            let (s, _, _) = GBK.decode(body);
            s.to_string()
        }
        _ => String::from_utf8_lossy(body).to_string(),
    }
}

// ---------------------------------------------------------------------------
// regex 缓存
// ---------------------------------------------------------------------------

/// 从 `rules` 和 `field_rules` 中收集所有 regex pattern，预编译为缓存。
fn build_regex_cache(rules: &[ExtractRule], field_rules: &crate::FieldRules) -> RegexCache {
    let mut cache = RegexCache::new();
    for rule in rules {
        insert_regex_patterns(rule, &mut cache);
    }
    for rules in field_rules.values() {
        for rule in rules {
            insert_regex_patterns(rule, &mut cache);
        }
    }
    cache
}

/// 从单条规则中提取 regex pattern 并插入缓存。
fn insert_regex_patterns(rule: &ExtractRule, cache: &mut RegexCache) {
    match rule {
        ExtractRule::CssSelector { regex_clean, .. }
        | ExtractRule::XPath { regex_clean, .. }
        | ExtractRule::JsonPath { regex_clean, .. } => {
            if let Some(clean) = regex_clean
                && !cache.contains_key(&clean.pattern)
                && let Ok(re) = Regex::new(&clean.pattern)
            {
                cache.insert(clean.pattern.clone(), re);
            }
        }
        ExtractRule::Regex { pattern, .. } => {
            if !cache.contains_key(pattern)
                && let Ok(re) = Regex::new(pattern)
            {
                cache.insert(pattern.clone(), re);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// URL 拼接辅助
// ---------------------------------------------------------------------------

/// 相对路径→绝对路径拼接。
///
/// 用 `base_url`(如 `https://example.com`)拼接相对路径(如 `/book/123`)。
/// 已经是绝对 URL 则原样返回。`base_url` 为空或解析失败时返回原值。
#[must_use]
pub(crate) fn resolve_url(raw: &str, base_url: &str) -> String {
    if raw.is_empty() || base_url.is_empty() {
        return raw.to_string();
    }
    // 已经是绝对 URL 则原样返回
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return raw.to_string();
    }
    // 用 url crate 拼接
    match url::Url::parse(base_url) {
        Ok(base) => base
            .join(raw)
            .map_or_else(|_| raw.to_string(), |u| u.to_string()),
        Err(_) => raw.to_string(),
    }
}

/// XML/JSON 真实分发:按列表/单值模式提取。
fn extract_xml_or_json(
    expected_type: ExpectedDataType,
    body: &str,
    rules: &[ExtractRule],
    field_rules: &crate::FieldRules,
    regex_cache: &RegexCache,
    is_list: bool,
) -> Vec<NodeData> {
    let Some(path) = first_extract_path(rules) else {
        return vec![NodeData::Error(
            "XML/JSON 列表/detail 缺少 XPath/JsonPath 表达式".to_string(),
        )];
    };
    match expected_type {
        ExpectedDataType::Xml => {
            if is_list {
                xml::extract_xml_list_video(body, path, field_rules, regex_cache)
            } else {
                single_to_video(xml::extract_xml_single(body, rules, regex_cache))
            }
        }
        ExpectedDataType::Json => {
            let json_val: serde_json::Value = match serde_json::from_str(body) {
                Ok(v) => v,
                Err(e) => return vec![NodeData::Error(format!("JSON 解析失败: {e}"))],
            };
            if is_list {
                json::extract_json_list_video(&json_val, path, field_rules, regex_cache)
            } else {
                single_to_video(json::extract_json_single(&json_val, rules, regex_cache))
            }
        }
        ExpectedDataType::Html => unreachable!("Html 不走 XML/JSON 分发"),
    }
}

/// 单值提取结果 → 单条 JSON 中间记录(title=值) 或 `Error`。
fn single_to_video(result: Result<String, crate::error::ExtractError>) -> Vec<NodeData> {
    match result {
        Ok(s) if !s.is_empty() => vec![NodeData::Json(serde_json::json!({ "title": s }))],
        Ok(_) => vec![NodeData::Error("提取未匹配".to_string())],
        Err(e) => vec![NodeData::Error(format!("提取失败: {e}"))],
    }
}

/// 取 `rules` 首条 XPath/JsonPath 的表达式路径。
fn first_extract_path(rules: &[ExtractRule]) -> Option<&str> {
    for rule in rules {
        match rule {
            ExtractRule::XPath { expression, .. } => return Some(expression.as_str()),
            ExtractRule::JsonPath { path, .. } => return Some(path.as_str()),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_body_gbk() {
        // "主人" in GBK
        let gbk_bytes = [0xD6, 0xF7, 0xC8, 0xCB];
        assert_eq!(decode_body(&gbk_bytes, Some("gbk")), "主人");
    }

    #[test]
    fn test_decode_body_utf8() {
        let utf8_bytes = "你好".as_bytes();
        assert_eq!(decode_body(utf8_bytes, Some("utf-8")), "你好");
    }

    #[test]
    fn test_decode_body_fallback() {
        let utf8_bytes = "hello".as_bytes();
        assert_eq!(decode_body(utf8_bytes, None), "hello");
    }
}
