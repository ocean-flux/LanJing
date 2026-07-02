//! XML 提取 — 用 `xmloxide` 引擎按 `XPath` 提取字段。
//!
//! 配置 DTD/外部实体处理禁用(xmloxide 默认不解析外部实体,XXE 防护)。

use lj_core::extract_rule::{ExtractRule, ExtractType, PlayUrlParserSpec};
use lj_core::media::{Media, VideoMedia};
use lj_core::node_data::NodeData;
use xmloxide::Document;
use xmloxide::xpath::{self, XPathValue};

use crate::regex_extract::{RegexCache, apply_regex_clean};

/// 从 XML 按 `XPath` 提取单个值(回退链,首个非空胜出)。
///
/// # Errors
///
/// 返回 `UnsupportedFormat` 当 XML 解析失败,`NoMatch` 当所有回退规则均未匹配。
pub fn extract_xml_single(
    xml: &str,
    rules: &[ExtractRule],
    regex_cache: &RegexCache,
) -> Result<String, crate::error::ExtractError> {
    let doc = Document::parse_str(xml)
        .map_err(|e| crate::error::ExtractError::UnsupportedFormat(format!("XML 解析失败: {e}")))?;
    let root = doc
        .root_element()
        .ok_or_else(|| crate::error::ExtractError::NoMatch("XML 无根元素".to_string()))?;
    for rule in rules {
        if let ExtractRule::XPath {
            expression,
            extract_type,
            regex_clean,
        } = rule
        {
            match evaluate_xpath(&doc, root, expression, extract_type) {
                Ok(s) if !s.is_empty() => {
                    return Ok(apply_clean(&s, regex_clean.as_ref(), regex_cache));
                }
                Ok(_) | Err(crate::error::ExtractError::NoMatch(_)) => {}
                Err(e) => return Err(e),
            }
        }
    }
    Err(crate::error::ExtractError::NoMatch(
        "所有 XPath 回退规则均未匹配".to_string(),
    ))
}

/// 列表模式:按 `list_xpath` 取 N 个 item,对每 item 按 `field_rules` 逐字段提取产 `Vec<VideoMedia>`。
///
/// `field_rules` key 约定: `name`/`cover`/`vodId`/`kind`/`remarks`。
#[must_use]
pub fn extract_xml_list_video(
    xml: &str,
    list_xpath: &str,
    field_rules: &crate::FieldRules,
    regex_cache: &RegexCache,
) -> Vec<NodeData> {
    let doc = match Document::parse_str(xml) {
        Ok(d) => d,
        Err(e) => return vec![NodeData::Error(format!("XML 解析失败: {e}"))],
    };
    let Some(root) = doc.root_element() else {
        return vec![NodeData::Error("XML 无根元素".to_string())];
    };

    let items = match xpath::evaluate(&doc, root, list_xpath) {
        Ok(XPathValue::NodeSet(ids)) if !ids.is_empty() => ids,
        _ => {
            return vec![NodeData::Error(format!(
                "列表 XPath '{list_xpath}' 未匹配任何节点"
            ))];
        }
    };

    items
        .iter()
        .map(|&item| {
            NodeData::Media(Media::Video(build_video_from_node(
                &doc,
                item,
                field_rules,
                regex_cache,
            )))
        })
        .collect()
}

/// Detail 模式:按 `item_xpath` 取单 item,提取元数据 + `vod_play_url`/`vod_play_from`,
/// 经 `play_url_parser` 解析产含 `play_lines` 的单 `VideoMedia`。
///
/// # Errors
///
/// 返回含错误的 `NodeData::Error` 当提取或解析失败。
#[must_use]
pub fn extract_xml_detail_video(
    xml: &str,
    item_xpath: &str,
    field_rules: &crate::FieldRules,
    play_url_spec: &PlayUrlParserSpec,
    regex_cache: &RegexCache,
) -> Vec<NodeData> {
    let doc = match Document::parse_str(xml) {
        Ok(d) => d,
        Err(e) => return vec![NodeData::Error(format!("XML 解析失败: {e}"))],
    };
    let Some(root) = doc.root_element() else {
        return vec![NodeData::Error("XML 无根元素".to_string())];
    };

    let item = match xpath::evaluate(&doc, root, item_xpath) {
        Ok(XPathValue::NodeSet(ids)) if !ids.is_empty() => ids[0],
        _ => {
            return vec![NodeData::Error(format!(
                "Detail XPath '{item_xpath}' 未匹配"
            ))];
        }
    };

    let mut vm = build_video_from_node(&doc, item, field_rules, regex_cache);
    let vod_play_url = field_str_on_node(&doc, item, field_rules, "playUrl", regex_cache);
    let vod_play_from = field_str_on_node(&doc, item, field_rules, "playFrom", regex_cache);
    match crate::play_url::parse_play_lines(&vod_play_url, &vod_play_from, play_url_spec) {
        Ok(lines) if !lines.is_empty() => {
            vm.play_lines = lines;
            vec![NodeData::Media(Media::Video(vm))]
        }
        Ok(_) => vec![NodeData::Error("Detail play_lines 解析为空".to_string())],
        Err(e) => vec![NodeData::Error(format!("play_url_parser 解析失败: {e}"))],
    }
}

