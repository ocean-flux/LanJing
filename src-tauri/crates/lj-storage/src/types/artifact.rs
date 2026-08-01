//! Artifact 与 secret 输入 DTO。
//!
//! `ArtifactInput` 只描述可内容寻址的 body bytes。secret 走随机 identity、owner-bound vault
//! API，不进入通用 Event artifact 输入。

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::StorageError;

/// 需要写入 artifact 的逻辑字节。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactInput {
    /// 要持久化的逻辑 body bytes。
    pub bytes: Vec<u8>,
}

/// artifact 的存储形式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ArtifactKind {
    /// 明文逻辑 body，磁盘上仅以 zstd frame 保存。
    Body,
}

/// 与 plaintext 无关、随机生成的加密 secret artifact 身份。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretArtifactId(Uuid);

impl SecretArtifactId {
    /// 生成随机 UUID v4 secret artifact 身份。
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// 返回底层 UUID，供 storage ownership row 使用。
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for SecretArtifactId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for SecretArtifactId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretArtifactId(<opaque>)")
    }
}

impl fmt::Display for SecretArtifactId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.hyphenated(), formatter)
    }
}

impl From<Uuid> for SecretArtifactId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl FromStr for SecretArtifactId {
    type Err = StorageError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parsed = Uuid::parse_str(value)
            .map_err(|_| StorageError::InvalidInput("secret artifact ID 无效".to_string()))?;
        if parsed.hyphenated().to_string() != value {
            return Err(StorageError::InvalidInput(
                "secret artifact ID 必须是 canonical UUID".to_string(),
            ));
        }
        Ok(Self(parsed))
    }
}
