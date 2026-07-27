//! 规则 Definition / Package — 可编辑、可移植、可 canonicalize 的作者合同。
//!
//! v2 writer 只序列化闭集 typed 节点与 handle edge；v1 仅在 reader 内保留精确 legacy
//! hash material，不能由公开构造器生成。

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
    ContractSchemaVersion, LEGACY_CONTRACT_SCHEMA_VERSION, RULE_DEFINITION_SCHEMA_VERSION,
    RULE_PACKAGE_SCHEMA_VERSION, SchemaContract, SchemaReadError, invalid_data,
    parse_contract_json, validate_contract_value,
};
use serde::ser::Error as _;

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
    /// 由受限 QuickJS adapter 执行的源码。
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergeInput {
    /// 动态 handle；compiler 负责非空、唯一与 UTF-8 字节排序。
    pub handle: String,
    /// required/optional 激活合同。
    pub activation: MergeInputActivation,
}

/// Merge 的闭集聚合策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeStrategy {
    /// 恰好一个激活 input，原样透传。
    SingleActive,
    /// 按 canonical handle 顺序收集为 array。
    CollectArray,
    /// 按 canonical handle 顺序单层拼接 array。
    ConcatArrays,
    /// 按 canonical handle 顺序浅合并 object；后一个 handle 覆盖前一个。
    OverlayObjects,
}

/// Merge 节点配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MergeConfig {
    /// 命名 inputs；声明顺序不承载语义，compiler 按 handle UTF-8 字节排序。
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
    /// 受限 QuickJS 返回一个已声明 branch handle。
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
    /// 受限 QuickJS 必须返回 array。
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
/// 公开构造器只创建 v2；反序列化 v1 时会保留只读 legacy hash material。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleDefinition {
    source_identity: SourceIdentity,
    base_url: String,
    intent_exports: BTreeMap<StandardIntent, IntentExport>,
    flow: FlowGraph,
    capability_manifest: CapabilityManifest,
    source_id_rules: Vec<String>,
    legacy_v1: Option<Box<LegacyRuleDefinitionV1>>,
}

impl RuleDefinition {
    /// 创建只会写出 v2 的作者 Definition。
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
            legacy_v1: None,
        }
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

    /// 可变访问来源身份，并把旧 v1 read object 切换为 v2 authoring 语义。
    pub fn source_identity_mut(&mut self) -> &mut SourceIdentity {
        self.legacy_v1 = None;
        &mut self.source_identity
    }

    /// 可变访问基础 URL，并把旧 v1 read object 切换为 v2 authoring 语义。
    pub fn base_url_mut(&mut self) -> &mut String {
        self.legacy_v1 = None;
        &mut self.base_url
    }

    /// 可变访问意图导出表，并把旧 v1 read object 切换为 v2 authoring 语义。
    pub fn intent_exports_mut(&mut self) -> &mut BTreeMap<StandardIntent, IntentExport> {
        self.legacy_v1 = None;
        &mut self.intent_exports
    }

    /// 可变访问 Flow，并把旧 v1 read object 切换为 v2 authoring 语义。
    pub fn flow_mut(&mut self) -> &mut FlowGraph {
        self.legacy_v1 = None;
        &mut self.flow
    }

    /// 可变访问能力清单，并把旧 v1 read object 切换为 v2 authoring 语义。
    pub fn capability_manifest_mut(&mut self) -> &mut CapabilityManifest {
        self.legacy_v1 = None;
        &mut self.capability_manifest
    }

    /// 可变访问稳定 ID 规则，并把旧 v1 read object 切换为 v2 authoring 语义。
    pub fn source_id_rules_mut(&mut self) -> &mut Vec<String> {
        self.legacy_v1 = None;
        &mut self.source_id_rules
    }

    pub(crate) fn legacy_hash_material(&self) -> Option<&LegacyRuleDefinitionV1> {
        self.legacy_v1.as_deref()
    }
}

/// 规则包：Definition + 安装元数据。
///
/// 公开构造器与 serde writer 只产 v2；reader 仍接受已安装 v1 package。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RulePackage {
    source_identity: SourceIdentity,
    version: String,
    definition: RuleDefinition,
    schema_version: ContractSchemaVersion,
}

