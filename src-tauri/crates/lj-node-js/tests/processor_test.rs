//! `QuickJsEffectAdapter` 集成测试。
//!
//! 测试可取消 blocking lane 的简单 JS 表达式与超时中断。

use std::sync::Arc;
use std::time::Duration;

use lj_capability::IntentInput;
use lj_node_js::execute_js_blocking_cancellable;
use lj_node_js::processor::QuickJsEffectAdapter;
use lj_rule_model::PolicyCapabilities;
use lj_runtime::{
    CancellationHandle, EffectErrorCode, EffectInput, EffectOutput, EffectWitness,
    QuickJsEffectHandler, QuickJsEffectRequest, QuickJsHostCall, QuickJsOutput,
};
use uuid::Uuid;
fn run_js(
    code: &str,
    page: Option<u32>,
    key: Option<&str>,
    timeout_ms: u64,
) -> Result<String, lj_node_js::error::JsError> {
    let cancellation = CancellationHandle::new().token();
    execute_js_blocking_cancellable(code, page, key, timeout_ms, &cancellation)
}

/// 简单 JS 表达式求值。
#[test]
fn test_simple_expression() {
    let result = run_js("1 + 1", None, None, 5000).unwrap();
    assert_eq!(result, "2");
}

/// JS 字符串拼接。
#[test]
fn test_string_concat() {
    let result = run_js(r#""hello" + " " + "world""#, None, None, 5000).unwrap();
    assert_eq!(result, "hello world");
}

#[test]
fn test_legado_explore_url() {
    let code = r"
var result=[];
result.push({title:'测试',url:'/test?page={{page}}'});
JSON.stringify(result);
";
    let result = run_js(code, Some(1), None, 5000).unwrap();
    assert!(result.contains("测试"), "应包含标题: {result}");
    assert!(
        result.contains("/test?page=1"),
        "{{page}} 应被替换为 1: {result}"
    );
}

#[test]
fn test_legado_multi_categories() {
    let code = r"
var result=[];
var push=function(t,u,s){result.push({title:t,url:u})};
push('分类浏览',null);
var cats=[['豪门','/haomenzongcai?page={{page}}'],['悬疑','/xuanyilingyi?page={{page}}']];
cats.forEach(function(c){push(c[0],c[1])});
JSON.stringify(result);
";
    let result = run_js(code, Some(2), None, 5000).unwrap();
    assert!(result.contains("分类浏览"), "应包含分类浏览: {result}");
    assert!(result.contains("豪门"), "应包含豪门: {result}");
    assert!(result.contains("悬疑"), "应包含悬疑: {result}");
    assert!(
        result.contains("/haomenzongcai?page=2"),
        "{{page}} 应被替换为 2: {result}"
    );
    assert!(
        result.contains("/xuanyilingyi?page=2"),
        "{{page}} 应被替换为 2: {result}"
    );
}

/// 超时中断 —— while(true) 无限循环应被 watchdog 中断。
#[test]
fn test_timeout_interrupt() {
    let result = run_js("while(true){}", None, None, 500);
    assert!(result.is_err(), "无限循环应超时中断: {result:?}");
    let err = result.unwrap_err();
    match err {
        lj_node_js::error::JsError::Timeout(ms) => {
            assert_eq!(ms, 500, "超时时间应匹配: {ms}");
        }
        other => panic!("应返回 Timeout 错误, 实际: {other:?}"),
    }
}

/// 空代码 —— 空字符串应返回空结果。
#[test]
fn test_empty_code() {
    let result = run_js("", None, None, 5000).unwrap();
    assert_eq!(result, "");
}

/// JSON 数组结果 —— 确认 JSON.stringify 产出可解析的 JSON。
#[test]
fn test_json_array_result() {
    let code = r"
var result=[];
result.push({a:1,b:'x'});
result.push({a:2,b:'y'});
JSON.stringify(result);
";
    let result = run_js(code, None, None, 5000).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).expect("结果应为合法 JSON");
    assert!(parsed.is_array(), "应为 JSON 数组");
    assert_eq!(parsed.as_array().unwrap().len(), 2);
}

