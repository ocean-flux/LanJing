//! Legado JSON 类型定义 — 反序列化目标 struct。

use std::fmt;

use serde::Deserialize;

/// Legado 书源 JSON(反序列化目标,字段名 `camelCase`)。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegadoSourceJson {
    /// 书源名称。
    pub book_source_name: String,
    /// 书源 URL。
    pub book_source_url: String,
    /// 书源分组（展示用，可空）。
    pub book_source_group: Option<String>,
    /// 搜索 URL 模板。
    pub search_url: Option<String>,
    /// 发现/浏览 URL(含 `@js:` 前缀)。
    pub explore_url: Option<String>,
    /// 搜索来源规则段。
    pub rule_search: Option<RuleSearch>,
    /// 发现来源规则段。
    pub rule_explore: Option<RuleExplore>,
    /// 详情来源规则段。
    pub rule_book_info: Option<RuleBookInfo>,
    /// 目录来源规则段。
    pub rule_toc: Option<RuleToc>,
    /// 正文来源规则段。
    pub rule_content: Option<RuleContent>,
    /// HTTP 请求头(JSON 字符串)。
    pub header: Option<String>,
}

impl fmt::Debug for LegadoSourceJson {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LegadoSourceJson")
            .field("has_name", &!self.book_source_name.trim().is_empty())
            .field("has_base_url", &!self.book_source_url.trim().is_empty())
            .field("has_group", &self.book_source_group.is_some())
            .field("has_search", &self.search_url.is_some())
            .field("has_explore", &self.explore_url.is_some())
            .field("has_rule_search", &self.rule_search.is_some())
            .field("has_rule_explore", &self.rule_explore.is_some())
            .field("has_rule_book_info", &self.rule_book_info.is_some())
            .field("has_rule_toc", &self.rule_toc.is_some())
            .field("has_rule_content", &self.rule_content.is_some())
            .field("has_header", &self.header.is_some())
            .finish()
    }
}

/// `Search` 来源规则段。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleSearch {
    /// 书籍列表选择器。
    pub book_list: Option<String>,
    /// 书名选择器。
    pub name: Option<String>,
    /// 作者选择器。
    pub author: Option<String>,
    /// 书籍 URL 选择器。
    pub book_url: Option<String>,
    /// 封面 URL 选择器。
    pub cover_url: Option<String>,
    /// 分类选择器。
    pub kind: Option<String>,
}

/// `Explore` 来源规则段。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleExplore {
    /// 书籍列表选择器。
    pub book_list: Option<String>,
    /// 书名选择器。
    pub name: Option<String>,
    /// 作者选择器。
    pub author: Option<String>,
    /// 书籍 URL 选择器。
    pub book_url: Option<String>,
    /// 封面 URL 选择器。
    pub cover_url: Option<String>,
}

/// `BookInfo` 来源规则段。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleBookInfo {
    /// 书名选择器。
    pub name: Option<String>,
    /// 作者选择器。
    pub author: Option<String>,
    /// 封面 URL 选择器。
    pub cover_url: Option<String>,
    /// 简介选择器。
    pub intro: Option<String>,
    /// 分类选择器。
    pub kind: Option<String>,
    /// 字数选择器。
    pub word_count: Option<String>,
}

/// 目录规则。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleToc {
    /// 章节列表选择器。
    pub chapter_list: Option<String>,
    /// 章节名选择器。
    pub chapter_name: Option<String>,
    /// 章节 URL 选择器。
    pub chapter_url: Option<String>,
    /// 目录下一页 URL 规则; 空值表示单页目录。
    pub next_toc_url: Option<String>,
}

/// 正文规则。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleContent {
    /// 正文内容选择器。
    pub content: Option<String>,
    /// 正文下一页 URL 规则; 空值表示正文单页。
    pub next_content_url: Option<String>,
    /// 替换正则表达式(管道符分隔)。
    pub replace_regex: Option<String>,
}
