//! 规则 Definition / Package — 可编辑、可移植、可 canonicalize 的作者合同。
//!
//! 与 `plan` 模块类型使用不同 serde tag，禁止互相直接反序列化。

use std::collections::BTreeMap;

use lj_capability::{IntentExport, StandardIntent};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::endpoint::HttpSpec;
use crate::extract_rule::ExtractSpec;
use crate::policy::PolicyCapabilities;

/// 逻辑来源稳定身份（安装版本另有 version/hash）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceIdentity {
    /// 稳定身份字符串（来源持有，不随安装版本变化）。
    pub id: String,
}

/// 诊断源码定位。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
pub struct ControlledMapper {
    /// Mapper 产出的标准资源类型。
    pub output: MapperOutputKind,
    /// 可用于生成来源内稳定 ID 的字段名，至少一个。
    pub identity_fields: Vec<String>,
}

/// Flow 节点类型。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FlowNodeKind {
    /// HTTP 请求节点。
    Http,
    /// JS 执行节点。
    Js,
    /// 提取节点。
    Extract,
    /// 受控 Mapper 节点。
    Mapper,
    /// 合并多上游 stream。
    Merge,
    /// 条件路由。
    Condition,
    /// 迭代执行子图。
    Loop,
}

/// Flow 节点配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowNode {
    /// 节点稳定 ID。
    pub id: Uuid,
    /// 节点类型。
    pub kind: FlowNodeKind,
    /// HTTP 节点配置。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpSpec>,
    /// JS 源码。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub js_code: Option<String>,
    /// 提取配置。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extract: Option<ExtractSpec>,
    /// Mapper 配置。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mapper: Option<ControlledMapper>,
    /// 可选源码 span。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
}

/// Flow 边。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowEdge {
    /// 起始节点。
    pub from: Uuid,
    /// 目标节点。
    pub to: Uuid,
    /// 条件分支标签（非 Condition 为 None）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition_branch: Option<String>,
}

/// 类型化 Flow 图。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowGraph {
    /// 节点列表。
    pub nodes: Vec<FlowNode>,
    /// 边列表。
    pub edges: Vec<FlowEdge>,
}

/// 规则定义（作者合同）。
///
/// serde tag 固定为 `rule_definition`，与 `ExecutionPlan` 区分。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "contract", rename = "rule_definition")]
pub struct RuleDefinition {
    /// 定义 schema 版本。
    pub schema_version: u32,
    /// 逻辑来源身份。
    pub source_identity: SourceIdentity,
    /// 展示/安装用基础 URL。
    pub base_url: String,
    /// 标准意图导出表。
    pub intent_exports: BTreeMap<StandardIntent, IntentExport>,
    /// 类型化 Flow。
    pub flow: FlowGraph,
    /// 能力清单。
    pub capability_manifest: CapabilityManifest,
    /// 来源持有的 ID 规则描述（自由文本或字段名列表）。
    pub source_id_rules: Vec<String>,
}

/// 规则包：Definition + 元数据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "contract", rename = "rule_package")]
pub struct RulePackage {
    /// 包 schema 版本。
    pub schema_version: u32,
    /// 逻辑来源身份。
    pub source_identity: SourceIdentity,
    /// 安装/预览版本号（同一 identity 可有多版本）。
    pub version: String,
    /// 作者 Definition。
    pub definition: RuleDefinition,
}
