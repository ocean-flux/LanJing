//! Canonical JSON 与 Definition hash。

use sha2::{Digest, Sha256};

use crate::definition::RuleDefinition;
use crate::error::Error;

/// 将值序列化为确定性 JSON（`serde_json` 默认 map 键序 + 稳定结构）。
///
/// # Errors
///
/// 序列化失败时返回 [`Error::Json`]。
pub fn canonical_json<T: serde::Serialize>(value: &T) -> Result<String, Error> {
    // BTreeMap 字段保证 key 有序；其余结构按类型定义稳定写出。
    Ok(serde_json::to_string(value)?)
}

/// 计算 Definition 的稳定 canonical hash（sha256 hex）。
///
/// # Errors
///
/// 序列化失败时返回 [`Error::Json`]。
pub fn definition_hash(definition: &RuleDefinition) -> Result<String, Error> {
    let canonical = canonical_json(definition)?;
    let digest = Sha256::digest(canonical.as_bytes());
    Ok(hex_encode(&digest))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