/// 内存上限 —— 分配超大数组应触发 `JsError`。
#[test]
fn test_memory_limit() {
    let result = run_js("new Array(100000000)", None, None, 5000);
    assert!(result.is_err(), "超大内存分配应返回错误: {result:?}");
}

#[tokio::test]
async fn plan_quickjs_effect_returns_typed_json() {
    let processor = QuickJsEffectAdapter;
    let output = processor
        .execute_quickjs(
            quickjs_effect_request("JSON.stringify({answer: 42})"),
            CancellationHandle::new().token(),
        )
        .await
        .expect("QuickJS effect 应返回类型化输出");

    assert_eq!(
        output.output,
        EffectOutput::QuickJs(QuickJsOutput::Json(serde_json::json!({"answer": 42})))
    );
    output
        .validate()
        .expect("QuickJS capture witness 必须绑定类型化输出");
}

#[tokio::test]
async fn plan_quickjs_effect_records_time_and_random_host_calls() {
    let output = QuickJsEffectAdapter
        .execute_quickjs(
            quickjs_effect_request("JSON.stringify([Date.now(), Math.random()])"),
            CancellationHandle::new().token(),
        )
        .await
        .expect("QuickJS effect 应产生 capture");
    let EffectWitness::QuickJs(witness) = &output.witness else {
        panic!("QuickJS effect 必须返回 QuickJS witness");
    };
    assert!(matches!(
        witness.host_calls.as_slice(),
        [
            lj_runtime::QuickJsHostCallWitness {
                sequence: 1,
                call: QuickJsHostCall::Time { .. },
            },
            lj_runtime::QuickJsHostCallWitness {
                sequence: 2,
                call: QuickJsHostCall::Random { .. },
            }
        ]
    ));
    output
        .validate()
        .expect("host calls 必须进入连续的可 replay witness");
}

#[tokio::test]
async fn plan_quickjs_effect_archives_typed_evaluation_failure() {
    let output = QuickJsEffectAdapter
        .execute_quickjs(
            quickjs_effect_request("throw new Error('private script failure')"),
            CancellationHandle::new().token(),
        )
        .await
        .expect("执行过的 QuickJS 失败必须返回可 archive 输出");
    assert_eq!(
        output.output,
        EffectOutput::QuickJs(QuickJsOutput::Error(
            lj_runtime::QuickJsErrorKind::Evaluation
        ))
    );
    output
        .validate()
        .expect("失败 output 必须与安全 witness 绑定");
}

#[tokio::test]
async fn plan_quickjs_effect_cancellation_interrupts_watchdog() {
    let cancellation = CancellationHandle::new();
    let processor = Arc::new(QuickJsEffectAdapter);
    let task_processor = processor.clone();
    let task_cancellation = cancellation.token();
    let task = tokio::spawn(async move {
        task_processor
            .execute_quickjs(quickjs_effect_request("while(true) {}"), task_cancellation)
            .await
    });

    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(cancellation.cancel());
    let result = tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .expect("取消必须 interrupt QuickJS watchdog")
        .expect("QuickJS task 不应 panic");
    let Err(error) = result else {
        panic!("应被取消");
    };
    assert_eq!(error.code, EffectErrorCode::Cancelled);
}

fn quickjs_effect_request(code: &str) -> QuickJsEffectRequest {
    QuickJsEffectRequest {
        execution_id: Uuid::new_v4(),
        source_id: "quickjs-effect-test".to_string(),
        node_id: Uuid::new_v4(),
        effect_id: Uuid::new_v4(),
        trace_id: "quickjs-effect-trace".to_string(),
        code: code.to_string(),
        input: EffectInput::Intent(IntentInput::Query("typed".to_string())),
        capabilities: PolicyCapabilities {
            network: true,
            ..PolicyCapabilities::default()
        },
    }
}
