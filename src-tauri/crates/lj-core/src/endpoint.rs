//! 端点 spec — 5 端点 struct + HttpSpec(ADR-0007)。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::extract_rule::ExtractSpec;

/// 端点类型(5 variant)。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EndpointKind {
    /// 搜索。
    Search,
    /// 发现/分类浏览。
    Discover,
    /// 详情。
    Detail,
    /// 目录。
    Toc,
    /// 正文。
    Content,
}

/// HTTP 方法。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    /// GET 请求。
    Get,
    /// POST 请求。
    Post,
}

/// HTTP spec(ADR-0009 中等字段集)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpSpec {
    /// 关联的端点类型(用于 tracing span 命名 + 子图裁剪)。
    pub endpoint_kind: EndpointKind,
    /// HTTP 方法。
    pub method: HttpMethod,
    /// URL 模板(含 `{{key}}`/`{{page}}` 变量)。
    pub url: String,
    /// HTTP 请求头。
    pub headers: HashMap<String, String>,
    /// 请求体(POST)。
    pub body: Option<String>,
    /// 字符集。
    pub charset: Option<String>,
    /// 预期响应数据类型(决定 Extract 用哪个解析器)。
    pub expected_type: crate::extract_rule::ExpectedDataType,
}

/// 端点 spec(ADR-0007)。
/// 首刀端点子图模板固定为 Http→Extract，EndpointSpec 暂为组合容器。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EndpointSpec {
    /// 端点类型。
    pub kind: EndpointKind,
    /// HTTP 请求配置。
    pub http: HttpSpec,
    /// 提取规则配置。
    pub extract: ExtractSpec,
}
