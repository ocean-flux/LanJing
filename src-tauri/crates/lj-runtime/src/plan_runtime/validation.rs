//! immutable Plan 的启动前验证、控制程序与 fingerprint。
//!
//! 所有检查只读取 compiler 产出的 current `ExecutionPlan`。控制程序保留命名 handle、
//! Condition activation、Merge 显式顺序与 structured Loop region；effect fingerprint 绑定 exact
//! invocation path，但不包含凭据或原始 payload。

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use blake3::Hasher;
use lj_capability::StandardIntent;
use lj_rule_model::{
    CollectionSelector, ConditionConfig, ControlExpression, ControlRegion, EffectDeclaration,
    EffectKind, ExecutionPlan, InvocationPath, LoopControlRegion, MergeConfig, PlanEdge, PlanNode,
    PlanNodeConfig, PlanNodeKind, canonical_json, execution_plan_hash,
};
use serde::Serialize;
use uuid::Uuid;

use crate::effect::EffectInput;

use super::api::{PlanRuntimeConfig, PlanRuntimeError, PlanSupport};

/// 一个 compiler 已证明、runtime 再次收敛的 Loop body 程序。
#[derive(Debug, Clone)]
pub(super) struct LoopProgram {
    pub(super) region: LoopControlRegion,
    pub(super) node_ids: Vec<Uuid>,
}

/// 当前 intent 从 entry 到 Mapper 的 deterministic control program。
#[derive(Debug, Clone)]
pub(super) struct ExecutionPath {
    pub(super) entry_node: Uuid,
    pub(super) mapper_output: Uuid,
    pub(super) node_ids: Vec<Uuid>,
    pub(super) loops: BTreeMap<Uuid, LoopProgram>,
}

/// 根据 current typed Plan 分类 scheduler 支持。
pub(super) fn plan_support(plan: &ExecutionPlan) -> PlanSupport {
    if plan.has_control_flow() {
        PlanSupport::ControlFlow
    } else {
        PlanSupport::Linear
    }
}

/// 校验 Plan 的 immutable 身份、节点配置、effect 声明与 structured control region。
pub(super) fn validate_plan(
    plan: &ExecutionPlan,
    config: &PlanRuntimeConfig,
) -> Result<(), PlanRuntimeError> {
    if plan.compiler_version() != config.compiler_version {
        return Err(PlanRuntimeError::CompilerVersionMismatch);
    }
    if plan.plan_hash() != calculated_plan_hash(plan)? {
        return Err(PlanRuntimeError::PlanHashMismatch);
    }
    if plan.definition_hash().trim().is_empty() {
        return Err(PlanRuntimeError::InvalidPlan("definition_hash 不能为空"));
    }
    if plan.intent_entries().is_empty() {
        return Err(PlanRuntimeError::InvalidPlan("Plan 缺少标准意图入口"));
    }

    let mut node_ids = BTreeSet::new();
    for node in plan.nodes() {
        if !node_ids.insert(node.id) {
            return Err(PlanRuntimeError::InvalidPlan("Plan 节点 ID 重复"));
        }
        validate_node(node)?;
    }
    let mut edges = BTreeSet::new();
    for edge in plan.edges() {
        if !node_ids.contains(&edge.from.node_id) || !node_ids.contains(&edge.to.node_id) {
            return Err(PlanRuntimeError::InvalidPlan("Plan 边引用不存在的节点"));
        }
        if !edges.insert(edge.semantic_identity()) {
            return Err(PlanRuntimeError::InvalidPlan(
                "Plan 边 semantic identity 重复",
            ));
        }
    }
    for (intent, entry) in plan.intent_entries() {
        if entry.intent != *intent {
            return Err(PlanRuntimeError::InvalidPlan("意图入口键与内容不一致"));
        }
        let Some(mapper) = plan
            .nodes()
            .iter()
            .find(|node| node.id == entry.mapper_output)
        else {
            return Err(PlanRuntimeError::MissingNode(entry.mapper_output));
        };
        if mapper.kind() != PlanNodeKind::Mapper {
            return Err(PlanRuntimeError::InvalidPlan("意图输出节点必须是 Mapper"));
        }
        if !node_ids.contains(&entry.entry_node) {
            return Err(PlanRuntimeError::MissingNode(entry.entry_node));
        }
        execution_path(plan, *intent)?;
    }
    validate_effect_declarations(plan)?;
    validate_loop_regions(plan, &node_ids)?;
    Ok(())
}

