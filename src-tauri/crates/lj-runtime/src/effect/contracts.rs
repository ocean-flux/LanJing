//! effect seam 的类型化输入、输出、请求与 adapter 合同。
//!
//! runtime 通过这些 trait 调度真实 HTTP、`QuickJS` 与 Extract adapter，但不依赖 node crate、
//! storage 或 Tauri。`QuickJS` adapter 必须把非 `Send` 引擎对象留在 blocking lane，不能让
//! 句柄越过 trait 的 async 边界。

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;
use lj_capability::IntentInput;
use lj_rule_model::{EffectKind, ExtractSpec, HttpSpec, PolicyCapabilities};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::HttpResponse;

use super::cancellation::EffectCancellation;
use super::witness::CapturedEffectOutput;

/// `QuickJS` effect 的安全失败类别。
///
/// 此枚举不携带引擎错误字符串、脚本或输入，因而可随 durable witness 重放校验。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuickJsErrorKind {
    /// 脚本求值失败。
    Evaluation,
    /// `QuickJS` runtime 创建失败。
    RuntimeInitialization,
    /// `QuickJS` context 创建失败。
    ContextInitialization,
    /// watchdog 到达执行时限。
    Timeout,
    /// watchdog 清理失败。
    Watchdog,
    /// blocking worker 意外结束。
    WorkerFailure,
}

/// `QuickJS` effect 的类型化返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuickJsOutput {
    /// 脚本返回可解析的 JSON。
    Json(serde_json::Value),
    /// 脚本返回非 JSON 文本。
    Raw(String),
    /// 脚本已执行但产生可安全归档、可 replay 的失败结果。
    Error(QuickJsErrorKind),
}

/// HTTP effect 的安全失败类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpEffectErrorKind {
    /// DNS/SSRF target 校验失败。
    TargetValidation,
    /// request 构建或网络发送失败。
    Request,
    /// redirect 超过上限或缺少安全 Location。
    Redirect,
    /// response body 读取或大小校验失败。
    ResponseRead,
}

/// 已发生且可安全归档/replay 的 effect 失败。
///
/// 不保存底层错误文本、URL query、body 或 secret；失败仍须通过 durable archive，避免 replay
/// 重新调用 live effect。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EffectFailure {
    /// HTTP effect 失败。
    Http {
        /// 稳定 HTTP 失败类别。
        error: HttpEffectErrorKind,
    },
    /// `QuickJS` effect 失败。
    QuickJs {
        /// 稳定 `QuickJS` 失败类别。
        error: QuickJsErrorKind,
    },
    /// Extract effect 失败。
    Extract,
}

impl EffectFailure {
    /// 返回失败所属的 effect 类型。
    #[must_use]
    pub fn kind(&self) -> EffectKind {
        match self {
            Self::Http { .. } => EffectKind::Http,
            Self::QuickJs { .. } => EffectKind::QuickJs,
            Self::Extract => EffectKind::Extract,
        }
    }
}

/// Extract effect 的类型化返回值。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractOutput {
    /// 按 Extract spec 得到的来源中间记录。
    pub records: Vec<serde_json::Value>,
}

/// 三类 effect 的类型化输出。
///
/// HTTP body 只在 runtime 与 archive seam 内存中传递；runtime 不会把它写进 tracing。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectOutput {
    /// HTTP 请求结果。
    Http(HttpResponse),
    /// `QuickJS` 执行结果。
    QuickJs(QuickJsOutput),
    /// 内容提取结果。
    Extract(ExtractOutput),
    /// 已发生并需要 durable capture 的安全失败；它不伪造协议响应。
    Failure(EffectFailure),
}

impl EffectOutput {
    /// 返回本输出所属的 effect 类型。
    #[must_use]
    pub fn kind(&self) -> EffectKind {
        match self {
            Self::Http(_) => EffectKind::Http,
            Self::QuickJs(_) => EffectKind::QuickJs,
            Self::Extract(_) => EffectKind::Extract,
            Self::Failure(failure) => failure.kind(),
        }
    }
}

/// effect 的上游输入。
///
/// `Intent` 是 Plan 入口的标准输入；`Output` 是已通过 durable capture 的上游输出。
/// 使用 `Arc` 避免为 HTTP body 在分支边上复制内存。
#[derive(Debug, Clone)]
pub enum EffectInput {
    /// 标准意图输入。
    Intent(IntentInput),
    /// 上游 effect 的已确认输出。
    Output(Arc<EffectOutput>),
}

impl EffectInput {
    /// 返回入口标准输入（非入口 effect 返回 `None`）。
    #[must_use]
    pub fn intent(&self) -> Option<&IntentInput> {
        match self {
            Self::Intent(input) => Some(input),
            Self::Output(_) => None,
        }
    }

    /// 返回上游输出（入口 effect 返回 `None`）。
    #[must_use]
    pub fn output(&self) -> Option<&EffectOutput> {
        match self {
            Self::Intent(_) => None,
            Self::Output(output) => Some(output),
        }
    }
}

/// HTTP handler 的执行请求。
#[derive(Clone)]
pub struct HttpEffectRequest {
    /// 当前执行 ID。
    pub execution_id: Uuid,
    /// 已安装来源的稳定 ID。
    pub source_id: String,
    /// 当前 Plan 节点 ID。
    pub node_id: Uuid,
    /// 当前 effect ID。
    pub effect_id: Uuid,
    /// 关联 tracing ID。
    pub trace_id: String,
    /// 已编译、已 canonicalize 的 HTTP spec。
    pub spec: HttpSpec,
    /// Plan 上游输入。
    pub input: EffectInput,
    /// 安装 grant 后的有效能力。
    pub capabilities: PolicyCapabilities,
    /// 已安装来源的 base URL。
    pub base_url: String,
    /// execution-only source 凭据；不得写入 tracing、事件或 effect fingerprint。
    pub credentials: super::credentials::HttpExecutionCredentials,
}

