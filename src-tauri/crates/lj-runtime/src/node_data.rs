//! Plan effect adapter 的类型化中间数据。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// HTTP 响应(不绑 reqwest)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpResponse {
    /// HTTP 状态码。
    pub status: u16,
    /// 响应头。
    pub headers: HashMap<String, String>,
    /// 响应体字节。
    pub body: Vec<u8>,
    /// 字符集(如 "utf-8", "gbk")。
    pub charset: Option<String>,
}

/// Plan effect adapter 内部传递的轻量数据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeData {
    /// 原始文本或脚本中间值。
    Raw(String),
    /// HTTP 响应。
    HttpResponse(HttpResponse),
    /// 结构化中间数据。
    Json(serde_json::Value),
    /// adapter 内部的安全错误消息。
    Error(String),
}
