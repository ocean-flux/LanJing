//! typed edge、handle 与 producer 合同。

use super::super::{
    BTreeMap, BTreeSet, CONDITION_INPUT_HANDLE, Diagnostic, FlowEdge, FlowNode, FlowNodeConfig,
    FlowPortRef, LINEAR_INPUT_HANDLE, LOOP_BODY_HANDLE, LOOP_COLLECTION_HANDLE, LOOP_DONE_HANDLE,
    LOOP_YIELD_HANDLE, MERGE_OUTPUT_HANDLE, NodePorts, RuleDefinition, Uuid, edge_diagnostic,
    edges_by_input, edges_by_output, find_port, node_diagnostic, pointer_token,
    ports_are_compatible,
};

pub(in crate::compiler) fn validate_edges<'a>(
    definition: &'a RuleDefinition,
    nodes: &BTreeMap<Uuid, &'a FlowNode>,
    ports: &BTreeMap<Uuid, NodePorts>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<&'a FlowEdge> {
    let mut seen = BTreeSet::new();
    let mut valid_edges = Vec::new();

    for edge in &definition.flow().edges {
        if !seen.insert(edge.clone()) {
            diagnostics.push(edge_diagnostic(
                "DUPLICATE_EDGE",
                "语义边 identity 重复",
                edge,
            ));
            continue;
        }

        let from = nodes.get(&edge.from.node_id);
        let to = nodes.get(&edge.to.node_id);
        match (from, to) {
            (None, None) => {
                diagnostics.push(edge_diagnostic(
                    "EDGE_ENDPOINT_MISSING",
                    "语义边的起点和终点节点都不存在",
                    edge,
                ));
                continue;
            }
            (None, Some(_)) => {
                diagnostics.push(edge_diagnostic(
                    "EDGE_SOURCE_MISSING",
                    format!("边起点节点 {} 不存在", edge.from.node_id),
                    edge,
                ));
                continue;
            }
            (Some(_), None) => {
                diagnostics.push(edge_diagnostic(
                    "EDGE_TARGET_MISSING",
                    format!("边终点节点 {} 不存在", edge.to.node_id),
                    edge,
                ));
                continue;
            }
            (Some(_), Some(_)) => {}
        }

        let source_port = ports
            .get(&edge.from.node_id)
            .and_then(|ports| find_port(&ports.outputs, &edge.from.handle));
        let target_port = ports
            .get(&edge.to.node_id)
            .and_then(|ports| find_port(&ports.inputs, &edge.to.handle));
        if source_port.is_none() {
            diagnostics.push(edge_diagnostic(
                "SOURCE_HANDLE_MISSING",
                format!(
                    "节点 {} 未声明 output handle {}",
                    edge.from.node_id, edge.from.handle
                ),
                edge,
            ));
        }
        if target_port.is_none() {
            diagnostics.push(edge_diagnostic(
                "TARGET_HANDLE_MISSING",
                format!(
                    "节点 {} 未声明 input handle {}",
                    edge.to.node_id, edge.to.handle
                ),
                edge,
            ));
        }
        let (Some(source_port), Some(target_port)) = (source_port, target_port) else {
            continue;
        };
        if !ports_are_compatible(&source_port.value_type, &target_port.value_type) {
            diagnostics.push(edge_diagnostic(
                "PORT_TYPE_MISMATCH",
                format!(
                    "节点 {} 的 output {} 不能连接到节点 {} 的 input {}",
                    edge.from.node_id, edge.from.handle, edge.to.node_id, edge.to.handle
                ),
                edge,
            ));
        }
        valid_edges.push(edge);
    }

    valid_edges
}

