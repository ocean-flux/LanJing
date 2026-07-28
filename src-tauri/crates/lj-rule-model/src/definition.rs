//! 规则 Definition / Package — 可编辑、可移植、可 canonicalize 的唯一 current 作者合同。
//!
//! 只序列化闭集 typed 节点与 handle edge；历史线性 shape 在 preflight 阶段以
//! `LEGACY_RULE_CONTRACT_UNSUPPORTED` 稳定拒绝，不反序列化、不投影、不写兼容 reader。

use std::collections::BTreeMap;

use lj_capability::{IntentExport, StandardIntent};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::endpoint::HttpSpec;
use crate::extract_rule::ExtractSpec;
use crate::literal::TypedLiteral;
use crate::policy::PolicyCapabilities;
use crate::schema::{
    RULE_CONTRACT_SCHEMA_VERSION, SchemaContract, SchemaReadError, invalid_data,
    parse_contract_json, validate_contract_value,
};

/// 单个 Loop 配置允许的全局 hard ceiling。
pub const MAX_LOOP_ITERATIONS: u32 = 256;

/// 逻辑来源稳定身份（安装版本另有 version/hash）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    /// 稳定身份字符串（来源持有，不随安装版本变化）。
    pub id: String,
}

/// 诊断源码定位。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSpan {
    /// 起始字节偏移。
    pub start: usize,
    /// 结束字节偏移。
    pub end: usize,
    /// 可选路径/字段路径。
    pub path: Option<String>,
}

/// 能力清单。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityManifest {
    /// 声明所需能力。
    pub required: PolicyCapabilities,
}

/// 受控 Mapper 输出类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapperOutputKind {
    /// 媒体主体列表或详情。
    Items,
    /// 发现集合或继续动作。
    Discovery,
    /// 消费单元。
    Units,
    /// 可消费资产。
    Assets,
}

/// 受控 Mapper 定义。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlledMapper {
    /// Mapper 产出的标准资源类型。
    pub output: MapperOutputKind,
    /// 可用于生成来源内稳定 ID 的字段名，至少一个。
    pub identity_fields: Vec<String>,
}

/// JS 节点声明的输出形状。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JsOutputKind {
    /// 脚本输出必须解析为 JSON。
    Json,
    /// 脚本输出作为原始文本传递。
    Raw,
}

/// JS 节点配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JsConfig {
    /// 由受限 `QuickJS` adapter 执行的源码。
    pub code: String,
    /// compiler 用于确定输出 port kind 的显式声明。
    pub output: JsOutputKind,
}

/// Merge input 在本次控制路径中的激活策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeInputActivation {
    /// input 被激活时必须产生值。
    Required,
    /// input 可不被激活或不产生值。
    Optional,
}

/// Merge 的一个命名 input 声明。
///
/// 语义身份由 `input_id` 与 `handle` 表达；显式 `order` 唯一连续覆盖 `0..n-1`，
/// 物理数组声明顺序不承载语义。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergeInput {
    /// 稳定 input 身份；compiler 负责非空与唯一。
    pub input_id: String,
    /// 动态 handle；compiler 负责非空与唯一。
    pub handle: String,
    /// 显式聚合顺序；必须唯一且连续覆盖 `0..n-1`。
    pub order: u32,
    /// required/optional 激活合同。
    pub activation: MergeInputActivation,
}

/// Merge 的闭集聚合策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    /// 恰好一个激活 input，原样透传。
    SingleActive,
    /// 按显式 `order` 收集为 array。
    CollectArray,
    /// 按显式 `order` 单层拼接 array。
    ConcatArrays,
    /// 按显式 `order` 浅合并 object；后一个 order 覆盖前一个。
    OverlayObjects,
}

/// Merge 节点配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergeConfig {
    /// 命名 inputs；声明顺序不承载语义，canonicalize/hash 按显式 `order` 排序。
    pub inputs: Vec<MergeInput>,
    /// 聚合策略。
    pub strategy: MergeStrategy,
}

/// typed Condition 的闭集 operator。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    /// JSON Pointer 命中任意值。
    Exists,
    /// JSON Pointer 命中 null。
    IsNull,
    /// canonical deep equality。
    Eq,
    /// canonical deep inequality。
    Ne,
    /// number 小于。
    Lt,
    /// number 小于等于。
    Lte,
    /// number 大于。
    Gt,
    /// number 大于等于。
    Gte,
    /// string substring 或 array element 包含。
    Contains,
}

