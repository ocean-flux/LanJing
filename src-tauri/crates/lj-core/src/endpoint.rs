//! HTTP 请求 spec — `HttpMethod` + `HttpSpec`。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// HTTP 方法。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    /// GET 请求。
    Get,
    /// POST 请求。
    Post,
}

/// HTTP spec。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpSpec {
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