/// 从节点按 `field_rules` 构建 `VideoMedia`(不含 `play_lines`)。
fn build_video_from_node(
    doc: &Document,
    node: xmloxide::NodeId,
    field_rules: &crate::FieldRules,
    regex_cache: &RegexCache,
) -> VideoMedia {
    VideoMedia {
        title: field_str_on_node(doc, node, field_rules, "name", regex_cache),
        cover_url: field_opt_on_node(doc, node, field_rules, "cover", regex_cache),
        description: field_opt_on_node(doc, node, field_rules, "description", regex_cache),
        kind: field_opt_on_node(doc, node, field_rules, "kind", regex_cache),
        remarks: field_opt_on_node(doc, node, field_rules, "remarks", regex_cache),
        vod_id: field_opt_on_node(doc, node, field_rules, "vodId", regex_cache),
        play_lines: vec![],
    }
}

/// 从节点按 `field_rules` 取必须字段(空时返 "未知")。
fn field_str_on_node(
    doc: &Document,
    node: xmloxide::NodeId,
    field_rules: &crate::FieldRules,
    field: &str,
    regex_cache: &RegexCache,
) -> String {
    field_opt_on_node(doc, node, field_rules, field, regex_cache)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "未知".to_string())
}

/// 从节点按 `field_rules` 取可选字段(回退链,首个非空胜出)。
fn field_opt_on_node(
    doc: &Document,
    node: xmloxide::NodeId,
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

/// 在节点上按 `XPath` 求值并按 `ExtractType` 取值。
///
/// # Errors
///
/// 返回 `UnsupportedFormat` 当 `XPath` 求值失败,`NoMatch` 当节点集为空。
pub(crate) fn evaluate_xpath(
    doc: &Document,
    ctx: xmloxide::NodeId,
    expr: &str,
    extract_type: &ExtractType,
) -> Result<String, crate::error::ExtractError> {
    let value = xpath::evaluate(doc, ctx, expr).map_err(|e| {
        crate::error::ExtractError::UnsupportedFormat(format!("XPath 求值失败: {e}"))
    })?;
    match value {
        XPathValue::String(s) => Ok(s),
        XPathValue::NodeSet(ids) => {
            if ids.is_empty() {
                return Err(crate::error::ExtractError::NoMatch(format!(
                    "XPath '{expr}' 节点集为空"
                )));
            }
            // 多节点拼接（Maccms XML 的 dl/dd 结构需要取所有匹配节点）
            let values: Vec<String> = ids
                .iter()
                .map(|n| extract_node_value(doc, *n, extract_type))
                .filter(|s| !s.is_empty())
                .collect();
            Ok(values.join("$$$"))
        }
        XPathValue::Number(n) => Ok(format_xpath_number(n)),
        XPathValue::Boolean(b) => Ok(b.to_string()),
    }
}

/// 从 XML 节点按 `ExtractType` 取值(Text/Attr/Href/Src/Html/OwnText)。
fn extract_node_value(
    doc: &Document,
    node: xmloxide::NodeId,
    extract_type: &ExtractType,
) -> String {
    match extract_type {
        ExtractType::Text | ExtractType::OwnText | ExtractType::Html => doc.text_content(node),
        ExtractType::Href => doc.attribute(node, "href").unwrap_or("").to_string(),
        ExtractType::Src => doc.attribute(node, "src").unwrap_or("").to_string(),
        ExtractType::Attr(name) => doc.attribute(node, name).unwrap_or("").to_string(),
    }
}

/// 可选应用 `regex_clean`。
pub(crate) fn apply_clean(
    text: &str,
    clean: Option<&RegexClean>,
    regex_cache: &RegexCache,
) -> String {
    match clean {
        Some(c) => apply_regex_clean(text, c, regex_cache),
        None => text.to_string(),
    }
}

/// `XPath` number → string(去除多余尾零)。
pub(crate) fn format_xpath_number(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{n:.0}")
    } else {
        n.to_string()
    }
}

// 兼容旧 stub API(测试可能引用)
use lj_core::extract_rule::RegexClean;

#[cfg(test)]
mod tests {
    use super::*;
    use lj_core::extract_rule::{ExtractType, PlayUrlParserSpec};

    fn regex_cache() -> RegexCache {
        RegexCache::new()
    }

    fn play_url_spec() -> PlayUrlParserSpec {
        PlayUrlParserSpec {
            line_sep: "###".to_string(),
            episode_sep: "#".to_string(),
            name_url_sep: "$".to_string(),
            play_from_sep: ",".to_string(),
        }
    }