/// typed Condition predicate。
///
/// serde tag 为 `operator`；需要 operand 的 operator 在 Rust 类型上始终携带 `value`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operator", rename_all = "snake_case", deny_unknown_fields)]
pub enum ConditionPredicate {
    /// Pointer 存在。
    Exists {
        /// RFC 6901 JSON Pointer。
        pointer: String,
    },
    /// Pointer 命中 null。
    IsNull {
        /// RFC 6901 JSON Pointer。
        pointer: String,
    },
    /// canonical deep equality。
    Eq {
        /// RFC 6901 JSON Pointer。
        pointer: String,
        /// 比较 literal。
        value: TypedLiteral,
    },
    /// canonical deep inequality。
    Ne {
        /// RFC 6901 JSON Pointer。
        pointer: String,
        /// 比较 literal。
        value: TypedLiteral,
    },
    /// number 小于。
    Lt {
        /// RFC 6901 JSON Pointer。
        pointer: String,
        /// number literal。
        value: TypedLiteral,
    },
    /// number 小于等于。
    Lte {
        /// RFC 6901 JSON Pointer。
        pointer: String,
        /// number literal。
        value: TypedLiteral,
    },
    /// number 大于。
    Gt {
        /// RFC 6901 JSON Pointer。
        pointer: String,
        /// number literal。
        value: TypedLiteral,
    },
    /// number 大于等于。
    Gte {
        /// RFC 6901 JSON Pointer。
        pointer: String,
        /// number literal。
        value: TypedLiteral,
    },
    /// string substring 或 array element 包含。
    Contains {
        /// RFC 6901 JSON Pointer。
        pointer: String,
        /// substring 或 element literal。
        value: TypedLiteral,
    },
}

impl ConditionPredicate {
    /// 返回 predicate 的闭集 operator。
    #[must_use]
    pub const fn operator(&self) -> ConditionOperator {
        match self {
            Self::Exists { .. } => ConditionOperator::Exists,
            Self::IsNull { .. } => ConditionOperator::IsNull,
            Self::Eq { .. } => ConditionOperator::Eq,
            Self::Ne { .. } => ConditionOperator::Ne,
            Self::Lt { .. } => ConditionOperator::Lt,
            Self::Lte { .. } => ConditionOperator::Lte,
            Self::Gt { .. } => ConditionOperator::Gt,
            Self::Gte { .. } => ConditionOperator::Gte,
            Self::Contains { .. } => ConditionOperator::Contains,
        }
    }

    /// 返回 RFC 6901 JSON Pointer。
    #[must_use]
    pub fn pointer(&self) -> &str {
        match self {
            Self::Exists { pointer }
            | Self::IsNull { pointer }
            | Self::Eq { pointer, .. }
            | Self::Ne { pointer, .. }
            | Self::Lt { pointer, .. }
            | Self::Lte { pointer, .. }
            | Self::Gt { pointer, .. }
            | Self::Gte { pointer, .. }
            | Self::Contains { pointer, .. } => pointer,
        }
    }

    /// 返回 operator operand；`exists` 与 `is_null` 没有 operand。
    #[must_use]
    pub const fn value(&self) -> Option<&TypedLiteral> {
        match self {
            Self::Exists { .. } | Self::IsNull { .. } => None,
            Self::Eq { value, .. }
            | Self::Ne { value, .. }
            | Self::Lt { value, .. }
            | Self::Lte { value, .. }
            | Self::Gt { value, .. }
            | Self::Gte { value, .. }
            | Self::Contains { value, .. } => Some(value),
        }
    }
}

/// Condition 当前 active 的 typed/JS 表达式。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum ControlExpression {
    /// 纯 typed predicate；true/false 必须映射到已声明 branch。
    Typed {
        /// typed predicate。
        predicate: ConditionPredicate,
        /// predicate 为 true 时激活的 branch handle。
        true_branch: String,
        /// predicate 为 false 时激活的 branch handle。
        false_branch: String,
    },
    /// 受限 `QuickJS` 返回一个已声明 branch handle。
    Js {
        /// 控制脚本源码；compiler/runtime 不把源码写入 diagnostic。
        code: String,
    },
}

/// Condition 节点配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConditionConfig {
    /// 声明的动态 branch output handles。
    pub branches: Vec<String>,
    /// 当前唯一 active 表达式。
    pub expression: ControlExpression,
}

