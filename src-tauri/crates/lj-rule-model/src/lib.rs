//! 规则模型 crate。
//!
//! 只承载可序列化合同：`Definition`、`Plan`、`EventEnvelope`、`Diagnostic`、
//! `Policy` DTO，以及节点配置 IR。不引入 Diesel、Tokio、Tauri、HTTP/QuickJS 实现。

pub mod credential;
pub mod definition;
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

pub use credential::{
    CREDENTIAL_SCHEMA_VERSION, CREDENTIAL_SENTINEL_PREFIX, CredentialSentinelError, CredentialSlot,
    CredentialSlotId, CredentialSlotManifest, CredentialTargetIdentity, SourceDocumentFormat,
    credential_sentinel, parse_credential_sentinel,
};
pub use definition::{
    CapabilityManifest, CollectionSelector, ConditionConfig, ConditionOperator, ConditionPredicate,
    ControlExpression, ControlledMapper, FlowEdge, FlowGraph, FlowNode, FlowNodeConfig,
    FlowNodeKind, FlowPortRef, ForEachConfig, JsConfig, JsOutputKind, LoopIterationLimit,
    LoopIterationLimitError, MAX_LOOP_ITERATIONS, MapperOutputKind, MergeConfig, MergeInput,
    MergeInputActivation, MergeStrategy, RuleDefinition, RulePackage, SourceIdentity, SourceSpan,
    read_rule_definition, read_rule_package,
};
pub use diagnostic::{AuthoringDiagnostic, Diagnostic, DiagnosticSeverity, SupportClass};
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
pub use policy::{Capability, CapabilityError, PolicyCapabilities, SystemCapabilities};
pub use schema::{RULE_CONTRACT_SCHEMA_VERSION, SchemaContract, SchemaReadError};
pub use sensitive::{RequestHeaderDisposition, SensitiveNamePolicy};
