//! `QuickJS` Plan effect adapter。
//!
//! `rquickjs` Runtime 非 `Send`，所有对象都留在 `spawn_blocking` 的 blocking lane；
//! runtime 通过 typed `QuickJsEffectHandler` 收集结果与取消状态。

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rquickjs::prelude::Func;
use rquickjs::{Context, Runtime as JsRuntime};

use lj_rule_model::Capability;
use lj_runtime::check_capability;
use lj_runtime::{
    CapturedEffectOutput, EffectCancellation, EffectError, EffectErrorCode, EffectInput,
    EffectOutput, EffectWitness, QuickJsEffectHandler, QuickJsEffectRequest, QuickJsEffectWitness,
    QuickJsErrorKind, QuickJsHostCall, QuickJsHostCallWitness, QuickJsOutput, effect_input_hash,
    effect_output_hash, quickjs_script_hash,
};

use crate::error::JsError;

/// JS 内存上限 16 MB。
const JS_MEMORY_LIMIT: usize = 16 * 1024 * 1024;

/// JS 执行超时时间（毫秒）。
const JS_TIMEOUT_MS: u64 = 5000;

/// 对 JS 字符串字面量中的特殊字符进行转义，防止注入攻击。
///
/// 转义 `\`、`'`、`"`、`/`、换行符、回车符和制表符。
fn escape_js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '"' => out.push_str("\\\""),
            '/' => out.push_str("\\/"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

/// `QuickJS` Plan effect adapter。
///
/// 所有 `rquickjs` 对象都只在 `spawn_blocking` 闭包内创建与销毁。取消 token 会被
/// watchdog 轮询并触发 `QuickJS` interrupt，task 返回前 join watchdog，避免后台线程泄漏。
pub struct QuickJsEffectAdapter;

#[async_trait::async_trait]
impl QuickJsEffectHandler for QuickJsEffectAdapter {
    async fn execute_quickjs(
        &self,
        request: QuickJsEffectRequest,
        cancellation: EffectCancellation,
    ) -> Result<CapturedEffectOutput, EffectError> {
        if cancellation.is_cancelled() {
            return Err(EffectError::new(
                EffectErrorCode::Cancelled,
                "QuickJS effect 已取消",
            ));
        }
        check_capability(&request.capabilities, Capability::Network).map_err(|_| {
            EffectError::new(
                EffectErrorCode::CapabilityDenied,
                "安装 grant 未允许 network capability",
            )
        })?;

        let script_hash = quickjs_script_hash(&request.code);
        let input_hash = effect_input_hash(&request.input).map_err(|_| {
            EffectError::new(EffectErrorCode::Internal, "QuickJS 输入 hash 计算失败")
        })?;
        let (page, key) = quickjs_template_input(&request.input);
        let code = request.code;
        let blocking_cancellation = cancellation.clone();
        let started = Instant::now();
        let execution = match tokio::task::spawn_blocking(move || {
            execute_js_blocking_with_witness(
                &code,
                page,
                key.as_deref(),
                JS_TIMEOUT_MS,
                &blocking_cancellation,
            )
        })
        .await
        {
            Ok(execution) => execution,
            Err(_) => JsExecution::worker_failure(),
        };

        if matches!(execution.result.as_ref(), Err(JsError::Cancelled)) {
            return Err(EffectError::new(
                EffectErrorCode::Cancelled,
                "QuickJS effect 已取消",
            ));
        }
        let JsExecution {
            result,
            host_calls,
            error_kind,
        } = execution;
        let output = match result {
            Ok(output) => match serde_json::from_str::<serde_json::Value>(&output) {
                Ok(value) => QuickJsOutput::Json(value),
                Err(_) => QuickJsOutput::Raw(output),
            },
            Err(error) => {
                QuickJsOutput::Error(error_kind.unwrap_or_else(|| quickjs_error_kind(&error)))
            }
        };
        let effect_output = EffectOutput::QuickJs(output.clone());
        let output_hash = effect_output_hash(&effect_output).map_err(|_| {
            EffectError::new(EffectErrorCode::Internal, "QuickJS 输出 hash 计算失败")
        })?;
        let error = match output {
            QuickJsOutput::Error(error) => Some(error),
            QuickJsOutput::Json(_) | QuickJsOutput::Raw(_) => None,
        };
        let host_calls = host_calls
            .into_iter()
            .enumerate()
            .map(|(index, call)| QuickJsHostCallWitness {
                sequence: u32::try_from(index + 1).unwrap_or(u32::MAX),
                call,
            })
            .collect();
        Ok(CapturedEffectOutput::new(
            effect_output,
            EffectWitness::QuickJs(QuickJsEffectWitness {
                script_hash,
                input_hash,
                output_hash,
                error,
                host_calls,
                duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            }),
        ))
    }
}

fn quickjs_template_input(input: &EffectInput) -> (Option<u32>, Option<String>) {
    match input {
        EffectInput::Intent(lj_capability::IntentInput::Page(value)) => {
            (value.parse::<u32>().ok(), Some(value.clone()))
        }
        EffectInput::Intent(
            lj_capability::IntentInput::Query(value)
            | lj_capability::IntentInput::ItemId(value)
            | lj_capability::IntentInput::UnitId(value)
            | lj_capability::IntentInput::ActionId(value),
        ) => (Some(1), Some(value.clone())),
        EffectInput::Intent(lj_capability::IntentInput::Opaque(value)) => {
            (Some(1), serde_json::to_string(value).ok())
        }
        EffectInput::Intent(lj_capability::IntentInput::None) | EffectInput::Output(_) => {
            (Some(1), None)
        }
    }
}

fn quickjs_error_kind(error: &JsError) -> QuickJsErrorKind {
    match error {
        JsError::RuntimeCreate(_) => QuickJsErrorKind::RuntimeInitialization,
        JsError::ContextCreate(_) => QuickJsErrorKind::ContextInitialization,
        JsError::EvalError(_) => QuickJsErrorKind::Evaluation,
        JsError::Timeout(_) => QuickJsErrorKind::Timeout,
        JsError::Watchdog => QuickJsErrorKind::Watchdog,
        JsError::Cancelled | JsError::CapabilityBlocked(_) => QuickJsErrorKind::WorkerFailure,
    }
}

struct JsExecution {
    result: Result<String, JsError>,
    host_calls: Vec<QuickJsHostCall>,
    error_kind: Option<QuickJsErrorKind>,
}

impl JsExecution {
    fn worker_failure() -> Self {
        Self {
            result: Err(JsError::EvalError("worker failure".to_string())),
            host_calls: Vec::new(),
            error_kind: Some(QuickJsErrorKind::WorkerFailure),
        }
    }
}

/// 在 blocking lane 执行可取消的 JS 代码。
///
/// watchdog 与 `rquickjs` Runtime 在同一 blocking 线程生命周期内结束；此函数不会把
/// 非 `Send` `QuickJS` 句柄跨线程返回。
///
/// # Errors
///
/// Runtime/Context 创建、脚本求值、watchdog 清理、超时或取消时返回对应 [`JsError`]。
pub fn execute_js_blocking_cancellable(
    code: &str,
    page: Option<u32>,
    key: Option<&str>,
    timeout_ms: u64,
    cancellation: &EffectCancellation,
) -> Result<String, JsError> {
    execute_js_blocking_with_witness(code, page, key, timeout_ms, cancellation).result
}

fn execute_js_blocking_with_witness(
    code: &str,
    page: Option<u32>,
    key: Option<&str>,
    timeout_ms: u64,
    cancellation: &EffectCancellation,
) -> JsExecution {
    let host_calls = Arc::new(Mutex::new(Vec::new()));
    let result = execute_js_inner(
        code,
        page,
        key,
        timeout_ms,
        cancellation,
        host_calls.clone(),
    );
    let host_calls = match host_calls.lock() {
        Ok(mut calls) => std::mem::take(&mut *calls),
        Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
    };
    JsExecution {
        result,
        host_calls,
        error_kind: None,
    }
}

fn execute_js_inner(
    code: &str,
    page: Option<u32>,
    key: Option<&str>,
    timeout_ms: u64,
    cancellation: &EffectCancellation,
    host_calls: Arc<Mutex<Vec<QuickJsHostCall>>>,
) -> Result<String, JsError> {
    let runtime = JsRuntime::new().map_err(|error| JsError::RuntimeCreate(error.to_string()))?;
    runtime.set_memory_limit(JS_MEMORY_LIMIT);
    runtime.set_max_stack_size(256 * 1024);
    let context =
        Context::full(&runtime).map_err(|error| JsError::ContextCreate(error.to_string()))?;

    let mut replaced_code = code.to_string();
    if let Some(page) = page {
        replaced_code = replaced_code.replace("{{page}}", &page.to_string());
    }
    if let Some(key) = key {
        replaced_code = replaced_code.replace("{{key}}", &escape_js_string(key));
    }

    let interrupted = Arc::new(AtomicBool::new(false));
    let timed_out = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));
    let stop_watchdog = Arc::new(AtomicBool::new(false));
    let interrupt_for_handler = interrupted.clone();
    runtime.set_interrupt_handler(Some(Box::new(move || {
        interrupt_for_handler.load(Ordering::Acquire)
    })));

    let watchdog_interrupt = interrupted.clone();
    let watchdog_timed_out = timed_out.clone();
    let watchdog_cancelled = cancelled.clone();
    let watchdog_stop = stop_watchdog.clone();
    let watchdog_cancellation = cancellation.clone();
    let watchdog = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_millis(timeout_ms);
        while !watchdog_stop.load(Ordering::Acquire) {
            if watchdog_cancellation.is_cancelled() {
                watchdog_cancelled.store(true, Ordering::Release);
                watchdog_interrupt.store(true, Ordering::Release);
                break;
            }
            if Instant::now() >= deadline {
                watchdog_timed_out.store(true, Ordering::Release);
                watchdog_interrupt.store(true, Ordering::Release);
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });

    let evaluation = context
        .with(|context| -> Result<String, rquickjs::Error> {
            install_host_functions(&context, host_calls)?;
            let value: rquickjs::Value = context.eval(replaced_code.as_str())?;
            if value.is_undefined() {
                return try_extract_global_result(&context);
            }
            if value.is_string()
                && let Ok(value) = value.get::<String>()
            {
                return Ok(value);
            }
            let json: rquickjs::Object = context.globals().get("JSON")?;
            let stringify: rquickjs::Function = json.get("stringify")?;
            stringify.call::<_, String>((value,))
        })
        .map_err(|error| JsError::EvalError(error.to_string()));

    stop_watchdog.store(true, Ordering::Release);
    if watchdog.join().is_err() {
        return Err(JsError::Watchdog);
    }
    if cancelled.load(Ordering::Acquire) || cancellation.is_cancelled() {
        return Err(JsError::Cancelled);
    }
    if timed_out.load(Ordering::Acquire) {
        return Err(JsError::Timeout(timeout_ms));
    }
    evaluation
}

