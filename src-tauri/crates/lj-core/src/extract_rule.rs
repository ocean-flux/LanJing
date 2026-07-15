//! 提取规则 — 规则语法解析的 IR(中间表示)。

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
    /// 非空时启用列表模式，对每 item 逐字段提取。
    /// 字段名约定: name/author/bookUrl/coverUrl/kind 等。
    #[serde(default)]
    pub field_rules: HashMap<String, Vec<ExtractRule>>,
    /// 预期数据类型(HTML/XML/JSON，决定用哪个解析器)。
    pub expected_type: ExpectedDataType,
    /// 产出目标类型——决定 Extract 节点产出哪种 `NodeData` 变体。
    /// 导入器填入，处理器按此分发提取逻辑。
    #[serde(default)]
    pub output_target: OutputTarget,
}

/// 产出目标类型——标注 Extract 节点的产出中间记录语义。
/// 导入器在构造 `ExtractSpec` 时填入，处理器按此决定提取逻辑。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OutputTarget {
    /// 产出媒体主体中间记录（单值或列表，取决于 `field_rules` 是否非空）。
    #[default]
    Media,
    /// 产出媒体单元中间记录（章节、分集、曲目等）。
    Units,
    /// 产出媒体资产中间记录（正文文本、图片、流地址等）。
    Asset,
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
