//! 单次构建节点、端口、边与 control region 分析结果。

use super::{
    BTreeMap, BTreeSet, ControlRegion, Diagnostic, FlowEdge, FlowNode, LoopControlRegion, PlanPort,
    RuleDefinition, Uuid, adjacency, diagnostic, has_cycle, node_diagnostic, ports_for_node,
    schema_span, sort_diagnostics, validate_definition_header, validate_edges,
    validate_input_and_control_handles, validate_intent_exports, validate_loop_region_overlap,
    validate_loop_regions, validate_node_configuration,
};

pub(in crate::compiler) struct Analysis {
    pub(in crate::compiler) diagnostics: Vec<Diagnostic>,
    pub(in crate::compiler) control_regions: Vec<ControlRegion>,
}

#[derive(Debug, Clone)]
pub(in crate::compiler) struct NodePorts {
    pub(in crate::compiler) inputs: Vec<PlanPort>,
    pub(in crate::compiler) outputs: Vec<PlanPort>,
}

#[derive(Debug, Clone)]
pub(in crate::compiler) struct ValidatedLoopRegion {
    pub(in crate::compiler) region: LoopControlRegion,
    pub(in crate::compiler) backedge: FlowEdge,
    pub(in crate::compiler) body_nodes: BTreeSet<Uuid>,
}

pub(in crate::compiler) fn analyze(definition: &RuleDefinition) -> Analysis {
    let mut diagnostics = Vec::new();
    let mut nodes = BTreeMap::<Uuid, &FlowNode>::new();
    let mut ports = BTreeMap::<Uuid, NodePorts>::new();

    validate_definition_header(definition, &mut diagnostics);
    for node in &definition.flow().nodes {
        if nodes.contains_key(&node.id) {
            diagnostics.push(node_diagnostic(
                "DUPLICATE_NODE_ID",
                format!("节点 {} 重复声明", node.id),
                node,
                "",
            ));
            continue;
        }
        validate_node_configuration(node, definition, &mut diagnostics);
        ports.insert(node.id, ports_for_node(node));
        nodes.insert(node.id, node);
    }

    let valid_edges = validate_edges(definition, &nodes, &ports, &mut diagnostics);
    validate_input_and_control_handles(&nodes, &valid_edges, &mut diagnostics);

    let mut loop_regions =
        validate_loop_regions(definition, &nodes, &valid_edges, &mut diagnostics);
    validate_loop_region_overlap(&mut loop_regions, &nodes, &mut diagnostics);

    let accepted_backedges = loop_regions
        .iter()
        .map(|region| region.backedge.clone())
        .collect::<BTreeSet<_>>();
    let acyclic_adjacency = adjacency(&valid_edges, &accepted_backedges);
    if has_cycle(nodes.keys().copied(), &acyclic_adjacency) {
        diagnostics.push(diagnostic(
            "FLOW_CYCLE_UNSTRUCTURED",
            "Flow 含有非 structured Loop yield 的裸循环",
            schema_span("/flow/edges"),
        ));
    }

    let full_adjacency = adjacency(&valid_edges, &BTreeSet::new());
    validate_intent_exports(
        definition,
        &nodes,
        &ports,
        &full_adjacency,
        &mut diagnostics,
    );

    sort_diagnostics(&mut diagnostics);
    let mut control_regions = loop_regions
        .into_iter()
        .map(|region| ControlRegion::Loop(region.region))
        .collect::<Vec<_>>();
    control_regions.sort_by_key(|region| match region {
        ControlRegion::Loop(region) => region.loop_node,
    });

    Analysis {
        diagnostics,
        control_regions,
    }
}
