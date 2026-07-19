//! Canonical JSON 与 Definition hash。

use blake3::Hasher;

use crate::definition::RuleDefinition;
use crate::error::Error;

/// 将值序列化为递归规范化的确定性 JSON。
///
/// object key 会在每一层按字节序排序；因此 `HashMap` 的随机迭代顺序不能影响
/// Definition、Plan 或 effect fingerprint 的 content hash。数组顺序保持不变，因为
/// 它是作者合同的一部分。
///
/// # Errors
///
/// 序列化失败时返回 [`Error::Json`]。
pub fn canonical_json<T: serde::Serialize>(value: &T) -> Result<String, Error> {
    let mut value = serde_json::to_value(value)?;
    canonicalize_json_value(&mut value);
    Ok(serde_json::to_string(&value)?)
}

/// 计算 Definition 的稳定 canonical BLAKE3 hash（hex）。
///
/// # Errors
///
/// 序列化失败时返回 [`Error::Json`]。
pub fn definition_hash(definition: &RuleDefinition) -> Result<String, Error> {
    let canonical = canonical_json(definition)?;
    let mut hasher = Hasher::new();
    hasher.update(canonical.as_bytes());
    Ok(hasher.finalize().to_hex().to_string())
}

fn canonicalize_json_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                canonicalize_json_value(value);
            }
        }
        serde_json::Value::Object(object) => {
            let mut entries = std::mem::take(object).into_iter().collect::<Vec<_>>();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            for (_, value) in &mut entries {
                canonicalize_json_value(value);
            }
            for (key, value) in entries {
                object.insert(key, value);
            }
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
}
