//! immutable Plan 的 current sealing、canonical hash 与 wire reader/writer。

use std::collections::BTreeMap;

use blake3::Hasher;
use lj_capability::StandardIntent;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::types::{
    ControlRegion, EffectDeclaration, ExecutionPlanParts, IntentEntry, PlanEdge, PlanNode,
    PlanNodeConfig, PortValueType,
};
use crate::error::Error;
use crate::hash::canonical_json;
use crate::schema::{
    RULE_CONTRACT_SCHEMA_VERSION, SchemaContract, SchemaReadError, invalid_data,
    parse_contract_json, validate_contract_value,
};

/// 不可变执行计划。
///
/// 公开构造器创建并 seal 唯一 current Plan。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    compiler_version: String,
    definition_hash: String,
    descriptor_digest: String,
    plan_hash: String,
    nodes: Vec<PlanNode>,
    edges: Vec<PlanEdge>,
    intent_entries: BTreeMap<StandardIntent, IntentEntry>,
    effects: Vec<EffectDeclaration>,
    capability_requirements: Vec<String>,
    control_regions: Vec<ControlRegion>,
}

impl ExecutionPlan {
    /// 创建、hash 并 seal 一个 current immutable Plan。
    ///
    /// # Errors
    ///
    /// typed Plan 无法 canonical JSON 序列化时返回 [`Error::Json`]。
    pub fn new(
        compiler_version: impl Into<String>,
        definition_hash: impl Into<String>,
        parts: ExecutionPlanParts,
    ) -> Result<Self, Error> {
        let mut plan = Self {
            compiler_version: compiler_version.into(),
            definition_hash: definition_hash.into(),
            descriptor_digest: crate::descriptor::descriptor_set_digest().to_string(),
            plan_hash: String::new(),
            nodes: parts.nodes,
            edges: parts.edges,
            intent_entries: parts.intent_entries,
            effects: parts.effects,
            capability_requirements: parts.capability_requirements,
            control_regions: parts.control_regions,
        };
        plan.plan_hash = execution_plan_hash(&plan)?;
        Ok(plan)
    }

    /// 返回 compiler 身份。
    #[must_use]
    pub fn compiler_version(&self) -> &str {
        &self.compiler_version
    }

    /// 返回源 Definition canonical hash。
    #[must_use]
    pub fn definition_hash(&self) -> &str {
        &self.definition_hash
    }

    /// 返回 sealed Plan hash。
    #[must_use]
    pub fn plan_hash(&self) -> &str {
        &self.plan_hash
    }

    /// 返回 seal 时使用的节点能力声明表 digest。
    ///
    /// Plan 的 port/字段语义由该版本声明表决定；runtime 在启动前比对当前表 digest，不一致
    /// 即拒绝，不静默执行语义已漂移的 Plan。
    #[must_use]
    pub fn descriptor_digest(&self) -> &str {
        &self.descriptor_digest
    }

    /// 返回 typed Plan 节点。
    #[must_use]
    pub fn nodes(&self) -> &[PlanNode] {
        &self.nodes
    }

    /// 返回 typed handle edges。
    #[must_use]
    pub fn edges(&self) -> &[PlanEdge] {
        &self.edges
    }

    /// 返回意图入口表。
    #[must_use]
    pub const fn intent_entries(&self) -> &BTreeMap<StandardIntent, IntentEntry> {
        &self.intent_entries
    }

    /// 返回 effect 声明。
    #[must_use]
    pub fn effects(&self) -> &[EffectDeclaration] {
        &self.effects
    }

    /// 返回能力需求汇总。
    #[must_use]
    pub fn capability_requirements(&self) -> &[String] {
        &self.capability_requirements
    }

    /// 返回 compiler 已证明的 structured control regions。
    #[must_use]
    pub fn control_regions(&self) -> &[ControlRegion] {
        &self.control_regions
    }

    /// 判断 Plan 是否包含当前阶段 runtime 尚未开放的控制节点/region。
    #[must_use]
    pub fn has_control_flow(&self) -> bool {
        !self.control_regions.is_empty()
            || self.nodes.iter().any(|node| {
                matches!(
                    node.config,
                    PlanNodeConfig::Merge(_)
                        | PlanNodeConfig::Condition(_)
                        | PlanNodeConfig::Loop(_)
                )
            })
    }
}

/// 计算 Plan 的 current canonical BLAKE3 hash（hex）。
///
/// 覆盖全部 typed config、ports、edges 与 control regions；计算前把 `plan_hash` 清空。
/// 物理数组排列不进入 hash，显式 Merge `order` 与全部控制语义进入 hash。
///
/// # Errors
///
/// hash material 无法 canonical JSON 序列化时返回 [`Error::Json`]。
pub fn execution_plan_hash(plan: &ExecutionPlan) -> Result<String, Error> {
    let mut nodes = plan.nodes.clone();
    for node in &mut nodes {
        canonicalize_plan_node(node);
    }
    nodes.sort_by_key(|node| node.id);

    let mut edges = plan.edges.clone();
    edges.sort();

    let mut effects = plan.effects.clone();
    for effect in &mut effects {
        effect
            .required_capabilities
            .sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    }
    effects.sort_by_key(|effect| effect.node_id);

    let mut capability_requirements = plan.capability_requirements.clone();
    capability_requirements.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));

    let mut control_regions = plan.control_regions.clone();
    for region in &mut control_regions {
        let ControlRegion::Loop(region) = region;
        region.body_nodes.sort_unstable();
    }
    control_regions.sort_by_key(|region| match region {
        ControlRegion::Loop(region) => region.loop_node,
    });

    hash_canonical(&ExecutionPlanWireRef {
        schema_version: RULE_CONTRACT_SCHEMA_VERSION,
        compiler_version: &plan.compiler_version,
        definition_hash: &plan.definition_hash,
        descriptor_digest: &plan.descriptor_digest,
        plan_hash: "",
        nodes: &nodes,
        edges: &edges,
        intent_entries: &plan.intent_entries,
        effects: &effects,
        capability_requirements: &capability_requirements,
        control_regions: &control_regions,
    })
}

