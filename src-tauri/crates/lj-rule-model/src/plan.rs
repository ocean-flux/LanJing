//! Execution Plan — compiler 产出的不可变 runtime 投影。
//!
//! Plan v2 只包含 typed config、closed port value union、handle edge 与 structured control
//! region。v1 installed Plan 由 reader 投影到同一内存 API，同时保留精确 legacy hash material。

use std::collections::BTreeMap;

use blake3::Hasher;
use lj_capability::StandardIntent;
use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::definition::{
    CollectionSelector, ConditionConfig, ControlledMapper, FlowPortRef, JsConfig, JsOutputKind,
    LoopIterationLimit, MergeConfig,
};
use crate::endpoint::HttpSpec;
use crate::error::Error;
use crate::extract_rule::ExtractSpec;
use crate::hash::canonical_json;
use crate::schema::{
    ContractSchemaVersion, EXECUTION_PLAN_SCHEMA_VERSION, LEGACY_CONTRACT_SCHEMA_VERSION,
    SchemaContract, SchemaReadError, invalid_data, parse_contract_json, validate_contract_value,
};

/// 普通线性节点的固定 input handle。
pub const LINEAR_INPUT_HANDLE: &str = "input";
/// 普通线性节点的固定 output handle。
pub const LINEAR_OUTPUT_HANDLE: &str = "output";
/// Condition 的固定 input handle。
pub const CONDITION_INPUT_HANDLE: &str = "input";
/// Merge 的固定 output handle。
pub const MERGE_OUTPUT_HANDLE: &str = "output";
/// Loop collection input 的固定 handle。
pub const LOOP_COLLECTION_HANDLE: &str = "collection";
/// Loop body entry payload 的固定 output handle。
pub const LOOP_BODY_HANDLE: &str = "body";
/// Loop body structured return 的固定 input handle。
pub const LOOP_YIELD_HANDLE: &str = "yield";
/// Loop collected result 的固定 output handle。
pub const LOOP_DONE_HANDLE: &str = "done";

/// Plan 节点类型判别值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanNodeKind {
    /// HTTP effect 节点。
    Http,
    /// QuickJS effect 节点。
    Js,
    /// 提取节点。
    Extract,
    /// 受控 Mapper。
    Mapper,
    /// 控制流合并。
    Merge,
    /// 条件分支。
    Condition,
    /// bounded for-each。
    Loop,
}

/// Plan port 可引用的闭集 runtime value kind。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortValueKind {
    /// 标准意图输入。
    IntentInput,
    /// 原始文本/bytes 语义。
    Raw,
    /// HTTP response envelope。
    HttpResponse,
    /// JSON 值。
    Json,
    /// 标准媒体 Delta。
    Delta,
    /// Loop body 的 `{ item, index }` typed binding payload。
    LoopBinding,
}

/// 单一 kind 或显式 closed union；不存在开放 `any`/string type tag。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum PortValueType {
    /// 单一 runtime value kind。
    Kind {
        /// closed kind。
        kind: PortValueKind,
    },
    /// compiler 明确列举的 closed kind union。
    Union {
        /// union 成员；compiler 负责非空、唯一与 canonical sort。
        kinds: Vec<PortValueKind>,
    },
}

impl PortValueType {
    /// 创建单一 kind。
    #[must_use]
    pub const fn kind(kind: PortValueKind) -> Self {
        Self::Kind { kind }
    }

    /// 创建显式 closed union。
    #[must_use]
    pub fn union(kinds: impl IntoIterator<Item = PortValueKind>) -> Self {
        Self::Union {
            kinds: kinds.into_iter().collect(),
        }
    }
}

/// 类型化 Plan port。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanPort {
    /// 固定或经 compiler 验证的动态 handle。
    pub handle: String,
    /// closed value kind/union。
    pub value_type: PortValueType,
}

impl PlanPort {
    /// 创建 typed port。
    #[must_use]
    pub fn new(handle: impl Into<String>, value_type: PortValueType) -> Self {
        Self {
            handle: handle.into(),
            value_type,
        }
    }
}

/// Effect 种类。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectKind {
    /// HTTP 外部效应。
    Http,
    /// QuickJS 外部效应。
    QuickJs,
    /// 纯提取（无外部效应）。
    Extract,
}

/// Effect 声明。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectDeclaration {
    /// 所属 plan 节点。
    pub node_id: Uuid,
    /// 效应种类。
    pub kind: EffectKind,
    /// 所需能力标签。
    pub required_capabilities: Vec<String>,
}

