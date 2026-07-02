//! 节点数据 — 节点间传递的数据载荷 enum(ADR-0022)。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::media::Media;

/// HTTP 响应(lj-core 自有 struct，不绑 reqwest)。
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

/// 节点间传递的数据载荷(闭集 enum,ADR-0022)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeData {
    /// 原始文本(JS 产出/中间文本)。
    Raw(String),
    /// HTTP 响应。
    HttpResponse(HttpResponse),
    /// 媒体模型。
    Media(Media),
    /// JSON 数据。
    Json(serde_json::Value),
    /// 错误(节点执行失败时产出,替代静默终结 stream)。
    Error(String),
}

/// `NodeData` variant 标识(用于 `NodeProcessor::input_type`/`output_type` 静态声明)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeDataVariant {
    /// 原始文本。
    Raw,
    /// HTTP 响应。
    HttpResponse,
    /// 媒体模型。
    Media,
    /// JSON 数据。
    Json,
    /// 错误。
    Error,
}

impl NodeData {
    /// 返回当前 variant 标识。
    #[must_use]
    pub fn variant(&self) -> NodeDataVariant {
        match self {
            Self::Raw(_) => NodeDataVariant::Raw,
            Self::HttpResponse(_) => NodeDataVariant::HttpResponse,
            Self::Media(_) => NodeDataVariant::Media,
            Self::Json(_) => NodeDataVariant::Json,
            Self::Error(_) => NodeDataVariant::Error,
        }
    }
}
