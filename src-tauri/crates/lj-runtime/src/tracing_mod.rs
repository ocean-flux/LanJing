//! tracing span tree — 每节点一层 `span`，`trace_id` 贯穿(ADR-0016)
//!
//! 中文 message + 英文 `snake_case` field key。

use lj_core::endpoint::EndpointKind;
use lj_core::node::Node;
use tracing::{Level, span};

/// 为节点创建 tracing span。
///
/// span 包含 `node_id`、`node_kind`、`endpoint_kind`、`trace_id` 等字段，
/// 以及一条中文 message 描述当前执行步骤。
#[must_use]
pub fn node_span(
    node: &Node,
    endpoint_kind: Option<&EndpointKind>,
    trace_id: &str,
) -> tracing::Span {
    let endpoint_name = endpoint_kind.map_or_else(|| "未知端点".to_string(), |k| format!("{k:?}"));
    let kind_name = format!("{:?}", node.spec.kind);

    span!(
        Level::INFO,
        "节点执行",
        node_id = %node.node_id.0,
        node_kind = %kind_name,
        endpoint = %endpoint_name,
        trace_id = %trace_id,
        message = format!("开始执行 {} 端点的 {} 节点", endpoint_name, kind_name),
    )
}
