//! Artifact 与 secret 输入 DTO。
//!
//! `ArtifactInput` 的逻辑 bytes 只在 single writer 的 blocking lane 内短暂存在；`Secret`
//! 一律先经安装级主密钥的 AES-256-GCM 加密，再按 BLAKE3 内容寻址落盘。事件和公开查询
//! 只保留 artifact ref/hash，绝不携带明文。

/// 需要写入 artifact 的逻辑字节。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactInput {
    /// artifact 的保密级别。
    pub kind: ArtifactKind,
    /// 要持久化的逻辑明文字节；secret 只会在加密前短暂存在于 writer blocking lane。
    pub bytes: Vec<u8>,
}

/// artifact 的存储形式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    /// 明文逻辑 body，磁盘上仅以 zstd frame 保存。
    Body,
    /// 敏感快照，磁盘上以 AES-256-GCM 密文保存。
    Secret,
}