fn validate_node(node: &PlanNode) -> Result<(), PlanRuntimeError> {
    if node.id.is_nil() {
        return Err(PlanRuntimeError::InvalidPlan("Plan 节点 ID 不能为 nil"));
    }
    match &node.config {
        PlanNodeConfig::Js(config) if config.code.trim().is_empty() => {
            Err(PlanRuntimeError::InvalidPlan("JS 节点配置无效"))
        }
        PlanNodeConfig::Merge(config) => validate_merge(config),
        PlanNodeConfig::Condition(config) => validate_condition(config),
        PlanNodeConfig::Loop(config) => {
            if config.item_binding.trim().is_empty()
                || config.index_binding.trim().is_empty()
                || config.item_binding == config.index_binding
            {
                return Err(PlanRuntimeError::InvalidPlan("Loop binding 无效"));
            }
            match &config.collection {
                CollectionSelector::Typed { pointer } if !is_json_pointer(pointer) => {
                    Err(PlanRuntimeError::InvalidPlan("Loop JSON Pointer 无效"))
                }
                CollectionSelector::Js { code } if code.trim().is_empty() => {
                    Err(PlanRuntimeError::InvalidPlan("Loop JS selector 为空"))
                }
                CollectionSelector::Typed { .. } | CollectionSelector::Js { .. } => Ok(()),
            }
        }
        PlanNodeConfig::Http(_)
        | PlanNodeConfig::Js(_)
        | PlanNodeConfig::Extract(_)
        | PlanNodeConfig::Mapper(_) => Ok(()),
    }
}

fn validate_merge(config: &MergeConfig) -> Result<(), PlanRuntimeError> {
    if config.inputs.is_empty() {
        return Err(PlanRuntimeError::InvalidPlan("Merge 缺少输入"));
    }
    let mut ordered_inputs = config.inputs.iter().collect::<Vec<_>>();
    ordered_inputs.sort_by_key(|input| input.order);
    let mut input_ids = BTreeSet::new();
    let mut handles = BTreeSet::new();
    for (expected_order, input) in ordered_inputs.into_iter().enumerate() {
        if input.input_id.trim().is_empty()
            || input.handle.trim().is_empty()
            || !input_ids.insert(input.input_id.as_str())
            || !handles.insert(input.handle.as_str())
            || usize::try_from(input.order).ok() != Some(expected_order)
        {
            return Err(PlanRuntimeError::InvalidPlan("Merge input 合同无效"));
        }
    }
    Ok(())
}

fn validate_condition(config: &ConditionConfig) -> Result<(), PlanRuntimeError> {
    let branches = config
        .branches
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if branches.len() != config.branches.len()
        || branches.len() < 2
        || branches.iter().any(|branch| branch.trim().is_empty())
    {
        return Err(PlanRuntimeError::InvalidPlan("Condition branch 合同无效"));
    }
    match &config.expression {
        ControlExpression::Typed {
            predicate,
            true_branch,
            false_branch,
        } => {
            if !is_json_pointer(predicate.pointer())
                || true_branch == false_branch
                || !branches.contains(true_branch.as_str())
                || !branches.contains(false_branch.as_str())
            {
                return Err(PlanRuntimeError::InvalidPlan("Condition typed 配置无效"));
            }
        }
        ControlExpression::Js { code } if code.trim().is_empty() => {
            return Err(PlanRuntimeError::InvalidPlan("Condition JS 配置无效"));
        }
        ControlExpression::Js { .. } => {}
    }
    Ok(())
}

fn is_json_pointer(pointer: &str) -> bool {
    if pointer.is_empty() {
        return true;
    }
    let Some(tokens) = pointer.strip_prefix('/') else {
        return false;
    };
    let bytes = tokens.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'~' {
            index += 1;
            if index == bytes.len() || !matches!(bytes[index], b'0' | b'1') {
                return false;
            }
        }
        index += 1;
    }
    true
}

fn validate_effect_declarations(plan: &ExecutionPlan) -> Result<(), PlanRuntimeError> {
    let mut declared = BTreeSet::new();
    for effect in plan.effects() {
        if !declared.insert(effect.node_id) {
            return Err(PlanRuntimeError::InvalidPlan("effect 声明重复"));
        }
        let Some(node) = plan.nodes().iter().find(|node| node.id == effect.node_id) else {
            return Err(PlanRuntimeError::MissingNode(effect.node_id));
        };
        if expected_effect_kind(node) != Some(effect.kind.clone()) {
            return Err(PlanRuntimeError::InvalidPlan("effect 声明与节点类型不一致"));
        }
    }
    for node in plan.nodes() {
        if expected_effect_kind(node).is_some() && !declared.contains(&node.id) {
            return Err(PlanRuntimeError::InvalidPlan("effect 节点缺少声明"));
        }
    }
    Ok(())
}

