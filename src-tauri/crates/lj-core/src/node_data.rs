//! 节点数据 — Flow 节点间传递的中间值与资源图增量。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::media::MediaGraphDelta;

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

/// 节点间传递的数据载荷。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeData {
    /// 原始文本或脚本中间值。
    Raw(String),
    /// HTTP 响应。
    HttpResponse(HttpResponse),
    /// 结构化中间数据。
    Json(serde_json::Value),
    /// 标准媒体资源图增量。
    Delta(MediaGraphDelta),
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
    /// JSON 数据。
    Json,
    /// 媒体资源图增量。
    Delta,
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
            Self::Json(_) => NodeDataVariant::Json,
            Self::Delta(_) => NodeDataVariant::Delta,
            Self::Error(_) => NodeDataVariant::Error,
        }
    }
}