/// Loop collection selector。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum CollectionSelector {
    /// 用 RFC 6901 JSON Pointer 选择 array。
    Typed {
        /// RFC 6901 JSON Pointer。
        pointer: String,
    },
    /// 受限 `QuickJS` 必须返回 array。
    Js {
        /// collection selector 源码。
        code: String,
    },
}

/// Loop iteration limit 超出 `1..=MAX_LOOP_ITERATIONS`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("Loop max_iterations 必须在 1..={max}，实际为 {actual}")]
pub struct LoopIterationLimitError {
    /// 非法配置值。
    pub actual: u32,
    /// model hard ceiling。
    pub max: u32,
}

/// 已验证的 Loop iteration limit。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct LoopIterationLimit(u32);

impl LoopIterationLimit {
    /// 创建 `1..=MAX_LOOP_ITERATIONS` 的 limit。
    ///
    /// # Errors
    ///
    /// `value` 为零或超过 [`MAX_LOOP_ITERATIONS`] 时返回 [`LoopIterationLimitError`]。
    pub const fn new(value: u32) -> Result<Self, LoopIterationLimitError> {
        if value == 0 || value > MAX_LOOP_ITERATIONS {
            return Err(LoopIterationLimitError {
                actual: value,
                max: MAX_LOOP_ITERATIONS,
            });
        }
        Ok(Self(value))
    }

    /// 返回已验证的 limit。
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for LoopIterationLimit {
    type Error = LoopIterationLimitError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl<'de> Deserialize<'de> for LoopIterationLimit {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Self::new(value).map_err(D::Error::custom)
    }
}

/// bounded for-each Loop 配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForEachConfig {
    /// typed/JS collection selector。
    pub collection: CollectionSelector,
    /// body payload 中 item 的 binding 名。
    pub item_binding: String,
    /// body payload 中 index 的 binding 名。
    pub index_binding: String,
    /// 作者声明的原始上限；compiler 必须校验 `1..=MAX_LOOP_ITERATIONS`。
    pub max_iterations: u16,
}

/// Flow 节点类型判别值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowNodeKind {
    /// HTTP 请求节点。
    Http,
    /// JS 执行节点。
    Js,
    /// 提取节点。
    Extract,
    /// 受控 Mapper 节点。
    Mapper,
    /// 合并多上游值。
    Merge,
    /// 条件路由。
    Condition,
    /// bounded for-each。
    Loop,
}

/// 无 kind/config mismatch 的闭集 Flow 节点配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum FlowNodeConfig {
    /// HTTP 请求。
    Http(HttpSpec),
    /// `QuickJS` 计算。
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
    Loop(ForEachConfig),
}

impl FlowNodeConfig {
    /// 返回与 active config 一致的节点判别值。
    #[must_use]
    pub const fn kind(&self) -> FlowNodeKind {
        match self {
            Self::Http(_) => FlowNodeKind::Http,
            Self::Js(_) => FlowNodeKind::Js,
            Self::Extract(_) => FlowNodeKind::Extract,
            Self::Mapper(_) => FlowNodeKind::Mapper,
            Self::Merge(_) => FlowNodeKind::Merge,
            Self::Condition(_) => FlowNodeKind::Condition,
            Self::Loop(_) => FlowNodeKind::Loop,
        }
    }
}

/// Flow 节点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowNode {
    /// 节点稳定 ID。
    pub id: Uuid,
    /// 当前唯一 active typed config。
    pub config: FlowNodeConfig,
    /// 可选源码 span。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
}

impl FlowNode {
    /// 创建无源码定位的 typed Flow 节点。
    #[must_use]
    pub const fn new(id: Uuid, config: FlowNodeConfig) -> Self {
        Self {
            id,
            config,
            span: None,
        }
    }

    /// 附加作者源码定位。
    #[must_use]
    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    /// 返回与 config 一致的节点类型。
    #[must_use]
    pub const fn kind(&self) -> FlowNodeKind {
        self.config.kind()
    }
}

/// 节点上的语义 port 引用。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowPortRef {
    /// 节点稳定 ID。
    pub node_id: Uuid,
    /// 固定或经 compiler 验证的动态 handle。
    pub handle: String,
}

impl FlowPortRef {
    /// 创建 typed port 引用。
    #[must_use]
    pub fn new(node_id: Uuid, handle: impl Into<String>) -> Self {
        Self {
            node_id,
            handle: handle.into(),
        }
    }
}

/// Flow 语义边；identity 恰为 `from node/handle + to node/handle`。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowEdge {
    /// 起始 output port。
    pub from: FlowPortRef,
    /// 目标 input port。
    pub to: FlowPortRef,
}

