//! 标准 intent entry、mapper 与可达性合同。

use super::super::{
    BTreeMap, Diagnostic, FlowNode, FlowNodeConfig, LINEAR_INPUT_HANDLE, NodePorts, PortValueKind,
    RuleDefinition, Uuid, accepts_kind, diagnostic, find_port, is_reachable, node_diagnostic,
    node_path, schema_span, semantic_span,
};

pub(in crate::compiler) fn validate_intent_exports(
    definition: &RuleDefinition,
    nodes: &BTreeMap<Uuid, &FlowNode>,
    ports: &BTreeMap<Uuid, NodePorts>,
    adjacency: &BTreeMap<Uuid, Vec<Uuid>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (intent, export) in definition.intent_exports() {
        let entry = nodes.get(&export.flow_entry);
        let mapper = nodes.get(&export.mapper_output);
        if entry.is_none() {
            diagnostics.push(diagnostic(
                "INTENT_ENTRY_MISSING",
                format!("{intent:?} 的入口节点 {} 不存在", export.flow_entry),
                schema_span(format!("/intent_exports/{intent:?}/flow_entry")),
            ));
        }
        if let Some(entry) = entry {
            let accepts_intent = ports
                .get(&entry.id)
                .and_then(|ports| find_port(&ports.inputs, LINEAR_INPUT_HANDLE))
                .is_some_and(|port| accepts_kind(&port.value_type, PortValueKind::IntentInput));
            if !accepts_intent {
                diagnostics.push(node_diagnostic(
                    "INTENT_ENTRY_PORT_MISMATCH",
                    format!("{intent:?} 入口不接受 IntentInput"),
                    entry,
                    "/input",
                ));
            }
        }
        match mapper {
            Some(node) if matches!(node.config, FlowNodeConfig::Mapper(_)) => {}
            Some(node) => diagnostics.push(node_diagnostic(
                "INTENT_MAPPER_INVALID",
                format!("{intent:?} 的输出节点 {} 不是 Mapper", node.id),
                node,
                "",
            )),
            None => diagnostics.push(diagnostic(
                "INTENT_MAPPER_MISSING",
                format!("{intent:?} 的 Mapper 节点 {} 不存在", export.mapper_output),
                schema_span(format!("/intent_exports/{intent:?}/mapper_output")),
            )),
        }
        if entry.is_some()
            && mapper.is_some()
            && !is_reachable(adjacency, export.flow_entry, export.mapper_output)
        {
            diagnostics.push(diagnostic(
                "MAPPER_UNREACHABLE",
                format!(
                    "{intent:?} 的入口 {} 无法到达 Mapper {}",
                    export.flow_entry, export.mapper_output
                ),
                mapper.map_or_else(
                    || schema_span("/intent_exports"),
                    |node| semantic_span(node.span.as_ref(), node_path(node.id, "")),
                ),
            ));
        }
    }

    for mapper in nodes
        .values()
        .filter(|node| matches!(node.config, FlowNodeConfig::Mapper(_)))
    {
        let declared = definition
            .intent_exports()
            .values()
            .any(|export| export.mapper_output == mapper.id);
        let reachable = definition
            .intent_exports()
            .values()
            .any(|export| is_reachable(adjacency, export.flow_entry, mapper.id));
        if !declared || !reachable {
            diagnostics.push(node_diagnostic(
                "MAPPER_UNREACHABLE",
                format!("Mapper 节点 {} 未被可达的标准意图导出使用", mapper.id),
                mapper,
                "",
            ));
        }
    }
}
