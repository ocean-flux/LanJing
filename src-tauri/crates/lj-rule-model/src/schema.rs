//! Rule/Package/Plan 共享唯一 current schema 与 typed reader 错误。
//!
//! `lj-rule-model` 独占 ingest preflight：只分类 current contract tag、numeric schema 与
//! current typed data；不识别历史 shape，也不向 storage/runtime 暴露第二套 reader。

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
            Self::InvalidData { .. } => "RULE_CONTRACT_INVALID_DATA",
        }
    }
}

/// 解析 JSON 并完成 model 独占 preflight。
///
/// # Errors
///
/// 返回 malformed、contract mismatch、未知 schema 或 current invalid data。
pub(crate) fn parse_contract_json(
    bytes: &[u8],
    expected: SchemaContract,
) -> Result<Value, SchemaReadError> {
    let value = serde_json::from_slice(bytes).map_err(SchemaReadError::Malformed)?;
    validate_contract_value(&value, expected)?;
    Ok(value)
}

/// 校验 current object、contract tag 与 numeric schema。
///
/// # Errors
///
/// 根非 object、contract 不匹配、schema 缺失/非 u32或未知 schema 时失败。
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
    Ok(())
}

pub(crate) fn invalid_data(contract: SchemaContract, reason: impl Into<String>) -> SchemaReadError {
    SchemaReadError::InvalidData {
        contract,
        reason: reason.into(),
    }
}