fn expected_effect_kind(node: &PlanNode) -> Option<EffectKind> {
    match &node.config {
        PlanNodeConfig::Http(_) => Some(EffectKind::Http),
        PlanNodeConfig::Js(_)
        | PlanNodeConfig::Condition(ConditionConfig {
            expression: ControlExpression::Js { .. },
            ..
        })
        | PlanNodeConfig::Loop(lj_rule_model::PlanForEachConfig {
            collection: CollectionSelector::Js { .. },
            ..
        }) => Some(EffectKind::QuickJs),
        PlanNodeConfig::Extract(_) => Some(EffectKind::Extract),
        PlanNodeConfig::Mapper(_)
        | PlanNodeConfig::Merge(_)
        | PlanNodeConfig::Condition(_)
        | PlanNodeConfig::Loop(_) => None,
    }
}

fn validate_loop_regions(
    plan: &ExecutionPlan,
    node_ids: &BTreeSet<Uuid>,
) -> Result<(), PlanRuntimeError> {
    let loop_nodes = plan
        .nodes()
        .iter()
        .filter_map(|node| (node.kind() == PlanNodeKind::Loop).then_some(node.id))
        .collect::<BTreeSet<_>>();
    let mut region_owners = BTreeSet::new();
    let mut body_owners = BTreeMap::new();
    for region in plan.control_regions() {
        let ControlRegion::Loop(region) = region;
        if !loop_nodes.contains(&region.loop_node)
            || !region_owners.insert(region.loop_node)
            || region.body_nodes.is_empty()
            || region.body_entry.node_id == region.loop_node
            || region.yield_source.node_id == region.loop_node
            || !region.body_nodes.contains(&region.body_entry.node_id)
            || !region.body_nodes.contains(&region.yield_source.node_id)
        {
            return Err(PlanRuntimeError::InvalidPlan("Loop control region 无效"));
        }
        for body_node in &region.body_nodes {
            if !node_ids.contains(body_node)
                || loop_nodes.contains(body_node)
                || body_owners.insert(*body_node, region.loop_node).is_some()
            {
                return Err(PlanRuntimeError::InvalidPlan("Loop body region 重叠或嵌套"));
            }
        }
    }
    if region_owners != loop_nodes {
        return Err(PlanRuntimeError::InvalidPlan(
            "Loop 缺少唯一 control region",
        ));
    }
    Ok(())
}

/// 选择请求 intent 从入口到 Mapper 的 deterministic control program。
pub(super) fn execution_path(
    plan: &ExecutionPlan,
    intent: StandardIntent,
) -> Result<ExecutionPath, PlanRuntimeError> {
    let Some(entry) = plan.intent_entries().get(&intent) else {
        return Err(PlanRuntimeError::MissingIntent);
    };
    let forward = adjacency(plan.edges(), false);
    let reverse = adjacency(plan.edges(), true);
    let reachable_from_entry = reachable(entry.entry_node, &forward);
    let reaches_mapper = reachable(entry.mapper_output, &reverse);
    let selected = reachable_from_entry
        .intersection(&reaches_mapper)
        .copied()
        .collect::<BTreeSet<_>>();
    if !selected.contains(&entry.entry_node) || !selected.contains(&entry.mapper_output) {
        return Err(PlanRuntimeError::InvalidPlan("意图入口无法到达 Mapper"));
    }

    let mut loops = BTreeMap::new();
    let mut body_nodes = BTreeSet::new();
    for region in plan.control_regions() {
        let ControlRegion::Loop(region) = region;
        if !selected.contains(&region.loop_node) {
            continue;
        }
        let selected_body = region
            .body_nodes
            .iter()
            .filter(|node_id| selected.contains(node_id))
            .copied()
            .collect::<BTreeSet<_>>();
        if selected_body.len() != region.body_nodes.len() {
            return Err(PlanRuntimeError::InvalidPlan(
                "Loop body 不在当前 intent 路径",
            ));
        }
        if selected_body
            .iter()
            .any(|node_id| !body_nodes.insert(*node_id))
        {
            return Err(PlanRuntimeError::InvalidPlan("Loop body region 重叠"));
        }
        let node_ids = topological_order(&selected_body, plan.edges())?;
        loops.insert(
            region.loop_node,
            LoopProgram {
                region: region.clone(),
                node_ids,
            },
        );
    }

    let outer_nodes = selected
        .difference(&body_nodes)
        .copied()
        .collect::<BTreeSet<_>>();
    let node_ids = topological_order(&outer_nodes, plan.edges())?;
    if node_ids.first().copied() != Some(entry.entry_node)
        || node_ids.last().copied() != Some(entry.mapper_output)
    {
        return Err(PlanRuntimeError::InvalidPlan(
            "意图 control program 必须从 entry 到 Mapper",
        ));
    }
    Ok(ExecutionPath {
        entry_node: entry.entry_node,
        mapper_output: entry.mapper_output,
        node_ids,
        loops,
    })
}

