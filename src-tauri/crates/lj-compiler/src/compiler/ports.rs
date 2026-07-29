//! closed port 声明与唯一 compatibility matrix。

use super::{
    CONDITION_INPUT_HANDLE, FlowNode, FlowNodeConfig, LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE,
    LOOP_BODY_HANDLE, LOOP_COLLECTION_HANDLE, LOOP_DONE_HANDLE, LOOP_YIELD_HANDLE,
    MERGE_OUTPUT_HANDLE, NodePorts, PlanPort, PortValueKind, PortValueType,
};

pub(in crate::compiler) fn ports_for_node(node: &FlowNode) -> NodePorts {
    let is_merge = matches!(node.config, FlowNodeConfig::Merge(_));
    let (mut inputs, mut outputs) = match &node.config {
        FlowNodeConfig::Http(_) => (
            vec![PlanPort::new(LINEAR_INPUT_HANDLE, controlled_value_type())],
            vec![PlanPort::new(
                LINEAR_OUTPUT_HANDLE,
                PortValueType::kind(PortValueKind::HttpResponse),
            )],
        ),
        FlowNodeConfig::Js(config) => {
            let output_kind = match config.output {
                lj_rule_model::definition::JsOutputKind::Json => PortValueKind::Json,
                lj_rule_model::definition::JsOutputKind::Raw => PortValueKind::Raw,
            };
            (
                vec![PlanPort::new(LINEAR_INPUT_HANDLE, controlled_value_type())],
                vec![PlanPort::new(
                    LINEAR_OUTPUT_HANDLE,
                    PortValueType::kind(output_kind),
                )],
            )
        }
        FlowNodeConfig::Extract(_) => (
            vec![PlanPort::new(
                LINEAR_INPUT_HANDLE,
                PortValueType::kind(PortValueKind::HttpResponse),
            )],
            vec![PlanPort::new(
                LINEAR_OUTPUT_HANDLE,
                PortValueType::kind(PortValueKind::Json),
            )],
        ),
        FlowNodeConfig::Mapper(_) => (
            vec![PlanPort::new(
                LINEAR_INPUT_HANDLE,
                PortValueType::kind(PortValueKind::Json),
            )],
            vec![PlanPort::new(
                LINEAR_OUTPUT_HANDLE,
                PortValueType::kind(PortValueKind::Delta),
            )],
        ),
        FlowNodeConfig::Merge(config) => {
            let mut ordered = config.inputs.iter().collect::<Vec<_>>();
            ordered.sort_by_key(|input| input.order);
            (
                ordered
                    .into_iter()
                    .map(|input| {
                        PlanPort::new(
                            input.handle.clone(),
                            PortValueType::kind(PortValueKind::Json),
                        )
                    })
                    .collect(),
                vec![PlanPort::new(
                    MERGE_OUTPUT_HANDLE,
                    PortValueType::kind(PortValueKind::Json),
                )],
            )
        }
        FlowNodeConfig::Condition(config) => (
            vec![PlanPort::new(
                CONDITION_INPUT_HANDLE,
                PortValueType::kind(PortValueKind::Json),
            )],
            config
                .branches
                .iter()
                .map(|branch| {
                    PlanPort::new(branch.clone(), PortValueType::kind(PortValueKind::Json))
                })
                .collect(),
        ),
        FlowNodeConfig::Loop(_) => (
            vec![
                PlanPort::new(
                    LOOP_COLLECTION_HANDLE,
                    PortValueType::kind(PortValueKind::Json),
                ),
                PlanPort::new(LOOP_YIELD_HANDLE, PortValueType::kind(PortValueKind::Json)),
            ],
            vec![
                PlanPort::new(
                    LOOP_BODY_HANDLE,
                    PortValueType::kind(PortValueKind::LoopBinding),
                ),
                PlanPort::new(LOOP_DONE_HANDLE, PortValueType::kind(PortValueKind::Json)),
            ],
        ),
    };
    // Merge input 顺序由显式 order 决定；其他节点 handle 排序只保证稳定展示，不承载语义。
    if !is_merge {
        inputs.sort_by(|left, right| left.handle.as_bytes().cmp(right.handle.as_bytes()));
    }
    outputs.sort_by(|left, right| left.handle.as_bytes().cmp(right.handle.as_bytes()));
    NodePorts { inputs, outputs }
}

pub(in crate::compiler) fn controlled_value_type() -> PortValueType {
    let mut kinds = vec![
        PortValueKind::IntentInput,
        PortValueKind::Raw,
        PortValueKind::Json,
        PortValueKind::LoopBinding,
    ];
    kinds.sort_unstable();
    kinds.dedup();
    PortValueType::union(kinds)
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