fn install_host_functions(
    context: &rquickjs::Ctx<'_>,
    host_calls: Arc<Mutex<Vec<QuickJsHostCall>>>,
) -> Result<(), rquickjs::Error> {
    let date_calls = host_calls.clone();
    let date: rquickjs::Object = context.globals().get("Date")?;
    date.set(
        "now",
        Func::new(move || -> i64 {
            let epoch_millis = current_epoch_millis();
            push_host_call(&date_calls, QuickJsHostCall::Time { epoch_millis });
            epoch_millis
        }),
    )?;
    let random_calls = host_calls;
    let math: rquickjs::Object = context.globals().get("Math")?;
    math.set(
        "random",
        Func::new(move || -> f64 {
            let value = next_random();
            push_host_call(
                &random_calls,
                QuickJsHostCall::Random {
                    value_bits: value.to_bits(),
                },
            );
            value
        }),
    )?;
    Ok(())
}

fn push_host_call(host_calls: &Arc<Mutex<Vec<QuickJsHostCall>>>, call: QuickJsHostCall) {
    match host_calls.lock() {
        Ok(mut calls) => calls.push(call),
        Err(poisoned) => poisoned.into_inner().push(call),
    }
}

fn current_epoch_millis() -> i64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    i64::try_from(millis).unwrap_or(i64::MAX)
}

