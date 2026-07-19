//! Plan runtime 与 session 的公开 API。
//!
//! 本模块只持有 immutable Plan 的启动合同、会话事件和并发状态。实际调度交给
//! `scheduler`，结构/hash/path 验证交给 `validation`；因此调用方不能借此注入作者格式、
//! storage 或 Tauri 句柄。

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};

use futures::stream::{self, BoxStream, StreamExt};
use lj_capability::{IntentInput, StandardIntent};
use lj_media::MediaGraphDelta;
use lj_rule_model::{EffectKind, ExecutionPlan, PolicyCapabilities};
use tokio::sync::{Semaphore, mpsc};
use tracing::Instrument;
use uuid::Uuid;

use crate::effect::{CancellationHandle, EffectArchive, EffectHandlers, HttpExecutionCredentials};

use super::{scheduler, validation};

/// 当前 runtime 支持的 Plan schema 版本。
pub const SUPPORTED_PLAN_SCHEMA_VERSION: u32 = 1;

/// Plan runtime 的固定并发与版本配置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRuntimeConfig {
    /// runtime 接受的 compiler 身份；必须与 Plan 内版本完全一致。
    pub compiler_version: String,
    /// runtime 接受的 Plan schema 版本。
    pub plan_schema_version: u32,
    /// session event channel 的固定容量。
    pub event_channel_capacity: usize,
    /// 同时运行的 execution 上限。
    pub max_concurrent_executions: usize,
    /// 所有来源合计的同时 effect 上限。
    pub max_concurrent_effects: usize,
    /// 单一来源的同时 effect 上限。
    pub max_concurrent_effects_per_source: usize,
}

/// Plan 在启动前的验证失败。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanRuntimeError {
    /// runtime 配置包含零容量或空 compiler 身份。
    #[error("Plan runtime 配置无效: {0}")]
    InvalidConfiguration(&'static str),

    /// 当前线程没有 Tokio runtime，无法启动异步执行会话。
    #[error("Plan runtime 需要正在运行的 Tokio runtime")]
    MissingTokioRuntime,

    /// Plan schema 与 runtime 支持版本不一致。
    #[error("Plan schema 版本不匹配: 期望 {expected}，实际 {actual}")]
    SchemaVersionMismatch {
        /// runtime 支持的版本。
        expected: u32,
        /// Plan 声明的版本。
        actual: u32,
    },

    /// Plan compiler 身份与安装时 pin 的身份不一致。
    #[error("Plan compiler 版本不匹配")]
    CompilerVersionMismatch,

    /// 重新计算后的 immutable Plan hash 不匹配。
    #[error("Plan hash 校验失败")]
    PlanHashMismatch,

    /// Plan 内缺少可引用的节点。
    #[error("Plan 节点 {0} 不存在")]
    MissingNode(Uuid),

    /// Plan 没有声明请求的标准意图。
    #[error("Plan 未声明请求的标准意图")]
    MissingIntent,

    /// Plan 的结构、effect 声明或节点配置不符合 runtime 合同。
    #[error("Plan 结构无效: {0}")]
    InvalidPlan(&'static str),

    /// Plan 运行时不支持该控制流节点。
    #[error("Plan 含不支持的控制流节点")]
    UnsupportedControlFlow,

    /// Plan 的 canonical 序列化失败。
    #[error("Plan canonical 序列化失败")]
    CanonicalSerialization,
}

/// 运行中执行失败的稳定类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFailureCode {
    /// capability grant 拒绝 effect。
    CapabilityDenied,
    /// effect adapter 返回失败。
    EffectFailed,
    /// live effect 无法获得 durable capture 收据。
    CaptureFailed,
    /// 实际 handler 返回的 output/witness 不符合类型或安全合同。
    CaptureWitnessInvalid,
    /// archive 收据与刚执行的 effect 不一致。
    CaptureReceiptMismatch,
    /// replay archive 缺少记录。
    ReplayCaptureMissing,
    /// replay archive 中的记录不属于请求的 execution/node/effect。
    ReplayRecordMismatch,
    /// replay fingerprint 不等于当前 Plan/input fingerprint。
    ReplayFingerprintMismatch,
    /// replay 输出无法通过 output hash 校验。
    ReplayOutputHashMismatch,
    /// replay witness 缺失、损坏、类型不匹配或安全字段无效。
    ReplayWitnessMismatch,
    /// Plan 节点间的类型不匹配。
    InputTypeMismatch,
    /// Plan runtime 内部不变量被破坏。
    Internal,
}

