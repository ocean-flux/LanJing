//! 节点执行流转可观测 — tap stream emit node-output event
//!
//! 用 async adapter(then + async 闭包),非同步 inspect。
//! tap 是独立 wrapper 层,不焊进 stream 中间层。

use std::sync::Arc;

use futures::stream::{BoxStream, StreamExt};
use lj_core::media::Media;
use lj_core::node::NodeId;
use lj_core::node_data::NodeData;

/// `NodeData` 摘要(emit 给前端,不含完整 body)。
#[derive(Debug, Clone, serde::Serialize)]
pub struct NodeDataSummary {
    /// 节点 ID。
    pub node_id: String,
    /// 数据类型 variant。
    pub variant: String,
    /// 摘要文本。
    pub summary: String,
}

/// 从 `NodeData` 生成摘要。
#[must_use]
pub fn summarize(node_id: &NodeId, data: &NodeData) -> NodeDataSummary {
    let (variant, summary) = match data {
        NodeData::Raw(s) => ("Raw", truncate(s, 200)),
        NodeData::HttpResponse(resp) => (
            "HttpResponse",
            format!("status={}, body_len={}", resp.status, resp.body.len()),
        ),
        NodeData::Media(m) => {
            let (variant, summary) = match m {
                Media::Book(b) => (
                    "Media::Book",
                    format!(
                        "title={}, author={:?}, chapters={}, book_url={:?}",
                        b.title,
                        b.author,
                        b.chapters.len(),
                        b.book_url
                    ),
                ),
                Media::Video(v) => (
                    "Media::Video",
                    format!("title={}, play_lines={}", v.title, v.play_lines.len()),
                ),
                Media::Audio(a) => (
                    "Media::Audio",
                    format!("title={}, url={:?}", a.title, a.url),
                ),
            };
            (variant, truncate(&summary, 200))
        }
        NodeData::Json(v) => ("Json", truncate(&v.to_string(), 200)),
        NodeData::Error(e) => ("Error", truncate(e, 200)),
    };
    NodeDataSummary {
        node_id: node_id.0.to_string(),
        variant: variant.to_string(),
        summary,
    }
}

/// 截断字符串到 `max` 字符数(非字节),防止多字节字符边界 panic。
fn truncate(s: &str, max: usize) -> String {
    let trimmed: String = s.chars().take(max).collect();
    if trimmed.len() < s.len() {
        format!("{trimmed}...")
    } else {
        trimmed
    }
}

/// tap stream:每个 item 经过时调 callback(用于 emit Tauri event)。
///
/// 用 then + async 闭包(非同步 inspect),在 item 流过时异步调 `on_item`。
#[must_use]
pub fn tap_stream<'a, F, Fut>(
    input: BoxStream<'a, NodeData>,
    node_id: NodeId,
    on_item: F,
) -> BoxStream<'a, NodeData>
where
    F: Fn(NodeDataSummary) -> Fut + Send + Sync + 'a,
    Fut: std::future::Future<Output = ()> + Send + 'a,
{
    let on_item = Arc::new(on_item);

    input
        .then(move |item| {
            let summary = summarize(&node_id, &item);
            let on_item = Arc::clone(&on_item);
            async move {
                on_item(summary).await;
                item
            }
        })
        .boxed()
}
