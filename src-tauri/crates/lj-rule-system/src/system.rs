//! concrete `RuleSystem` 的私有组合根。
//!
//! `RuleSystem` 是规则生命周期唯一 concrete façade：调用方只能准备 candidate、安装来源、
//! 启动已安装来源的 session，或通过独立 query adapter 读取安全投影。Definition、immutable
//! Plan、node effect adapter、C2 storage transaction 与 execution registry 始终保持私有。
//!
//! 具体职责拆分为：`lifecycle` 处理 candidate/install/execute，`session_delivery` 保证持久化
//! 事件顺序和取消语义，`query_adapter` 映射安全查询，`capture` 提供 test-only witness seam，
//! `error_mapping` 收敛脱敏错误。

mod capture;
mod error_mapping;
mod lifecycle;
mod query_adapter;
mod session_delivery;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use lj_compiler::Compiler;
use lj_node_extract::processor::ExtractEffectAdapter;
use lj_node_http::processor::HttpEffectAdapter;
use lj_node_js::processor::QuickJsEffectAdapter;
#[cfg(feature = "test-support")]
use lj_runtime::EffectReplayLookup;
use lj_runtime::{CancellationHandle, EffectHandlers, PlanRuntime, PlanRuntimeConfig};
use lj_storage::{CandidateSummary, EventProjectionStorage, StorageConfig};
use uuid::Uuid;

use self::error_mapping::{runtime_error, storage_error};
use self::lifecycle::PlanCache;
use crate::{ExecutionEvent, ExecutionId, RuleError, RuleErrorStage, RuleSystemConfig};

/// 规则生命周期唯一 concrete façade。
///
/// 其内部组合 importer、compiler、Plan runtime、真实 node effect adapter 和 C2 Event Store；
/// 外部不能注入 Graph、Plan JSON、processor registry、SQLite connection 或 storage handle。
#[derive(Clone)]
pub struct RuleSystem {
    state: Arc<RuleSystemState>,
}

/// 所有跨命令共享的私有基础设施。
///
/// `executions` 只保存取消句柄；session runner 在唯一 terminal 后必定移除相应条目。candidate
/// preview 和 Plan LRU 都是缓存，C2 durable record 才是安装与 replay 的唯一真相。
struct RuleSystemState {
    storage: EventProjectionStorage,
    compiler: Compiler,
    runtime: PlanRuntime,
    handlers: EffectHandlers,
    candidate_ttl_ms: i64,
    session_event_capacity: usize,
    candidates: Mutex<HashMap<Uuid, CandidateSummary>>,
    plans: Mutex<PlanCache>,
    executions: Mutex<HashMap<Uuid, CancellationHandle>>,
    #[cfg(feature = "test-support")]
    effect_capture_lookups: Mutex<HashMap<(Uuid, Uuid), EffectReplayLookup>>,
}

