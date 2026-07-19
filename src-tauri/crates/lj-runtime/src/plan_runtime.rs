//! immutable `ExecutionPlan` 的公开运行时入口。
//!
//! runtime 只接收 compiler 产出的 Plan，而不解析 `RuleDefinition`、作者 JSON 或旧 `Graph`。
//! 公开 session API 位于本模块；调度、effect 执行及 Plan 验证分别收敛到私有职责模块，避免
//! 把 durable/replay 不变量重新聚合为一个巨型文件。

mod api;
mod scheduler;
mod validation;

pub use api::{
    ExecutionEvent, ExecutionEventKind, ExecutionFailure, ExecutionMode, ExecutionSession,
    PlanExecutionRequest, PlanRuntime, PlanRuntimeConfig, PlanRuntimeError, RuntimeFailureCode,
    SUPPORTED_PLAN_SCHEMA_VERSION,
};
