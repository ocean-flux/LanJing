//! 规则模型 crate。
//!
//! 只承载可序列化合同：`Definition`、`Plan`、`EventEnvelope`、`Diagnostic`、
//! `Policy` DTO，以及节点配置 IR。不引入 ORM、Tokio、Tauri、HTTP/QuickJS 实现。

pub mod budget;
pub mod definition;
pub mod descriptor;
pub mod diagnostic;
pub mod endpoint;
pub mod error;
pub mod event;
pub mod extract_rule;
pub mod hash;
pub mod invocation;
pub mod literal;
pub mod mapper_vocab;
pub mod plan;
pub mod policy;
pub mod schema;
pub mod sensitive;

pub use budget::{JsBudget, JsBudgetCeiling};
pub use definition::{
    CapabilityManifest, CollectionSelector, ConditionConfig, ConditionOperator, ConditionPredicate,
    ControlExpression, ControlledMapper, FlowEdge, FlowGraph, FlowNode, FlowNodeConfig,
    FlowNodeKind, FlowPortRef, ForEachConfig, JsConfig, JsOutputKind, LoopIterationLimit,
    LoopIterationLimitError, MAX_LOOP_ITERATIONS, MapperOutputKind, MergeConfig, MergeInput,
    MergeInputActivation, MergeStrategy, RuleDefinition, RulePackage, SourceIdentity, SourceSpan,
    UnavailableNodeConfig, read_rule_definition, read_rule_package,
};
pub use diagnostic::{Diagnostic, DiagnosticSeverity};
pub use endpoint::{HttpMethod, HttpSpec};
pub use error::Error;
pub use event::{ArtifactRef, EventEnvelope, EventType, SecretRef};
pub use extract_rule::{
    ExpectedDataType, ExtractRule, ExtractSpec, ExtractType, FieldRules, OutputTarget, RegexClean,
};
pub use hash::{canonical_json, definition_hash};
pub use invocation::{ControlTrace, InvocationPath, InvocationPathError, LoopInvocationSegment};
pub use literal::{
    CanonicalNumber, TypedLiteral, canonical_json_deep_eq, canonical_number_cmp,
    canonical_number_eq, typed_literal_matches_json,
};
pub use plan::{
    CONDITION_INPUT_HANDLE, ControlRegion, EffectDeclaration, EffectKind, ExecutionPlan,
    ExecutionPlanParts, IntentEntry, LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE, LOOP_BODY_HANDLE,
    LOOP_COLLECTION_HANDLE, LOOP_DONE_HANDLE, LOOP_YIELD_HANDLE, LoopControlRegion,
    MERGE_OUTPUT_HANDLE, PlanEdge, PlanForEachConfig, PlanNode, PlanNodeConfig, PlanNodeKind,
    PlanPort, PortValueKind, PortValueType, execution_plan_hash, read_execution_plan,
};
pub use policy::{Capability, CapabilityError, SystemCapabilities};
pub use schema::{RULE_CONTRACT_SCHEMA_VERSION, SchemaContract, SchemaReadError};
pub use sensitive::{RequestHeaderDisposition, SensitiveNamePolicy};