impl FlowEdge {
    /// 创建语义边。
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

/// 类型化 Flow 图。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowGraph {
    /// 节点列表；声明顺序不承载语义。
    pub nodes: Vec<FlowNode>,
    /// 语义边列表；声明顺序不承载语义。
    pub edges: Vec<FlowEdge>,
}

/// 规则定义（作者合同）。
///
/// 公开构造器与 serde writer 只产唯一 current shape。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDefinition {
    source_identity: SourceIdentity,
    base_url: String,
    intent_exports: BTreeMap<StandardIntent, IntentExport>,
    flow: FlowGraph,
    capability_manifest: CapabilityManifest,
    source_id_rules: Vec<String>,
}

impl RuleDefinition {
    /// 创建唯一 current 作者 Definition。
    #[must_use]
    pub fn new(
        source_identity: SourceIdentity,
        base_url: impl Into<String>,
        intent_exports: BTreeMap<StandardIntent, IntentExport>,
        flow: FlowGraph,
        capability_manifest: CapabilityManifest,
        source_id_rules: Vec<String>,
    ) -> Self {
        Self {
            source_identity,
            base_url: base_url.into(),
            intent_exports,
            flow,
            capability_manifest,
            source_id_rules,
        }
    }

    /// 返回来源稳定身份。
    #[must_use]
    pub const fn source_identity(&self) -> &SourceIdentity {
        &self.source_identity
    }

    /// 返回基础 URL。
    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// 返回标准意图导出表。
    #[must_use]
    pub const fn intent_exports(&self) -> &BTreeMap<StandardIntent, IntentExport> {
        &self.intent_exports
    }

    /// 返回 typed Flow。
    #[must_use]
    pub const fn flow(&self) -> &FlowGraph {
        &self.flow
    }

    /// 返回能力清单。
    #[must_use]
    pub const fn capability_manifest(&self) -> &CapabilityManifest {
        &self.capability_manifest
    }

    /// 返回来源持有的稳定 ID 规则。
    #[must_use]
    pub fn source_id_rules(&self) -> &[String] {
        &self.source_id_rules
    }

    /// 可变访问来源身份。
    pub fn source_identity_mut(&mut self) -> &mut SourceIdentity {
        &mut self.source_identity
    }

    /// 可变访问基础 URL。
    pub fn base_url_mut(&mut self) -> &mut String {
        &mut self.base_url
    }

    /// 可变访问意图导出表。
    pub fn intent_exports_mut(&mut self) -> &mut BTreeMap<StandardIntent, IntentExport> {
        &mut self.intent_exports
    }

    /// 可变访问 Flow。
    pub fn flow_mut(&mut self) -> &mut FlowGraph {
        &mut self.flow
    }

    /// 可变访问能力清单。
    pub fn capability_manifest_mut(&mut self) -> &mut CapabilityManifest {
        &mut self.capability_manifest
    }

    /// 可变访问稳定 ID 规则。
    pub fn source_id_rules_mut(&mut self) -> &mut Vec<String> {
        &mut self.source_id_rules
    }
}

/// 规则包：Definition + 安装元数据。
///
/// 公开构造器与 serde writer 只产唯一 current shape。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulePackage {
    source_identity: SourceIdentity,
    version: String,
    definition: RuleDefinition,
}

impl RulePackage {
    /// 创建 current `RulePackage`。
    #[must_use]
    pub fn new(
        source_identity: SourceIdentity,
        version: impl Into<String>,
        definition: RuleDefinition,
    ) -> Self {
        Self {
            source_identity,
            version: version.into(),
            definition,
        }
    }

    /// 返回 package 来源身份。
    #[must_use]
    pub const fn source_identity(&self) -> &SourceIdentity {
        &self.source_identity
    }

    /// 返回安装版本。
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// 返回作者 Definition。
    #[must_use]
    pub const fn definition(&self) -> &RuleDefinition {
        &self.definition
    }
}

/// 从 JSON bytes 读取唯一 current `RuleDefinition`。
///
/// # Errors
///
/// JSON 无效、contract tag 不匹配、未知 schema、历史结构签名、字段未知或 current shape
/// 损坏时返回 [`SchemaReadError`]。
pub fn read_rule_definition(bytes: &[u8]) -> Result<RuleDefinition, SchemaReadError> {
    let value = parse_contract_json(bytes, SchemaContract::RuleDefinition)?;
    definition_from_value(value)
}

