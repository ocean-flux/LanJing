//! 规则模型 crate。
//!
//! 只承载可序列化合同：`Definition`、`Plan`、`EventEnvelope`、`Diagnostic`、
//! `Policy` DTO，以及节点配置 IR。不引入 Diesel、Tokio、Tauri、HTTP/QuickJS 实现。

pub mod definition;
pub mod diagnostic;
pub mod endpoint;
pub mod error;
pub mod event;
pub mod extract_rule;
pub mod hash;
pub mod mapper_vocab;
pub mod plan;
pub mod policy;

pub use definition::{
    CapabilityManifest, ControlledMapper, FlowEdge, FlowGraph, FlowNode, FlowNodeKind,
    RuleDefinition, RulePackage, SourceIdentity, SourceSpan,
};
pub use diagnostic::{Diagnostic, DiagnosticSeverity};
pub use endpoint::{HttpMethod, HttpSpec};
pub use error::Error;
pub use event::{ArtifactRef, EventEnvelope, EventType, SecretRef};
pub use extract_rule::{
    ExpectedDataType, ExtractRule, ExtractSpec, ExtractType, FieldRules, OutputTarget, RegexClean,
};
pub use hash::{canonical_json, definition_hash};
pub use plan::{
    EffectDeclaration, EffectKind, ExecutionPlan, IntentEntry, PlanNode, PlanNodeKind, PlanPort,
};
pub use policy::{Capability, CapabilityError, PolicyCapabilities, SystemCapabilities};
