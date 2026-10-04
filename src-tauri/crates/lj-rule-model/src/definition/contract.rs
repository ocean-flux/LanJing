//! Definition graph、current wire reader/writer 与 package 合同。

use std::collections::BTreeMap;

use lj_capability::{IntentExport, StandardIntent};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use super::config::{CapabilityManifest, FlowNodeConfig, FlowNodeKind, SourceIdentity, SourceSpan};
use crate::schema::{
    RULE_CONTRACT_SCHEMA_VERSION, SchemaContract, SchemaReadError, invalid_data,
    parse_contract_json, validate_contract_value,
};

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

    /// 返回与 config 一致的节点类型；未安装能力返回 `None`。
    #[must_use]
    pub const fn kind(&self) -> Option<FlowNodeKind> {
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
/// JSON 无效、contract tag 不匹配、未知 schema、字段未知或 current shape
/// 损坏时返回 [`SchemaReadError`]。
pub fn read_rule_definition(bytes: &[u8]) -> Result<RuleDefinition, SchemaReadError> {
    let value = parse_contract_json(bytes, SchemaContract::RuleDefinition)?;
    definition_from_value(value)
}

/// 从 JSON bytes 读取唯一 current `RulePackage`。
///
/// # Errors
///
/// JSON 无效、contract tag 不匹配、未知 schema、字段未知或 package 与
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
    if wire.source_identity != *definition.source_identity() {
        return Err(invalid_data(
            SchemaContract::RulePackage,
            "package source_identity 必须与嵌套 Definition 的 source_identity 一致".to_string(),
        ));
    }
    Ok(RulePackage {
        source_identity: wire.source_identity,
        version: wire.version,
        definition,
    })
}
