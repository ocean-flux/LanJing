//! JSON 提取 — 用 `jsonpath-rust` 引擎按 `JSONPath` 提取字段。

use jsonpath_rust::JsonPath;
use lj_core::extract_rule::{ExtractRule, ExtractType};
use lj_core::node_data::NodeData;
use serde_json::Value;

use crate::regex_extract::{RegexCache, apply_regex_clean};

/// 从 `JSON Value` 按 `JSONPath` 回退链提取单值(首个非空胜出)。
///
/// # Errors
///
/// 返回 `UnsupportedFormat` 当 `JSONPath` 求值失败,`NoMatch` 当所有回退规则均未匹配。
pub fn extract_json_single(
    json: &Value,
    rules: &[ExtractRule],
    regex_cache: &RegexCache,
) -> Result<String, crate::error::ExtractError> {
    for rule in rules {
        if let ExtractRule::JsonPath {
            path,
            extract_type,
            regex_clean,
        } = rule
        {
            match query_value(json, path, extract_type) {
                Ok(s) if !s.is_empty() => {
                    return Ok(apply_clean(&s, regex_clean.as_ref(), regex_cache));
                }
                Ok(_) | Err(crate::error::ExtractError::NoMatch(_)) => {}
                Err(e) => return Err(e),
            }
        }
    }
    Err(crate::error::ExtractError::NoMatch(
        "所有 JSONPath 回退规则均未匹配".to_string(),
    ))
}

/// 列表模式:按 `list_path` 取 N 个 item,对每 item 按 `field_rules` 逐字段提取产 JSON 中间记录。
///
/// `field_rules` key 约定: `name`/`cover`/`vodId`/`kind`/`remarks`。
/// `field_rules` 的 `JSONPath` 相对 item(如 `$.vod_name`),以 item 为根求值。
#[must_use]
pub fn extract_json_list_video(
    json: &Value,
    list_path: &str,
    field_rules: &crate::FieldRules,
    regex_cache: &RegexCache,
) -> Vec<NodeData> {
    let items = match query_items(json, list_path) {
        Ok(items) if !items.is_empty() => items,
        Ok(_) => {
            return vec![NodeData::Error(format!(
                "列表 JSONPath '{list_path}' 未匹配任何 item"
            ))];
        }
        Err(e) => {
            return vec![NodeData::Error(format!("列表 JSONPath 求值失败: {e}"))];
        }
    };

    items
        .iter()
        .map(|item| NodeData::Json(build_video_from_value(item, field_rules, regex_cache)))
        .collect()
}

/// 从 JSON Value 按 `field_rules` 构建视频来源中间记录。
fn build_video_from_value(
    item: &Value,
    field_rules: &crate::FieldRules,
    regex_cache: &RegexCache,
) -> serde_json::Value {
    serde_json::json!({
        "title": field_str_on_value(item, field_rules, "name", regex_cache),
        "cover_url": field_opt_on_value(item, field_rules, "cover", regex_cache),
        "description": field_opt_on_value(item, field_rules, "description", regex_cache),
        "kind": field_opt_on_value(item, field_rules, "kind", regex_cache),
        "remarks": field_opt_on_value(item, field_rules, "remarks", regex_cache),
        "play_url": field_opt_on_value(item, field_rules, "playUrl", regex_cache),
        "play_from": field_opt_on_value(item, field_rules, "playFrom", regex_cache),
        "source_item_id": field_opt_on_value(item, field_rules, "vodId", regex_cache),
    })
}

/// 从 Value 按 `field_rules` 取必须字段(空时返 "未知")。
fn field_str_on_value(
    item: &Value,
    field_rules: &crate::FieldRules,
    field: &str,
    regex_cache: &RegexCache,
) -> String {
    field_opt_on_value(item, field_rules, field, regex_cache)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "未知".to_string())
}

/// 从 Value 按 `field_rules` 取可选字段(回退链,首个非空胜出)。
fn field_opt_on_value(
    item: &Value,
    field_rules: &crate::FieldRules,
    field: &str,
    regex_cache: &RegexCache,
) -> Option<String> {
    let rules = field_rules.get(field)?;
    for rule in rules {
        if let ExtractRule::JsonPath {
            path,
            extract_type,
            regex_clean,
        } = rule
        {
            match query_value(item, path, extract_type) {
                Ok(s) if !s.is_empty() => {
                    return Some(apply_clean(&s, regex_clean.as_ref(), regex_cache));
                }
                _ => {}
            }
        }
    }
    None
}

