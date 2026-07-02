//! Legado JSON 类型定义 — 反序列化目标 struct。

use serde::Deserialize;

/// Legado 书源 JSON(反序列化目标,字段名 `camelCase`)。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegadoSourceJson {
    /// 书源名称。
    pub book_source_name: String,
    /// 书源 URL。
    pub book_source_url: String,
    /// 搜索 URL 模板。
    pub search_url: Option<String>,
    /// 发现/浏览 URL(含 `@js:` 前缀)。
    pub explore_url: Option<String>,
    /// 搜索端点规则。
    pub rule_search: Option<RuleSearch>,
    /// 发现端点规则。
    pub rule_explore: Option<RuleExplore>,
    /// 详情端点规则。
    pub rule_book_info: Option<RuleBookInfo>,
    /// 目录端点规则。
    pub rule_toc: Option<RuleToc>,
    /// 正文端点规则。
    pub rule_content: Option<RuleContent>,
    /// HTTP 请求头(JSON 字符串)。
    pub header: Option<String>,
}

/// `Search` 端点规则。
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

/// `Explore` 端点规则。
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

/// `BookInfo` 端点规则。
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

/// `Toc` 端点规则。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleToc {
    /// 章节列表选择器。
    pub chapter_list: Option<String>,
    /// 章节名选择器。
    pub chapter_name: Option<String>,
    /// 章节 URL 选择器。
    pub chapter_url: Option<String>,
}

/// `Content` 端点规则。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleContent {
    /// 正文内容选择器。
    pub content: Option<String>,
    /// 替换正则表达式(管道符分隔)。
    pub replace_regex: Option<String>,
}
