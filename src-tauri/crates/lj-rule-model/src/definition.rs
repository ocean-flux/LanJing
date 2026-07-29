//! 规则 Definition / Package 的稳定公开面。
//!
//! closed config/control 与 graph/current wire 分属私有 owner；外部仍从本模块或 crate root
//! 使用唯一 current 类型与 reader。

mod config;
mod contract;

pub use config::{
    CapabilityManifest, CollectionSelector, ConditionConfig, ConditionOperator, ConditionPredicate,
    ControlExpression, ControlledMapper, FlowNodeConfig, FlowNodeKind, ForEachConfig, JsConfig,
    JsOutputKind, LoopIterationLimit, LoopIterationLimitError, MAX_LOOP_ITERATIONS,
    MapperOutputKind, MergeConfig, MergeInput, MergeInputActivation, MergeStrategy, SourceIdentity,
    SourceSpan,
};
pub use contract::{
    FlowEdge, FlowGraph, FlowNode, FlowPortRef, RuleDefinition, RulePackage, read_rule_definition,
    read_rule_package,
};
