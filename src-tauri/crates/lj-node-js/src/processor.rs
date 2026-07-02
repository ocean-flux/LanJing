//! JS 节点处理器 — rquickjs 执行 @js: 块，产 `NodeData` stream。
//!
//! Runtime 非 Send（含 Rc），用 `spawn_blocking` + channel 模式：
//! JS 代码在 blocking 线程执行，结果通过 channel 传回 async stream。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_stream::stream;
use futures::stream::BoxStream;
use tokio::sync::mpsc;

use rquickjs::{Context, Runtime as JsRuntime};

use lj_core::node::{NodeKind, NodeSpec};
use lj_core::node_data::{NodeData, NodeDataVariant};
use lj_core::sandbox::Capability;
use lj_core::traits::{ExecutionContext, NodeProcessor};
use lj_sandbox::capabilities::check_capability;

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

/// JS 节点处理器。
pub struct JsNodeProcessor;

impl NodeProcessor for JsNodeProcessor {
    fn kind(&self) -> NodeKind {
        NodeKind::Js
    }

    fn input_type(&self) -> Option<NodeDataVariant> {
        None // Js 可以是源头（exploreUrl）
    }

    fn output_type(&self) -> NodeDataVariant {
        NodeDataVariant::Raw // 产 Raw(String)
    }

    fn process<'a>(
        &'a self,
        ctx: &'a ExecutionContext,
        spec: &'a NodeSpec,
        _input: BoxStream<'a, NodeData>,
    ) -> BoxStream<'a, NodeData> {
        let caps = ctx.caps.clone();
        let code = match &spec.js {
            Some(js) => js.code.clone(),
            None => {
                return Box::pin(futures::stream::once(async move {
                    NodeData::Error("JS 节点缺少代码".to_string())
                }));
            }
        };

        // 沙箱门控：首刀检查 network 能力
        if let Err(e) = check_capability(&caps, Capability::Network) {
            return Box::pin(futures::stream::once(async move {
                NodeData::Error(format!("JS 执行被沙箱阻止: {e}"))
            }));
        }

        let (tx, mut rx) = mpsc::channel::<Result<String, JsError>>(1);

        // page 首刀固定为 1，分页支持需后续设计
        let page = Some(1u32);
        let key: Option<String> = None;
        let timeout = JS_TIMEOUT_MS;

        tokio::task::spawn_blocking(move || {
            let result = execute_js_blocking(&code, page, key.as_deref(), timeout);
            let _ = tx.blocking_send(result);
        });

        Box::pin(stream! {
            match rx.recv().await {
                Some(Ok(json_str)) => yield NodeData::Raw(json_str),
                Some(Err(e)) => yield NodeData::Raw(format!("JS 错误: {e}")),
                None => yield NodeData::Error("JS 执行通道意外关闭".to_string()),
            }
        })
    }
}

/// 在 blocking 线程执行 JS 代码。
///
/// 创建独立的 rquickjs Runtime + Context，设置资源上限和 watchdog 超时。
/// 模板变量 `{{page}}` 和 `{{key}}` 在执行前替换。
///
/// # Errors
///
/// 返回 [`JsError::RuntimeCreate`] 当 rquickjs Runtime 创建失败。
/// 返回 [`JsError::ContextCreate`] 当 rquickjs Context 创建失败。
/// 返回 [`JsError::EvalError`] 当 JS 执行抛出异常。
/// 返回 [`JsError::Timeout`] 当执行超过 `timeout_ms`。
///
/// # Panics
///
/// 不会 panic，内部错误通过 `JsError` 返回。
pub fn execute_js_blocking(
    code: &str,
    page: Option<u32>,
    key: Option<&str>,
    timeout_ms: u64,
) -> Result<String, JsError> {
    // 1. 创建 Runtime
    let rt = JsRuntime::new().map_err(|e| JsError::RuntimeCreate(e.to_string()))?;

    // 2. 资源上限
    rt.set_memory_limit(JS_MEMORY_LIMIT);
    rt.set_max_stack_size(256 * 1024);

    // 3. watchdog 超时（用 interrupt handler + 定时器）
    let interrupt_flag = Arc::new(AtomicBool::new(false));
    let flag_for_handler = interrupt_flag.clone();
    let flag_for_watchdog = interrupt_flag.clone();

    rt.set_interrupt_handler(Some(Box::new(move || {
        flag_for_handler.load(Ordering::Relaxed)
    })));

    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(timeout_ms));
        flag_for_watchdog.store(true, Ordering::Relaxed);
    });

    // 4. 创建 Context
    let ctx = Context::full(&rt).map_err(|e| JsError::ContextCreate(e.to_string()))?;

    // 5. 模板变量替换（在进入上下文前完成，避免闭包生命周期问题）
    let mut replaced_code = code.to_string();
    if let Some(p) = page {
        replaced_code = replaced_code.replace("{{page}}", &p.to_string());
    }
    if let Some(k) = key {
        replaced_code = replaced_code.replace("{{key}}", &escape_js_string(k));
    }

    // 6. 在上下文中执行 JS，并将结果转为 String
    // ctx.eval::<String> 对 int/undefined 返回值会失败，
    // 因此先 eval 为 `Value` 再按需转换：
    // - 非 undefined 且是字符串 → 直接取出
    // - 非 undefined 且非字符串 → rquickjs JSON.stringify 序列化
    // - undefined → 尝试取全局 `result` 变量( Legado @js: 约定)
    let eval_result: Result<String, JsError> = ctx
        .with(|ctx| -> Result<String, rquickjs::Error> {
            let val: rquickjs::Value = ctx.eval(replaced_code.as_str())?;

            if val.is_undefined() {
                // Legado @js: 块约定:最后表达式可能因分号返回 undefined,
                // 回退到全局 `result` 变量
                return try_extract_global_result(&ctx);
            }

            if val.is_string()
                && let Ok(s) = val.get::<String>()
            {
                return Ok(s);
            }

            // 非字符串值（number/array/object）用 rquickjs JSON.stringify 序列化
            let json_obj: rquickjs::Object = ctx.globals().get("JSON")?;
            let stringify_fn: rquickjs::Function = json_obj.get("stringify")?;
            stringify_fn.call::<_, String>((val,))
        })
        .map_err(|e| JsError::EvalError(e.to_string()));

    // 7. 检查是否超时
    let timed_out = interrupt_flag.load(Ordering::Relaxed);

    if timed_out {
        return Err(JsError::Timeout(timeout_ms));
    }

    eval_result
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
