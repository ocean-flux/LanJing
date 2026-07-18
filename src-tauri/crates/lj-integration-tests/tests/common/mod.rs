//! 集成测试共享辅助函数。
//!
//! 各 test target 通过 `mod common;` 引入，避免 helper 重复。

use std::collections::HashMap;

use futures::StreamExt;
use lj_media::MediaGraphDelta;
use lj_node_extract::processor::ExtractNodeProcessor;
use lj_node_http::processor::HttpNodeProcessor;
use lj_node_js::processor::JsNodeProcessor;
use lj_rule_model::{PolicyCapabilities, SystemCapabilities};
use lj_runtime::NodeData;
use lj_runtime::executor::GraphExecutor;
use lj_runtime::{ExecutionContext, NodeProcessor, SegmentSpec};
use lj_runtime::{Graph, NodeKind};

/// 初始化 `tracing` subscriber（幂等，多次调用安全）。
///
/// 默认关闭测试日志，避免把 UTF-8 调试摘要打进标准输出。
/// 如需调试，可显式设置 `RUST_LOG` 开启，并统一写入 `stderr`。
pub fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("off")),
        )
        .with_writer(std::io::stderr)
        .try_init();
}

/// 构建处理器注册表（测试路径，允许本地 mock server）。
pub fn build_processors() -> HashMap<NodeKind, Box<dyn NodeProcessor>> {
    let mut map: HashMap<NodeKind, Box<dyn NodeProcessor>> = HashMap::new();
    map.insert(NodeKind::Http, Box::new(HttpNodeProcessor::new_test()));
    map.insert(NodeKind::Extract, Box::new(ExtractNodeProcessor));
    map.insert(NodeKind::Js, Box::new(JsNodeProcessor));
    map
}

/// 构建测试 `ExecutionContext`。
///
/// `trace_id` 区分各 test target 的日志来源。
pub fn make_ctx(base_url: &str, trace_id: &str) -> ExecutionContext {
    ExecutionContext {
        cookies: HashMap::new(),
        caps: PolicyCapabilities {
            network: true,
            system: SystemCapabilities::default(),
        },
        trace_id: trace_id.to_string(),
        base_url: base_url.to_string(),
    }
}

/// 执行段并收集所有 `NodeData` 产出。
pub async fn execute_and_collect(
    graph: &Graph,
    segment: SegmentSpec,
    base_url: &str,
    trace_id: &str,
) -> Vec<NodeData> {
    let ctx = make_ctx(base_url, trace_id);
    let processors = build_processors();
    let executor = GraphExecutor::new();
    let output = executor.execute(graph, &segment, &ctx, &processors);
    output.map(|(_id, data)| data).collect().await
}

/// 合并输出流中的所有媒体资源图增量。
pub fn collect_delta(results: &[NodeData]) -> MediaGraphDelta {
    let errors: Vec<&str> = results
        .iter()
        .filter_map(|item| match item {
            NodeData::Error(message) => Some(message.as_str()),
            _ => None,
        })
        .collect();
    assert!(errors.is_empty(), "执行不应产出错误事件: {errors:?}");

    let deltas: Vec<MediaGraphDelta> = results
        .iter()
        .filter_map(|item| match item {
            NodeData::Delta(delta) => Some(delta.clone()),
            _ => None,
        })
        .collect();
    assert!(
        !deltas.is_empty(),
        "执行应产出 MediaGraphDelta: {results:?}"
    );

    deltas
        .into_iter()
        .fold(MediaGraphDelta::default(), MediaGraphDelta::merge)
}