/// 意图入口表项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntentEntry {
    /// 标准意图。
    pub intent: StandardIntent,
    /// Flow/Plan 入口节点。
    pub entry_node: Uuid,
    /// Mapper 输出节点。
    pub mapper_output: Uuid,
}

/// compiler 校验后写入 immutable Plan 的 bounded for-each 配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanForEachConfig {
    /// typed/JS collection selector。
    pub collection: CollectionSelector,
    /// body payload 中 item 的 binding 名。
    pub item_binding: String,
    /// body payload 中 index 的 binding 名。
    pub index_binding: String,
    /// 已验证的 iteration limit。
    max_iterations: LoopIterationLimit,
}

impl PlanForEachConfig {
    /// 用 compiler 已验证的 limit 创建 Plan Loop config。
    #[must_use]
    pub fn new(
        collection: CollectionSelector,
        item_binding: impl Into<String>,
        index_binding: impl Into<String>,
        max_iterations: LoopIterationLimit,
    ) -> Self {
        Self {
            collection,
            item_binding: item_binding.into(),
            index_binding: index_binding.into(),
            max_iterations,
        }
    }

    /// 返回已验证的 iteration limit。
    #[must_use]
    pub const fn max_iterations(&self) -> LoopIterationLimit {
        self.max_iterations
    }
}

/// 无 kind/config mismatch 的 closed Plan 节点配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum PlanNodeConfig {
    /// HTTP 请求。
    Http(HttpSpec),
    /// QuickJS 计算。
    Js(JsConfig),
    /// typed 提取。
    Extract(ExtractSpec),
    /// 受控 Mapper。
    Mapper(ControlledMapper),
    /// 多输入合并。
    Merge(MergeConfig),
    /// 条件分支。
    Condition(ConditionConfig),
    /// bounded for-each。
    Loop(PlanForEachConfig),
}

impl PlanNodeConfig {
    /// 返回 active config 的节点判别值。
    #[must_use]
    pub const fn kind(&self) -> PlanNodeKind {
        match self {
            Self::Http(_) => PlanNodeKind::Http,
            Self::Js(_) => PlanNodeKind::Js,
            Self::Extract(_) => PlanNodeKind::Extract,
            Self::Mapper(_) => PlanNodeKind::Mapper,
            Self::Merge(_) => PlanNodeKind::Merge,
            Self::Condition(_) => PlanNodeKind::Condition,
            Self::Loop(_) => PlanNodeKind::Loop,
        }
    }
}

/// Plan 节点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanNode {
    /// 节点 ID。
    pub id: Uuid,
    /// 输入 ports。
    pub inputs: Vec<PlanPort>,
    /// 输出 ports。
    pub outputs: Vec<PlanPort>,
    /// 已 canonicalize 的 typed config。
    pub config: PlanNodeConfig,
}

impl PlanNode {
    /// 返回与 config 一致的节点类型。
    #[must_use]
    pub const fn kind(&self) -> PlanNodeKind {
        self.config.kind()
    }
}

/// immutable Plan 的 typed handle edge。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanEdge {
    /// 起始 output port。
    pub from: FlowPortRef,
    /// 目标 input port。
    pub to: FlowPortRef,
}

impl PlanEdge {
    /// 创建 typed Plan edge。
    #[must_use]
    pub const fn new(from: FlowPortRef, to: FlowPortRef) -> Self {
        Self { from, to }
    }

    /// 返回用于去重和 canonical sorting 的完整 semantic identity。
    #[must_use]
    pub const fn semantic_identity(&self) -> (&FlowPortRef, &FlowPortRef) {
        (&self.from, &self.to)
    }
}

/// compiler 已证明的 structured Loop region。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoopControlRegion {
    /// region owner Loop 节点。
    pub loop_node: Uuid,
    /// `body` edge 的 region 内目标 port。
    pub body_entry: FlowPortRef,
    /// 唯一 `yield` structured return 的 region 内来源 port。
    pub yield_source: FlowPortRef,
    /// region 内节点集合；声明顺序不承载语义。
    pub body_nodes: Vec<Uuid>,
}

/// immutable Plan 的闭集 control region。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ControlRegion {
    /// bounded for-each region。
    Loop(LoopControlRegion),
}