impl RulePackage {
    /// 创建 v2 RulePackage。
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
            schema_version: ContractSchemaVersion::V2,
        }
    }

    /// 返回读取来源的已知 schema 版本。
    #[must_use]
    pub const fn schema_version(&self) -> ContractSchemaVersion {
        self.schema_version
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

/// 从 JSON bytes 读取 v1/v2 RuleDefinition，并为未知版本返回 typed incompatible。
///
/// # Errors
///
/// JSON 无效、contract tag 不匹配、版本未知、字段未知或 v1 节点存在 kind/config mismatch
/// 时返回 [`SchemaReadError`]。
pub fn read_rule_definition(bytes: &[u8]) -> Result<RuleDefinition, SchemaReadError> {
    let (value, version) = parse_contract_json(bytes, SchemaContract::RuleDefinition)?;
    definition_from_value(value, version)
}

/// 从 JSON bytes 读取 v1/v2 RulePackage，并为未知版本返回 typed incompatible。
///
/// # Errors
///
/// JSON 无效、contract tag 不匹配、版本未知、字段未知，或 package 与嵌套 Definition
/// 版本组合非法时返回 [`SchemaReadError`]。
pub fn read_rule_package(bytes: &[u8]) -> Result<RulePackage, SchemaReadError> {
    let (value, version) = parse_contract_json(bytes, SchemaContract::RulePackage)?;
    package_from_value(value, version)
}

impl Serialize for RuleDefinition {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.legacy_v1.is_some() {
            return Err(S::Error::custom(
                "v1 RuleDefinition 只读；请由 compiler 构造新的 v2 Definition",
            ));
        }
        RuleDefinitionV2Ref {
            schema_version: RULE_DEFINITION_SCHEMA_VERSION,
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
        let version = validate_contract_value(&value, SchemaContract::RuleDefinition)
            .map_err(D::Error::custom)?;
        definition_from_value(value, version).map_err(D::Error::custom)
    }
}

impl Serialize for RulePackage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.schema_version == ContractSchemaVersion::V1 {
            return Err(S::Error::custom(
                "v1 RulePackage 只读；请由 importer 构造新的 v2 Package",
            ));
        }
        RulePackageV2Ref {
            schema_version: RULE_PACKAGE_SCHEMA_VERSION,
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
        let version = validate_contract_value(&value, SchemaContract::RulePackage)
            .map_err(D::Error::custom)?;
        package_from_value(value, version).map_err(D::Error::custom)
    }
}

