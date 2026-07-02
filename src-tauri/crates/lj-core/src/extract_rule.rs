//! 提取规则 — 规则语法解析的 IR(中间表示,ADR-0024)。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 字段提取规则表（字段名 → 规则列表），显式 `RandomState` 避免 `implicit_hasher`。
pub type FieldRules = HashMap<String, Vec<ExtractRule>, std::collections::hash_map::RandomState>;

/// 提取规则(闭集 enum，lj-compiler 解析规则字符串产出)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtractRule {
    /// CSS 选择器(scraper)。
    CssSelector {
        /// CSS 选择器字符串。
        selector: String,
        /// 提取类型。
        extract_type: ExtractType,
        /// 正则清理(##regex##replacement)。
        regex_clean: Option<RegexClean>,
    },
    /// XPath(xmloxide)。
    XPath {
        /// `XPath` 表达式。
        expression: String,
        /// 提取类型。
        extract_type: ExtractType,
        /// 正则清理。
        regex_clean: Option<RegexClean>,
    },
    /// JSONPath(jsonpath-rust)。
    JsonPath {
        /// `JSONPath` 路径。
        path: String,
        /// 提取类型。
        extract_type: ExtractType,
        /// 正则清理。
        regex_clean: Option<RegexClean>,
    },
    /// 正则表达式(regex)。
    Regex {
        /// 正则模式。
        pattern: String,
        /// 匹配组索引(0=整个匹配，1=第一组...)。
        group: usize,
        /// 正则清理。
        regex_clean: Option<RegexClean>,
    },
}

/// 提取类型(@text/@href/@src/@html 后缀)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtractType {
    /// @text — 元素文本内容。
    Text,
    /// @href — 链接属性。
    Href,
    /// @src — 源属性(图片等)。
    Src,
    /// @html — 元素 HTML 内容。
    Html,
    /// @ownText — 元素自身文本(不含子元素)。
    OwnText,
    /// 自定义属性名。
    Attr(String),
}

/// 正则清理(##regex##replacement)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegexClean {
    /// 正则模式。
    pub pattern: String,
    /// 替换文本。
    pub replacement: String,
}

/// 提取 spec(Extract 节点的 spec)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractSpec {
    /// 多选回退链(|| 分隔，按优先级尝试)。
    /// 列表模式下为 bookList 选择器，单值模式下为值回退链。
    pub rules: Vec<ExtractRule>,
    /// 列表模式的字段映射(`field_name` → 提取规则)。
    /// 非空时启用列表模式(Search/Discover)，对每 item 逐字段提取。
    /// 字段名约定: name/author/bookUrl/coverUrl/kind 等。
    #[serde(default)]
    pub field_rules: HashMap<String, Vec<ExtractRule>>,
    /// 关联的端点类型(用于 tracing span 命名)。
    pub endpoint_kind: Option<crate::endpoint::EndpointKind>,
    /// 预期数据类型(HTML/XML/JSON，决定用哪个解析器)。
    pub expected_type: ExpectedDataType,
    /// `vod_play_url` 解析 spec(Detail 端点产 `play_lines,可选`)。
    #[serde(default)]
    pub play_url_parser: Option<PlayUrlParserSpec>,
}

/// `vod_play_url` 两级嵌套树解析的分隔符 grammar(ADR-0028 附则)。
///
/// 由 importer 按源协议注入,Extract 节点消费产 `Vec<PlayLine>`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayUrlParserSpec {
    /// 线路间分隔符(Maccms default `###`)。
    pub line_sep: String,
    /// 集间分隔符(Maccms default `#`)。
    pub episode_sep: String,
    /// 名-URL 间分隔符(Maccms default `$`)。
    pub name_url_sep: String,
    /// `vod_play_from` 内线路名分隔符(Maccms default `,`)。
    pub play_from_sep: String,
}

/// 预期数据类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpectedDataType {
    /// HTML 文档。
    Html,
    /// XML 文档。
    Xml,
    /// JSON 文档。
    Json,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn play_url_parser_spec_defaults() {
        let spec = PlayUrlParserSpec {
            line_sep: "###".to_string(),
            episode_sep: "#".to_string(),
            name_url_sep: "$".to_string(),
            play_from_sep: ",".to_string(),
        };
        assert_eq!(spec.line_sep, "###");
        assert_eq!(spec.episode_sep, "#");
        assert_eq!(spec.name_url_sep, "$");
        assert_eq!(spec.play_from_sep, ",");
    }

    #[test]
    fn extract_spec_default_play_url_parser_none() {
        let spec = ExtractSpec {
            rules: vec![],
            field_rules: HashMap::new(),
            endpoint_kind: None,
            expected_type: ExpectedDataType::Json,
            play_url_parser: None,
        };
        assert!(spec.play_url_parser.is_none());
    }
}
