//! immutable Plan 的 closed node、port、effect 与 control-region 类型。

use std::collections::BTreeMap;

use lj_capability::StandardIntent;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::definition::{
    CollectionSelector, ConditionConfig, ControlledMapper, FlowPortRef, JsConfig,
    LoopIterationLimit, MergeConfig,
};
use crate::endpoint::HttpSpec;
use crate::extract_rule::ExtractSpec;

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
    /// `QuickJS` effect 节点。
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EffectKind {
    /// HTTP 外部效应。
    Http,
    /// `QuickJS` 外部效应。
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

/// 构建不可变执行计划所需的结构化内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPlanParts {
    pub nodes: Vec<PlanNode>,
    pub edges: Vec<PlanEdge>,
    pub intent_entries: BTreeMap<StandardIntent, IntentEntry>,
    pub effects: Vec<EffectDeclaration>,
    pub capability_requirements: Vec<String>,
    pub control_regions: Vec<ControlRegion>,
}
