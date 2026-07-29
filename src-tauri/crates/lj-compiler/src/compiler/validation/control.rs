//! structured Loop region 合同。

use super::super::{
    BTreeMap, BTreeSet, Diagnostic, FlowEdge, FlowNode, FlowNodeConfig, FlowPortRef,
    LOOP_BODY_HANDLE, LOOP_COLLECTION_HANDLE, LOOP_DONE_HANDLE, LOOP_YIELD_HANDLE,
    LoopControlRegion, RuleDefinition, Uuid, ValidatedLoopRegion, VecDeque, edge_diagnostic,
    edges_by_input, edges_by_output, node_diagnostic,
};

pub(in crate::compiler) fn validate_loop_regions(
    definition: &RuleDefinition,
    nodes: &BTreeMap<Uuid, &FlowNode>,
    edges: &[&FlowEdge],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ValidatedLoopRegion> {
    let incoming = edges_by_input(edges);
    let outgoing = edges_by_output(edges);
    let mut regions = Vec::new();

    for node in definition
        .flow()
        .nodes
        .iter()
        .filter(|node| matches!(node.config, FlowNodeConfig::Loop(_)))
    {
        let before = diagnostics.len();
        let collection_ref = FlowPortRef::new(node.id, LOOP_COLLECTION_HANDLE);
        let body_ref = FlowPortRef::new(node.id, LOOP_BODY_HANDLE);
        let yield_ref = FlowPortRef::new(node.id, LOOP_YIELD_HANDLE);
        let done_ref = FlowPortRef::new(node.id, LOOP_DONE_HANDLE);

        let collection_edges = incoming.get(&collection_ref).map_or(&[][..], Vec::as_slice);
        let body_edges = outgoing.get(&body_ref).map_or(&[][..], Vec::as_slice);
        let yield_edges = incoming.get(&yield_ref).map_or(&[][..], Vec::as_slice);
        let done_edges = outgoing.get(&done_ref).map_or(&[][..], Vec::as_slice);
        if collection_edges.len() != 1
            || body_edges.len() != 1
            || yield_edges.len() != 1
            || done_edges.is_empty()
        {
            continue;
        }

        let body_edge = body_edges[0];
        let yield_edge = yield_edges[0];
        let body_entry = body_edge.to.clone();
        let yield_source = yield_edge.from.clone();
        if body_entry.node_id == node.id || yield_source.node_id == node.id {
            diagnostics.push(node_diagnostic(
                "LOOP_BODY_INVALID",
                "Loop body entry/yield source 必须是 region 内的其他节点",
                node,
                "/body",
            ));
            continue;
        }

        let forward = reachable_loop_body_nodes(body_entry.node_id, node.id, yield_edge, edges);
        let reverse = nodes_reaching_loop_yield(yield_source.node_id, node.id, yield_edge, edges);
        if !forward.contains(&yield_source.node_id) {
            diagnostics.push(node_diagnostic(
                "LOOP_YIELD_UNREACHABLE",
                "Loop body entry 无法到达唯一 yield source",
                node,
                "/yield",
            ));
            continue;
        }

        let body_nodes = forward
            .intersection(&reverse)
            .copied()
            .collect::<BTreeSet<_>>();
        if forward
            .iter()
            .any(|candidate| !body_nodes.contains(candidate))
        {
            diagnostics.push(node_diagnostic(
                "LOOP_BODY_BYPASS",
                "Loop body 存在不能恰好到达 yield 的旁路或终点",
                node,
                "/body",
            ));
        }
        if body_nodes.iter().any(|node_id| {
            nodes
                .get(node_id)
                .is_some_and(|candidate| matches!(candidate.config, FlowNodeConfig::Loop(_)))
        }) {
            diagnostics.push(node_diagnostic(
                "LOOP_NESTING_UNSUPPORTED",
                "首期不支持 nested Loop region",
                node,
                "/body",
            ));
        }

        for edge in edges {
            let from_inside = body_nodes.contains(&edge.from.node_id);
            let to_inside = body_nodes.contains(&edge.to.node_id);
            let valid_entry = *edge == body_edge;
            let valid_yield = *edge == yield_edge;
            if (!from_inside && to_inside && !valid_entry)
                || (from_inside && !to_inside && !valid_yield)
            {
                diagnostics.push(edge_diagnostic(
                    "LOOP_CROSS_REGION_EDGE",
                    format!("边跨越 Loop {} region 边界", node.id),
                    edge,
                ));
            }
        }
        if collection_edges
            .iter()
            .any(|edge| body_nodes.contains(&edge.from.node_id))
            || done_edges
                .iter()
                .any(|edge| body_nodes.contains(&edge.to.node_id))
        {
            diagnostics.push(node_diagnostic(
                "LOOP_CROSS_REGION_EDGE",
                "Loop collection 必须来自 region 外，done 必须离开 region",
                node,
                "/region",
            ));
        }

        if diagnostics.len() == before {
            regions.push(ValidatedLoopRegion {
                region: LoopControlRegion {
                    loop_node: node.id,
                    body_entry,
                    yield_source,
                    body_nodes: body_nodes.iter().copied().collect(),
                },
                backedge: (*yield_edge).clone(),
                body_nodes,
            });
        }
    }

    regions
}

pub(in crate::compiler) fn validate_loop_region_overlap(
    regions: &mut Vec<ValidatedLoopRegion>,
    nodes: &BTreeMap<Uuid, &FlowNode>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut invalid = BTreeSet::new();
    for left_index in 0..regions.len() {
        for right_index in (left_index + 1)..regions.len() {
            let left = &regions[left_index];
            let right = &regions[right_index];
            if left.body_nodes.is_disjoint(&right.body_nodes) {
                continue;
            }
            invalid.insert(left.region.loop_node);
            invalid.insert(right.region.loop_node);
        }
    }
    for loop_node in &invalid {
        if let Some(node) = nodes.get(loop_node) {
            diagnostics.push(node_diagnostic(
                "LOOP_REGION_OVERLAP",
                "Loop body regions 不得重叠或嵌套",
                node,
                "/body",
            ));
        }
    }
    regions.retain(|region| !invalid.contains(&region.region.loop_node));
}

pub(in crate::compiler) fn reachable_loop_body_nodes(
    start: Uuid,
    loop_node: Uuid,
    backedge: &FlowEdge,
    edges: &[&FlowEdge],
) -> BTreeSet<Uuid> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([start]);
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current) {
            continue;
        }
        for edge in edges {
            if *edge == backedge || edge.from.node_id != current || edge.to.node_id == loop_node {
                continue;
            }
            queue.push_back(edge.to.node_id);
        }
    }
    seen
}

pub(in crate::compiler) fn nodes_reaching_loop_yield(
    target: Uuid,
    loop_node: Uuid,
    backedge: &FlowEdge,
    edges: &[&FlowEdge],
) -> BTreeSet<Uuid> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([target]);
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current) {
            continue;
        }
        for edge in edges {
            if *edge == backedge || edge.to.node_id != current || edge.from.node_id == loop_node {
                continue;
            }
            queue.push_back(edge.from.node_id);
        }
    }
    seen
}
