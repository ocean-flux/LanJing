//! `QuickJS` 沙箱能力边界的证据测试。
//!
//! ticket #63 要求证明 JS 变换节点够不到 SQLite、任意文件系统、环境变量、进程、event
//! sequence、revision、archive 与明文 credential。这里逐项取证：全局表面是穷举白名单快照
//! （任何新增宿主全局都会让测试失败），危险 API 逐个确认不可达，宿主注入的唯一值就是声明的
//! 输入。

use lj_capability::IntentInput;
use lj_node_js::processor::QuickJsEffectAdapter;
use lj_rule_model::{JsBudget, PolicyCapabilities};
use lj_runtime::{
    CancellationHandle, EffectInput, EffectOutput, QuickJsEffectHandler, QuickJsEffectRequest,
    QuickJsOutput,
};
use serde_json::{Value, json};
use uuid::Uuid;

/// 沙箱可达的全局名字快照：全部是 ECMAScript / `QuickJS` 内置，没有任何宿主桥。
///
/// 这份名单只允许**缩小或经评审后扩充**：多出任何名字都说明出现了新的宿主能力通道。
const QUICKJS_BUILTIN_GLOBALS: &[&str] = &[
    "AggregateError",
    "Array",
    "ArrayBuffer",
    "AsyncDisposableStack",
    "Atomics",
    "BigInt",
    "BigInt64Array",
    "BigUint64Array",
    "Boolean",
    "DOMException",
    "DataView",
    "Date",
    "DisposableStack",
    "Error",
    "EvalError",
    "FinalizationRegistry",
    "Float16Array",
    "Float32Array",
    "Float64Array",
    "Function",
    "Infinity",
    "Int16Array",
    "Int32Array",
    "Int8Array",
    "InternalError",
    "Iterator",
    "JSON",
    "Map",
    "Math",
    "NaN",
    "Number",
    "Object",
    "Promise",
    "Proxy",
    "RangeError",
    "ReferenceError",
    "Reflect",
    "RegExp",
    "Set",
    "SharedArrayBuffer",
    "String",
    "SuppressedError",
    "Symbol",
    "SyntaxError",
    "TypeError",
    "URIError",
    "Uint16Array",
    "Uint32Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "WeakMap",
    "WeakRef",
    "WeakSet",
    "atob",
    "btoa",
    "decodeURI",
    "decodeURIComponent",
    "encodeURI",
    "encodeURIComponent",
    "escape",
    "eval",
    "globalThis",
    "isFinite",
    "isNaN",
    "parseFloat",
    "parseInt",
    "performance",
    "queueMicrotask",
    "undefined",
    "unescape",
];

/// 逐个点名 ticket 要求证明不可达的能力；这些名字在任何情况下都不得成为全局。
const FORBIDDEN_GLOBALS: &[&str] = &[
    // SQLite / 数据库
    "SQLite",
    "Database",
    "db",
    "sqlite",
    "sqlite3",
    // 文件系统 / 模块加载
    "require",
    "module",
    "exports",
    "__dirname",
    "__filename",
    "fs",
    "os",
    "path",
    "Deno",
    "global",
    "Buffer",
    "open",
    // 环境变量 / 进程
    "process",
    "env",
    "child_process",
    "Worker",
    // 网络 / 宿主 IPC
    "fetch",
    "XMLHttpRequest",
    "WebSocket",
    "invoke",
    "__TAURI__",
    "__tauri__",
    "__lanjing_input_json",
    // execution identity / revision / archive
    "execution_id",
    "executionId",
    "trace_id",
    "traceId",
    "node_id",
    "nodeId",
    "effect_id",
    "effectId",
    "sequence",
    "revision",
    "source_revision",
    "plan",
    "witness",
    "capture",
    "archive",
    "lookup",
    // 明文 credential
    "credential",
    "credentials",
    "secret",
    "token",
    "cookie",
    "headers",
    "authorization",
];

async fn run_quickjs(code: &str, input: EffectInput) -> QuickJsOutput {
    let request = QuickJsEffectRequest {
        execution_id: Uuid::new_v4(),
        source_id: "sandbox-test".to_string(),
        node_id: Uuid::new_v4(),
        effect_id: Uuid::new_v4(),
        trace_id: "sandbox-test".to_string(),
        code: code.to_string(),
        budgets: JsBudget::default(),
        input,
        capabilities: PolicyCapabilities {
            network: true,
            ..PolicyCapabilities::default()
        },
    };
    let captured = QuickJsEffectAdapter
        .execute_quickjs(request, CancellationHandle::new().token())
        .await
        .expect("沙箱执行必须返回 capture");
    let EffectOutput::QuickJs(output) = captured.output else {
        panic!("QuickJS adapter 必须返回 QuickJS 输出");
    };
    output
}