/// 不可变执行计划。
///
/// 公开构造器只创建并 seal v2；v1 仅由 reader 产生，且 serde writer 会拒绝只读 v1 对象。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlan {
    compiler_version: String,
    definition_hash: String,
    plan_hash: String,
    nodes: Vec<PlanNode>,
    edges: Vec<PlanEdge>,
    intent_entries: BTreeMap<StandardIntent, IntentEntry>,
    effects: Vec<EffectDeclaration>,
    capability_requirements: Vec<String>,
    control_regions: Vec<ControlRegion>,
    legacy_v1: Option<Box<LegacyExecutionPlanV1>>,
}

impl ExecutionPlan {
    /// 创建、hash 并 seal 一个 v2 immutable Plan。
    ///
    /// # Errors
    ///
    /// typed Plan 无法 canonical JSON 序列化时返回 [`Error::Json`]。
    pub fn new(
        compiler_version: impl Into<String>,
        definition_hash: impl Into<String>,
        nodes: Vec<PlanNode>,
        edges: Vec<PlanEdge>,
        intent_entries: BTreeMap<StandardIntent, IntentEntry>,
        effects: Vec<EffectDeclaration>,
        capability_requirements: Vec<String>,
        control_regions: Vec<ControlRegion>,
    ) -> Result<Self, Error> {
        let mut plan = Self {
            compiler_version: compiler_version.into(),
            definition_hash: definition_hash.into(),
            plan_hash: String::new(),
            nodes,
            edges,
            intent_entries,
            effects,
            capability_requirements,
            control_regions,
            legacy_v1: None,
        };
        plan.plan_hash = execution_plan_hash(&plan)?;
        Ok(plan)
    }

    /// 返回内存对象来源的已知 schema 版本。
    #[must_use]
    pub const fn schema_version(&self) -> ContractSchemaVersion {
        if self.legacy_v1.is_some() {
            ContractSchemaVersion::V1
        } else {
            ContractSchemaVersion::V2
        }
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

    /// 返回版本正确的 sealed Plan hash。
    #[must_use]
    pub fn plan_hash(&self) -> &str {
        &self.plan_hash
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

/// 计算 Plan 的版本正确 canonical BLAKE3 hash（hex）。
///
/// v1 reader object 使用保留的精确旧 wire material；v2 使用全部 typed config、ports、edges
/// 与 control regions。两种版本都在计算前把 `plan_hash` 清空。
///
/// # Errors
///
/// hash material 无法 canonical JSON 序列化时返回 [`Error::Json`]。
pub fn execution_plan_hash(plan: &ExecutionPlan) -> Result<String, Error> {
    if let Some(legacy) = &plan.legacy_v1 {
        let mut material = legacy.as_ref().clone();
        material.plan_hash.clear();
        let mut value = serde_json::to_value(&material)?;
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "contract".to_string(),
                serde_json::Value::String("execution_plan".to_string()),
            );
        }
        return hash_canonical(&value);
    }
    execution_plan_v2_hash(plan)
}

/// 从 JSON bytes 读取 v1/v2 ExecutionPlan，并为未知版本返回 typed incompatible。
///
/// # Errors
///
/// JSON 无效、contract tag 不匹配、版本未知、字段未知，或旧 Plan 不是可转换的线性
/// Http/JS/Extract/Mapper 图时返回 [`SchemaReadError`]。
pub fn read_execution_plan(bytes: &[u8]) -> Result<ExecutionPlan, SchemaReadError> {
    let (value, version) = parse_contract_json(bytes, SchemaContract::ExecutionPlan)?;
    execution_plan_from_value(value, version)
}

impl Serialize for ExecutionPlan {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.legacy_v1.is_some() {
            return Err(S::Error::custom(
                "v1 ExecutionPlan 只读；请由 compiler 构造并 seal 新的 v2 Plan",
            ));
        }
        self.v2_ref(&self.plan_hash).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ExecutionPlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let version = validate_contract_value(&value, SchemaContract::ExecutionPlan)
            .map_err(D::Error::custom)?;
        execution_plan_from_value(value, version).map_err(D::Error::custom)
    }
}

