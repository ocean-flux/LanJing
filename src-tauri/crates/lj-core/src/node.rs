//! 节点图结构 — 节点/边/图/子例程(ADR-0011, ADR-0022, ADR-0026)。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::endpoint::EndpointKind;
use crate::graph_schema::ConditionBranch;

/// 节点类型(6 种执行节点，端点不是 `NodeKind`)。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeKind {
    /// HTTP 请求节点。
    Http,
    /// JS 执行节点。
    Js,
    /// 提取节点(多数据类型多选择器)。
    Extract,
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
    // Merge/Condition/Loop 首刀 stub，spec 暂不定义。
}

/// JS 节点 spec。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsSpec {
    /// JS 代码。
    pub code: String,
    /// 关联的端点类型(用于 tracing span 命名)。
    pub endpoint_kind: Option<EndpointKind>,
}

/// 节点(含双 ID:ADR-0019)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// `UUIDv4` 恒久 ID。
    pub node_id: NodeId,
    /// 64 字符 hex sha256 canonical json spec(内容哈希)。
    pub import_hash: String,
    /// 节点 spec。
    pub spec: NodeSpec,
}

/// 边(含条件分支标签:ADR-0026)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    /// 起始节点。
    pub from: NodeId,
    /// 目标节点。
    pub to: NodeId,
    /// Condition 节点的出边标签(非 Condition 节点为 None)。
    pub condition_branch: Option<ConditionBranch>,
}

/// 节点图(含子例程表:ADR-0026)。
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
}
