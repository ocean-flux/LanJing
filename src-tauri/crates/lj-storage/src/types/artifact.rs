//! Artifact 与 secret 输入 DTO。
//!
//! `ArtifactInput` 的逻辑 bytes 只在 single writer 的 blocking lane 内短暂存在；`Secret`
//! 一律先经安装级主密钥的 AES-256-GCM 加密，再按 BLAKE3 内容寻址落盘。事件和公开查询
//! 只保留 artifact ref/hash，绝不携带明文。

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::StorageError;

/// 需要写入 artifact 的逻辑字节。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactInput {
    /// artifact 的保密级别。
    pub kind: ArtifactKind,
    /// 要持久化的逻辑明文字节；secret 只会在加密前短暂存在于 writer blocking lane。
    pub bytes: Vec<u8>,
}

/// artifact 的存储形式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactKind {
    /// 明文逻辑 body，磁盘上仅以 zstd frame 保存。
    Body,
    /// 敏感快照，磁盘上以 AES-256-GCM 密文保存。
    Secret,
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