/// 带 execution/node/effect/trace 归属的安全失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionFailure {
    /// 稳定失败类别。
    pub code: RuntimeFailureCode,
    /// 当前执行 ID。
    pub execution_id: Uuid,
    /// 关联 Plan 节点；执行级失败时为 `None`。
    pub node_id: Option<Uuid>,
    /// 关联 effect；非 effect 节点失败时为 `None`。
    pub effect_id: Option<Uuid>,
    /// 关联 trace ID。
    pub trace_id: String,
    /// 可安全展示的短消息，不含 body、cookie、token 或 URL query。
    pub message: String,
}

/// session 流中的事件。
#[derive(Debug, Clone)]
pub struct ExecutionEvent {
    /// 当前执行 ID。
    pub execution_id: Uuid,
    /// 单调递增的 session 序列号，从 1 开始。
    pub sequence: u64,
    /// 关联 trace ID。
    pub trace_id: String,
    /// 事件内容。
    pub kind: ExecutionEventKind,
}

/// session 流中的可观察执行状态。
#[derive(Debug, Clone)]
pub enum ExecutionEventKind {
    /// execution task 已创建，可能仍在等待并发 permit。
    Started,
    /// live effect 已获得匹配的 durable capture 收据。
    EffectCaptured {
        /// Plan 节点 ID。
        node_id: Uuid,
        /// effect ID。
        effect_id: Uuid,
        /// effect 类型。
        kind: EffectKind,
        /// 不泄露 payload 的 effect fingerprint。
        fingerprint: String,
        /// 不泄露 payload 的输出 hash。
        output_hash: String,
        /// 不泄露 payload 的安全 witness hash。
        witness_hash: String,
    },
    /// replay archive 的类型化输出已通过所有严格校验。
    EffectReplayed {
        /// Plan 节点 ID。
        node_id: Uuid,
        /// archive 中的 effect ID。
        effect_id: Uuid,
        /// effect 类型。
        kind: EffectKind,
        /// 不泄露 payload 的 effect fingerprint。
        fingerprint: String,
        /// 不泄露 payload 的输出 hash。
        output_hash: String,
        /// 不泄露 payload 的安全 witness hash。
        witness_hash: String,
    },
    /// Plan Mapper 已产生标准媒体增量；持久化/投影由 C2 seam 负责。
    DeltaProduced {
        /// Mapper 节点 ID。
        node_id: Uuid,
        /// 标准媒体资源图增量。
        delta: MediaGraphDelta,
    },
    /// execution 正常完成。
    Completed,
    /// execution 失败；同一 session 只会发出一个终态。
    Failed {
        /// 带完整归属的安全失败。
        failure: ExecutionFailure,
    },
    /// 取消已被观察到；同一 session 只会发出一个终态。
    Cancelled,
}

/// Plan 执行模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// 使用真实 handler，并要求每个输出先 durable capture。
    Live,
    /// 只从指定历史 execution 的 archive 读取 effect；禁止 live 回退。
    Replay {
        /// 作为 replay 输入的历史 execution ID。
        archived_execution_id: Uuid,
    },
}