impl RuleSystem {
    /// 打开真实 C2 Event Store，并私有装配 compiler、`PlanRuntime` 与 Plan effect adapter。
    ///
    /// 配置中的缓存、delivery channel 与 runtime 并发上限必须为正值，以保证 LRU 和 stream
    /// 背压有界。`local_fixture` 仅在 test-support 配置中关闭 loopback SSRF 拒绝，仍使用真实
    /// SQLite、artifact archive 和 adapter。
    ///
    /// # Errors
    ///
    /// 配置容量无效、SQLite/artifact/keyring 初始化失败，或 `PlanRuntime` 无法接受固定版本与
    /// 并发配置时返回 [`RuleError`]。
    pub async fn open(config: RuleSystemConfig) -> Result<Self, RuleError> {
        let trace_id = trace_id();
        let candidate_ttl_ms = duration_millis(config.candidate_ttl, &trace_id)?;
        if candidate_ttl_ms == 0 {
            return Err(RuleError::new(
                RuleErrorStage::Candidate,
                "candidate_ttl_invalid",
                "candidate 到期时长必须大于零",
                trace_id,
                false,
                Vec::new(),
            ));
        }
        if config.plan_cache_capacity == 0 || config.session_event_capacity == 0 {
            return Err(RuleError::new(
                RuleErrorStage::Internal,
                "bounded_capacity_invalid",
                "RuleSystem 的缓存与 session 容量必须大于零",
                trace_id,
                false,
                Vec::new(),
            ));
        }

        let mut storage_config = StorageConfig::desktop(config.database_path, config.artifact_root);
        storage_config.keyring_service = config.keyring_service;
        let storage = EventProjectionStorage::open(storage_config)
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))?;
        let compiler = Compiler::default();
        let runtime = PlanRuntime::new(PlanRuntimeConfig {
            compiler_version: compiler.version().to_string(),
            plan_schema_version: lj_runtime::SUPPORTED_PLAN_SCHEMA_VERSION,
            event_channel_capacity: config.session_event_capacity,
            max_concurrent_executions: config.max_concurrent_executions,
            max_concurrent_effects: config.max_concurrent_effects,
            max_concurrent_effects_per_source: config.max_concurrent_effects_per_source,
        })
        .map_err(|error| runtime_error(&error, &trace_id))?;
        let http = if config.local_fixture_http {
            Arc::new(HttpEffectAdapter::new_test())
        } else {
            Arc::new(HttpEffectAdapter::new())
        };
        let handlers = EffectHandlers::new(
            http,
            Arc::new(QuickJsEffectAdapter),
            Arc::new(ExtractEffectAdapter),
        );

        Ok(Self {
            state: Arc::new(RuleSystemState {
                storage,
                compiler,
                runtime,
                handlers,
                candidate_ttl_ms,
                session_event_capacity: config.session_event_capacity,
                candidates: Mutex::new(HashMap::new()),
                plans: Mutex::new(PlanCache::new(config.plan_cache_capacity)),
                executions: Mutex::new(HashMap::new()),
                #[cfg(feature = "test-support")]
                effect_capture_lookups: Mutex::new(HashMap::new()),
            }),
        })
    }

    /// 仅供 integration test 关闭私有 C2 writer，确保重开同一 durable archive 时没有活跃锁。
    ///
    /// # Errors
    ///
    /// writer 已停止或关闭失败时返回 [`RuleError`]。
    #[cfg(feature = "test-support")]
    pub async fn shutdown_for_test(&self) -> Result<(), RuleError> {
        let trace_id = trace_id();
        self.state
            .storage
            .shutdown()
            .await
            .map_err(|error| storage_error(&error, RuleErrorStage::Persistence, &trace_id))
    }
}

/// 供 [`crate::ExecutionSession`] 委托的私有 catch-up bridge。
///
/// sequence 连续性验证位于 `session_delivery`，避免 session 持有或暴露 C2 storage handle。
pub(crate) async fn catch_up_execution(
    storage: &EventProjectionStorage,
    execution_id: ExecutionId,
    after_sequence: u64,
) -> Result<Vec<ExecutionEvent>, RuleError> {
    session_delivery::catch_up_execution(storage, execution_id, after_sequence).await
}

/// 将配置时长安全转换为 C2 使用的毫秒值。
pub(super) fn duration_millis(
    duration: std::time::Duration,
    trace_id: &str,
) -> Result<i64, RuleError> {
    i64::try_from(duration.as_millis()).map_err(|_| {
        RuleError::new(
            RuleErrorStage::Candidate,
            "candidate_ttl_overflow",
            "candidate 到期时长超出支持范围",
            trace_id.to_string(),
            false,
            Vec::new(),
        )
    })
}

/// 返回当前 UTC epoch milliseconds，拒绝不合法系统时钟而不是生成伪造时间。
pub(super) fn now_millis(trace_id: &str) -> Result<i64, RuleError> {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| {
        RuleError::new(
            RuleErrorStage::Internal,
            "system_clock_invalid",
            "系统时钟早于 Unix epoch",
            trace_id.to_string(),
            false,
            Vec::new(),
        )
    })?;
    i64::try_from(duration.as_millis()).map_err(|_| {
        RuleError::new(
            RuleErrorStage::Internal,
            "system_clock_overflow",
            "系统时钟超出支持范围",
            trace_id.to_string(),
            false,
            Vec::new(),
        )
    })
}

/// 生成不含输入/凭证内容的 execution trace 标识。
pub(super) fn trace_id() -> String {
    format!("rule-system-{}", Uuid::new_v4())
}

/// 恢复 poison 后的私有缓存锁；绝不跨 `.await` 持有该 guard。
pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
