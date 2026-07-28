//! Rule/Package/Plan 共享唯一 current schema 与 typed reader 错误。
//!
//! `lj-rule-model` 独占 ingest preflight：未知 numeric schema 与 schema=1 的历史结构签名
//! 分别映射为稳定错误；不反序列化 legacy DTO，也不向 storage/runtime 暴露第二套分类器。

use serde_json::Value;

/// Definition / Package / Plan 共享的唯一 current schema 版本。
pub const RULE_CONTRACT_SCHEMA_VERSION: u32 = 1;

/// 可版本化的规则合同种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SchemaContract {
    /// 作者 `RuleDefinition`。
    RuleDefinition,
    /// 安装 `RulePackage`。
    RulePackage,
    /// immutable `ExecutionPlan`。
    ExecutionPlan,
}

impl SchemaContract {
    /// 返回 serde `contract` tag 的稳定 wire 名称。
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::RuleDefinition => "rule_definition",
            Self::RulePackage => "rule_package",
            Self::ExecutionPlan => "execution_plan",
        }
    }
}

/// 版本化规则合同读取错误。
#[derive(Debug, thiserror::Error)]
pub enum SchemaReadError {
    /// 输入不是合法 JSON。
    #[error("规则合同 JSON 无效: {0}")]
    Malformed(#[source] serde_json::Error),
    /// `contract` tag 与目标 Rust 类型不匹配。
    #[error("规则合同类型不匹配: 期望 {expected:?}, 实际 {actual}")]
    ContractMismatch {
        /// reader 期望的合同。
        expected: SchemaContract,
        /// 输入携带的 tag；缺失时为空字符串。
        actual: String,
    },
    /// wire 版本不是唯一 current schema。
    #[error("规则合同 schema 不受支持: {contract:?} schema_version={version}")]
    SchemaUnsupported {
        /// 被读取的合同。
        contract: SchemaContract,
        /// 未知 wire 版本。
        version: u32,
    },
    /// schema=1 但命中已删除的历史线性结构签名。
    #[error("历史规则合同不受支持: {contract:?}")]
    LegacyUnsupported {
        /// 被读取的合同。
        contract: SchemaContract,
    },
    /// current shape 字段损坏、未知字段或类型错误。
    #[error("规则合同数据无效: {contract:?}: {reason}")]
    InvalidData {
        /// 失败的合同。
        contract: SchemaContract,
        /// 不包含原始输入内容的安全原因。
        reason: String,
    },
}

impl SchemaReadError {
    /// 返回跨层稳定错误码。
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Malformed(_) => "RULE_CONTRACT_MALFORMED",
            Self::ContractMismatch { .. } => "RULE_CONTRACT_CONTRACT_MISMATCH",
            Self::SchemaUnsupported { .. } => "RULE_CONTRACT_SCHEMA_UNSUPPORTED",
            Self::LegacyUnsupported { .. } => "LEGACY_RULE_CONTRACT_UNSUPPORTED",
            Self::InvalidData { .. } => "RULE_CONTRACT_INVALID_DATA",
        }
    }
}

/// 解析 JSON 并完成 model 独占 preflight。
///
/// # Errors
///
/// 返回 malformed、contract mismatch、未知 schema、历史签名或 current invalid data。
pub(crate) fn parse_contract_json(
    bytes: &[u8],
    expected: SchemaContract,
) -> Result<Value, SchemaReadError> {
    let value = serde_json::from_slice(bytes).map_err(SchemaReadError::Malformed)?;
    validate_contract_value(&value, expected)?;
    Ok(value)
}

/// 校验 object/contract/numeric schema，并识别 schema=1 历史结构签名。
///
/// # Errors
///
/// 根非 object、contract 不匹配、schema 缺失/非 u32、未知 schema、历史签名时失败。
pub(crate) fn validate_contract_value(
    value: &Value,
    expected: SchemaContract,
) -> Result<(), SchemaReadError> {
    let Some(object) = value.as_object() else {
        return Err(SchemaReadError::InvalidData {
            contract: expected,
            reason: "合同根必须是 object".to_string(),
        });
    };
    let actual = object
        .get("contract")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if actual != expected.wire_name() {
        return Err(SchemaReadError::ContractMismatch {
            expected,
            actual: actual.to_string(),
        });
    }
    let version = object
        .get("schema_version")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| SchemaReadError::InvalidData {
            contract: expected,
            reason: "schema_version 必须是 u32".to_string(),
        })?;
    if version != RULE_CONTRACT_SCHEMA_VERSION {
        return Err(SchemaReadError::SchemaUnsupported {
            contract: expected,
            version,
        });
    }
    if matches_legacy_structure(value, expected) {
        return Err(SchemaReadError::LegacyUnsupported { contract: expected });
    }
    Ok(())
}

