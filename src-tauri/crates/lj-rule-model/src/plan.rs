//! Execution Plan — compiler 产出的不可变 runtime 投影。
//!
//! 与 Definition 使用不同 serde tag，禁止互相直接反序列化。

use std::collections::BTreeMap;

use lj_capability::StandardIntent;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Plan 节点类型（已解析、类型检查后的投影）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// 循环。
    Loop,
}

/// 类型化端口。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanPort {
    /// 端口名。
    pub name: String,
    /// 端口类型标签（如 `raw`/`json`/`delta`）。
    pub type_tag: String,
}

/// Effect 种类。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
pub struct IntentEntry {
    /// 标准意图。
    pub intent: StandardIntent,
    /// Flow/Plan 入口节点。
    pub entry_node: Uuid,
    /// Mapper 输出节点。
    pub mapper_output: Uuid,
}

/// Plan 节点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanNode {
    /// 节点 ID。
    pub id: Uuid,
    /// 节点种类。
    pub kind: PlanNodeKind,
    /// 输入端口。
    pub inputs: Vec<PlanPort>,
    /// 输出端口。
    pub outputs: Vec<PlanPort>,
    /// 节点配置（已 canonicalize 的 JSON 对象）。
    pub config: serde_json::Value,
}

/// 不可变执行计划。
///
/// serde tag 固定为 `execution_plan`，与 `RuleDefinition` 区分。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "contract", rename = "execution_plan")]
pub struct ExecutionPlan {
    /// Plan schema 版本。
    pub schema_version: u32,
    /// 产出本 Plan 的 compiler 身份/版本。
    pub compiler_version: String,
    /// 源 Definition 的 canonical hash。
    pub definition_hash: String,
    /// Plan 自身 content hash（安装/pin 用）。
    pub plan_hash: String,
    /// 已解析节点。
    pub nodes: Vec<PlanNode>,
    /// 有向边 `(from, to)`。
    pub edges: Vec<(Uuid, Uuid)>,
    /// 意图入口表。
    pub intent_entries: BTreeMap<StandardIntent, IntentEntry>,
    /// Effect 声明。
    pub effects: Vec<EffectDeclaration>,
    /// 能力需求标签汇总。
    pub capability_requirements: Vec<String>,
}
