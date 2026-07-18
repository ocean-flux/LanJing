//! 运行时处理器 seam — `NodeProcessor` 与执行上下文。
//!
//! 浅层 `Executor` trait 已删除；`GraphExecutor` 提供具体 `execute` 方法。

use std::collections::HashMap;

use futures::stream::BoxStream;
use lj_capability::{IntentInput, StandardIntent};
use lj_rule_model::PolicyCapabilities;

use crate::graph::{NodeKind, NodeSpec};
use crate::node_data::{NodeData, NodeDataVariant};

/// 节点处理器接口(stream-to-stream)。
///
/// `process` 同步返回 `BoxStream`，非 `async_trait`。
/// 生命周期绑定 `self`/`ctx`/`spec`/`input`(借用语义)。
pub trait NodeProcessor: Send + Sync {
    /// 节点类型。
    fn kind(&self) -> NodeKind;
    /// 输入数据类型(静态声明，图构建时验证边类型匹配)。
    fn input_type(&self) -> Option<NodeDataVariant>;
    /// 输出数据类型。
    fn output_type(&self) -> NodeDataVariant;
    /// 处理节点:消费 input stream 产出 output stream。
    ///
    /// sync 返回 stream，管道内部异步流动。
    fn process<'a>(
        &'a self,
        ctx: &'a ExecutionContext,
        spec: &'a NodeSpec,
        input: BoxStream<'a, NodeData>,
    ) -> BoxStream<'a, NodeData>;
}

/// 执行上下文(节点执行时的共享状态)。
pub struct ExecutionContext {
    /// cookie jar(同源共享)。
    pub cookies: HashMap<String, String>,
    /// 策略能力（安装 grant / 沙箱边界）。
    pub caps: PolicyCapabilities,
    /// tracing span 上下文。
    pub trace_id: String,
    /// 源站基础 URL(用于相对路径→绝对路径拼接)。
    pub base_url: String,
}

/// 段执行规格。
#[derive(Debug, Clone)]
pub struct SegmentSpec {
    /// 要执行的标准意图。
    pub intent: StandardIntent,
    /// 标准意图输入。
    pub input: IntentInput,
}
