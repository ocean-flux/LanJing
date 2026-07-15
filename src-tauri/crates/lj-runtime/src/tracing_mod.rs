//! tracing helpers.

use lj_capability::StandardIntent;
use lj_core::node::Node;

/// 创建节点执行 span。
#[must_use]
pub fn node_span(node: &Node, intent: Option<&StandardIntent>, trace_id: &str) -> tracing::Span {
    tracing::info_span!(
        "node_execute",
        trace_id = %trace_id,
        node_id = %node.node_id.0,
        node_kind = ?node.spec.kind,
        intent = ?intent
    )
}