/// 从 JSON bytes 读取唯一 current `RulePackage`。
///
/// # Errors
///
/// JSON 无效、contract tag 不匹配、未知 schema、历史结构签名、字段未知或 package 与
/// 嵌套 Definition 非法时返回 [`SchemaReadError`]。
pub fn read_rule_package(bytes: &[u8]) -> Result<RulePackage, SchemaReadError> {
    let value = parse_contract_json(bytes, SchemaContract::RulePackage)?;
    package_from_value(value)
}

impl Serialize for RuleDefinition {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RuleDefinitionWireRef {
            schema_version: RULE_CONTRACT_SCHEMA_VERSION,
            source_identity: &self.source_identity,
            base_url: &self.base_url,
            intent_exports: &self.intent_exports,
            flow: &self.flow,
            capability_manifest: &self.capability_manifest,
            source_id_rules: &self.source_id_rules,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RuleDefinition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        validate_contract_value(&value, SchemaContract::RuleDefinition)
            .map_err(D::Error::custom)?;
        definition_from_value(value).map_err(D::Error::custom)
    }
}

impl Serialize for RulePackage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RulePackageWireRef {
            schema_version: RULE_CONTRACT_SCHEMA_VERSION,
            source_identity: &self.source_identity,
            version: &self.version,
            definition: &self.definition,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RulePackage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        validate_contract_value(&value, SchemaContract::RulePackage).map_err(D::Error::custom)?;
        package_from_value(value).map_err(D::Error::custom)
    }
}

#[derive(Serialize)]
#[serde(tag = "contract", rename = "rule_definition", deny_unknown_fields)]
struct RuleDefinitionWireRef<'a> {
    schema_version: u32,
    source_identity: &'a SourceIdentity,
    base_url: &'a str,
    intent_exports: &'a BTreeMap<StandardIntent, IntentExport>,
    flow: &'a FlowGraph,
    capability_manifest: &'a CapabilityManifest,
    source_id_rules: &'a [String],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuleDefinitionWireOwned {
    schema_version: u32,
    source_identity: SourceIdentity,
    base_url: String,
    intent_exports: BTreeMap<StandardIntent, IntentExport>,
    flow: FlowGraph,
    capability_manifest: CapabilityManifest,
    source_id_rules: Vec<String>,
}

#[derive(Serialize)]
#[serde(tag = "contract", rename = "rule_package", deny_unknown_fields)]
struct RulePackageWireRef<'a> {
    schema_version: u32,
    source_identity: &'a SourceIdentity,
    version: &'a str,
    definition: &'a RuleDefinition,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RulePackageWireOwned {
    schema_version: u32,
    source_identity: SourceIdentity,
    version: String,
    definition: serde_json::Value,
}

fn strip_contract_tag(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(object) = value.as_object_mut() {
        object.remove("contract");
    }
    value
}

fn definition_from_value(value: serde_json::Value) -> Result<RuleDefinition, SchemaReadError> {
    let wire = serde_json::from_value::<RuleDefinitionWireOwned>(strip_contract_tag(value))
        .map_err(|error| invalid_data(SchemaContract::RuleDefinition, error.to_string()))?;
    if wire.schema_version != RULE_CONTRACT_SCHEMA_VERSION {
        return Err(SchemaReadError::SchemaUnsupported {
            contract: SchemaContract::RuleDefinition,
            version: wire.schema_version,
        });
    }
    Ok(RuleDefinition {
        source_identity: wire.source_identity,
        base_url: wire.base_url,
        intent_exports: wire.intent_exports,
        flow: wire.flow,
        capability_manifest: wire.capability_manifest,
        source_id_rules: wire.source_id_rules,
    })
}

fn package_from_value(value: serde_json::Value) -> Result<RulePackage, SchemaReadError> {
    let wire = serde_json::from_value::<RulePackageWireOwned>(strip_contract_tag(value))
        .map_err(|error| invalid_data(SchemaContract::RulePackage, error.to_string()))?;
    if wire.schema_version != RULE_CONTRACT_SCHEMA_VERSION {
        return Err(SchemaReadError::SchemaUnsupported {
            contract: SchemaContract::RulePackage,
            version: wire.schema_version,
        });
    }
    validate_contract_value(&wire.definition, SchemaContract::RuleDefinition)?;
    let definition = definition_from_value(wire.definition)?;
    Ok(RulePackage {
        source_identity: wire.source_identity,
        version: wire.version,
        definition,
    })
}