/// `QuickJS` handler 的执行请求。
#[derive(Debug, Clone)]
pub struct QuickJsEffectRequest {
    /// 当前执行 ID。
    pub execution_id: Uuid,
    /// 已安装来源的稳定 ID。
    pub source_id: String,
    /// 当前 Plan 节点 ID。
    pub node_id: Uuid,
    /// 当前 effect ID。
    pub effect_id: Uuid,
    /// 关联 tracing ID。
    pub trace_id: String,
    /// compiler 写入 Plan 的脚本源码。
    pub code: String,
    /// Plan 上游输入。
    pub input: EffectInput,
    /// 安装 grant 后的有效能力。
    pub capabilities: PolicyCapabilities,
}

/// Extract handler 的执行请求。
#[derive(Debug, Clone)]
pub struct ExtractEffectRequest {
    /// 当前执行 ID。
    pub execution_id: Uuid,
    /// 已安装来源的稳定 ID。
    pub source_id: String,
    /// 当前 Plan 节点 ID。
    pub node_id: Uuid,
    /// 当前 effect ID。
    pub effect_id: Uuid,
    /// 关联 tracing ID。
    pub trace_id: String,
    /// 已编译、已 canonicalize 的 Extract spec。
    pub spec: ExtractSpec,
    /// Plan 上游输入，必须为 [`EffectOutput::Http`]。
    pub input: EffectInput,
    /// 已安装来源的 base URL。
    pub base_url: String,
}

/// effect handler 返回的安全错误类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectErrorCode {
    /// effect 被取消。
    Cancelled,
    /// capability grant 拒绝该 effect。
    CapabilityDenied,
    /// HTTP 请求或响应读取失败。
    HttpRequest,
    /// `QuickJS` 创建、执行或 watchdog 中断失败。
    QuickJs,
    /// Extract 规则无法处理输入。
    Extract,
    /// 上游类型与 effect 输入合同不匹配。
    InputType,
    /// adapter 内部失败。
    Internal,
}

/// effect handler 返回的安全错误。
///
/// `message` 面向诊断，不能包含 cookie、token、完整 URL query 或 body。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectError {
    /// 稳定错误类别。
    pub code: EffectErrorCode,
    /// 可安全展示的简短消息。
    pub message: String,
}

impl EffectError {
    /// 用稳定类别和安全消息创建 handler 错误。
    #[must_use]
    pub fn new(code: EffectErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for EffectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for EffectError {}

/// 可执行已编译 HTTP effect 的真实 adapter。
#[async_trait]
pub trait HttpEffectHandler: Send + Sync {
    /// 执行 HTTP effect 并返回类型化输出与安全 witness。
    ///
    /// # Errors
    ///
    /// capability 被拒绝、请求/响应失败或取消时返回 [`EffectError`]。实现必须在
    /// `cancellation` 触发时中止 HTTP future，不能继续产生新网络请求。
    async fn execute_http(
        &self,
        request: HttpEffectRequest,
        cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError>;
}

/// 可执行已编译 `QuickJS` effect 的真实 adapter。
#[async_trait]
pub trait QuickJsEffectHandler: Send + Sync {
    /// 执行 `QuickJS` effect 并返回类型化输出与安全 witness。
    ///
    /// 脚本执行失败会作为 [`QuickJsOutput::Error`] 归档，以便 replay 复现同一失败；取消和
    /// capability 拒绝仍返回 [`EffectError`]，因为它们没有可推进的 live 结果。
    ///
    /// # Errors
    ///
    /// capability 被拒绝或取消时返回 [`EffectError`]。实现必须把 `rquickjs` 的非 `Send`
    /// 句柄留在 blocking lane 内。
    async fn execute_quickjs(
        &self,
        request: QuickJsEffectRequest,
        cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError>;
}

/// 可执行已编译 Extract effect 的真实 adapter。
#[async_trait]
pub trait ExtractEffectHandler: Send + Sync {
    /// 执行 Extract effect 并返回类型化输出与安全 witness。
    ///
    /// # Errors
    ///
    /// 输入不是 HTTP 响应、规则无法处理内容或取消时返回 [`EffectError`]。
    async fn execute_extract(
        &self,
        request: ExtractEffectRequest,
        cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError>;
}

/// 三类实际 effect handler 的集合。
#[derive(Clone)]
pub struct EffectHandlers {
    /// HTTP adapter。
    pub http: Arc<dyn HttpEffectHandler>,
    /// `QuickJS` adapter。
    pub quickjs: Arc<dyn QuickJsEffectHandler>,
    /// Extract adapter。
    pub extract: Arc<dyn ExtractEffectHandler>,
}

impl EffectHandlers {
    /// 用三个实际 adapter 创建 handler 集合。
    #[must_use]
    pub fn new(
        http: Arc<dyn HttpEffectHandler>,
        quickjs: Arc<dyn QuickJsEffectHandler>,
        extract: Arc<dyn ExtractEffectHandler>,
    ) -> Self {
        Self {
            http,
            quickjs,
            extract,
        }
    }
}
