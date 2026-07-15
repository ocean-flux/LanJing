//! importer / runtime 共用的 Mapper 词汇入口。
//!
//! 这里只放跨 crate 共享的中性字段与模板变量，来源专属协议词汇留在各自模块。

/// 发现页分组标题候选键。
pub const DISCOVERY_TITLE_KEYS: &[&str] = &["title", "label", "name"];
/// 发现页动作/跳转候选键。
pub const DISCOVERY_ACTION_KEYS: &[&str] = &["url", "href", "key"];
/// 发现页 section 级 identity 字段。
pub const DISCOVERY_SECTION_IDENTITY_FIELDS: &[&str] = &["title"];
/// 发现页 action 级 identity 字段。
pub const DISCOVERY_ACTION_IDENTITY_FIELDS: &[&str] = &["title", "url", "href", "key"];
/// 发现页媒体卡片 identity 字段。
pub const DISCOVERY_MEDIA_IDENTITY_FIELDS: &[&str] = &["source_item_id", "url", "title", "name"];
/// 媒体主体 identity 字段。
pub const ITEM_IDENTITY_FIELDS: &[&str] = &["source_item_id", "url", "book_url", "title", "name"];
/// 媒体主体标题候选键。
pub const ITEM_TITLE_KEYS: &[&str] = &["title", "name"];
/// 媒体主体 URL 候选键。
pub const ITEM_URL_KEYS: &[&str] = &["url", "book_url", "source_url"];
/// 媒体主体创建者候选键。
pub const ITEM_CREATOR_KEYS: &[&str] = &["author", "creator"];
/// 媒体主体封面候选键。
pub const ITEM_COVER_KEYS: &[&str] = &["cover_url", "cover"];
/// 媒体主体简介候选键。
pub const ITEM_DESCRIPTION_KEYS: &[&str] = &["description", "intro"];
/// 媒体主体来源键候选。
pub const ITEM_SOURCE_KEY_KEYS: &[&str] = &["source_item_id", "url", "book_url", "title", "name"];
/// 单元 identity 字段。
pub const UNIT_IDENTITY_FIELDS: &[&str] = &["url", "href", "source_unit_id", "title"];
/// 单元标题候选键。
pub const UNIT_TITLE_KEYS: &[&str] = &["title", "name"];
/// 单元定位候选键。
pub const UNIT_LOCATOR_KEYS: &[&str] =
    &["source_unit_id", "chapter_url", "chapterUrl", "url", "href"];
/// 资产 identity 字段。
pub const ASSET_IDENTITY_FIELDS: &[&str] = &["url", "href", "content", "text", "title"];
/// 资产正文候选键。
pub const ASSET_CONTENT_KEYS: &[&str] = &["content", "text", "title"];
/// 资产 URL 候选键。
pub const ASSET_URL_KEYS: &[&str] = &["url", "href"];
/// 播放地址候选键。
pub const PLAY_URL_KEYS: &[&str] = &["play_url", "playUrl"];
/// 播放线路候选键。
pub const PLAY_FROM_KEYS: &[&str] = &["play_from", "playFrom"];
/// 正文/详情模板变量 `bookUrl`。
pub const BOOK_URL_TEMPLATE_VAR: &str = "bookUrl";
/// 正文模板变量 `chapterUrl`。
pub const CHAPTER_URL_TEMPLATE_VAR: &str = "chapterUrl";
