//! immutable Plan 的启动前验证、执行路径与 fingerprint。
//!
//! 所有检查都只读取 compiler 产出的 `ExecutionPlan`。路径选择使用有序集合保证同一 Plan
//! 得到稳定调度顺序；effect fingerprint 不含凭据或原始 payload，因此可安全作为 archive
//! 绑定键和 replay 严格校验的一部分。

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use blake3::Hasher;
use lj_capability::StandardIntent;
use lj_rule_model::{
    ControlledMapper, EffectDeclaration, EffectKind, ExecutionPlan, HttpSpec, PlanNode,
    PlanNodeKind, canonical_json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::effect::EffectInput;

use super::api::{PlanRuntimeConfig, PlanRuntimeError};

/// 已通过入口、拓扑和单上游限制验证的运行路径。
#[derive(Debug)]
pub(super) struct ExecutionPath {
    pub(super) node_ids: Vec<Uuid>,
    pub(super) predecessors: BTreeMap<Uuid, Uuid>,
}

/// 校验 Plan 的 immutable 身份、节点配置和 effect 声明。
pub(super) fn validate_plan(
    plan: &ExecutionPlan,
    config: &PlanRuntimeConfig,
) -> Result<(), PlanRuntimeError> {
    if plan.schema_version != config.plan_schema_version {
        return Err(PlanRuntimeError::SchemaVersionMismatch {
            expected: config.plan_schema_version,
            actual: plan.schema_version,
        });
    }
    if plan.compiler_version != config.compiler_version {
        return Err(PlanRuntimeError::CompilerVersionMismatch);
    }
    if plan.plan_hash != calculated_plan_hash(plan)? {
        return Err(PlanRuntimeError::PlanHashMismatch);
    }
    if plan.definition_hash.trim().is_empty() {
        return Err(PlanRuntimeError::InvalidPlan("definition_hash 不能为空"));
    }

    let mut node_ids = BTreeSet::new();
    for node in &plan.nodes {
        if !node_ids.insert(node.id) {
            return Err(PlanRuntimeError::InvalidPlan("Plan 节点 ID 重复"));
        }
        validate_node(node)?;
    }
    for (from, to) in &plan.edges {
        if !node_ids.contains(from) || !node_ids.contains(to) {
            return Err(PlanRuntimeError::InvalidPlan("Plan 边引用不存在的节点"));
        }
    }
    for (intent, entry) in &plan.intent_entries {
        if entry.intent != *intent {
            return Err(PlanRuntimeError::InvalidPlan("意图入口键与内容不一致"));
        }
        let Some(mapper) = plan
            .nodes
            .iter()
            .find(|node| node.id == entry.mapper_output)
        else {
            return Err(PlanRuntimeError::MissingNode(entry.mapper_output));
        };
        if mapper.kind != PlanNodeKind::Mapper {
            return Err(PlanRuntimeError::InvalidPlan("意图输出节点必须是 Mapper"));
        }
        if !node_ids.contains(&entry.entry_node) {
            return Err(PlanRuntimeError::MissingNode(entry.entry_node));
        }
    }
    validate_effect_declarations(plan)?;
    Ok(())
}

fn validate_node(node: &PlanNode) -> Result<(), PlanRuntimeError> {
    match node.kind {
        PlanNodeKind::Http => {
            let _: HttpSpec = serde_json::from_value(node.config.clone())
                .map_err(|_| PlanRuntimeError::InvalidPlan("HTTP 节点配置无效"))?;
        }
        PlanNodeKind::Js => {
            let valid = node
                .config
                .get("code")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|code| !code.trim().is_empty());
            if !valid {
                return Err(PlanRuntimeError::InvalidPlan("JS 节点配置无效"));
            }
        }
        PlanNodeKind::Extract => {
            let _: lj_rule_model::ExtractSpec = serde_json::from_value(node.config.clone())
                .map_err(|_| PlanRuntimeError::InvalidPlan("Extract 节点配置无效"))?;
        }
        PlanNodeKind::Mapper => {
            let _: ControlledMapper = serde_json::from_value(node.config.clone())
                .map_err(|_| PlanRuntimeError::InvalidPlan("Mapper 节点配置无效"))?;
        }
        PlanNodeKind::Merge | PlanNodeKind::Condition | PlanNodeKind::Loop => {
            return Err(PlanRuntimeError::UnsupportedControlFlow);
        }
    }
    Ok(())
}

fn validate_effect_declarations(plan: &ExecutionPlan) -> Result<(), PlanRuntimeError> {
    let mut declared = BTreeSet::new();
    for effect in &plan.effects {
        if !declared.insert(effect.node_id) {
            return Err(PlanRuntimeError::InvalidPlan("effect 声明重复"));
        }
        let Some(node) = plan.nodes.iter().find(|node| node.id == effect.node_id) else {
            return Err(PlanRuntimeError::MissingNode(effect.node_id));
        };
        if expected_effect_kind(&node.kind) != Some(effect.kind.clone()) {
            return Err(PlanRuntimeError::InvalidPlan("effect 声明与节点类型不一致"));
        }
    }
    for node in &plan.nodes {
        if expected_effect_kind(&node.kind).is_some() && !declared.contains(&node.id) {
            return Err(PlanRuntimeError::InvalidPlan("effect 节点缺少声明"));
        }
    }
    Ok(())
}