fn next_random() -> f64 {
    static RANDOM_SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = RANDOM_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let time = u64::try_from(current_epoch_millis()).unwrap_or_default();
    let mixed = splitmix64(time ^ sequence.rotate_left(17));
    let numerator = mixed >> 11;
    let high = u32::try_from(numerator >> 32).unwrap_or_default();
    let low = u32::try_from(numerator & u64::from(u32::MAX)).unwrap_or_default();
    (f64::from(high) * 4_294_967_296.0 + f64::from(low)) / 9_007_199_254_740_992.0
}

const fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

/// 从全局 `result` 变量取值( Legado @js: 块约定回退)。
///
/// 当 `ctx.eval(code)` 返回 `undefined` 时(如最后语句以分号结尾),
/// 尝试取全局 `result` 变量并用 `JSON.stringify` 序列化。
///
/// # Panics
///
/// 不会 panic，若 rquickjs 内部状态损坏可能 panic。
fn try_extract_global_result(ctx: &rquickjs::Ctx<'_>) -> Result<String, rquickjs::Error> {
    let result_val: rquickjs::Value = ctx.globals().get("result")?;
    if result_val.is_undefined() {
        return Ok(String::new());
    }
    let json_obj: rquickjs::Object = ctx.globals().get("JSON")?;
    let stringify_fn: rquickjs::Function = json_obj.get("stringify")?;
    stringify_fn.call::<_, String>((result_val,))
}