    #[test]
    fn covers_ae1_xml_list_extract_name_cdata() {
        // 红牛 XML 样本: <name><![CDATA[...]]></name>
        let xml = r#"<?xml version="1.0" encoding="utf-8"?><rss version="5.1"><list><video><id>1</id><name><![CDATA[爱情没有神话]]></name><type>国产剧</type><dt>hnyun,hnm3u8</dt><note><![CDATA[第32集]]></note></video><video><id>2</id><name><![CDATA[别横了]]></name><type>短剧</type><dt>hnyun</dt><note><![CDATA[完结]]></note></video></list></rss>"#;
        let cache = regex_cache();
        let mut field_rules = crate::FieldRules::new();
        field_rules.insert(
            "name".to_string(),
            vec![ExtractRule::XPath {
                expression: "name/text()".to_string(),
                extract_type: ExtractType::Text,
                regex_clean: None,
            }],
        );
        let result = extract_xml_list_video(xml, "/rss/list/video", &field_rules, &cache);
        assert_eq!(result.len(), 2);
        match &result[0] {
            NodeData::Media(Media::Video(v)) => assert_eq!(v.title, "爱情没有神话"),
            other => panic!("期望 VideoMedia, got {other:?}"),
        }
    }

    #[test]
    fn xml_detail_play_lines_parse() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?><rss><list><video><name><![CDATA[测试]]></name><url><![CDATA[第1集$http://x/1.m3u8#第2集$http://x/2.m3u8###第1集$http://y/1.m3u8]]></url><dt>hnyun,hnm3u8</dt></video></list></rss>"#;
        let cache = regex_cache();
        let mut field_rules = crate::FieldRules::new();
        field_rules.insert(
            "name".to_string(),
            vec![ExtractRule::XPath {
                expression: "name/text()".to_string(),
                extract_type: ExtractType::Text,
                regex_clean: None,
            }],
        );
        field_rules.insert(
            "playUrl".to_string(),
            vec![ExtractRule::XPath {
                expression: "url/text()".to_string(),
                extract_type: ExtractType::Text,
                regex_clean: None,
            }],
        );
        field_rules.insert(
            "playFrom".to_string(),
            vec![ExtractRule::XPath {
                expression: "dt/text()".to_string(),
                extract_type: ExtractType::Text,
                regex_clean: None,
            }],
        );
        let result = extract_xml_detail_video(
            xml,
            "/rss/list/video",
            &field_rules,
            &play_url_spec(),
            &cache,
        );
        assert_eq!(result.len(), 1);
        match &result[0] {
            NodeData::Media(Media::Video(v)) => {
                assert_eq!(v.play_lines.len(), 2);
                assert_eq!(v.play_lines[0].name, "hnyun");
                assert_eq!(v.play_lines[0].episodes.len(), 2);
                assert_eq!(v.play_lines[1].name, "hnm3u8");
                assert_eq!(v.play_lines[1].episodes.len(), 1);
            }
            other => panic!("期望 VideoMedia, got {other:?}"),
        }
    }

    #[test]
    fn xml_no_match_returns_error() {
        let xml = "<root></root>";
        let cache = regex_cache();
        let result = extract_xml_single(
            xml,
            &[ExtractRule::XPath {
                expression: "/root/foo/text()".to_string(),
                extract_type: ExtractType::Text,
                regex_clean: None,
            }],
            &cache,
        );
        assert!(result.is_err());
    }

    /// #3 XXE 防护回归: `xmloxide` 默认拒绝外部实体引用(`parse_str` 用 default
    /// options, `entity_resolver=None`),解析含 XXE payload 的 XML 应报错而非泄露
    /// 本地文件内容。验证解析层免疫 + extract 路径不泄露(返回错误而非文件内容)。
    #[test]
    fn xxe_payload_rejected_by_default() {
        // 经典 XXE payload: 声明外部实体指向 /etc/passwd 并引用
        let xxe_xml = r#"<!DOCTYPE r [
<!ENTITY xxe SYSTEM "file:///etc/passwd">
]>
<r>&xxe;</r>"#;
        let cache = regex_cache();
        let result = extract_xml_single(
            xxe_xml,
            &[ExtractRule::XPath {
                expression: "/r/text()".to_string(),
                extract_type: ExtractType::Text,
                regex_clean: None,
            }],
            &cache,
        );
        // 解析阶段即拒绝外部实体引用,返回 UnsupportedFormat 错误而非文件内容
        assert!(
            result.is_err(),
            "XXE payload 应被解析层拒绝,不应泄露文件内容"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("external entity") || err_msg.contains("XML 解析失败"),
            "错误应指向外部实体拒绝或解析失败,实际: {err_msg}"
        );
    }
}
