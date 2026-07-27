//! Schema-v1 credential target、slot 与 sentinel primitive。
//!
//! 这里只承载不含明文 secret 的稳定 DTO。原文分离/重组由来源 adapter 完成；slot ID 只能
//! 随机生成，不能从 secret、document 或 path 推导。

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// credential 合同版本。
pub const CREDENTIAL_SCHEMA_VERSION: u32 = 1;
/// JSON string sentinel 的固定前缀。
pub const CREDENTIAL_SENTINEL_PREFIX: &str = "__LANJING_CREDENTIAL_SLOT_V1__:";

/// credential slot 所属作者格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceDocumentFormat {
    /// Legado 书源 JSON。
    Legado,
    /// Maccms10 endpoint 配置。
    Maccms10Endpoint,
}

/// slot 必须绑定的文档身份。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialTargetIdentity {
    /// 来源作者格式。
    pub format: SourceDocumentFormat,
    /// vault 分配的不透明文档身份。
    pub document_id: String,
    /// 保存版本；跨 revision 不能复用 sentinel。
    pub revision: u64,
}

/// 不透明随机 slot ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CredentialSlotId(Uuid);

impl CredentialSlotId {
    /// 生成与 secret 内容无关的随机 UUID v4。
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 返回 UUID 值，供 sentinel codec 使用。
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for CredentialSlotId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CredentialSlotId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.hyphenated(), formatter)
    }
}

/// 一个 schema-v1 credential slot。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialSlot {
    /// 必须等于 [`CREDENTIAL_SCHEMA_VERSION`]。
    pub schema_version: u32,
    /// 随机、不透明且在 manifest 内唯一。
    pub slot_id: CredentialSlotId,
    /// format/document/revision 绑定。
    pub target: CredentialTargetIdentity,
    /// RFC 6901 JSON Pointer。
    pub path: String,
    /// adapter 识别的逻辑敏感名称；不含值。
    pub name: String,
}

/// 一个保存版本的 credential slots 清单。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialSlotManifest {
    /// manifest schema 版本。
    pub schema_version: u32,
    /// 所有 slot 必须完全匹配的 target。
    pub target: CredentialTargetIdentity,
    /// 稳定 path 顺序的 slot 集合。
    pub slots: Vec<CredentialSlot>,
}

/// sentinel 解析失败；错误不携带原文。
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CredentialSentinelError {
    /// 前缀匹配但 slot ID 不是 canonical lowercase hyphenated UUID。
    #[error("credential sentinel 无效")]
    Invalid,
}

/// 为 slot 生成合法 JSON string value 的内容。
#[must_use]
pub fn credential_sentinel(slot_id: CredentialSlotId) -> String {
    format!("{CREDENTIAL_SENTINEL_PREFIX}{slot_id}")
}

/// 严格解析 sentinel 内容。
///
/// 非 sentinel 返回 `Ok(None)`；匹配前缀后必须是 canonical lowercase hyphenated UUID。
///
/// # Errors
///
/// 匹配固定前缀但 UUID 缺失、格式非 canonical 或不是 UUID 时返回
/// [`CredentialSentinelError::Invalid`]。
pub fn parse_credential_sentinel(
    value: &str,
) -> Result<Option<CredentialSlotId>, CredentialSentinelError> {
    let Some(raw_id) = value.strip_prefix(CREDENTIAL_SENTINEL_PREFIX) else {
        return Ok(None);
    };
    let uuid = Uuid::parse_str(raw_id).map_err(|_| CredentialSentinelError::Invalid)?;
    if uuid.hyphenated().to_string() != raw_id {
        return Err(CredentialSentinelError::Invalid);
    }
    Ok(Some(CredentialSlotId(uuid)))
}

#[cfg(test)]
mod tests {
    use super::{CredentialSlotId, credential_sentinel, parse_credential_sentinel};

    #[test]
    fn sentinel_codec_only_accepts_canonical_uuid() {
        let slot_id = CredentialSlotId::new();
        let sentinel = credential_sentinel(slot_id);
        assert_eq!(parse_credential_sentinel(&sentinel), Ok(Some(slot_id)));
        assert!(
            parse_credential_sentinel("ordinary value")
                .unwrap()
                .is_none()
        );
        assert!(parse_credential_sentinel("__LANJING_CREDENTIAL_SLOT_V1__:NOT-A-UUID").is_err());
    }
}