fn topological_order(
    selected: &BTreeSet<Uuid>,
    edges: &[PlanEdge],
) -> Result<Vec<Uuid>, PlanRuntimeError> {
    let mut indegrees = selected
        .iter()
        .map(|node_id| (*node_id, 0_usize))
        .collect::<BTreeMap<_, _>>();
    let mut children = BTreeMap::<Uuid, Vec<Uuid>>::new();
    for edge in edges {
        if selected.contains(&edge.from.node_id) && selected.contains(&edge.to.node_id) {
            let Some(indegree) = indegrees.get_mut(&edge.to.node_id) else {
                return Err(PlanRuntimeError::InvalidPlan("Plan 拓扑状态无效"));
            };
            *indegree = indegree.saturating_add(1);
            children
                .entry(edge.from.node_id)
                .or_default()
                .push(edge.to.node_id);
        }
    }
    for node_children in children.values_mut() {
        node_children.sort_unstable();
    }
    let mut ready = indegrees
        .iter()
        .filter_map(|(node_id, indegree)| (*indegree == 0).then_some(*node_id))
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(selected.len());
    while let Some(node_id) = ready.pop_first() {
        ordered.push(node_id);
        if let Some(node_children) = children.get(&node_id) {
            for child in node_children {
                let Some(indegree) = indegrees.get_mut(child) else {
                    return Err(PlanRuntimeError::InvalidPlan("Plan 拓扑状态无效"));
                };
                *indegree = indegree.saturating_sub(1);
                if *indegree == 0 {
                    ready.insert(*child);
                }
            }
        }
    }
    if ordered.len() != selected.len() {
        return Err(PlanRuntimeError::InvalidPlan("Plan 存在非结构化循环"));
    }
    Ok(ordered)
}

fn adjacency(edges: &[PlanEdge], reverse: bool) -> BTreeMap<Uuid, Vec<Uuid>> {
    let mut result = BTreeMap::<Uuid, Vec<Uuid>>::new();
    for edge in edges {
        let (start, end) = if reverse {
            (edge.to.node_id, edge.from.node_id)
        } else {
            (edge.from.node_id, edge.to.node_id)
        };
        result.entry(start).or_default().push(end);
    }
    for children in result.values_mut() {
        children.sort_unstable();
        children.dedup();
    }
    result
}

fn reachable(start: Uuid, adjacency: &BTreeMap<Uuid, Vec<Uuid>>) -> BTreeSet<Uuid> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([start]);
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current) {
            continue;
        }
        if let Some(children) = adjacency.get(&current) {
            queue.extend(children.iter().copied());
        }
    }
    seen
}

#[derive(Serialize)]
struct EffectFingerprint<'a> {
    plan_hash: &'a str,
    invocation_path: &'a InvocationPath,
    kind: &'a EffectKind,
    config_hash: String,
    input_hash: String,
}

/// 计算由 pinned Plan、exact invocation、节点配置与输入唯一确定的 effect fingerprint。
pub(super) fn effect_fingerprint(
    plan: &ExecutionPlan,
    node: &PlanNode,
    declaration: &EffectDeclaration,
    invocation_path: &InvocationPath,
    input: &EffectInput,
) -> Result<String, PlanRuntimeError> {
    let config_hash = hash_value(&node.config)?;
    let input_hash = match input {
        EffectInput::Intent(intent) => hash_value(intent)?,
        EffectInput::Output(output) => hash_value(output.as_ref())?,
        EffectInput::Json(value) => hash_value(value.as_ref())?,
    };
    hash_value(&EffectFingerprint {
        plan_hash: plan.plan_hash(),
        invocation_path,
        kind: &declaration.kind,
        config_hash,
        input_hash,
    })
}

fn calculated_plan_hash(plan: &ExecutionPlan) -> Result<String, PlanRuntimeError> {
    execution_plan_hash(plan).map_err(|_| PlanRuntimeError::CanonicalSerialization)
}

fn hash_value<T>(value: &T) -> Result<String, PlanRuntimeError>
where
    T: Serialize,
{
    let canonical = canonical_json(value).map_err(|_| PlanRuntimeError::CanonicalSerialization)?;
    let mut hasher = Hasher::new();
    hasher.update(canonical.as_bytes());
    Ok(hasher.finalize().to_hex().to_string())
}
