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

/// 计算 Definition 的 current canonical BLAKE3 hash（hex）。
///
/// 使用唯一 current semantic projection：忽略节点、边、声明顺序与源码 span；Merge input
/// 按显式 `order` 规范化。物理数组换序不改变 hash，显式 order 与全部控制语义改变 hash。
///
/// # Errors
///
/// 序列化失败时返回 [`Error::Json`]。
pub fn definition_hash(definition: &RuleDefinition) -> Result<String, Error> {
    let canonical = canonical_json(&canonical_definition(definition))?;
    let mut hasher = Hasher::new();
    hasher.update(canonical.as_bytes());
    Ok(hasher.finalize().to_hex().to_string())
}

fn canonical_definition(definition: &RuleDefinition) -> RuleDefinition {
    let mut flow = definition.flow().clone();
    for node in &mut flow.nodes {
        node.span = None;
        match &mut node.config {
            FlowNodeConfig::Mapper(config) => config.identity_fields.sort(),
            FlowNodeConfig::Merge(config) => {
                config.inputs.sort_by_key(|input| input.order);
            }
            FlowNodeConfig::Condition(config) => {
                config
                    .branches
                    .sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
            }
            FlowNodeConfig::Http(_)
            | FlowNodeConfig::Js(_)
            | FlowNodeConfig::Extract(_)
            | FlowNodeConfig::Loop(_)
            | FlowNodeConfig::Unavailable(_) => {}
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