fn expected_effect_kind(kind: &PlanNodeKind) -> Option<EffectKind> {
    match kind {
        PlanNodeKind::Http => Some(EffectKind::Http),
        PlanNodeKind::Js => Some(EffectKind::QuickJs),
        PlanNodeKind::Extract => Some(EffectKind::Extract),
        PlanNodeKind::Mapper
        | PlanNodeKind::Merge
        | PlanNodeKind::Condition
        | PlanNodeKind::Loop => None,
    }
}

/// 选择请求 intent 从入口到 Mapper 的唯一、有界拓扑路径。
pub(super) fn execution_path(
    plan: &ExecutionPlan,
    intent: StandardIntent,
) -> Result<ExecutionPath, PlanRuntimeError> {
    let Some(entry) = plan.intent_entries.get(&intent) else {
        return Err(PlanRuntimeError::MissingIntent);
    };
    let forward = adjacency(&plan.edges, false);
    let reverse = adjacency(&plan.edges, true);
    let reachable_from_entry = reachable(entry.entry_node, &forward);
    let reaches_mapper = reachable(entry.mapper_output, &reverse);
    let selected: BTreeSet<Uuid> = reachable_from_entry
        .intersection(&reaches_mapper)
        .copied()
        .collect();
    if !selected.contains(&entry.entry_node) || !selected.contains(&entry.mapper_output) {
        return Err(PlanRuntimeError::InvalidPlan("意图入口无法到达 Mapper"));
    }

    let mut indegrees = BTreeMap::<Uuid, usize>::new();
    let mut predecessors = BTreeMap::<Uuid, Uuid>::new();
    for node_id in &selected {
        indegrees.insert(*node_id, 0);
    }
    for (from, to) in &plan.edges {
        if selected.contains(from) && selected.contains(to) {
            let Some(indegree) = indegrees.get_mut(to) else {
                return Err(PlanRuntimeError::InvalidPlan("Plan 拓扑状态无效"));
            };
            *indegree += 1;
            if *to != entry.entry_node && predecessors.insert(*to, *from).is_some() {
                return Err(PlanRuntimeError::InvalidPlan("Plan 节点有多个上游输入"));
            }
        }
    }
    if indegrees
        .get(&entry.entry_node)
        .copied()
        .unwrap_or_default()
        != 0
    {
        return Err(PlanRuntimeError::InvalidPlan("意图入口不能有上游依赖"));
    }
    for node_id in &selected {
        if *node_id != entry.entry_node && !predecessors.contains_key(node_id) {
            return Err(PlanRuntimeError::InvalidPlan("Plan 节点缺少上游输入"));
        }
    }

    let mut ready = indegrees
        .iter()
        .filter_map(|(node_id, indegree)| (*indegree == 0).then_some(*node_id))
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(selected.len());
    while let Some(node_id) = ready.pop_first() {
        ordered.push(node_id);
        if let Some(children) = forward.get(&node_id) {
            for child in children {
                if !selected.contains(child) {
                    continue;
                }
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
        return Err(PlanRuntimeError::InvalidPlan("Plan 拓扑存在循环"));
    }
    if ordered.last().copied() != Some(entry.mapper_output) {
        return Err(PlanRuntimeError::InvalidPlan("意图路径必须以 Mapper 结束"));
    }
    Ok(ExecutionPath {
        node_ids: ordered,
        predecessors,
    })
}

fn adjacency(edges: &[(Uuid, Uuid)], reverse: bool) -> BTreeMap<Uuid, Vec<Uuid>> {
    let mut result = BTreeMap::<Uuid, Vec<Uuid>>::new();
    for (from, to) in edges {
        let (start, end) = if reverse { (*to, *from) } else { (*from, *to) };
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
    node_id: Uuid,
    kind: &'a EffectKind,
    config_hash: String,
    input_hash: String,
}

/// 计算由 pinned Plan、节点和已确认输入唯一确定的 effect fingerprint。
pub(super) fn effect_fingerprint(
    plan: &ExecutionPlan,
    node: &PlanNode,
    declaration: &EffectDeclaration,
    input: &EffectInput,
) -> Result<String, PlanRuntimeError> {
    let config_hash = hash_value(&node.config)?;
    let input_hash = match input {
        EffectInput::Intent(intent) => hash_value(intent)?,
        EffectInput::Output(output) => hash_value(output.as_ref())?,
    };
    hash_value(&EffectFingerprint {
        plan_hash: &plan.plan_hash,
        node_id: node.id,
        kind: &declaration.kind,
        config_hash,
        input_hash,
    })
}

fn calculated_plan_hash(plan: &ExecutionPlan) -> Result<String, PlanRuntimeError> {
    let mut hashed = plan.clone();
    hashed.plan_hash.clear();
    hash_value(&hashed)
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
