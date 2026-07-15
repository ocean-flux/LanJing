//! 运行时 tap：压缩 `NodeData` 事件摘要。

use futures::StreamExt;
use futures::stream::BoxStream;
use lj_core::node::NodeId;
use lj_core::node_data::NodeData;
use serde::Serialize;

/// 节点输出摘要。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TapSummary {
    pub node_id: NodeId,
    pub variant: String,
    pub summary: String,
}

#[must_use]
pub fn summarize_data(data: &NodeData) -> (String, String) {
    match data {
        NodeData::Raw(s) => ("Raw".to_string(), truncate(s, 200)),
        NodeData::HttpResponse(resp) => (
            "HttpResponse".to_string(),
            format!("status={}, bytes={}", resp.status, resp.body.len()),
        ),
        NodeData::Json(v) => ("Json".to_string(), truncate(&v.to_string(), 200)),
        NodeData::Delta(delta) => (
            "Delta".to_string(),
            format!(
                "sources={}, items={}, collections={}, units={}, assets={}, actions={}",
                delta.sources.len(),
                delta.items.len(),
                delta.collections.len(),
                delta.units.len(),
                delta.assets.len(),
                delta.actions.len()
            ),
        ),
        NodeData::Error(e) => ("Error".to_string(), truncate(e, 200)),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

#[must_use]
pub fn tap_stream<'a>(
    node_id: NodeId,
    input: BoxStream<'a, NodeData>,
    mut on_summary: impl FnMut(TapSummary) + Send + 'a,
) -> BoxStream<'a, NodeData> {
    input
        .inspect(move |data| {
            let (variant, summary) = summarize_data(data);
            on_summary(TapSummary {
                node_id: node_id.clone(),
                variant,
                summary,
            });
        })
        .boxed()
}
