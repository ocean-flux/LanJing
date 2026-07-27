//! Legado authoring catalog、limits 与正式 JSON 生成器的唯一 Rust owner。
//!
//! 常规生成与 drift check 只使用本文件中的冻结合同，不读取 `.tmp`。只有显式
//! `--audit-upstream` 命令会读取固定 commit 的只读上游 checkout，并确认 Kotlin model、
//! rule data class 与 Web 编辑字段仍和 catalog 完全对齐。

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use lj_rule_model::{DiagnosticSeverity, SupportClass};
use serde::Serialize;
use serde_json::{Map, Value, json};

/// field catalog schema 版本。
pub const FIELD_CATALOG_SCHEMA_VERSION: u32 = 1;
/// generator 实现版本。
pub const GENERATOR_VERSION: u32 = 1;
/// JSON Schema 的项目内固定版本 URI。
pub const SOURCE_SCHEMA_ID: &str = "lanjing://schemas/sources/legado/source.schema.v1.json";
/// 只读上游审计 commit。
pub const UPSTREAM_COMMIT: &str = "0486da3c0255bb5b2d13427c7c4cb829ac37dd0d";

const CATALOG_PATH: &str = "schemas/sources/legado/field-catalog.v1.json";
const SCHEMA_PATH: &str = "schemas/sources/legado/source.schema.v1.json";
const DIAGNOSTIC_FIXTURE_PATH: &str = "schemas/sources/legado/fixtures/diagnostic-parity.v1.json";
const AUDIT_MANIFEST_PATH: &str = "schemas/sources/legado/audit-manifest.v1.json";

/// 后端最终权威的 authoring limits；TypeScript 只能读取生成 catalog 的 `limits`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AuthoringLimits {
    /// 原文 UTF-8 字节上限。
    pub max_utf8_bytes: usize,
    /// 根值深度为 1 的最大 JSON 深度。
    pub max_depth: usize,
    /// JSON value 节点总数上限；property key 不计节点。
    pub max_nodes: usize,
    /// 整文档 object member 出现次数上限，duplicate 也计数。
    pub max_properties: usize,
    /// decoded property name 的 UTF-8 字节上限。
    pub max_property_name_utf8_bytes: usize,
    /// decoded JSON string value 的 UTF-8 字节上限。
    pub max_string_utf8_bytes: usize,
}