pub(crate) fn invalid_data(contract: SchemaContract, reason: impl Into<String>) -> SchemaReadError {
    SchemaReadError::InvalidData {
        contract,
        reason: reason.into(),
    }
}

/// 在不反序列化 legacy DTO 的前提下识别 schema=1 历史线性结构签名。
fn matches_legacy_structure(value: &Value, contract: SchemaContract) -> bool {
    match contract {
        SchemaContract::RuleDefinition => is_legacy_definition_shape(value),
        SchemaContract::RulePackage => value.get("definition").is_some_and(|definition| {
            definition.get("contract").and_then(Value::as_str)
                == Some(SchemaContract::RuleDefinition.wire_name())
                && definition.get("schema_version").and_then(Value::as_u64)
                    == Some(u64::from(RULE_CONTRACT_SCHEMA_VERSION))
                && is_legacy_definition_shape(definition)
        }),
        SchemaContract::ExecutionPlan => is_legacy_plan_shape(value),
    }
}

fn is_legacy_definition_shape(value: &Value) -> bool {
    let Some(flow) = value.get("flow").and_then(Value::as_object) else {
        return false;
    };
    if flow
        .get("nodes")
        .and_then(Value::as_array)
        .is_some_and(|nodes| nodes.iter().any(is_legacy_flow_node_shape))
    {
        return true;
    }
    flow.get("edges")
        .and_then(Value::as_array)
        .is_some_and(|edges| edges.iter().any(is_legacy_flow_edge_shape))
}

fn is_legacy_flow_node_shape(node: &Value) -> bool {
    let Some(object) = node.as_object() else {
        return false;
    };
    // 历史线性节点：顶层 kind + 分散 config 字段，没有 current tagged `config`。
    object.contains_key("kind")
        && !object.contains_key("config")
        && (object.contains_key("http")
            || object.contains_key("js_code")
            || object.contains_key("extract")
            || object.contains_key("mapper")
            || object.contains_key("merge")
            || object.contains_key("condition")
            || object.contains_key("loop"))
}

fn is_legacy_flow_edge_shape(edge: &Value) -> bool {
    let Some(object) = edge.as_object() else {
        return false;
    };
    // 历史边：from/to 为裸 UUID，无 handle port 对象。
    matches!(object.get("from"), Some(Value::String(_)))
        && matches!(object.get("to"), Some(Value::String(_)))
        && !object.contains_key("from_handle")
        && object.get("from").is_some_and(|from| !from.is_object())
        && object.get("to").is_some_and(|to| !to.is_object())
}

fn is_legacy_plan_shape(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    let legacy_top_level_fields = [
        "compiler_version",
        "definition_hash",
        "plan_hash",
        "nodes",
        "edges",
        "intent_entries",
        "effects",
        "capability_requirements",
    ];
    if !object.contains_key("control_regions")
        && legacy_top_level_fields
            .iter()
            .all(|field| object.contains_key(*field))
    {
        return true;
    }

    if value
        .get("edges")
        .and_then(Value::as_array)
        .is_some_and(|edges| edges.iter().any(is_legacy_plan_edge_shape))
    {
        return true;
    }
    value
        .get("nodes")
        .and_then(Value::as_array)
        .is_some_and(|nodes| nodes.iter().any(is_legacy_plan_node_shape))
}

fn is_legacy_plan_edge_shape(edge: &Value) -> bool {
    // 历史 Plan edge 为 `[from_uuid, to_uuid]` 二元组。
    edge.as_array()
        .is_some_and(|items| items.len() == 2 && items.iter().all(Value::is_string))
}

fn is_legacy_plan_node_shape(node: &Value) -> bool {
    let Some(object) = node.as_object() else {
        return false;
    };
    if object.contains_key("kind")
        && object.contains_key("config")
        && !object
            .get("config")
            .and_then(Value::as_object)
            .is_some_and(|config| config.contains_key("kind"))
    {
        // 历史节点：顶层 kind + 未 tagged 的 config object/value。
        return true;
    }
    let ports_legacy = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|ports| {
                ports.iter().any(|port| {
                    port.as_object().is_some_and(|port| {
                        port.contains_key("type_tag") || port.contains_key("name")
                    })
                })
            })
    };
    ports_legacy("inputs") || ports_legacy("outputs")
}
