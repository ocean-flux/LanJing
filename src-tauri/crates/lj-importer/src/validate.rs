//! 图 schema 验证 — 检查图结构是否符合端点子图模板。

use std::collections::{HashMap, HashSet};

use lj_core::endpoint::EndpointKind;
use lj_core::error::CoreError;
use lj_core::graph_schema::GraphSchema;
use lj_core::node::{Graph, Node, NodeId, NodeKind};
use lj_core::node_data::NodeDataVariant;

/// 验证图结构是否符合 schema。
///
/// 检查:
/// 1. 每端点子图节点序列匹配模板。
/// 2. 所有边目标节点存在。
/// 3. 边类型匹配(`from.output_type` == `to.input_type`)。
/// 4. 无子例程递归引用自身(简单检查)。
///
/// # Errors
///
/// 返回 `CoreError::GraphValidation` 当图结构不符合模板要求。
pub fn validate_graph(graph: &Graph, schema: &GraphSchema) -> Result<(), CoreError> {
    // 按 endpoint_kind 分组节点
    let mut endpoint_groups: HashMap<EndpointKind, Vec<&Node>> = HashMap::new();
    for node in &graph.nodes {
        if let Some(ek) = get_endpoint_kind(node) {
            endpoint_groups.entry(ek).or_default().push(node);
        }
    }

    // 对每模板检查节点序列
    for template in &schema.templates {
        if let Some(nodes) = endpoint_groups.get(&template.kind) {
            // 检查存在性：期望的每种 NodeKind 至少出现一次
            for expected_kind in &template.node_sequence {
                let has_kind = nodes.iter().any(|n| n.spec.kind == *expected_kind);
                if !has_kind {
                    return Err(CoreError::GraphValidation(format!(
                        "端点 {:?} 缺少 {:?} 节点",
                        template.kind, expected_kind,
                    )));
                }
            }

            // 检查节点序列顺序:按边拓扑遍历,与模板顺序匹配
            check_node_sequence(nodes, &graph.edges, template)?;
        }
        // 缺少整个端点群组不算错误(可选的端点)
    }

    // 检查所有边的目标节点存在 + 边类型匹配
    for edge in &graph.edges {
        let from_node = graph
            .nodes
            .iter()
            .find(|n| n.node_id == edge.from)
            .ok_or_else(|| {
                CoreError::GraphValidation(format!("边 from 节点 {} 不存在", edge.from.0))
            })?;
        let to_node = graph
            .nodes
            .iter()
            .find(|n| n.node_id == edge.to)
            .ok_or_else(|| {
                CoreError::GraphValidation(format!("边 to 节点 {} 不存在", edge.to.0))
            })?;

        // 边类型匹配检查
        check_edge_type(from_node, to_node)?;
    }

    // 子例程递归引用检查(简单:子图不能含与父同 ID 的子例程)
    for (sub_id, sub_graph) in &graph.subroutines {
        if sub_graph.subroutines.contains_key(sub_id) {
            return Err(CoreError::GraphValidation(
                "子例程不能递归引用自身".to_string(),
            ));
        }
    }

    Ok(())
}

/// 检查端点内节点序列与模板顺序匹配。
fn check_node_sequence(
    nodes: &[&Node],
    edges: &[lj_core::node::Edge],
    template: &lj_core::graph_schema::EndpointTemplate,
) -> Result<(), CoreError> {
    let node_ids: HashSet<&NodeId> = nodes.iter().map(|n| &n.node_id).collect();

    // 计算子图入度
    let mut in_degree: HashMap<&NodeId, usize> = HashMap::new();
    for n in nodes {
        in_degree.entry(&n.node_id).or_insert(0);
    }
    for edge in edges {
        if node_ids.contains(&edge.from) && node_ids.contains(&edge.to) {
            *in_degree.entry(&edge.to).or_insert(0) += 1;
        }
    }

    // 找源节点(入度为 0)
    let sources: Vec<&&NodeId> = in_degree
        .iter()
        .filter(|&(_, &deg)| deg == 0)
        .map(|(id, _)| id)
        .collect();

    if sources.len() != 1 {
        return Err(CoreError::GraphValidation(format!(
            "端点 {:?} 子图源节点数={},需要恰 1 个",
            template.kind,
            sources.len(),
        )));
    }

    // 拓扑遍历收集节点序列
    let mut seq = Vec::new();
    let mut current = *sources[0];
    loop {
        let node = nodes
            .iter()
            .find(|n| &n.node_id == current)
            .ok_or_else(|| {
                CoreError::GraphValidation(format!("端点 {:?} 遍历时节点丢失", template.kind))
            })?;
        seq.push(node.spec.kind.clone());

        let outgoing: Vec<&NodeId> = edges
            .iter()
            .filter(|e| &e.from == current && node_ids.contains(&e.to))
            .map(|e| &e.to)
            .collect();

        if outgoing.is_empty() {
            break;
        }
        if outgoing.len() > 1 {
            return Err(CoreError::GraphValidation(format!(
                "端点 {:?} 子图有分支,暂不支持",
                template.kind,
            )));
        }
        current = outgoing[0];
    }

    if seq != template.node_sequence {
        return Err(CoreError::GraphValidation(format!(
            "端点 {:?} 节点序列 {:?} 不匹配模板 {:?}",
            template.kind, seq, template.node_sequence,
        )));
    }

    Ok(())
}

/// 检查边类型匹配:`from.output_type` == `to.input_type`。
fn check_edge_type(from_node: &Node, to_node: &Node) -> Result<(), CoreError> {
    let (_, from_output) = node_kind_io(&from_node.spec.kind);
    let (to_input, _) = node_kind_io(&to_node.spec.kind);

    // to_input == None 表示不限制输入(源头节点或通配 stub)
    if let Some(to_input) = to_input
        && let Some(from_output) = from_output
        && from_output != to_input
    {
        return Err(CoreError::GraphValidation(format!(
            "边类型不匹配: {:?}(output={:?}) → {:?}(input={:?})",
            from_node.spec.kind, from_output, to_node.spec.kind, to_input,
        )));
    }

    Ok(())
}

/// 获取 `NodeKind` 的静态输入/输出类型映射。
fn node_kind_io(kind: &NodeKind) -> (Option<NodeDataVariant>, Option<NodeDataVariant>) {
    match kind {
        NodeKind::Http => (None, Some(NodeDataVariant::HttpResponse)),
        NodeKind::Js => (None, Some(NodeDataVariant::Raw)),
        NodeKind::Extract => (
            Some(NodeDataVariant::HttpResponse),
            Some(NodeDataVariant::Media),
        ),
        // Merge/Condition/Loop: any → any(stub)
        NodeKind::Merge | NodeKind::Condition | NodeKind::Loop => (None, None),
    }
}

/// 从节点 spec 中提取端点类型。
fn get_endpoint_kind(node: &Node) -> Option<EndpointKind> {
    match node.spec.kind {
        NodeKind::Http => node.spec.http.as_ref().map(|h| h.endpoint_kind.clone()),
        NodeKind::Extract => node
            .spec
            .extract
            .as_ref()
            .and_then(|e| e.endpoint_kind.clone()),
        NodeKind::Js => node.spec.js.as_ref().and_then(|j| j.endpoint_kind.clone()),
        _ => None,
    }
}
