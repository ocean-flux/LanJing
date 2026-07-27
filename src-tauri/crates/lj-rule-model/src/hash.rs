//! Canonical JSON 与 Definition hash。

use blake3::Hasher;

use crate::definition::{FlowNodeConfig, RuleDefinition};
use crate::error::Error;

/// 将值序列化为递归规范化的确定性 JSON。
///
/// object key 会在每一层按字节序排序；数组顺序保持不变。Definition/Plan 中声明顺序不
/// 承载语义的数组由各自 typed hash projection 在调用本函数前排序。
///
/// # Errors
///
/// 序列化失败时返回 [`Error::Json`]。
pub fn canonical_json<T: serde::Serialize>(value: &T) -> Result<String, Error> {
    let mut value = serde_json::to_value(value)?;
    canonicalize_json_value(&mut value);
    Ok(serde_json::to_string(&value)?)
}

/// 计算 Definition 的版本正确 canonical BLAKE3 hash（hex）。
///
/// v1 reader object 使用保留的精确旧 wire material；普通 authoring object 使用 writer-only
/// v2 semantic projection。v2 projection 忽略节点、边、声明顺序与源码 span。
///
/// # Errors
///
/// 序列化失败时返回 [`Error::Json`]。
pub fn definition_hash(definition: &RuleDefinition) -> Result<String, Error> {
    let canonical = if let Some(legacy) = definition.legacy_hash_material() {
        let mut value = serde_json::to_value(legacy)?;
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "contract".to_string(),
                serde_json::Value::String("rule_definition".to_string()),
            );
        }
        canonicalize_json_value(&mut value);
        serde_json::to_string(&value)?
    } else {
        canonical_json(&canonical_v2_definition(definition))?
    };
    let mut hasher = Hasher::new();
    hasher.update(canonical.as_bytes());
    Ok(hasher.finalize().to_hex().to_string())
}

fn canonical_v2_definition(definition: &RuleDefinition) -> RuleDefinition {
    let mut flow = definition.flow().clone();
    for node in &mut flow.nodes {
        node.span = None;
        match &mut node.config {
            FlowNodeConfig::Mapper(config) => config.identity_fields.sort(),
            FlowNodeConfig::Merge(config) => {
                config
                    .inputs
                    .sort_by(|left, right| left.handle.as_bytes().cmp(right.handle.as_bytes()));
            }
            FlowNodeConfig::Condition(config) => {
                config
                    .branches
                    .sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
            }
            FlowNodeConfig::Http(_)
            | FlowNodeConfig::Js(_)
            | FlowNodeConfig::Extract(_)
            | FlowNodeConfig::Loop(_) => {}
        }
    }
    flow.nodes.sort_by_key(|node| node.id);
    flow.edges.sort();
    let mut source_id_rules = definition.source_id_rules().to_vec();
    source_id_rules.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    RuleDefinition::new(
        definition.source_identity().clone(),
        definition.base_url(),
        definition.intent_exports().clone(),
        flow,
        definition.capability_manifest().clone(),
        source_id_rules,
    )
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