/// 从 JSON bytes 读取唯一 current `ExecutionPlan`。
///
/// # Errors
///
/// JSON 无效、contract tag 不匹配、未知 schema、字段未知或 current shape
/// 损坏时返回 [`SchemaReadError`]。
pub fn read_execution_plan(bytes: &[u8]) -> Result<ExecutionPlan, SchemaReadError> {
    let value = parse_contract_json(bytes, SchemaContract::ExecutionPlan)?;
    execution_plan_from_value(value)
}

impl Serialize for ExecutionPlan {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.wire_ref(&self.plan_hash).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ExecutionPlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        validate_contract_value(&value, SchemaContract::ExecutionPlan).map_err(D::Error::custom)?;
        execution_plan_from_value(value).map_err(D::Error::custom)
    }
}

impl ExecutionPlan {
    fn wire_ref<'a>(&'a self, plan_hash: &'a str) -> ExecutionPlanWireRef<'a> {
        ExecutionPlanWireRef {
            schema_version: RULE_CONTRACT_SCHEMA_VERSION,
            compiler_version: &self.compiler_version,
            definition_hash: &self.definition_hash,
            descriptor_digest: &self.descriptor_digest,
            plan_hash,
            nodes: &self.nodes,
            edges: &self.edges,
            intent_entries: &self.intent_entries,
            effects: &self.effects,
            capability_requirements: &self.capability_requirements,
            control_regions: &self.control_regions,
        }
    }
}

fn canonicalize_plan_node(node: &mut PlanNode) {
    for port in node.inputs.iter_mut().chain(&mut node.outputs) {
        if let PortValueType::Union { kinds } = &mut port.value_type {
            kinds.sort_unstable();
        }
    }
    node.inputs
        .sort_by(|left, right| left.handle.as_bytes().cmp(right.handle.as_bytes()));
    node.outputs
        .sort_by(|left, right| left.handle.as_bytes().cmp(right.handle.as_bytes()));
    match &mut node.config {
        PlanNodeConfig::Mapper(config) => config.identity_fields.sort(),
        PlanNodeConfig::Merge(config) => {
            config.inputs.sort_by_key(|input| input.order);
        }
        PlanNodeConfig::Condition(config) => {
            config
                .branches
                .sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
        }
        PlanNodeConfig::Http(_)
        | PlanNodeConfig::Js(_)
        | PlanNodeConfig::Extract(_)
        | PlanNodeConfig::Loop(_) => {}
    }
}

fn hash_canonical(value: &impl Serialize) -> Result<String, Error> {
    let canonical = canonical_json(value)?;
    let mut hasher = Hasher::new();
    hasher.update(canonical.as_bytes());
    Ok(hasher.finalize().to_hex().to_string())
}

#[derive(Serialize)]
#[serde(tag = "contract", rename = "execution_plan", deny_unknown_fields)]
struct ExecutionPlanWireRef<'a> {
    schema_version: u32,
    compiler_version: &'a str,
    definition_hash: &'a str,
    descriptor_digest: &'a str,
    plan_hash: &'a str,
    nodes: &'a [PlanNode],
    edges: &'a [PlanEdge],
    intent_entries: &'a BTreeMap<StandardIntent, IntentEntry>,
    effects: &'a [EffectDeclaration],
    capability_requirements: &'a [String],
    control_regions: &'a [ControlRegion],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutionPlanWireOwned {
    schema_version: u32,
    compiler_version: String,
    definition_hash: String,
    descriptor_digest: String,
    plan_hash: String,
    nodes: Vec<PlanNode>,
    edges: Vec<PlanEdge>,
    intent_entries: BTreeMap<StandardIntent, IntentEntry>,
    effects: Vec<EffectDeclaration>,
    capability_requirements: Vec<String>,
    control_regions: Vec<ControlRegion>,
}

fn strip_contract_tag(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(object) = value.as_object_mut() {
        object.remove("contract");
    }
    value
}

fn execution_plan_from_value(value: serde_json::Value) -> Result<ExecutionPlan, SchemaReadError> {
    let wire = serde_json::from_value::<ExecutionPlanWireOwned>(strip_contract_tag(value))
        .map_err(|error| invalid_data(SchemaContract::ExecutionPlan, error.to_string()))?;
    if wire.schema_version != RULE_CONTRACT_SCHEMA_VERSION {
        return Err(SchemaReadError::SchemaUnsupported {
            contract: SchemaContract::ExecutionPlan,
            version: wire.schema_version,
        });
    }
    Ok(ExecutionPlan {
        compiler_version: wire.compiler_version,
        definition_hash: wire.definition_hash,
        descriptor_digest: wire.descriptor_digest,
        plan_hash: wire.plan_hash,
        nodes: wire.nodes,
        edges: wire.edges,
        intent_entries: wire.intent_entries,
        effects: wire.effects,
        capability_requirements: wire.capability_requirements,
        control_regions: wire.control_regions,
    })
}
