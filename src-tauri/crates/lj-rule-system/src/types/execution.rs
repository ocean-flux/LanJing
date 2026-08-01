//! execution request、event、session 与 test-support witness DTO。

use futures::stream::BoxStream;
use lj_capability::{IntentInput, StandardIntent};
use lj_media::MediaGraphDelta;
use lj_rule_model::ArtifactRef;
use lj_runtime::CancellationHandle;
use lj_storage::EventProjectionStorage;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::RuleError;

/// execution stable opaque ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExecutionId(Uuid);

impl ExecutionId {
    pub(crate) const fn from_uuid(value: Uuid) -> Self {
        Self(value)
    }

    pub(crate) const fn as_uuid(self) -> Uuid {
        self.0
    }
}

/// execution 请求模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ExecutionMode {
    /// 调用真实 effect adapter。
    Live,
    /// 读取指定历史 execution pin 与 archive。
    Replay {
        /// 被重放 execution。
        execution_id: ExecutionId,
    },
}

/// 启动标准意图 execution 请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecuteRequest {
    /// installed source ID。
    pub source_id: super::SourceId,
    /// 标准意图。
    pub intent: StandardIntent,
    /// 标准输入。
    pub input: IntentInput,
    /// live 或 replay。
    pub mode: ExecutionMode,
}

/// durable 后可 delivery 的 execution event。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionEvent {
    /// execution ID。
    pub execution_id: ExecutionId,
    /// stream sequence。
    pub sequence: u64,
    /// 安全 trace ID。
    pub trace_id: String,
    /// UTC epoch milliseconds。
    pub occurred_at_ms: i64,
    /// event 内容。
    pub kind: ExecutionEventKind,
}

/// execution 对外状态转换。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecutionEventKind {
    /// start 已 durable。
    Started,
    /// 安全诊断。
    Diagnostic {
        /// 稳定 code。
        code: String,
        /// 安全消息。
        message: String,
    },
    /// live effect archive 已 durable。
    EffectCaptured {
        /// execution-local effect ID。
        effect_id: Uuid,
        /// durable artifact refs。
        artifact_refs: Vec<ArtifactRef>,
        /// effect output BLAKE3。
        output_hash: String,
    },
    /// 媒体 delta 与 projection 已原子提交。
    DeltaCommitted {
        /// global revision。
        global_revision: u64,
        /// source-local revision。
        source_revision: u64,
        /// 标准媒体增量。
        delta: MediaGraphDelta,
    },
    /// 正常结束。
    Completed,
    /// 安全失败。
    Failed {
        /// 完整安全错误。
        error: RuleError,
    },
    /// cancellation 已观察。
    Cancelled,
}

/// opaque execution cancellation handle。
#[derive(Clone)]
pub struct ExecutionCancellation {
    inner: CancellationHandle,
}

impl ExecutionCancellation {
    pub(crate) const fn new(inner: CancellationHandle) -> Self {
        Self { inner }
    }

    /// 请求取消；返回是否首次改变状态。
    #[must_use]
    pub fn cancel(&self) -> bool {
        self.inner.cancel()
    }

    /// 是否已请求取消。
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }
}

/// 有界 delivery、幂等取消与 durable catch-up session。
pub struct ExecutionSession {
    /// execution ID。
    pub id: ExecutionId,
    /// durable event stream。
    pub events: BoxStream<'static, ExecutionEvent>,
    cancellation: ExecutionCancellation,
    storage: EventProjectionStorage,
}

impl ExecutionSession {
    pub(crate) fn new(
        id: ExecutionId,
        events: BoxStream<'static, ExecutionEvent>,
        cancellation: CancellationHandle,
        storage: EventProjectionStorage,
    ) -> Self {
        Self {
            id,
            events,
            cancellation: ExecutionCancellation::new(cancellation),
            storage,
        }
    }

    /// 请求取消；返回是否首次改变状态。
    #[must_use]
    pub fn cancel(&self) -> bool {
        self.cancellation.cancel()
    }

    /// 是否已请求取消。
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    /// 克隆 opaque cancellation handle。
    #[must_use]
    pub fn cancellation_handle(&self) -> ExecutionCancellation {
        self.cancellation.clone()
    }

    /// 取得 delivery stream。
    #[must_use]
    pub fn into_events(self) -> BoxStream<'static, ExecutionEvent> {
        self.events
    }

    /// 从 durable stream 补读指定 sequence 后事件。
    ///
    /// # Errors
    ///
    /// execution、event payload 或 read lane 失败时返回 `RuleError`。
    pub async fn catch_up(&self, after_sequence: u64) -> Result<Vec<ExecutionEvent>, RuleError> {
        crate::system::catch_up_execution(&self.storage, self.id, after_sequence).await
    }
}

/// 已归档安全 witness。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectWitnessCaptureForTest {
    pub witness_hash: String,
    pub witness: EffectWitnessForTest,
}

/// effect 分类 witness。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EffectWitnessForTest {
    Http(HttpEffectWitnessForTest),
    QuickJs(QuickJsEffectWitnessForTest),
    Extract(ExtractEffectWitnessForTest),
}

/// HTTP witness。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpEffectWitnessForTest {
    pub request: HttpRequestWitnessForTest,
    pub redirects: Vec<HttpRedirectWitnessForTest>,
    pub dns_targets: Vec<HttpDnsTargetWitnessForTest>,
    pub error: Option<HttpEffectErrorKindForTest>,
    pub duration_ms: u64,
}

/// HTTP request witness。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestWitnessForTest {
    pub method: HttpMethodForTest,
    pub safe_url: String,
    pub headers: Vec<HttpRequestHeaderWitnessForTest>,
    pub body: Option<HttpRequestBodyWitnessForTest>,
}

/// 非敏感 request header 摘要。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestHeaderWitnessForTest {
    pub name: String,
    pub value_hash: String,
}

/// request body 摘要。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestBodyWitnessForTest {
    pub hash: String,
    pub byte_len: u64,
}

/// HTTP redirect witness。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRedirectWitnessForTest {
    pub status: u16,
    pub from_url: String,
    pub to_url: String,
}

/// DNS target witness。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpDnsTargetWitnessForTest {
    pub host: String,
    pub addresses: Vec<String>,
    pub kind: HttpDnsTargetKindForTest,
}

/// HTTP method。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpMethodForTest {
    Get,
    Post,
}

/// HTTP effect 失败分类。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpEffectErrorKindForTest {
    TargetValidation,
    Request,
    Redirect,
    ResponseRead,
}

/// DNS target 来源。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpDnsTargetKindForTest {
    PinnedDns,
    IpLiteral,
    DirectHost,
}

/// `QuickJS` witness。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickJsEffectWitnessForTest {
    pub script_hash: String,
    pub input_hash: String,
    pub output_hash: String,
    pub error: Option<QuickJsErrorKindForTest>,
    pub host_calls: Vec<QuickJsHostCallWitnessForTest>,
    pub duration_ms: u64,
}

/// `QuickJS` host-call witness。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickJsHostCallWitnessForTest {
    pub sequence: u32,
    pub call: QuickJsHostCallForTest,
}

/// `QuickJS` host-call result。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuickJsHostCallForTest {
    Time { epoch_millis: i64 },
    Random { value_bits: u64 },
}

/// `QuickJS` 失败分类。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuickJsErrorKindForTest {
    Evaluation,
    RuntimeInitialization,
    ContextInitialization,
    Timeout,
    Watchdog,
    WorkerFailure,
}

/// Extract witness。
#[cfg(feature = "test-support")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractEffectWitnessForTest {
    pub input_hash: String,
    pub duration_ms: u64,
}