/// 只含 immutable Plan 的执行请求。
///
/// runtime 故意没有接收 `RuleDefinition` 或来源作者 JSON 的字段；节点配置只能来自
/// compiler 写入 `ExecutionPlan` 的 canonical config。
#[derive(Clone)]
pub struct PlanExecutionRequest {
    /// 当前 execution ID。
    pub execution_id: Uuid,
    /// 已安装来源的稳定 ID，用于来源级并发限制和错误归属。
    pub source_id: String,
    /// 关联 trace ID。
    pub trace_id: String,
    /// 已安装、immutable 的执行计划。
    pub plan: ExecutionPlan,
    /// 要执行的标准意图。
    pub intent: StandardIntent,
    /// 标准意图输入。
    pub input: IntentInput,
    /// live 或指定 archive 的 replay。
    pub mode: ExecutionMode,
    /// 安装 grant 后有效的能力。
    pub capabilities: PolicyCapabilities,
    /// 已安装来源的 base URL。
    pub base_url: String,
    /// execution-only source 凭据；runtime 不会将其放入 tracing、事件或 effect fingerprint。
    /// replay 启动时 runtime 会在生成 session 前丢弃该字段，避免静态凭据进入 replay task。
    pub credentials: HttpExecutionCredentials,
}

/// 执行会话；调用方消费有界事件流并可幂等取消。
pub struct ExecutionSession {
    id: Uuid,
    cancellation: CancellationHandle,
    events: BoxStream<'static, ExecutionEvent>,
}

impl ExecutionSession {
    /// 返回当前 execution ID。
    #[must_use]
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// 请求取消，并返回本次调用是否首次改变取消状态。
    #[must_use]
    pub fn cancel(&self) -> bool {
        self.cancellation.cancel()
    }

    /// 克隆可在消费事件流后继续使用的取消句柄。
    ///
    /// delivery adapter 应在调用 [`Self::into_events`] 前保存该句柄；例如 Tauri 或
    /// `RuleSystem` 可按 execution ID 注册它，再把 session event stream 移交给独立的
    /// 消费任务。克隆句柄与 session 共用同一幂等取消状态。
    #[must_use]
    pub fn cancellation_handle(&self) -> CancellationHandle {
        self.cancellation.clone()
    }

    /// 返回当前是否已经请求取消。
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    /// 取得消费 session 事件的有界流。
    #[must_use]
    pub fn into_events(self) -> BoxStream<'static, ExecutionEvent> {
        self.events
    }
}

/// immutable Plan runtime。
///
/// 直接以已验证的 immutable Plan 调度 typed effect，不执行作者格式转换。
#[derive(Clone)]
pub struct PlanRuntime {
    state: Arc<RuntimeState>,
}

pub(super) struct RuntimeState {
    pub(super) config: PlanRuntimeConfig,
    pub(super) execution_permits: Arc<Semaphore>,
    pub(super) effect_permits: Arc<Semaphore>,
    source_effect_permits: Mutex<BTreeMap<String, Weak<Semaphore>>>,
}

impl PlanRuntime {
    /// 用固定版本、事件容量和并发上限创建 runtime。
    ///
    /// # Errors
    ///
    /// 任一容量为零或 compiler 身份为空时返回 [`PlanRuntimeError::InvalidConfiguration`]。
    pub fn new(config: PlanRuntimeConfig) -> Result<Self, PlanRuntimeError> {
        if config.compiler_version.trim().is_empty() {
            return Err(PlanRuntimeError::InvalidConfiguration(
                "compiler_version 不能为空",
            ));
        }
        if config.plan_schema_version == 0 {
            return Err(PlanRuntimeError::InvalidConfiguration(
                "plan_schema_version 必须大于零",
            ));
        }
        if config.event_channel_capacity == 0 {
            return Err(PlanRuntimeError::InvalidConfiguration(
                "event_channel_capacity 必须大于零",
            ));
        }
        if config.max_concurrent_executions == 0 {
            return Err(PlanRuntimeError::InvalidConfiguration(
                "max_concurrent_executions 必须大于零",
            ));
        }
        if config.max_concurrent_effects == 0 {
            return Err(PlanRuntimeError::InvalidConfiguration(
                "max_concurrent_effects 必须大于零",
            ));
        }
        if config.max_concurrent_effects_per_source == 0 {
            return Err(PlanRuntimeError::InvalidConfiguration(
                "max_concurrent_effects_per_source 必须大于零",
            ));
        }

        Ok(Self {
            state: Arc::new(RuntimeState {
                execution_permits: Arc::new(Semaphore::new(config.max_concurrent_executions)),
                effect_permits: Arc::new(Semaphore::new(config.max_concurrent_effects)),
                source_effect_permits: Mutex::new(BTreeMap::new()),
                config,
            }),
        })
    }

