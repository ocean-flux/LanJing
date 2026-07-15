//! 节点图结构 — 节点/边/图/子例程。

use lj_capability::{IntentExport, StandardIntent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 节点类型(执行节点 + 受控 Mapper)。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    /// HTTP 请求节点。
    Http,
    /// JS 执行节点。
    Js,
    /// 提取节点(多数据类型多选择器)。
    Extract,
    /// 受控 Mapper 节点。
    Mapper,
    /// 合并多上游 stream(首刀 stub)。
    Merge,
    /// 条件路由(首刀 stub)。
    Condition,
    /// 迭代执行子图(首刀 stub)。
    Loop,
}

/// 节点 ID(`UUIDv4` 恒久)。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(
    /// `UUIDv4`。
    pub Uuid,
);

/// 子例程 ID。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubroutineId(
    /// `UUIDv4`。
    pub Uuid,
);

/// 源 ID。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId(
    /// `UUIDv4`。
    pub Uuid,
);

/// 节点 spec(按 kind 鉴别 union)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeSpec {
    /// 节点类型。
    pub kind: NodeKind,
    /// HTTP 节点的 spec(当 kind == Http)。
    pub http: Option<crate::endpoint::HttpSpec>,
    /// JS 节点的 spec(当 kind == Js)。
    pub js: Option<JsSpec>,
    /// 提取节点的 spec(当 kind == Extract)。
    pub extract: Option<crate::extract_rule::ExtractSpec>,
    /// Mapper 节点的 spec(当 kind == Mapper)。
    #[serde(default)]
    pub mapper: Option<MapperSpec>,
    // Merge/Condition/Loop 首刀 stub，spec 暂不定义。
}

/// JS 节点 spec。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsSpec {
    /// JS 代码。
    pub code: String,
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

/// 受控 Mapper spec。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapperSpec {
    /// Mapper 产出的标准资源类型。
    pub output: MapperOutputKind,
    /// 可用于生成来源内稳定 ID 的字段名，至少一个。
    pub identity_fields: Vec<String>,
}

/// 节点(含双 ID)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// `UUIDv4` 恒久 ID。
    pub node_id: NodeId,
    /// 64 字符 hex sha256 canonical json spec(内容哈希)。
    pub import_hash: String,
    /// 节点 spec。
    pub spec: NodeSpec,
}

/// 边(含条件分支标签)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    /// 起始节点。
    pub from: NodeId,
    /// 目标节点。
    pub to: NodeId,
    /// Condition 节点的出边标签(非 Condition 节点为 None)。
    pub condition_branch: Option<ConditionBranch>,
}

/// 条件分支标签(Condition 节点出边)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionBranch {
    /// if 条件为真。
    True,
    /// if 条件为假。
    False,
    /// match 分支(按值路由)。
    Case(String),
}

/// 节点图(含子例程表 + 标准意图导出表)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Graph {
    /// 节点列表。
    pub nodes: Vec<Node>,
    /// 边列表。
    pub edges: Vec<Edge>,
    /// 子例程表(Loop 节点引用)。
    pub subroutines: HashMap<SubroutineId, Graph>,
    /// 源 ID。
    pub source_id: SourceId,
    /// 源站基础 URL(用于相对路径→绝对路径拼接,从 bookSourceUrl 提取)。
    pub base_url: String,
    /// 标准意图导出表——声明调用入口与 mapper 输出。
    #[serde(default)]
    pub intent_exports: HashMap<StandardIntent, IntentExport>,
}