impl ExecutionPlan {
    fn v2_ref<'a>(&'a self, plan_hash: &'a str) -> ExecutionPlanV2Ref<'a> {
        ExecutionPlanV2Ref {
            schema_version: EXECUTION_PLAN_SCHEMA_VERSION,
            compiler_version: &self.compiler_version,
            definition_hash: &self.definition_hash,
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

fn execution_plan_v2_hash(plan: &ExecutionPlan) -> Result<String, Error> {
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

    hash_canonical(&ExecutionPlanV2Ref {
        schema_version: EXECUTION_PLAN_SCHEMA_VERSION,
        compiler_version: &plan.compiler_version,
        definition_hash: &plan.definition_hash,
        plan_hash: "",
        nodes: &nodes,
        edges: &edges,
        intent_entries: &plan.intent_entries,
        effects: &effects,
        capability_requirements: &capability_requirements,
        control_regions: &control_regions,
    })
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
            config
                .inputs
                .sort_by(|left, right| left.handle.as_bytes().cmp(right.handle.as_bytes()));
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
struct ExecutionPlanV2Ref<'a> {
    schema_version: u32,
    compiler_version: &'a str,
    definition_hash: &'a str,
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
struct ExecutionPlanV2Owned {
    schema_version: u32,
    compiler_version: String,
    definition_hash: String,
    plan_hash: String,
    nodes: Vec<PlanNode>,
    edges: Vec<PlanEdge>,
    intent_entries: BTreeMap<StandardIntent, IntentEntry>,
    effects: Vec<EffectDeclaration>,
    capability_requirements: Vec<String>,
    control_regions: Vec<ControlRegion>,
}

fn strip_contract_tag(mut value: serde_json::Value) -> serde_json::Value {
    strip_contract_tag_in_place(&mut value);
    value
}

fn strip_contract_tag_in_place(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            object.remove("contract");
            for child in object.values_mut() {
                strip_contract_tag_in_place(child);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                strip_contract_tag_in_place(item);
            }
        }
        _ => {}
    }
}

fn execution_plan_from_value(
    value: serde_json::Value,
    version: ContractSchemaVersion,
) -> Result<ExecutionPlan, SchemaReadError> {
    match version {
        ContractSchemaVersion::V1 => {
            let legacy = serde_json::from_value::<LegacyExecutionPlanV1>(strip_contract_tag(value))
                .map_err(|error| invalid_data(SchemaContract::ExecutionPlan, error.to_string()))?;
            ExecutionPlan::try_from_legacy(legacy)
        }
        ContractSchemaVersion::V2 => {
            let wire = serde_json::from_value::<ExecutionPlanV2Owned>(strip_contract_tag(value))
                .map_err(|error| invalid_data(SchemaContract::ExecutionPlan, error.to_string()))?;
            if wire.schema_version != EXECUTION_PLAN_SCHEMA_VERSION {
                return Err(SchemaReadError::IncompatibleVersion {
                    contract: SchemaContract::ExecutionPlan,
                    version: wire.schema_version,
                });
            }
            Ok(ExecutionPlan {
                compiler_version: wire.compiler_version,
                definition_hash: wire.definition_hash,
                plan_hash: wire.plan_hash,
                nodes: wire.nodes,
                edges: wire.edges,
                intent_entries: wire.intent_entries,
                effects: wire.effects,
                capability_requirements: wire.capability_requirements,
                control_regions: wire.control_regions,
                legacy_v1: None,
            })
        }
    }
}

impl ExecutionPlan {
    fn try_from_legacy(legacy: LegacyExecutionPlanV1) -> Result<Self, SchemaReadError> {
        if legacy.schema_version != LEGACY_CONTRACT_SCHEMA_VERSION {
            return Err(SchemaReadError::IncompatibleVersion {
                contract: SchemaContract::ExecutionPlan,
                version: legacy.schema_version,
            });
        }
        let nodes = legacy
            .nodes
            .iter()
            .map(LegacyPlanNodeV1::to_v2)
            .collect::<Result<Vec<_>, _>>()?;
        let handles = legacy
            .nodes
            .iter()
            .map(|node| {
                (
                    node.id,
                    (
                        node.outputs.first().map(|port| port.name.clone()),
                        node.inputs.first().map(|port| port.name.clone()),
                    ),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let edges = legacy
            .edges
            .iter()
            .map(|(from, to)| {
                let from_handle = handles
                    .get(from)
                    .and_then(|(output, _)| output.clone())
                    .ok_or_else(|| {
                        invalid_data(
                            SchemaContract::ExecutionPlan,
                            "v1 edge source 缺少 output port",
                        )
                    })?;
                let to_handle = handles
                    .get(to)
                    .and_then(|(_, input)| input.clone())
                    .ok_or_else(|| {
                        invalid_data(
                            SchemaContract::ExecutionPlan,
                            "v1 edge target 缺少 input port",
                        )
                    })?;
                Ok(PlanEdge::new(
                    FlowPortRef::new(*from, from_handle),
                    FlowPortRef::new(*to, to_handle),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let material = legacy.clone();
        Ok(Self {
            compiler_version: legacy.compiler_version,
            definition_hash: legacy.definition_hash,
            plan_hash: legacy.plan_hash,
            nodes,
            edges,
            intent_entries: legacy.intent_entries,
            effects: legacy.effects,
            capability_requirements: legacy.capability_requirements,
            control_regions: Vec::new(),
            legacy_v1: Some(Box::new(material)),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyPlanPortV1 {
    name: String,
    type_tag: String,
}

impl LegacyPlanPortV1 {
    fn to_v2(&self) -> Result<PlanPort, SchemaReadError> {
        let value_type = match self.type_tag.as_str() {
            "value" => PortValueType::union([
                PortValueKind::IntentInput,
                PortValueKind::Raw,
                PortValueKind::Json,
            ]),
            "raw" => PortValueType::kind(PortValueKind::Raw),
            "http_response" => PortValueType::kind(PortValueKind::HttpResponse),
            "json" => PortValueType::kind(PortValueKind::Json),
            "delta" => PortValueType::kind(PortValueKind::Delta),
            _ => {
                return Err(invalid_data(
                    SchemaContract::ExecutionPlan,
                    "v1 PlanPort 包含未知 type_tag",
                ));
            }
        };
        Ok(PlanPort::new(self.name.clone(), value_type))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum LegacyPlanNodeKindV1 {
    Http,
    Js,
    Extract,
    Mapper,
    Merge,
    Condition,
    Loop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyPlanNodeV1 {
    id: Uuid,
    kind: LegacyPlanNodeKindV1,
    inputs: Vec<LegacyPlanPortV1>,
    outputs: Vec<LegacyPlanPortV1>,
    config: serde_json::Value,
}

impl LegacyPlanNodeV1 {
    fn to_v2(&self) -> Result<PlanNode, SchemaReadError> {
        let inputs = self
            .inputs
            .iter()
            .map(LegacyPlanPortV1::to_v2)
            .collect::<Result<Vec<_>, _>>()?;
        let outputs = self
            .outputs
            .iter()
            .map(LegacyPlanPortV1::to_v2)
            .collect::<Result<Vec<_>, _>>()?;
        let parse_config =
            |message: &'static str| invalid_data(SchemaContract::ExecutionPlan, message);
        let config = match self.kind {
            LegacyPlanNodeKindV1::Http => PlanNodeConfig::Http(
                serde_json::from_value::<HttpSpec>(self.config.clone())
                    .map_err(|_| parse_config("v1 HTTP Plan config 无效"))?,
            ),
            LegacyPlanNodeKindV1::Js => {
                let legacy = serde_json::from_value::<LegacyJsPlanConfigV1>(self.config.clone())
                    .map_err(|_| parse_config("v1 JS Plan config 无效"))?;
                let output = if self.outputs.iter().any(|port| port.type_tag == "raw") {
                    JsOutputKind::Raw
                } else {
                    JsOutputKind::Json
                };
                PlanNodeConfig::Js(JsConfig {
                    code: legacy.code,
                    output,
                })
            }
            LegacyPlanNodeKindV1::Extract => PlanNodeConfig::Extract(
                serde_json::from_value::<ExtractSpec>(self.config.clone())
                    .map_err(|_| parse_config("v1 Extract Plan config 无效"))?,
            ),
            LegacyPlanNodeKindV1::Mapper => PlanNodeConfig::Mapper(
                serde_json::from_value::<ControlledMapper>(self.config.clone())
                    .map_err(|_| parse_config("v1 Mapper Plan config 无效"))?,
            ),
            LegacyPlanNodeKindV1::Merge
            | LegacyPlanNodeKindV1::Condition
            | LegacyPlanNodeKindV1::Loop => {
                return Err(parse_config("v1 reader 仅接受旧线性 Plan 节点"));
            }
        };
        Ok(PlanNode {
            id: self.id,
            inputs,
            outputs,
            config,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyJsPlanConfigV1 {
    code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyExecutionPlanV1 {
    schema_version: u32,
    compiler_version: String,
    definition_hash: String,
    plan_hash: String,
    nodes: Vec<LegacyPlanNodeV1>,
    edges: Vec<(Uuid, Uuid)>,
    intent_entries: BTreeMap<StandardIntent, IntentEntry>,
    effects: Vec<EffectDeclaration>,
    capability_requirements: Vec<String>,
}