/// 按 `JSONPath` 求值并按 `ExtractType` 取值(JSON 主要用 Text,Attr/Href/Src 退化取文本)。
///
/// # Errors
///
/// 返回 `UnsupportedFormat` 当 `JSONPath` 求值失败,`NoMatch` 当结果为空。
fn query_value(
    json: &Value,
    path: &str,
    extract_type: &ExtractType,
) -> Result<String, crate::error::ExtractError> {
    let queried = json.query(path).map_err(|e| {
        crate::error::ExtractError::UnsupportedFormat(format!("JSONPath 求值失败: {e}"))
    })?;
    let value = queried
        .first()
        .ok_or_else(|| crate::error::ExtractError::NoMatch(format!("JSONPath '{path}' 未匹配")))?;
    Ok(value_to_string(value, extract_type))
}

/// 按 `JSONPath` 求值取多个 item(列表模式)。
///
/// # Errors
///
/// 返回 `UnsupportedFormat` 当 `JSONPath` 求值失败。
fn query_items(json: &Value, path: &str) -> Result<Vec<Value>, crate::error::ExtractError> {
    let queried = json.query(path).map_err(|e| {
        crate::error::ExtractError::UnsupportedFormat(format!("JSONPath 求值失败: {e}"))
    })?;
    Ok(queried.into_iter().cloned().collect())
}

/// JSON Value → string(ExtractType 对 JSON 主要 Text;Attr/Href/Src 取对应字段名)。
fn value_to_string(value: &Value, extract_type: &ExtractType) -> String {
    match extract_type {
        ExtractType::Text | ExtractType::OwnText | ExtractType::Html => value_to_text(value),
        ExtractType::Href => value
            .get("href")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        ExtractType::Src => value
            .get("src")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        ExtractType::Attr(name) => value
            .get(name)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    }
}

/// Value → 文本(字符串原样,数字/布尔转字符串,对象/数组 JSON 序列化)。
fn value_to_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// 可选应用 `regex_clean`。
fn apply_clean(
    text: &str,
    clean: Option<&lj_core::extract_rule::RegexClean>,
    regex_cache: &RegexCache,
) -> String {
    match clean {
        Some(c) => apply_regex_clean(text, c, regex_cache),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn regex_cache() -> RegexCache {
        RegexCache::new()
    }

    fn hongniu_list_json() -> Value {
        // 红牛 JSON list 样本(补 vod_pic,list[0] 原无)
        serde_json::json!({
            "code": 1, "page": 1, "pagecount": 5_164, "limit": 20, "total": 103_274,
            "list": [
                {"vod_id": 140_789, "vod_name": "爱情没有神话", "type_id": 12, "type_name": "国产剧",
                 "vod_remarks": "第32集", "vod_play_from": "hnyun,hnm3u8",
                 "vod_pic": "https://x/p140789.jpg"},
                {"vod_id": 146_872, "vod_name": "别横了", "type_id": 30, "type_name": "短剧",
                 "vod_remarks": "完结", "vod_play_from": "hnyun,hnm3u8",
                 "vod_pic": "https://x/p146872.jpg"}
            ]
        })
    }

    #[test]
    fn covers_ae2_json_list_extract_multi_field() {
        let json = hongniu_list_json();
        let cache = regex_cache();
        let mut field_rules = crate::FieldRules::new();
        field_rules.insert(
            "name".to_string(),
            vec![ExtractRule::JsonPath {
                path: "$.vod_name".to_string(),
                extract_type: ExtractType::Text,
                regex_clean: None,
            }],
        );
        field_rules.insert(
            "cover".to_string(),
            vec![ExtractRule::JsonPath {
                path: "$.vod_pic".to_string(),
                extract_type: ExtractType::Text,
                regex_clean: None,
            }],
        );
        field_rules.insert(
            "vodId".to_string(),
            vec![ExtractRule::JsonPath {
                path: "$.vod_id".to_string(),
                extract_type: ExtractType::Text,
                regex_clean: None,
            }],
        );
        let result = extract_json_list_video(&json, "$.list[*]", &field_rules, &cache);
        assert_eq!(result.len(), 2);
        match &result[0] {
            NodeData::Json(v) => {
                assert_eq!(v["title"].as_str().unwrap_or_default(), "爱情没有神话");
                assert_eq!(v["cover_url"].as_str(), Some("https://x/p140789.jpg"));
                assert_eq!(v["source_item_id"].as_str(), Some("140789"));
            }
            other => panic!("期望 Json, got {other:?}"),
        }
    }

    #[test]
    fn json_no_match_returns_error() {
        let json = serde_json::json!({"a": 1});
        let cache = regex_cache();
        let result = extract_json_single(
            &json,
            &[ExtractRule::JsonPath {
                path: "$.b".to_string(),
                extract_type: ExtractType::Text,
                regex_clean: None,
            }],
            &cache,
        );
        assert!(result.is_err());
    }
}