pub(in crate::compiler) fn validate_input_and_control_handles(
    nodes: &BTreeMap<Uuid, &FlowNode>,
    edges: &[&FlowEdge],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let incoming = edges_by_input(edges);
    let outgoing = edges_by_output(edges);

    for node in nodes.values() {
        match &node.config {
            FlowNodeConfig::Merge(config) => {
                let mut checked = BTreeSet::new();
                for input in &config.inputs {
                    if input.handle.trim().is_empty() || !checked.insert(input.handle.as_str()) {
                        continue;
                    }
                    let reference = FlowPortRef::new(node.id, input.handle.clone());
                    if incoming.get(&reference).map_or(0, Vec::len) != 1 {
                        diagnostics.push(node_diagnostic(
                            "MERGE_INPUT_POLICY_INVALID",
                            format!(
                                "Merge input {} 必须有唯一 producer，并保留声明的 activation policy",
                                input.handle
                            ),
                            node,
                            &format!("/inputs/{}", pointer_token(&input.handle)),
                        ));
                    }
                }
                let output = FlowPortRef::new(node.id, MERGE_OUTPUT_HANDLE);
                if outgoing.get(&output).map_or(0, Vec::len) == 0 {
                    diagnostics.push(node_diagnostic(
                        "MERGE_OUTPUT_UNCONNECTED",
                        "Merge output 必须连接到下游",
                        node,
                        "/output",
                    ));
                }
            }
            FlowNodeConfig::Condition(config) => {
                let input = FlowPortRef::new(node.id, CONDITION_INPUT_HANDLE);
                if incoming.get(&input).map_or(0, Vec::len) != 1 {
                    diagnostics.push(node_diagnostic(
                        "CONDITION_INPUT_INVALID",
                        "Condition input 必须有唯一 producer",
                        node,
                        "/input",
                    ));
                }
                let mut checked = BTreeSet::new();
                for branch in &config.branches {
                    if branch.trim().is_empty() || !checked.insert(branch.as_str()) {
                        continue;
                    }
                    let output = FlowPortRef::new(node.id, branch.clone());
                    if outgoing.get(&output).map_or(0, Vec::len) == 0 {
                        diagnostics.push(node_diagnostic(
                            "CONDITION_BRANCH_UNCONNECTED",
                            format!("Condition branch {branch} 未连接到下游"),
                            node,
                            &format!("/branches/{}", pointer_token(branch)),
                        ));
                    }
                }
            }
            FlowNodeConfig::Loop(_) => {
                validate_loop_handle_counts(node, &incoming, &outgoing, diagnostics);
            }
            // 未安装能力节点的 port 语义未知；由 Definition 校验统一拒绝。
            FlowNodeConfig::Unavailable(_) => {}
            FlowNodeConfig::Http(_)
            | FlowNodeConfig::Js(_)
            | FlowNodeConfig::Extract(_)
            | FlowNodeConfig::Mapper(_) => {
                let input = FlowPortRef::new(node.id, LINEAR_INPUT_HANDLE);
                if incoming.get(&input).map_or(0, Vec::len) > 1 {
                    diagnostics.push(node_diagnostic(
                        "INPUT_HANDLE_AMBIGUOUS",
                        "线性节点 input 不能有多个 producer",
                        node,
                        "/input",
                    ));
                }
            }
        }
    }
}

pub(in crate::compiler) fn validate_loop_handle_counts(
    node: &FlowNode,
    incoming: &BTreeMap<FlowPortRef, Vec<&FlowEdge>>,
    outgoing: &BTreeMap<FlowPortRef, Vec<&FlowEdge>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let collection = FlowPortRef::new(node.id, LOOP_COLLECTION_HANDLE);
    if incoming.get(&collection).map_or(0, Vec::len) != 1 {
        diagnostics.push(node_diagnostic(
            "LOOP_ENTRY_INVALID",
            "Loop collection 必须有唯一外部 producer",
            node,
            "/collection",
        ));
    }
    let body = FlowPortRef::new(node.id, LOOP_BODY_HANDLE);
    if outgoing.get(&body).map_or(0, Vec::len) != 1 {
        diagnostics.push(node_diagnostic(
            "LOOP_BODY_INVALID",
            "Loop body 必须有唯一 structured entry",
            node,
            "/body",
        ));
    }
    let yield_input = FlowPortRef::new(node.id, LOOP_YIELD_HANDLE);
    if incoming.get(&yield_input).map_or(0, Vec::len) != 1 {
        diagnostics.push(node_diagnostic(
            "LOOP_YIELD_INVALID",
            "Loop yield 必须有唯一 structured return",
            node,
            "/yield",
        ));
    }
    let done = FlowPortRef::new(node.id, LOOP_DONE_HANDLE);
    if outgoing.get(&done).map_or(0, Vec::len) == 0 {
        diagnostics.push(node_diagnostic(
            "LOOP_DONE_INVALID",
            "Loop done 必须连接到 region 外下游",
            node,
            "/done",
        ));
    }
}
