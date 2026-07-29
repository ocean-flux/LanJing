//! 只负责语义无损的确定性声明排序。

use super::{FlowNodeConfig, RuleDefinition};

/// 将 current 作者 Definition 规范化为与物理声明顺序无关的形式。
///
/// 节点按 id、语义边按四元组 identity 排序；Merge input 按显式 `order` 排序；
/// Condition branch 按原始 UTF-8 bytes 排序。Mapper identity fields 等真正有序字段保持
/// 原样。物理数组排列与 editor layout 永不进入 hash。
#[must_use]
pub fn canonicalize(definition: &RuleDefinition) -> RuleDefinition {
    let mut canonical = definition.clone();
    let flow = canonical.flow_mut();
    for node in &mut flow.nodes {
        match &mut node.config {
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
            | FlowNodeConfig::Mapper(_)
            | FlowNodeConfig::Loop(_) => {}
        }
    }
    flow.nodes.sort_by_key(|node| node.id);
    flow.edges.sort();
    canonical
}
