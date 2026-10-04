//! compatibility matrix；节点 port 声明来自 `lj_rule_model::descriptor`。

use super::{FlowNode, FlowNodeConfig, NodePorts, PlanPort, PortValueKind, PortValueType};

pub(in crate::compiler) fn ports_for_node(node: &FlowNode) -> NodePorts {
    // port 语义只有 descriptor 一个来源：compiler 与规则编辑器读同一份声明。
    let Some(resolved) = lj_rule_model::descriptor::resolve_config_ports(&node.config) else {
        // 未安装能力节点的 port 语义未知，不声明任何 port。
        return NodePorts {
            inputs: Vec::new(),
            outputs: Vec::new(),
        };
    };
    let to_plan_ports = |ports: Vec<lj_rule_model::descriptor::ResolvedPort>| {
        ports
            .into_iter()
            .map(|port| PlanPort::new(port.handle, port.value_type))
            .collect::<Vec<_>>()
    };
    let mut inputs = to_plan_ports(resolved.inputs);
    let mut outputs = to_plan_ports(resolved.outputs);
    // Merge input 顺序由显式 order 决定；其他节点 handle 排序只保证稳定展示，不承载语义。
    if !matches!(node.config, FlowNodeConfig::Merge(_)) {
        inputs.sort_by(|left, right| left.handle.as_bytes().cmp(right.handle.as_bytes()));
    }
    outputs.sort_by(|left, right| left.handle.as_bytes().cmp(right.handle.as_bytes()));
    NodePorts { inputs, outputs }
}

pub(in crate::compiler) fn ports_are_compatible(
    output: &PortValueType,
    input: &PortValueType,
) -> bool {
    match output {
        PortValueType::Kind { kind } => accepts_kind(input, *kind),
        PortValueType::Union { .. } => false,
    }
}

pub(in crate::compiler) fn accepts_kind(input: &PortValueType, output: PortValueKind) -> bool {
    match input {
        PortValueType::Kind { kind } => *kind == output,
        PortValueType::Union { kinds } => kinds.binary_search(&output).is_ok(),
    }
}

pub(in crate::compiler) fn find_port<'a>(
    ports: &'a [PlanPort],
    handle: &str,
) -> Option<&'a PlanPort> {
    ports.iter().find(|port| port.handle == handle)
}