/// v1 冻结 limits。
pub const AUTHORING_LIMITS: AuthoringLimits = AuthoringLimits {
    max_utf8_bytes: 2_097_152,
    max_depth: 64,
    max_nodes: 100_000,
    max_properties: 32_768,
    max_property_name_utf8_bytes: 1_024,
    max_string_utf8_bytes: 262_144,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CatalogFieldType {
    String,
    Integer,
    IntegerOrString,
    Boolean,
    ObjectOrString,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum FieldSensitivity {
    None,
    CredentialContainer,
    SensitiveUrl,
}

/// generator 与 validator 共用的已知字段定义。
#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct FieldSpec {
    pub(crate) pointer: &'static str,
    group: &'static str,
    #[serde(rename = "type")]
    pub(crate) field_type: CatalogFieldType,
    pub(crate) required: bool,
    hint: &'static str,
    pub(crate) rule_result_type: Option<&'static str>,
    sensitivity: FieldSensitivity,
    pub(crate) support: SupportClass,
    importer_owner: &'static str,
    runtime_owner: &'static str,
}

macro_rules! field {
    ($pointer:literal, $group:literal, $type:ident, $required:literal, $hint:literal,
     $result:expr, $sensitivity:ident, $support:ident, $importer:expr, $runtime:expr) => {
        FieldSpec {
            pointer: $pointer,
            group: $group,
            field_type: CatalogFieldType::$type,
            required: $required,
            hint: $hint,
            rule_result_type: $result,
            sensitivity: FieldSensitivity::$sensitivity,
            support: SupportClass::$support,
            importer_owner: $importer,
            runtime_owner: $runtime,
        }
    };
}

const IMPORTER_AUTHORING: &str = "lj-importer::authoring::document";
const IMPORTER_TRANSLATOR: &str = "lj-importer::legado::translator";
const RUNTIME_BLOCKED: &str = "blocked";
const RUNTIME_PRESERVED: &str = "not_executable";
const RUNTIME_EXTRACT: &str = "lj-node-extract::ExtractEffectAdapter";

pub(crate) static FIELD_SPECS: &[FieldSpec] = &[
    field!(
        "/bookSourceType",
        "basic",
        Integer,
        true,
        "源类型：0 文本、1 音频、2 图片、3 文件；v1 仅执行文本源",
        None,
        None,
        Executable,
        IMPORTER_AUTHORING,
        "lj-media::SourceProfile"
    ),
    field!(
        "/bookSourceUrl",
        "basic",
        String,
        true,
        "来源基础 URL，必须是无 credential query 的 http/https 地址",
        None,
        SensitiveUrl,
        Executable,
        IMPORTER_TRANSLATOR,
        "lj-node-http::HttpEffectAdapter"
    ),
    field!(
        "/bookSourceName",
        "basic",
        String,
        true,
        "显示在来源列表中的名称",
        None,
        None,
        Executable,
        "lj-importer::legado::types",
        "lj-media::SourceProfile"
    ),
    field!(
        "/bookSourceGroup",
        "basic",
        String,
        false,
        "来源分组与展示标签",
        None,
        None,
        Executable,
        "lj-importer::legado::types",
        "lj-media::SourceProfile"
    ),
    field!(
        "/bookSourceComment",
        "basic",
        String,
        false,
        "来源作者、状态与说明",
        None,
        None,
        Preserved,
        IMPORTER_AUTHORING,
        RUNTIME_PRESERVED
    ),
    field!(
        "/loginUrl",
        "basic",
        String,
        false,
        "上游登录地址；本版本不提供登录或 WebView",
        None,
        SensitiveUrl,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/loginUi",
        "basic",
        String,
        false,
        "上游自定义登录界面",
        None,
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/loginCheckJs",
        "basic",
        String,
        false,
        "上游登录检测 JavaScript",
        None,
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/coverDecodeJs",
        "basic",
        String,
        false,
        "上游封面解密 JavaScript",
        None,
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/bookUrlPattern",
        "basic",
        String,
        false,
        "详情 URL 的来源专属校验正则",
        None,
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/header",
        "basic",
        String,
        false,
        "JSON string header map；credential 由 slot codec 分离",
        None,
        CredentialContainer,
        Executable,
        IMPORTER_TRANSLATOR,
        "lj-node-http::HttpEffectAdapter"
    ),
    field!(
        "/variableComment",
        "basic",
        String,
        false,
        "来源变量说明",
        None,
        None,
        Preserved,
        IMPORTER_AUTHORING,
        RUNTIME_PRESERVED
    ),
    field!(
        "/concurrentRate",
        "basic",
        String,
        false,
        "上游请求并发率与访问间隔",
        None,
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/jsLib",
        "basic",
        String,
        false,
        "上游 JavaScript 库与远程脚本加载",
        None,
        SensitiveUrl,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/customOrder",
        "preferences",
        Integer,
        false,
        "客户端手动排序编号",
        None,
        None,
        Preserved,
        IMPORTER_AUTHORING,
        RUNTIME_PRESERVED
    ),
    field!(
        "/enabled",
        "preferences",
        Boolean,
        false,
        "客户端启用偏好",
        None,
        None,
        Preserved,
        IMPORTER_AUTHORING,
        RUNTIME_PRESERVED
    ),
    field!(
        "/enabledExplore",
        "preferences",
        Boolean,
        false,
        "客户端发现页启用偏好",
        None,
        None,
        Preserved,
        IMPORTER_AUTHORING,
        RUNTIME_PRESERVED
    ),
    field!(
        "/enabledCookieJar",
        "preferences",
        Boolean,
        false,
        "上游 CookieJar 自动持久化",
        None,
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/lastUpdateTime",
        "preferences",
        IntegerOrString,
        false,
        "上游客户端排序用更新时间；兼容 Long 与历史字符串表示",
        None,
        None,
        Preserved,
        IMPORTER_AUTHORING,
        RUNTIME_PRESERVED
    ),
    field!(
        "/respondTime",
        "preferences",
        Integer,
        false,
        "上游客户端排序用响应耗时",
        None,
        None,
        Preserved,
        IMPORTER_AUTHORING,
        RUNTIME_PRESERVED
    ),
    field!(
        "/weight",
        "preferences",
        Integer,
        false,
        "上游搜索排序权重",
        None,
        None,
        Preserved,
        IMPORTER_AUTHORING,
        RUNTIME_PRESERVED
    ),
    field!(
        "/searchUrl",
        "search",
        String,
        false,
        "搜索 URL 模板，可使用 {{key}} 与 {{page}}",
        None,
        SensitiveUrl,
        Executable,
        IMPORTER_TRANSLATOR,
        "lj-node-http::HttpEffectAdapter"
    ),
    field!(
        "/ruleSearch",
        "search",
        ObjectOrString,
        false,
        "搜索规则对象；上游 compact string 表示在 v1 阻断",
        None,
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleSearch/checkKeyWord",
        "search",
        String,
        false,
        "搜索结果关键字校验规则",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleSearch/bookList",
        "search",
        String,
        false,
        "选择搜索结果列表节点",
        Some("list_element"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleSearch/name",
        "search",
        String,
        false,
        "选择节点书名",
        Some("string"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleSearch/author",
        "search",
        String,
        false,
        "选择节点作者",
        Some("string"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleSearch/kind",
        "search",
        String,
        false,
        "选择节点分类信息",
        Some("string"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleSearch/wordCount",
        "search",
        String,
        false,
        "选择节点字数信息",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleSearch/lastChapter",
        "search",
        String,
        false,
        "选择节点最新章节",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleSearch/updateTime",
        "search",
        String,
        false,
        "选择节点更新时间",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleSearch/intro",
        "search",
        String,
        false,
        "选择节点简介",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleSearch/coverUrl",
        "search",
        String,
        false,
        "选择节点封面 URL",
        Some("string_url"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleSearch/bookUrl",
        "search",
        String,
        false,
        "选择详情页 URL",
        Some("string_url"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/exploreUrl",
        "explore",
        String,
        false,
        "发现入口；当前仅进入既有受限 QuickJS effect",
        None,
        SensitiveUrl,
        Executable,
        IMPORTER_TRANSLATOR,
        "lj-node-js::QuickJsEffectAdapter"
    ),
    field!(
        "/exploreScreen",
        "explore",
        String,
        false,
        "上游发现筛选规则",
        None,
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleExplore",
        "explore",
        ObjectOrString,
        false,
        "发现列表规则对象；compact string 在 v1 阻断",
        None,
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleExplore/bookList",
        "explore",
        String,
        false,
        "选择发现结果列表节点",
        Some("list_element"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleExplore/name",
        "explore",
        String,
        false,
        "选择节点书名",
        Some("string"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleExplore/author",
        "explore",
        String,
        false,
        "选择节点作者",
        Some("string"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleExplore/intro",
        "explore",
        String,
        false,
        "选择节点简介",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleExplore/kind",
        "explore",
        String,
        false,
        "选择节点分类信息",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleExplore/lastChapter",
        "explore",
        String,
        false,
        "选择节点最新章节",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleExplore/updateTime",
        "explore",
        String,
        false,
        "选择节点更新时间",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleExplore/bookUrl",
        "explore",
        String,
        false,
        "选择详情页 URL",
        Some("string_url"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleExplore/coverUrl",
        "explore",
        String,
        false,
        "选择封面 URL",
        Some("string_url"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleExplore/wordCount",
        "explore",
        String,
        false,
        "选择节点字数信息",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleBookInfo",
        "book_info",
        ObjectOrString,
        false,
        "详情规则对象；compact string 在 v1 阻断",
        None,
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleBookInfo/init",
        "book_info",
        String,
        false,
        "AllInOne 详情预处理规则",
        Some("element"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleBookInfo/name",
        "book_info",
        String,
        false,
        "选择书名",
        Some("string"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleBookInfo/author",
        "book_info",
        String,
        false,
        "选择作者",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleBookInfo/kind",
        "book_info",
        String,
        false,
        "选择分类信息",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleBookInfo/wordCount",
        "book_info",
        String,
        false,
        "选择字数信息",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleBookInfo/lastChapter",
        "book_info",
        String,
        false,
        "选择最新章节",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleBookInfo/updateTime",
        "book_info",
        String,
        false,
        "选择更新时间",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleBookInfo/intro",
        "book_info",
        String,
        false,
        "选择简介",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleBookInfo/coverUrl",
        "book_info",
        String,
        false,
        "选择封面 URL",
        Some("string_url"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleBookInfo/tocUrl",
        "book_info",
        String,
        false,
        "选择目录 URL",
        Some("string_url"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleBookInfo/canReName",
        "book_info",
        String,
        false,
        "允许规则修改书名或作者",
        Some("boolean"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleBookInfo/downloadUrls",
        "book_info",
        String,
        false,
        "文件类来源下载 URL",
        Some("string_or_list"),
        SensitiveUrl,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleToc",
        "toc",
        ObjectOrString,
        false,
        "目录规则对象；compact string 在 v1 阻断",
        None,
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleToc/preUpdateJs",
        "toc",
        String,
        false,
        "更新目录前执行 JavaScript",
        Some("string_url"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleToc/chapterList",
        "toc",
        String,
        false,
        "选择章节列表节点",
        Some("list_element"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleToc/chapterName",
        "toc",
        String,
        false,
        "选择章节名称",
        Some("string"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleToc/chapterUrl",
        "toc",
        String,
        false,
        "选择章节 URL",
        Some("string_url"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleToc/formatJs",
        "toc",
        String,
        false,
        "逐章标题处理 JavaScript",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleToc/isVolume",
        "toc",
        String,
        false,
        "章节卷名标识",
        Some("boolean"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleToc/updateTime",
        "toc",
        String,
        false,
        "选择章节更新时间",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleToc/isVip",
        "toc",
        String,
        false,
        "章节收费标识",
        Some("boolean"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleToc/isPay",
        "toc",
        String,
        false,
        "章节已购买标识",
        Some("boolean"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleToc/nextTocUrl",
        "toc",
        String,
        false,
        "目录下一页 URL",
        Some("string_or_list"),
        SensitiveUrl,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleContent",
        "content",
        ObjectOrString,
        false,
        "正文规则对象；compact string 在 v1 阻断",
        None,
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleContent/content",
        "content",
        String,
        false,
        "选择正文内容",
        Some("string"),
        None,
        Executable,
        IMPORTER_TRANSLATOR,
        RUNTIME_EXTRACT
    ),
    field!(
        "/ruleContent/title",
        "content",
        String,
        false,
        "从正文覆盖章节标题",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleContent/nextContentUrl",
        "content",
        String,
        false,
        "正文下一分页 URL",
        Some("string_url"),
        SensitiveUrl,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleContent/webJs",
        "content",
        String,
        false,
        "WebView JavaScript 注入",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleContent/sourceRegex",
        "content",
        String,
        false,
        "资源嗅探 URL 正则",
        Some("string"),
        SensitiveUrl,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleContent/replaceRegex",
        "content",
        String,
        false,
        "正文净化替换规则",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleContent/imageStyle",
        "content",
        String,
        false,
        "上游客户端图片展示样式",
        None,
        None,
        Preserved,
        IMPORTER_AUTHORING,
        RUNTIME_PRESERVED
    ),
    field!(
        "/ruleContent/imageDecode",
        "content",
        String,
        false,
        "图片 bytes 解密 JavaScript",
        Some("bytes"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleContent/payAction",
        "content",
        String,
        false,
        "购买链接或支付接口 JavaScript",
        Some("string_url"),
        SensitiveUrl,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleReview",
        "review",
        ObjectOrString,
        false,
        "段评与交互规则；v1 不执行",
        None,
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleReview/reviewUrl",
        "review",
        String,
        false,
        "段评 URL",
        Some("string_url"),
        SensitiveUrl,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleReview/avatarRule",
        "review",
        String,
        false,
        "段评发布者头像规则",
        Some("string_url"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleReview/contentRule",
        "review",
        String,
        false,
        "段评内容规则",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleReview/postTimeRule",
        "review",
        String,
        false,
        "段评发布时间规则",
        Some("string"),
        None,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleReview/reviewQuoteUrl",
        "review",
        String,
        false,
        "段评回复 URL",
        Some("string_url"),
        SensitiveUrl,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleReview/voteUpUrl",
        "review",
        String,
        false,
        "段评点赞 URL",
        Some("string_url"),
        SensitiveUrl,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleReview/voteDownUrl",
        "review",
        String,
        false,
        "段评点踩 URL",
        Some("string_url"),
        SensitiveUrl,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleReview/postReviewUrl",
        "review",
        String,
        false,
        "发布段评 URL",
        Some("string_url"),
        SensitiveUrl,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleReview/postQuoteUrl",
        "review",
        String,
        false,
        "发布段评回复 URL",
        Some("string_url"),
        SensitiveUrl,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
    field!(
        "/ruleReview/deleteUrl",
        "review",
        String,
        false,
        "删除段评 URL",
        Some("string_url"),
        SensitiveUrl,
        Blocked,
        IMPORTER_AUTHORING,
        RUNTIME_BLOCKED
    ),
];

/// 按 RFC 6901 pointer 查询已知字段。
#[must_use]
pub(crate) fn field_for_pointer(pointer: &str) -> Option<&'static FieldSpec> {
    FIELD_SPECS.iter().find(|field| field.pointer == pointer)
}

#[derive(Serialize)]
struct FieldCatalogDocument {
    schema_version: u32,
    generator_version: u32,
    limits: AuthoringLimits,
    fields: &'static [FieldSpec],
}

#[derive(Debug, Clone)]
pub struct GeneratedArtifact {
    /// 项目根下的 canonical 路径。
    pub path: &'static str,
    /// 带末尾换行的确定性 JSON。
    pub content: String,
}

/// generator 或 drift check 的安全失败。
#[derive(Debug)]
pub struct CatalogGenerationError {
    message: String,
}

impl CatalogGenerationError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for CatalogGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CatalogGenerationError {}

/// 生成 catalog、schema、diagnostic fixture 与 audit manifest 的完整 bundle。
///
/// # Errors
///
/// 内部 DTO 无法序列化为 JSON 时返回 [`CatalogGenerationError`]。
pub fn generated_artifacts() -> Result<Vec<GeneratedArtifact>, CatalogGenerationError> {
    let catalog = pretty_json(&FieldCatalogDocument {
        schema_version: FIELD_CATALOG_SCHEMA_VERSION,
        generator_version: GENERATOR_VERSION,
        limits: AUTHORING_LIMITS,
        fields: FIELD_SPECS,
    })?;
    let schema = pretty_json(&source_schema())?;
    let fixture = pretty_json(&diagnostic_fixture())?;
    let manifest = pretty_json(&audit_manifest(&catalog, &schema, &fixture))?;
    Ok(vec![
        GeneratedArtifact {
            path: CATALOG_PATH,
            content: catalog,
        },
        GeneratedArtifact {
            path: SCHEMA_PATH,
            content: schema,
        },
        GeneratedArtifact {
            path: DIAGNOSTIC_FIXTURE_PATH,
            content: fixture,
        },
        GeneratedArtifact {
            path: AUDIT_MANIFEST_PATH,
            content: manifest,
        },
    ])
}

/// 把确定性 bundle 写入项目正式路径。
///
/// # Errors
///
/// JSON 生成、目录创建或文件写入失败时返回 [`CatalogGenerationError`]。
pub fn write_generated_artifacts(project_root: &Path) -> Result<(), CatalogGenerationError> {
    for artifact in generated_artifacts()? {
        let path = project_root.join(artifact.path);
        let parent = path
            .parent()
            .ok_or_else(|| CatalogGenerationError::new("生成路径缺少父目录"))?;
        fs::create_dir_all(parent).map_err(|_| CatalogGenerationError::new("无法创建生成目录"))?;
        fs::write(path, artifact.content)
            .map_err(|_| CatalogGenerationError::new("无法写入生成物"))?;
    }
    Ok(())
}

/// 比较正式 JSON 与内存重生成结果，检测任何手改或 generator drift。
///
/// # Errors
///
/// 文件缺失、读取失败或 bytes 与 generator 输出不同时返回 [`CatalogGenerationError`]。
pub fn check_generated_artifacts(project_root: &Path) -> Result<(), CatalogGenerationError> {
    for artifact in generated_artifacts()? {
        let actual = fs::read(project_root.join(artifact.path))
            .map_err(|_| CatalogGenerationError::new(format!("生成物缺失: {}", artifact.path)))?;
        if actual != artifact.content.as_bytes() {
            return Err(CatalogGenerationError::new(format!(
                "生成物漂移: {}",
                artifact.path
            )));
        }
    }
    Ok(())
}

/// 显式审计固定 upstream checkout；常规 generator/drift test 不调用本函数。
///
/// # Errors
///
/// checkout commit 不匹配、存在 tracked 修改、审计输入缺失，Kotlin model/rule data class
/// 与 catalog 不一致，或 Web 编辑配置出现 catalog 未知字段时返回 [`CatalogGenerationError`]。
pub fn audit_upstream(upstream_root: &Path) -> Result<(), CatalogGenerationError> {
    let commit = git_output(upstream_root, &["rev-parse", "HEAD"])?;
    if commit.trim() != UPSTREAM_COMMIT {
        return Err(CatalogGenerationError::new("Legado upstream commit 不匹配"));
    }
    let status = git_output(
        upstream_root,
        &["status", "--porcelain", "--untracked-files=no"],
    )?;
    if !status.trim().is_empty() {
        return Err(CatalogGenerationError::new(
            "Legado upstream checkout 含 tracked 修改",
        ));
    }
    for input in AUDIT_INPUTS {
        if !upstream_root.join(input.path).is_file() {
            return Err(CatalogGenerationError::new(format!(
                "Legado 审计输入缺失: {}",
                input.path
            )));
        }
    }
    let mut model_pointers = BTreeSet::new();
    for (input_index, namespace) in [
        (0, None),
        (2, Some("ruleSearch")),
        (3, Some("ruleExplore")),
        (4, Some("ruleBookInfo")),
        (5, Some("ruleToc")),
        (6, Some("ruleContent")),
        (7, Some("ruleReview")),
    ] {
        let input = &AUDIT_INPUTS[input_index];
        let source = fs::read_to_string(upstream_root.join(input.path)).map_err(|_| {
            CatalogGenerationError::new(format!("无法读取 Legado Kotlin 审计输入: {}", input.path))
        })?;
        let pointers = kotlin_model_pointers(&source, namespace);
        if pointers.is_empty() {
            return Err(CatalogGenerationError::new(format!(
                "Legado Kotlin 审计未识别字段: {}",
                input.path
            )));
        }
        model_pointers.extend(pointers);
    }
    let catalog_pointers = FIELD_SPECS
        .iter()
        .map(|field| field.pointer.to_string())
        .collect::<BTreeSet<_>>();
    if let Some(pointer) = model_pointers.difference(&catalog_pointers).next() {
        return Err(CatalogGenerationError::new(format!(
            "Legado Kotlin 字段未进入 catalog: {pointer}"
        )));
    }
    if let Some(pointer) = catalog_pointers.difference(&model_pointers).next() {
        return Err(CatalogGenerationError::new(format!(
            "catalog 字段已不在 Legado Kotlin model: {pointer}"
        )));
    }
    let config = fs::read_to_string(upstream_root.join(AUDIT_INPUTS[1].path))
        .map_err(|_| CatalogGenerationError::new("无法读取 Legado Web 编辑配置"))?;
    for pointer in edit_config_pointers(&config) {
        if field_for_pointer(&pointer).is_none() {
            return Err(CatalogGenerationError::new(format!(
                "Legado Web 编辑字段未进入 catalog: {pointer}"
            )));
        }
    }
    Ok(())
}

fn pretty_json<T: Serialize>(value: &T) -> Result<String, CatalogGenerationError> {
    serde_json::to_string_pretty(value)
        .map(|mut json| {
            json.push('\n');
            json
        })
        .map_err(|_| CatalogGenerationError::new("authoring JSON 生成失败"))
}

fn source_schema() -> Value {
    let mut properties = Map::new();
    for field in FIELD_SPECS
        .iter()
        .filter(|field| pointer_depth(field.pointer) == 1)
    {
        properties.insert(pointer_tail(field.pointer).to_string(), schema_for(field));
    }
    for field in FIELD_SPECS
        .iter()
        .filter(|field| pointer_depth(field.pointer) == 2)
    {
        let mut segments = field.pointer[1..].split('/');
        let parent = segments.next().expect("depth checked");
        let child = segments.next().expect("depth checked");
        if let Some(parent_schema) = properties.get_mut(parent)
            && let Some(child_properties) = schema_object_properties_mut(parent_schema)
        {
            child_properties.insert(child.to_string(), schema_for(field));
        }
    }
    let required = FIELD_SPECS
        .iter()
        .filter(|field| field.required && pointer_depth(field.pointer) == 1)
        .map(|field| Value::String(pointer_tail(field.pointer).to_string()))
        .collect::<Vec<_>>();
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": SOURCE_SCHEMA_ID,
        "title": "LanJing Legado Source Authoring Schema v1",
        "type": "object",
        "x-lanjing-schema-version": FIELD_CATALOG_SCHEMA_VERSION,
        "x-lanjing-generator-version": GENERATOR_VERSION,
        "x-lanjing-limits": AUTHORING_LIMITS,
        "properties": properties,
        "required": required,
        "additionalProperties": true
    })
}

fn schema_for(field: &FieldSpec) -> Value {
    let base = match field.field_type {
        CatalogFieldType::String => json!({
            "type": "string",
            "maxLength": AUTHORING_LIMITS.max_string_utf8_bytes,
            "x-lanjing-max-utf8-bytes": AUTHORING_LIMITS.max_string_utf8_bytes
        }),
        CatalogFieldType::Integer => json!({ "type": "integer" }),
        CatalogFieldType::IntegerOrString => json!({
            "oneOf": [{ "type": "integer" }, { "type": "string", "pattern": "^-?[0-9]+$" }]
        }),
        CatalogFieldType::Boolean => json!({ "type": "boolean" }),
        CatalogFieldType::ObjectOrString => json!({
            "oneOf": [
                { "type": "object", "properties": {}, "additionalProperties": true },
                {
                    "type": "string",
                    "maxLength": AUTHORING_LIMITS.max_string_utf8_bytes,
                    "x-lanjing-max-utf8-bytes": AUTHORING_LIMITS.max_string_utf8_bytes
                }
            ],
            "x-lanjing-string-support": "blocked"
        }),
    };
    let base = if field.required {
        base
    } else {
        json!({ "oneOf": [base, { "type": "null" }] })
    };
    let mut object = base.as_object().cloned().unwrap_or_default();
    object.insert(
        "description".to_string(),
        Value::String(field.hint.to_string()),
    );
    object.insert(
        "x-lanjing-group".to_string(),
        Value::String(field.group.to_string()),
    );
    object.insert("x-lanjing-support".to_string(), json!(field.support));
    object.insert(
        "x-lanjing-sensitivity".to_string(),
        json!(field.sensitivity),
    );
    object.insert(
        "x-lanjing-importer-owner".to_string(),
        Value::String(field.importer_owner.to_string()),
    );
    object.insert(
        "x-lanjing-runtime-owner".to_string(),
        Value::String(field.runtime_owner.to_string()),
    );
    if let Some(result_type) = field.rule_result_type {
        object.insert(
            "x-lanjing-rule-result-type".to_string(),
            Value::String(result_type.to_string()),
        );
    }
    Value::Object(object)
}

fn schema_object_properties_mut(schema: &mut Value) -> Option<&mut Map<String, Value>> {
    let variants = schema.as_object_mut()?.get_mut("oneOf")?.as_array_mut()?;
    variants
        .first_mut()?
        .as_object_mut()?
        .get_mut("properties")?
        .as_object_mut()
}

fn pointer_depth(pointer: &str) -> usize {
    pointer.bytes().filter(|byte| *byte == b'/').count()
}

fn pointer_tail(pointer: &str) -> &str {
    pointer.rsplit('/').next().unwrap_or(pointer)
}

fn diagnostic_fixture() -> Value {
    let invalid_json =
        r#"{"bookSourceType":0,"bookSourceUrl":"https://example.test","bookSourceName":"源",}"#;
    let invalid_root = r#"["源"]"#;
    let executable = r#"{"bookSourceType":0,"bookSourceUrl":"https://example.test","bookSourceName":"源","searchUrl":"/search?q={{key}}","ruleSearch":{"name":"h1@text"}}"#;
    let duplicate = r#"{"bookSourceType":0,"bookSourceUrl":"https://example.test","bookSourceName":"源","ruleSearch":{"name":"甲","name":"乙"}}"#;
    let preserved = r#"{"bookSourceType":0,"bookSourceUrl":"https://example.test","bookSourceName":"源","bookSourceComment":"备注"}"#;
    let blocked = r#"{"bookSourceType":0,"bookSourceUrl":"https://example.test","bookSourceName":"源","loginUrl":"https://example.test/login"}"#;
    let unknown = r#"{"bookSourceType":0,"bookSourceUrl":"https://example.test","bookSourceName":"源","未知/键~":"值"}"#;
    let invalid_sentinel = r#"{"bookSourceType":0,"bookSourceUrl":"https://example.test","bookSourceName":"源","header":"__LANJING_CREDENTIAL_SLOT_V1__:not-a-uuid"}"#;
    let duplicate_sensitive = r#"{"bookSourceType":0,"bookSourceUrl":"https://example.test","bookSourceName":"源","header":"{\"Authorization\":\"一\",\"authorization\":\"二\"}"}"#;
    let blocked_request_header = r#"{"bookSourceType":0,"bookSourceUrl":"https://example.test","bookSourceName":"源","header":"{\"Proxy-Authorization\":\"secret\"}"}"#;

    let duplicate_span = nth_span(duplicate, "\"name\"", 2);
    let preserved_span = nth_span(preserved, "\"bookSourceComment\"", 1);
    let blocked_span = nth_span(blocked, "\"loginUrl\"", 1);
    let unknown_span = nth_span(unknown, "\"未知/键~\"", 1);
    let invalid_sentinel_span = value_span(
        invalid_sentinel,
        "__LANJING_CREDENTIAL_SLOT_V1__:not-a-uuid",
    );
    let duplicate_sensitive_span = property_value_token_span(duplicate_sensitive, "\"header\"");
    let blocked_request_span = property_value_token_span(blocked_request_header, "\"header\"");

    json!({
        "fixture_version": 1,
        "cases": [
            {
                "name": "invalid_json_trailing_comma",
                "scope": "document",
                "text": invalid_json,
                "diagnostics": [diagnostic_json(DiagnosticSeverity::Error, "invalid_json", "", invalid_json.len() - 1, 1, SupportClass::Blocked)]
            },
            {
                "name": "invalid_root_array",
                "scope": "document",
                "text": invalid_root,
                "diagnostics": [diagnostic_json(DiagnosticSeverity::Error, "invalid_root", "", 0, invalid_root.len(), SupportClass::Blocked)]
            },
            {
                "name": "known_executable_fields",
                "scope": "document",
                "text": executable,
                "diagnostics": []
            },
            {
                "name": "duplicate_key_uses_second_utf8_key_token",
                "scope": "document",
                "text": duplicate,
                "diagnostics": [diagnostic_json(DiagnosticSeverity::Error, "duplicate_key", "/ruleSearch/name", duplicate_span.0, duplicate_span.1, SupportClass::Blocked)]
            },
            {
                "name": "known_preserved_field",
                "scope": "document",
                "text": preserved,
                "diagnostics": [diagnostic_json(DiagnosticSeverity::Info, "known_field_preserved", "/bookSourceComment", preserved_span.0, preserved_span.1, SupportClass::Preserved)]
            },
            {
                "name": "known_blocked_field",
                "scope": "document",
                "text": blocked,
                "diagnostics": [diagnostic_json(DiagnosticSeverity::Error, "known_field_blocked", "/loginUrl", blocked_span.0, blocked_span.1, SupportClass::Blocked)]
            },
            {
                "name": "unknown_field_escapes_rfc6901_pointer",
                "scope": "document",
                "text": unknown,
                "diagnostics": [diagnostic_json(DiagnosticSeverity::Warning, "unknown_field", "/未知~1键~0", unknown_span.0, unknown_span.1, SupportClass::Unknown)]
            },
            {
                "name": "invalid_credential_sentinel",
                "scope": "document",
                "text": invalid_sentinel,
                "diagnostics": [diagnostic_json(DiagnosticSeverity::Error, "credential_sentinel_invalid", "/header", invalid_sentinel_span.0, invalid_sentinel_span.1, SupportClass::Blocked)]
            },
            {
                "name": "duplicate_sensitive_header_is_normalized",
                "scope": "credential_adapter",
                "text": duplicate_sensitive,
                "diagnostics": [diagnostic_json(DiagnosticSeverity::Error, "credential_duplicate_sensitive_key", "/header", duplicate_sensitive_span.0, duplicate_sensitive_span.1, SupportClass::Blocked)]
            },
            {
                "name": "proxy_authorization_request_header_is_blocked",
                "scope": "credential_adapter",
                "text": blocked_request_header,
                "diagnostics": [diagnostic_json(DiagnosticSeverity::Error, "credential_request_header_blocked", "/header", blocked_request_span.0, blocked_request_span.1, SupportClass::Blocked)]
            }
        ]
    })
}

fn diagnostic_json(
    severity: DiagnosticSeverity,
    code: &str,
    path: &str,
    byte_offset: usize,
    byte_length: usize,
    support: SupportClass,
) -> Value {
    json!({
        "severity": severity,
        "code": code,
        "path": path,
        "byte_offset": byte_offset,
        "byte_length": byte_length,
        "support": support
    })
}

fn nth_span(text: &str, needle: &str, occurrence: usize) -> (usize, usize) {
    let mut start = 0;
    for index in 1..=occurrence {
        let offset = text[start..]
            .find(needle)
            .expect("fixture needle must exist")
            + start;
        if index == occurrence {
            return (offset, needle.len());
        }
        start = offset + needle.len();
    }
    unreachable!("occurrence is non-zero")
}

fn value_span(text: &str, decoded_value: &str) -> (usize, usize) {
    let raw = format!("\"{decoded_value}\"");
    nth_span(text, &raw, 1)
}

fn property_value_token_span(text: &str, property: &str) -> (usize, usize) {
    let property_offset = text.find(property).expect("fixture property must exist");
    let after_property = property_offset + property.len();
    let colon = text[after_property..]
        .find(':')
        .expect("fixture property must have value")
        + after_property;
    let value_offset = colon + 1;
    let bytes = text.as_bytes();
    let mut cursor = value_offset + 1;
    let mut escaped = false;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            return (value_offset, cursor + 1 - value_offset);
        }
        cursor += 1;
    }
    panic!("fixture string must terminate")
}

#[derive(Serialize)]
struct AuditInput {
    path: &'static str,
    purpose: &'static str,
}

const AUDIT_INPUTS: &[AuditInput] = &[
    AuditInput {
        path: "app/src/main/java/io/legado/app/data/entities/BookSource.kt",
        purpose: "BookSource root fields",
    },
    AuditInput {
        path: "modules/web/src/config/bookSourceEditConfig.ts",
        purpose: "authoring groups, types, required flags and hints",
    },
    AuditInput {
        path: "app/src/main/java/io/legado/app/data/entities/rule/SearchRule.kt",
        purpose: "search rule fields",
    },
    AuditInput {
        path: "app/src/main/java/io/legado/app/data/entities/rule/ExploreRule.kt",
        purpose: "explore rule fields",
    },
    AuditInput {
        path: "app/src/main/java/io/legado/app/data/entities/rule/BookInfoRule.kt",
        purpose: "book info rule fields",
    },
    AuditInput {
        path: "app/src/main/java/io/legado/app/data/entities/rule/TocRule.kt",
        purpose: "toc rule fields",
    },
    AuditInput {
        path: "app/src/main/java/io/legado/app/data/entities/rule/ContentRule.kt",
        purpose: "content rule fields",
    },
    AuditInput {
        path: "app/src/main/java/io/legado/app/data/entities/rule/ReviewRule.kt",
        purpose: "review rule fields",
    },
];

fn audit_manifest(catalog: &str, schema: &str, fixture: &str) -> Value {
    json!({
        "manifest_version": 1,
        "generator_version": GENERATOR_VERSION,
        "upstream_repository": "https://github.com/gedoor/legado",
        "upstream_commit": UPSTREAM_COMMIT,
        "audit_inputs": AUDIT_INPUTS,
        "generated_outputs": [
            { "path": CATALOG_PATH, "blake3": blake3_hex(catalog) },
            { "path": SCHEMA_PATH, "blake3": blake3_hex(schema) },
            { "path": DIAGNOSTIC_FIXTURE_PATH, "blake3": blake3_hex(fixture) }
        ]
    })
}

fn blake3_hex(content: &str) -> String {
    blake3::hash(content.as_bytes()).to_hex().to_string()
}

fn git_output(root: &Path, arguments: &[&str]) -> Result<String, CatalogGenerationError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .output()
        .map_err(|_| CatalogGenerationError::new("无法执行 git upstream 审计"))?;
    if !output.status.success() {
        return Err(CatalogGenerationError::new("git upstream 审计失败"));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| CatalogGenerationError::new("git upstream 审计输出不是 UTF-8"))
}

fn kotlin_model_pointers(source: &str, namespace: Option<&str>) -> BTreeSet<String> {
    let mut in_constructor = false;
    let mut pointers = BTreeSet::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if !in_constructor {
            if trimmed.starts_with("data class ") && trimmed.contains('(') {
                in_constructor = true;
            }
            continue;
        }
        if trimmed.starts_with(')') {
            break;
        }
        let Some(variable) = trimmed
            .strip_prefix("var ")
            .or_else(|| trimmed.strip_prefix("override var "))
        else {
            continue;
        };
        let name = variable
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect::<String>();
        if name.is_empty() {
            continue;
        }
        pointers.insert(namespace.map_or_else(
            || format!("/{name}"),
            |namespace| format!("/{namespace}/{name}"),
        ));
    }
    pointers
}

fn edit_config_pointers(config: &str) -> Vec<String> {
    let mut namespace: Option<String> = None;
    let mut pointers = Vec::new();
    for line in config.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        if trimmed.starts_with("namespace:") {
            namespace = quoted_value(trimmed);
            continue;
        }
        if trimmed.starts_with("id:") {
            if let Some(id) = quoted_value(trimmed) {
                pointers.push(namespace.as_ref().map_or_else(
                    || format!("/{id}"),
                    |namespace| format!("/{namespace}/{id}"),
                ));
            }
            namespace = None;
        }
    }
    pointers
}

fn quoted_value(line: &str) -> Option<String> {
    let quote_offset = line.find(['\'', '"'])?;
    let quote = line.as_bytes()[quote_offset];
    let tail = &line[quote_offset + 1..];
    let end = tail.as_bytes().iter().position(|byte| *byte == quote)?;
    Some(tail[..end].to_string())
}

/// 从 importer crate manifest 定位项目根，仅供 generator binary 使用。
#[must_use]
pub fn project_root_from_manifest(manifest_dir: &Path) -> Option<PathBuf> {
    manifest_dir.ancestors().nth(3).map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::kotlin_model_pointers;

    #[test]
    fn kotlin_model_audit_reads_constructor_fields_and_namespace() {
        let source = r#"
            @Parcelize
            data class SearchRule(
                override var bookList: String? = null,
                var newlyAdded: String? = null
            ) : Parcelable {
                var implementationDetail: String = "ignored"
            }
        "#;
        assert_eq!(
            kotlin_model_pointers(source, Some("ruleSearch"))
                .into_iter()
                .collect::<Vec<_>>(),
            vec![
                "/ruleSearch/bookList".to_string(),
                "/ruleSearch/newlyAdded".to_string(),
            ]
        );
    }
}
