//! Rule/Package/Plan 共享 schema 版本与 typed reader 错误。
//!
//! 版本判断只在 `lj-rule-model` 内把 wire 整数映射为闭集枚举；调用方不得用裸整数猜测
//! installed artifact 的合同版本。

use serde_json::Value;

/// RuleDefinition 当前唯一 writer schema 版本。
pub const RULE_DEFINITION_SCHEMA_VERSION: u32 = 2;
/// RulePackage 当前唯一 writer schema 版本。
pub const RULE_PACKAGE_SCHEMA_VERSION: u32 = 2;
/// ExecutionPlan 当前唯一 writer schema 版本。
pub const EXECUTION_PLAN_SCHEMA_VERSION: u32 = 2;

pub(crate) const LEGACY_CONTRACT_SCHEMA_VERSION: u32 = 1;

/// 已知合同 schema 版本。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContractSchemaVersion {
    /// 旧线性 installed artifact 的只读版本。
    V1,
    /// 当前作者合同与 immutable Plan writer 版本。
    V2,
}

impl ContractSchemaVersion {
    /// 返回 JSON wire 使用的无符号整数。
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        match self {
            Self::V1 => LEGACY_CONTRACT_SCHEMA_VERSION,
            Self::V2 => 2,
        }
    }

    pub(crate) const fn from_u32(value: u32) -> Option<Self> {
        match value {
            LEGACY_CONTRACT_SCHEMA_VERSION => Some(Self::V1),
            2 => Some(Self::V2),
            _ => None,
        }
    }
}

/// 可版本化的规则合同种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SchemaContract {
    /// 作者 RuleDefinition。
    RuleDefinition,
    /// 安装 RulePackage。
    RulePackage,
    /// immutable ExecutionPlan。
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
    /// wire 版本不是当前 reader 明确支持的 v1/v2。
    #[error("规则合同版本不兼容: {contract:?} schema_version={version}")]
    IncompatibleVersion {
        /// 被读取的合同。
        contract: SchemaContract,
        /// 未知 wire 版本。
        version: u32,
    },
    /// 已知版本的字段或旧线性投影无法转换为 typed v2 内存模型。
    #[error("规则合同数据无效: {contract:?}: {reason}")]
    InvalidData {
        /// 失败的合同。
        contract: SchemaContract,
        /// 不包含原始输入内容的安全原因。
        reason: String,
    },
}

pub(crate) fn parse_contract_json(
    bytes: &[u8],
    expected: SchemaContract,
) -> Result<(Value, ContractSchemaVersion), SchemaReadError> {
    let value = serde_json::from_slice(bytes).map_err(SchemaReadError::Malformed)?;
    let version = validate_contract_value(&value, expected)?;
    Ok((value, version))
}

pub(crate) fn validate_contract_value(
    value: &Value,
    expected: SchemaContract,
) -> Result<ContractSchemaVersion, SchemaReadError> {
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
    ContractSchemaVersion::from_u32(version).ok_or(SchemaReadError::IncompatibleVersion {
        contract: expected,
        version,
    })
}

pub(crate) fn invalid_data(contract: SchemaContract, reason: impl Into<String>) -> SchemaReadError {
    SchemaReadError::InvalidData {
        contract,
        reason: reason.into(),
    }
}