    /// 校验 immutable Plan 的版本、hash、effect 声明和节点配置。
    ///
    /// # Errors
    ///
    /// Plan schema/compiler/hash 不匹配，或 Plan 结构/节点配置无效时返回
    /// [`PlanRuntimeError`]。本函数不解析 `RuleDefinition` 或作者 JSON。
    pub fn validate_plan(&self, plan: &ExecutionPlan) -> Result<(), PlanRuntimeError> {
        validation::validate_plan(plan, &self.state.config)
    }

    /// 启动只消费 immutable Plan 的执行会话。
    ///
    /// # Errors
    ///
    /// Plan 未通过 [`Self::validate_plan`]、请求没有对应意图，或当前线程没有 Tokio
    /// runtime 时返回 [`PlanRuntimeError`]。启动后的失败会进入 session 的
    /// [`ExecutionEventKind::Failed`]，不会改为同步错误。
    pub fn execute(
        &self,
        mut request: PlanExecutionRequest,
        handlers: EffectHandlers,
        archive: Arc<dyn EffectArchive>,
    ) -> Result<ExecutionSession, PlanRuntimeError> {
        self.validate_plan(&request.plan)?;
        let path = validation::execution_path(&request.plan, request.intent)?;
        if request.source_id.trim().is_empty() {
            return Err(PlanRuntimeError::InvalidPlan("source_id 不能为空"));
        }
        if request.trace_id.trim().is_empty() {
            return Err(PlanRuntimeError::InvalidPlan("trace_id 不能为空"));
        }
        tokio::runtime::Handle::try_current().map_err(|_| PlanRuntimeError::MissingTokioRuntime)?;
        if matches!(request.mode, ExecutionMode::Replay { .. }) {
            // replay 只读取历史 capture；静态 source credential 不得随 replay task 传递。
            request.credentials = HttpExecutionCredentials::default();
        }

        let (sender, receiver) = mpsc::channel(self.state.config.event_channel_capacity);
        let execution_id = request.execution_id;
        let cancellation = CancellationHandle::new();
        let runner_cancellation = cancellation.clone();
        let state = self.state.clone();
        let span = tracing::info_span!(
            "plan_execution",
            execution_id = %request.execution_id,
            trace_id = %request.trace_id,
            source_id = %request.source_id,
        );

        tokio::spawn(
            async move {
                scheduler::run_execution(
                    state,
                    request,
                    path,
                    handlers,
                    archive,
                    runner_cancellation,
                    sender,
                )
                .await;
            }
            .instrument(span),
        );

        let events = stream::unfold(receiver, |mut receiver| async {
            receiver.recv().await.map(|event| (event, receiver))
        })
        .boxed();
        Ok(ExecutionSession {
            id: execution_id,
            cancellation,
            events,
        })
    }
}

impl RuntimeState {
    pub(super) fn source_effect_permit(&self, source_id: &str) -> Result<Arc<Semaphore>, ()> {
        let mut permits = self.source_effect_permits.lock().map_err(|_| ())?;
        permits.retain(|_, semaphore| semaphore.strong_count() > 0);
        if let Some(semaphore) = permits.get(source_id).and_then(Weak::upgrade) {
            return Ok(semaphore);
        }
        let semaphore = Arc::new(Semaphore::new(
            self.config.max_concurrent_effects_per_source,
        ));
        permits.insert(source_id.to_string(), Arc::downgrade(&semaphore));
        Ok(semaphore)
    }
}
