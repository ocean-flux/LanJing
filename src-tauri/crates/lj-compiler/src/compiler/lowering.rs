//! 已验证 analysis 到 immutable Plan 的唯一 lowering。

use super::{
    BTreeSet, CollectionSelector, CompilerError, ConditionConfig, ControlExpression, ControlRegion,
    EffectDeclaration, EffectKind, ExecutionPlan, ExecutionPlanParts, FlowNode, FlowNodeConfig,
    IntentEntry, LoopIterationLimit, NodePorts, PlanEdge, PlanForEachConfig, PlanNode,
    PlanNodeConfig, RuleDefinition, definition_hash, ports_for_node,
    unavailable_capability_diagnostic,
};

pub(in crate::compiler) fn build_plan(
    definition: &RuleDefinition,
    compiler_version: &str,
    control_regions: Vec<ControlRegion>,
) -> Result<ExecutionPlan, CompilerError> {
    let definition_hash = definition_hash(definition)
        .map_err(|error| CompilerError::Serialization(error.to_string()))?;

    let (nodes, effects) = lower_nodes(definition)?;
    let capability_requirements = effects
        .iter()
        .flat_map(|effect| effect.required_capabilities.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let intent_entries = definition
        .intent_exports()
        .iter()
        .map(|(intent, export)| {
            (
                *intent,
                IntentEntry {
                    intent: *intent,
                    entry_node: export.flow_entry,
                    mapper_output: export.mapper_output,
                },
            )
        })
        .collect();
    let edges = definition
        .flow()
        .edges
        .iter()
        .map(|edge| PlanEdge::new(edge.from.clone(), edge.to.clone()))
        .collect();

    ExecutionPlan::new(
        compiler_version,
        definition_hash,
        ExecutionPlanParts {
            nodes,
            edges,
            intent_entries,
            effects,
            capability_requirements,
            control_regions,
        },
    )
    .map_err(|error| CompilerError::Serialization(error.to_string()))
}

fn lower_nodes(
    definition: &RuleDefinition,
) -> Result<(Vec<PlanNode>, Vec<EffectDeclaration>), CompilerError> {
    let mut effects = Vec::new();
    let mut nodes = Vec::with_capacity(definition.flow().nodes.len());
    for flow_node in &definition.flow().nodes {
        if let Some(kind) = effect_kind(&flow_node.config) {
            effects.push(EffectDeclaration {
                node_id: flow_node.id,
                kind,
                required_capabilities: Vec::new(),
            });
        }
        let NodePorts { inputs, outputs } = ports_for_node(flow_node);
        nodes.push(PlanNode {
            id: flow_node.id,
            inputs,
            outputs,
            config: plan_node_config(flow_node)?,
        });
    }
    effects.sort_by(|left, right| {
        left.node_id
            .cmp(&right.node_id)
            .then_with(|| effect_kind_rank(&left.kind).cmp(&effect_kind_rank(&right.kind)))
    });
    Ok((nodes, effects))
}

pub(in crate::compiler) fn effect_kind(config: &FlowNodeConfig) -> Option<EffectKind> {
    match config {
        FlowNodeConfig::Http(_) => Some(EffectKind::Http),
        FlowNodeConfig::Js(_)
        | FlowNodeConfig::Condition(ConditionConfig {
            expression: ControlExpression::Js { .. },
            ..
        })
        | FlowNodeConfig::Loop(lj_rule_model::definition::ForEachConfig {
            collection: CollectionSelector::Js { .. },
            ..
        }) => Some(EffectKind::QuickJs),
        FlowNodeConfig::Extract(_) => Some(EffectKind::Extract),
        FlowNodeConfig::Mapper(_)
        | FlowNodeConfig::Merge(_)
        | FlowNodeConfig::Condition(_)
        | FlowNodeConfig::Loop(_)
        | FlowNodeConfig::Unavailable(_) => None,
    }
}
pub(in crate::compiler) fn plan_node_config(
    node: &FlowNode,
) -> Result<PlanNodeConfig, CompilerError> {
    let config = match &node.config {
        FlowNodeConfig::Http(config) => PlanNodeConfig::Http(config.clone()),
        FlowNodeConfig::Js(config) => PlanNodeConfig::Js(config.clone()),
        FlowNodeConfig::Extract(config) => PlanNodeConfig::Extract(config.clone()),
        FlowNodeConfig::Mapper(config) => PlanNodeConfig::Mapper(config.clone()),
        FlowNodeConfig::Merge(config) => {
            let mut ordered = config.clone();
            ordered.inputs.sort_by_key(|input| input.order);
            PlanNodeConfig::Merge(ordered)
        }
        FlowNodeConfig::Condition(config) => PlanNodeConfig::Condition(config.clone()),
        FlowNodeConfig::Loop(config) => {
            let max_iterations = LoopIterationLimit::new(u32::from(config.max_iterations))
                .map_err(|error| CompilerError::Serialization(error.to_string()))?;
            PlanNodeConfig::Loop(PlanForEachConfig::new(
                config.collection.clone(),
                config.item_binding.clone(),
                config.index_binding.clone(),
                max_iterations,
            ))
        }
        FlowNodeConfig::Unavailable(config) => {
            return Err(CompilerError::validation(vec![
                unavailable_capability_diagnostic(node, config),
            ]));
        }
    };
    Ok(config)
}

pub(in crate::compiler) const fn effect_kind_rank(kind: &EffectKind) -> u8 {
    match kind {
        EffectKind::Http => 0,
        EffectKind::QuickJs => 1,
        EffectKind::Extract => 2,
    }
}
