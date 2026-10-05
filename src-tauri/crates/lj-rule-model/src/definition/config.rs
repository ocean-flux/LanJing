//! Definition 的 closed config 与 control expression。

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::budget::JsBudget;
use crate::endpoint::HttpSpec;
use crate::extract_rule::ExtractSpec;
use crate::literal::TypedLiteral;
use crate::policy::SystemCapabilities;

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
    pub required: SystemCapabilities,
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
    /// 规则声明的资源预算；生效值 = 本声明与 host policy 上限的交集。
    pub budgets: JsBudget,
}

impl JsConfig {
    /// 用 host policy 默认预算构造（新节点与导入翻译的默认声明）。
    ///
    /// 预算仍然随 wire 显式持久化：本构造函数只决定初始值，不代表可以省略声明。
    #[must_use]
    pub fn new(code: String, output: JsOutputKind) -> Self {
        Self {
            code,
            output,
            budgets: JsBudget::default(),
        }
    }
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

impl FlowNodeKind {
    /// 全部内置节点 kind。
    const ALL: [Self; 7] = [
        Self::Http,
        Self::Js,
        Self::Extract,
        Self::Mapper,
        Self::Merge,
        Self::Condition,
        Self::Loop,
    ];

    /// 返回 serde `kind` tag 的稳定 wire 名称。
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Js => "js",
            Self::Extract => "extract",
            Self::Mapper => "mapper",
            Self::Merge => "merge",
            Self::Condition => "condition",
            Self::Loop => "loop",
        }
    }

    /// 解析内置节点 kind；未安装能力返回 `None`。
    #[must_use]
    pub fn from_wire(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.wire_name() == name)
    }
}

/// 未安装能力的 opaque 节点 payload。
///
/// 原样保留作者 wire 的 `kind` 与 `value`：可读入、写出并 round-trip，但不参与
/// validate、compile 与 execute。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailableNodeConfig {
    /// 未安装能力的 wire kind。
    pub kind: String,
    /// 未安装能力的原始 payload。
    pub value: Value,
}

impl UnavailableNodeConfig {
    /// 创建 opaque payload。
    #[must_use]
    pub fn new(kind: impl Into<String>, value: Value) -> Self {
        Self {
            kind: kind.into(),
            value,
        }
    }
}

/// Flow 节点配置：内置能力的 typed config，或未安装能力的 opaque payload。
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// 未安装能力：只保存与 round-trip，不可 validate/compile/execute。
    Unavailable(UnavailableNodeConfig),
}

impl FlowNodeConfig {
    /// 返回内置节点判别值；未安装能力返回 `None`。
    #[must_use]
    pub const fn kind(&self) -> Option<FlowNodeKind> {
        match self {
            Self::Http(_) => Some(FlowNodeKind::Http),
            Self::Js(_) => Some(FlowNodeKind::Js),
            Self::Extract(_) => Some(FlowNodeKind::Extract),
            Self::Mapper(_) => Some(FlowNodeKind::Mapper),
            Self::Merge(_) => Some(FlowNodeKind::Merge),
            Self::Condition(_) => Some(FlowNodeKind::Condition),
            Self::Loop(_) => Some(FlowNodeKind::Loop),
            Self::Unavailable(_) => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FlowNodeConfigWire {
    kind: String,
    value: Value,
}

#[derive(Serialize)]
struct FlowNodeConfigWireRef<'a, T: Serialize> {
    kind: &'a str,
    value: T,
}

impl<'de> Deserialize<'de> for FlowNodeConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FlowNodeConfigWire::deserialize(deserializer)?;
        let Some(kind) = FlowNodeKind::from_wire(&wire.kind) else {
            return Ok(Self::Unavailable(UnavailableNodeConfig {
                kind: wire.kind,
                value: wire.value,
            }));
        };
        let payload = wire.value;
        let decoded = match kind {
            FlowNodeKind::Http => serde_json::from_value::<HttpSpec>(payload).map(Self::Http),
            FlowNodeKind::Js => serde_json::from_value::<JsConfig>(payload).map(Self::Js),
            FlowNodeKind::Extract => {
                serde_json::from_value::<ExtractSpec>(payload).map(Self::Extract)
            }
            FlowNodeKind::Mapper => {
                serde_json::from_value::<ControlledMapper>(payload).map(Self::Mapper)
            }
            FlowNodeKind::Merge => serde_json::from_value::<MergeConfig>(payload).map(Self::Merge),
            FlowNodeKind::Condition => {
                serde_json::from_value::<ConditionConfig>(payload).map(Self::Condition)
            }
            FlowNodeKind::Loop => serde_json::from_value::<ForEachConfig>(payload).map(Self::Loop),
        };
        decoded.map_err(D::Error::custom)
    }
}

impl Serialize for FlowNodeConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Http(config) => FlowNodeConfigWireRef {
                kind: FlowNodeKind::Http.wire_name(),
                value: config,
            }
            .serialize(serializer),
            Self::Js(config) => FlowNodeConfigWireRef {
                kind: FlowNodeKind::Js.wire_name(),
                value: config,
            }
            .serialize(serializer),
            Self::Extract(config) => FlowNodeConfigWireRef {
                kind: FlowNodeKind::Extract.wire_name(),
                value: config,
            }
            .serialize(serializer),
            Self::Mapper(config) => FlowNodeConfigWireRef {
                kind: FlowNodeKind::Mapper.wire_name(),
                value: config,
            }
            .serialize(serializer),
            Self::Merge(config) => FlowNodeConfigWireRef {
                kind: FlowNodeKind::Merge.wire_name(),
                value: config,
            }
            .serialize(serializer),
            Self::Condition(config) => FlowNodeConfigWireRef {
                kind: FlowNodeKind::Condition.wire_name(),
                value: config,
            }
            .serialize(serializer),
            Self::Loop(config) => FlowNodeConfigWireRef {
                kind: FlowNodeKind::Loop.wire_name(),
                value: config,
            }
            .serialize(serializer),
            Self::Unavailable(config) => FlowNodeConfigWireRef {
                kind: &config.kind,
                value: &config.value,
            }
            .serialize(serializer),
        }
    }
}
