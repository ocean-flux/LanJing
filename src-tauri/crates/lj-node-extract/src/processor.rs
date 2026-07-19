//! Extract Plan effect adapter。
//!
//! 根据已编译 `ExtractSpec` 消费上游 HTTP effect 的响应，并产出受控 JSON records。

use std::time::Instant;

use encoding_rs::GBK;
use regex::Regex;

use lj_rule_model::{ExpectedDataType, ExtractRule, ExtractSpec, OutputTarget};
use lj_runtime::{
    CapturedEffectOutput, EffectCancellation, EffectError, EffectErrorCode, EffectFailure,
    EffectOutput, EffectWitness, ExtractEffectHandler, ExtractEffectRequest, ExtractEffectWitness,
    HttpResponse, NodeData, effect_input_hash,
};

use crate::regex_extract::RegexCache;
use crate::{html_css, html_xpath, json, xml};

/// Extract Plan effect adapter。
pub struct ExtractEffectAdapter;

/// adapter 直接借用上游 HTTP body 进行提取，不会为 Plan runtime 的 `Arc` 输出复制 body。
#[async_trait::async_trait]
impl ExtractEffectHandler for ExtractEffectAdapter {
    async fn execute_extract(
        &self,
        request: ExtractEffectRequest,
        cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError> {
        if cancellation.is_cancelled() {
            return Err(EffectError::new(
                EffectErrorCode::Cancelled,
                "Extract effect 已取消",
            ));
        }
        let started = Instant::now();
        let input_hash = effect_input_hash(&request.input).map_err(|_| {
            EffectError::new(
                EffectErrorCode::Internal,
                "Extract effect 输入 hash 计算失败",
            )
        })?;
        let Some(EffectOutput::Http(response)) = request.input.output() else {
            return Err(EffectError::new(
                EffectErrorCode::InputType,
                "Extract effect 需要 HTTP 响应输入",
            ));
        };
        let output = match extract_http_response(&request.spec, response, &request.base_url) {
            Ok(records) => EffectOutput::Extract(lj_runtime::ExtractOutput { records }),
            Err(_) => EffectOutput::Failure(EffectFailure::Extract),
        };
        if cancellation.is_cancelled() {
            return Err(EffectError::new(
                EffectErrorCode::Cancelled,
                "Extract effect 已取消",
            ));
        }
        Ok(CapturedEffectOutput::new(
            output,
            EffectWitness::Extract(ExtractEffectWitness {
                input_hash,
                duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            }),
        ))
    }
}

/// 用已编译 Extract spec 从 HTTP 响应提取 JSON 中间记录。
///
/// # Errors
///
/// 当规则不匹配、解析失败或处理器返回非 JSON 结果时返回安全错误消息。
fn extract_http_response(
    extract_spec: &ExtractSpec,
    response: &HttpResponse,
    base_url: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let expected_type = extract_spec.expected_type;
    let rules = &extract_spec.rules;
    let field_rules = &extract_spec.field_rules;
    let output_target = extract_spec.output_target;
    let is_list = !field_rules.is_empty();
    let regex_cache = build_regex_cache(rules, field_rules);
    let body = decode_body(&response.body, response.charset.as_deref());
    let results = match expected_type {
        ExpectedDataType::Html => {
            let has_xpath = rules
                .iter()
                .any(|rule| matches!(rule, ExtractRule::XPath { .. }));
            if has_xpath {
                process_html_xpath(
                    &body,
                    rules,
                    field_rules,
                    &regex_cache,
                    base_url,
                    is_list,
                    output_target,
                )
            } else {
                process_html_css(
                    &body,
                    rules,
                    field_rules,
                    &regex_cache,
                    base_url,
                    is_list,
                    output_target,
                )
            }
        }
        ExpectedDataType::Xml | ExpectedDataType::Json => extract_xml_or_json(
            expected_type,
            &body,
            rules,
            field_rules,
            &regex_cache,
            is_list,
        ),
    };
    let mut records = Vec::with_capacity(results.len());
    for result in results {
        match result {
            NodeData::Json(serde_json::Value::Array(values)) => records.extend(values),
            NodeData::Json(value) => records.push(value),
            NodeData::Error(message) => return Err(message),
            NodeData::Raw(_) | NodeData::HttpResponse(_) => {
                return Err("Extract 返回了非 JSON 结果".to_string());
            }
        }
    }
    Ok(records)
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