#[derive(Serialize)]
#[serde(tag = "contract", rename = "rule_definition", deny_unknown_fields)]
struct RuleDefinitionV2Ref<'a> {
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
struct RuleDefinitionV2Owned {
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
struct RulePackageV2Ref<'a> {
    schema_version: u32,
    source_identity: &'a SourceIdentity,
    version: &'a str,
    definition: &'a RuleDefinition,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RulePackageV2Owned {
    schema_version: u32,
    source_identity: SourceIdentity,
    version: String,
    definition: RuleDefinition,
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

fn definition_from_value(
    value: serde_json::Value,
    version: ContractSchemaVersion,
) -> Result<RuleDefinition, SchemaReadError> {
    match version {
        ContractSchemaVersion::V1 => {
            let legacy =
                serde_json::from_value::<LegacyRuleDefinitionV1>(strip_contract_tag(value))
                    .map_err(|error| {
                        invalid_data(SchemaContract::RuleDefinition, error.to_string())
                    })?;
            RuleDefinition::try_from_legacy(legacy)
        }
        ContractSchemaVersion::V2 => {
            let wire = serde_json::from_value::<RuleDefinitionV2Owned>(strip_contract_tag(value))
                .map_err(|error| {
                invalid_data(SchemaContract::RuleDefinition, error.to_string())
            })?;
            if wire.schema_version != RULE_DEFINITION_SCHEMA_VERSION {
                return Err(SchemaReadError::IncompatibleVersion {
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
                legacy_v1: None,
            })
        }
    }
}

fn package_from_value(
    value: serde_json::Value,
    version: ContractSchemaVersion,
) -> Result<RulePackage, SchemaReadError> {
    match version {
        ContractSchemaVersion::V1 => {
            let legacy = serde_json::from_value::<LegacyRulePackageV1>(strip_contract_tag(value))
                .map_err(|error| {
                invalid_data(SchemaContract::RulePackage, error.to_string())
            })?;
            if legacy.schema_version != LEGACY_CONTRACT_SCHEMA_VERSION
                || legacy.definition.schema_version != LEGACY_CONTRACT_SCHEMA_VERSION
            {
                return Err(invalid_data(
                    SchemaContract::RulePackage,
                    "v1 package 必须包含 v1 definition",
                ));
            }
            let definition = RuleDefinition::try_from_legacy(legacy.definition)?;
            Ok(RulePackage {
                source_identity: legacy.source_identity,
                version: legacy.version,
                definition,
                schema_version: ContractSchemaVersion::V1,
            })
        }
        ContractSchemaVersion::V2 => {
            let wire = serde_json::from_value::<RulePackageV2Owned>(strip_contract_tag(value))
                .map_err(|error| invalid_data(SchemaContract::RulePackage, error.to_string()))?;
            if wire.schema_version != RULE_PACKAGE_SCHEMA_VERSION
                || wire.definition.schema_version() != ContractSchemaVersion::V2
            {
                return Err(invalid_data(
                    SchemaContract::RulePackage,
                    "v2 package 必须包含 v2 definition",
                ));
            }
            Ok(RulePackage {
                source_identity: wire.source_identity,
                version: wire.version,
                definition: wire.definition,
                schema_version: ContractSchemaVersion::V2,
            })
        }
    }
}

impl RuleDefinition {
    fn try_from_legacy(legacy: LegacyRuleDefinitionV1) -> Result<Self, SchemaReadError> {
        if legacy.schema_version != LEGACY_CONTRACT_SCHEMA_VERSION {
            return Err(SchemaReadError::IncompatibleVersion {
                contract: SchemaContract::RuleDefinition,
                version: legacy.schema_version,
            });
        }
        let nodes = legacy
            .flow
            .nodes
            .iter()
            .map(LegacyFlowNodeV1::to_v2)
            .collect::<Result<Vec<_>, _>>()?;
        let edges = legacy
            .flow
            .edges
            .iter()
            .map(|edge| {
                if edge.condition_branch.is_some() {
                    return Err(invalid_data(
                        SchemaContract::RuleDefinition,
                        "v1 linear edge 不能携带 condition branch",
                    ));
                }
                Ok(FlowEdge::new(
                    FlowPortRef::new(edge.from, "output"),
                    FlowPortRef::new(edge.to, "input"),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let material = legacy.clone();
        Ok(Self {
            source_identity: legacy.source_identity,
            base_url: legacy.base_url,
            intent_exports: legacy.intent_exports,
            flow: FlowGraph { nodes, edges },
            capability_manifest: legacy.capability_manifest,
            source_id_rules: legacy.source_id_rules,
            legacy_v1: Some(Box::new(material)),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum LegacyFlowNodeKindV1 {
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
struct LegacyFlowNodeV1 {
    id: Uuid,
    kind: LegacyFlowNodeKindV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    http: Option<HttpSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    js_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extract: Option<ExtractSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mapper: Option<ControlledMapper>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    span: Option<SourceSpan>,
}

impl LegacyFlowNodeV1 {
    fn to_v2(&self) -> Result<FlowNode, SchemaReadError> {
        let config_count = usize::from(self.http.is_some())
            + usize::from(self.js_code.is_some())
            + usize::from(self.extract.is_some())
            + usize::from(self.mapper.is_some());
        if config_count != 1 {
            return Err(invalid_data(
                SchemaContract::RuleDefinition,
                "v1 FlowNode 必须恰有一个 active config",
            ));
        }
        let config = match self.kind {
            LegacyFlowNodeKindV1::Http => {
                self.http.clone().map(FlowNodeConfig::Http).ok_or_else(|| {
                    invalid_data(SchemaContract::RuleDefinition, "v1 HTTP config mismatch")
                })?
            }
            LegacyFlowNodeKindV1::Js => self
                .js_code
                .clone()
                .map(|code| {
                    FlowNodeConfig::Js(JsConfig {
                        code,
                        output: JsOutputKind::Json,
                    })
                })
                .ok_or_else(|| {
                    invalid_data(SchemaContract::RuleDefinition, "v1 JS config mismatch")
                })?,
            LegacyFlowNodeKindV1::Extract => self
                .extract
                .clone()
                .map(FlowNodeConfig::Extract)
                .ok_or_else(|| {
                    invalid_data(SchemaContract::RuleDefinition, "v1 Extract config mismatch")
                })?,
            LegacyFlowNodeKindV1::Mapper => self
                .mapper
                .clone()
                .map(FlowNodeConfig::Mapper)
                .ok_or_else(|| {
                    invalid_data(SchemaContract::RuleDefinition, "v1 Mapper config mismatch")
                })?,
            LegacyFlowNodeKindV1::Merge
            | LegacyFlowNodeKindV1::Condition
            | LegacyFlowNodeKindV1::Loop => {
                return Err(invalid_data(
                    SchemaContract::RuleDefinition,
                    "v1 reader 仅接受旧线性节点",
                ));
            }
        };
        Ok(FlowNode {
            id: self.id,
            config,
            span: self.span.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyFlowEdgeV1 {
    from: Uuid,
    to: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    condition_branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyFlowGraphV1 {
    nodes: Vec<LegacyFlowNodeV1>,
    edges: Vec<LegacyFlowEdgeV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LegacyRuleDefinitionV1 {
    schema_version: u32,
    source_identity: SourceIdentity,
    base_url: String,
    intent_exports: BTreeMap<StandardIntent, IntentExport>,
    flow: LegacyFlowGraphV1,
    capability_manifest: CapabilityManifest,
    source_id_rules: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyRulePackageV1 {
    schema_version: u32,
    source_identity: SourceIdentity,
    version: String,
    definition: LegacyRuleDefinitionV1,
}
