//! `JsNodeProcessor` 集成测试。
//!
//! 测试 `execute_js_blocking` 的三种场景：
//! - 简单 JS 表达式
//! - 超时中断

use lj_node_js::execute_js_blocking;

/// 简单 JS 表达式求值。
#[test]
fn test_simple_expression() {
    let result = execute_js_blocking("1 + 1", None, None, 5000).unwrap();
    assert_eq!(result, "2");
}

/// JS 字符串拼接。
#[test]
fn test_string_concat() {
    let result = execute_js_blocking(r#""hello" + " " + "world""#, None, None, 5000).unwrap();
    assert_eq!(result, "hello world");
}

#[test]
fn test_legado_explore_url() {
    let code = r"
var result=[];
result.push({title:'测试',url:'/test?page={{page}}'});
JSON.stringify(result);
";
    let result = execute_js_blocking(code, Some(1), None, 5000).unwrap();
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
    let result = execute_js_blocking(code, Some(2), None, 5000).unwrap();
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
    let result = execute_js_blocking("while(true){}", None, None, 500);
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
    let result = execute_js_blocking("", None, None, 5000).unwrap();
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
    let result = execute_js_blocking(code, None, None, 5000).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).expect("结果应为合法 JSON");
    assert!(parsed.is_array(), "应为 JSON 数组");
    assert_eq!(parsed.as_array().unwrap().len(), 2);
}

/// 内存上限 —— 分配超大数组应触发 `JsError`。
#[test]
fn test_memory_limit() {
    let result = execute_js_blocking("new Array(100000000)", None, None, 5000);
    assert!(result.is_err(), "超大内存分配应返回错误: {result:?}");
}
