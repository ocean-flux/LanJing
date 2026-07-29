//! Execution Plan 的稳定公开面。
//!
//! closed graph/control DTO 与 current wire/hash sealing 分属私有 owner；runtime 只消费这里
//! 重导出的 immutable Plan。

mod contract;
mod types;

pub use contract::{ExecutionPlan, execution_plan_hash, read_execution_plan};
pub use types::{
    CONDITION_INPUT_HANDLE, ControlRegion, EffectDeclaration, EffectKind, ExecutionPlanParts,
    IntentEntry, LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE, LOOP_BODY_HANDLE,
    LOOP_COLLECTION_HANDLE, LOOP_DONE_HANDLE, LOOP_YIELD_HANDLE, LoopControlRegion,
    MERGE_OUTPUT_HANDLE, PlanEdge, PlanForEachConfig, PlanNode, PlanNodeConfig, PlanNodeKind,
    PlanPort, PortValueKind, PortValueType,
};