/// 执行脚本并取出它返回的 JSON，作为沙箱内部可观测状态。
async fn eval_json(code: &str) -> Value {
    match run_quickjs(
        code,
        EffectInput::Intent(IntentInput::Query("sandbox".into())),
    )
    .await
    {
        QuickJsOutput::Json(value) => value,
        other => panic!("探测脚本必须返回 JSON: {other:?}"),
    }
}

/// 全局表面是穷举过的 `QuickJS` 内置集合；多出任何名字都视为新增宿主桥。
#[tokio::test]
async fn sandbox_global_surface_is_quickjs_builtins_only() {
    let value = eval_json("JSON.stringify(Object.getOwnPropertyNames(globalThis).sort())").await;
    let names: Vec<String> = serde_json::from_value(value).expect("全局名单必须可解析");
    let unexpected = names
        .iter()
        .filter(|name| !QUICKJS_BUILTIN_GLOBALS.contains(&name.as_str()))
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "沙箱出现了未声明的宿主全局, 必须评审其能力: {unexpected:?}"
    );
}

/// ticket 点名要证明不可达的每一类能力，其全局名都必须不存在。
#[tokio::test]
async fn sandbox_exposes_no_forbidden_global() {
    let probes = FORBIDDEN_GLOBALS
        .iter()
        .map(|name| format!("{name}: typeof {name}"))
        .collect::<Vec<_>>()
        .join(",");
    let value = eval_json(&format!("JSON.stringify({{{probes}}})")).await;
    let probed_globals = value.as_object().expect("探测结果必须是对象");
    let reachable = FORBIDDEN_GLOBALS
        .iter()
        .filter(|name| {
            probed_globals
                .get(**name)
                .is_none_or(|kind| kind != "undefined")
        })
        .collect::<Vec<_>>();
    assert!(reachable.is_empty(), "沙箱暴露了禁止的全局: {reachable:?}");
}

/// 主动越狱尝试（模块加载、环境变量、`Function` 构造器提权）全部失败。
#[tokio::test]
async fn sandbox_escape_attempts_are_blocked() {
    let value = eval_json(
        r"
const attempt = (fn) => { try { fn(); return 'reachable'; } catch (error) { return error.name; } };
JSON.stringify({
  require_fs: attempt(() => require('fs')),
  require_sqlite: attempt(() => require('node:sqlite')),
  process_env: attempt(() => process.env.SECRET),
  function_constructor: attempt(() => globalThis.constructor.constructor('return process')().env),
  global_key: Object.getOwnPropertyNames(globalThis).includes('require'),
});
",
    )
    .await;
    assert_eq!(value["require_fs"], "ReferenceError", "{value}");
    assert_eq!(value["require_sqlite"], "ReferenceError", "{value}");
    assert_eq!(value["process_env"], "ReferenceError", "{value}");
    assert_eq!(value["function_constructor"], "ReferenceError", "{value}");
    assert_eq!(value["global_key"], false, "{value}");
}

/// 宿主注入的唯一值就是声明的输入，并且临时桥全局在解析后立即删除。
#[tokio::test]
async fn sandbox_injects_only_the_declared_input_and_cleans_up_the_bridge() {
    let declared = json!({ "items": [{ "id": 1 }, { "id": 2 }], "note": "declared" });
    let code = "JSON.stringify({ input: input, bridge: typeof globalThis.__lanjing_input_json, \
                keys: Object.getOwnPropertyNames(input).sort() })";
    let output = run_quickjs(
        code,
        EffectInput::Json(std::sync::Arc::new(declared.clone())),
    )
    .await;
    let QuickJsOutput::Json(value) = output else {
        panic!("探测脚本必须返回 JSON");
    };
    assert_eq!(value["input"], declared);
    assert_eq!(value["bridge"], "undefined", "临时输入桥必须被删除");
    assert_eq!(value["keys"], json!(["items", "note"]), "{value}");
}

/// 没有输入时连 `input` 都不存在：宿主不会为脚本凭空造一个可读通道。
#[tokio::test]
async fn sandbox_without_declared_input_exposes_no_input_global() {
    let value = eval_json(
        "JSON.stringify({ input: typeof input, bridge: typeof globalThis.__lanjing_input_json })",
    )
    .await;
    assert_eq!(value["input"], "undefined", "{value}");
    assert_eq!(value["bridge"], "undefined", "{value}");
}
