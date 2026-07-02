---
title: stream-to-stream 错误透传模式 — NodeData::Error 不应被静默吞没
date: 2026-06-27
category: docs/solutions/architecture-patterns/
module: runtime
problem_type: architecture_pattern
component: service_object
severity: high
applies_when:
  - "设计 stream-to-stream pipeline 的错误处理策略"
  - "使用 flat_map 消费 input stream 产 output stream 的处理器"
  - "NodeProcessor 或类似 stream transformer 的实现"
tags:
  - stream
  - error-handling
  - flat-map
  - node-processor
  - async
  - futures
  - architecture
---

# stream-to-stream 错误透传模式 — NodeData::Error 不应被静默吞没

## Context

LanJing 规则引擎的 NodeProcessor 采用 stream-to-stream 签名:`process(&self, ctx, spec, input: BoxStream<NodeData>) -> BoxStream<NodeData>`。每个处理器用 `flat_map` 消费 input stream 的每个 item,产出一个子 stream,所有子 stream 被 flatten 成 output stream。

问题:`flat_map` 闭包中如果对 `NodeData::Error` 返回 `stream::empty()`,错误被静默吞没 — 下游看到空输出,无法区分"没有数据"和"出错了"。

## Guidance

**Error 是一等公民,必须透传。** 在 stream-to-stream pipeline 中,错误通过 `NodeData::Error` variant 显式传播,任何处理器都不应丢弃它。

```rust
use futures::stream::{self, StreamExt};

fn process(&self, _: &ExecutionContext, _: &NodeSpec, input: BoxStream<NodeData>) -> BoxStream<NodeData> {
    input.flat_map(|item| {
        match item {
            // Error 透传:用 stream::once 产出单个 Error item
            NodeData::Error(err) => stream::once(async move {
                NodeData::Error(err)
            }).boxed(),

            // 正常处理:可能产出多个 item 或 Error
            NodeData::HttpResponse(resp) => {
                match self.extract(resp) {
                    Ok(items) => stream::iter(items).boxed(),
                    Err(e) => stream::once(async move {
                        NodeData::Error(e)
                    }).boxed(),
                }
            }

            // 不支持的输入类型:产出 Error 而非静默跳过
            other => stream::once(async move {
                NodeData::Error(CoreError::UnexpectedInput(other.kind()))
            }).boxed(),
        }
    }).boxed()
}
```

### 关键原则

1. **`stream::empty()` 只用于"确实没有数据"**:正常处理结果为空(如 CSS 选择器无匹配),不是错误
2. **`stream::once(Error)` 用于"出错了"**:处理器内部失败,或收到不支持的输入类型
3. **不支持的输入类型也要报错**:不要静默跳过,产出 `Error::UnexpectedInput` 让下游知道
4. **executor tap 处理 Error**:GraphExecutor 的 tap_stream 对 Error 也 emit `node-output` event + tracing 日志,前端能看到错误

### 反模式

```rust
// 反模式 1: Error 被静默吞没
NodeData::Error(_) => stream::empty(),  // 错误消失,下游看到空输出

// 反模式 2: 不支持的输入静默跳过
NodeData::Raw(_) => stream::empty(),  // 应该报 Error::UnexpectedInput

// 反模式 3: 处理器内部失败只打日志不产出 Error
Err(e) => {
    tracing::error!(error = %e, "提取失败");
    stream::empty()  // 错误只到日志,不到 stream,前端看不到
}
```

## Why This Matters

- **可诊断性**:错误透传让前端和日志都能看到失败原因,而不是"0 条结果"的谜题
- **调试效率**:本次全链路调试中,Bug 1(Error 吞没)导致首次测试 0 产出,花了大量时间误诊为 `select_subgraph` 问题。修 Error 透传后立即看到 SSRF 拦截的真实错误
- **用户体验**:前端可以展示"请求失败:SSRF 拦截"而非空白页面
- **架构一致性**:NodeData enum 是闭集,Error 是显式 variant,所有处理器应穷尽匹配并正确处理

## When to Apply

- 任何 stream-to-stream pipeline 的错误处理设计
- `flat_map` + `stream::empty()` 的使用场景审查
- NodeProcessor 或类似 stream transformer 的实现
- 任何"错误应该可见还是静默"的设计决策

## Examples

### 调试时间线对比

**Error 被吞没时(反模式)**:
```
测试: search 产出 0 条 NodeData
诊断: select_subgraph 没选到节点?stream 完全空?
误诊 2 小时,最终加大量 trace 日志才发现 Error 被吞掉
```

**Error 透传后(正确模式)**:
```
测试: search 产出 1 条 NodeData::Error("SSRF: 169.254.169.254 被拦截")
诊断: 立即看到 SSRF 拦截,5 分钟定位问题
```

### executor tap 对 Error 的处理

```rust
// GraphExecutor 的 tap_stream 对所有 NodeData(含 Error)emit event
fn tap_stream(stream: BoxStream<NodeData>, node_id: NodeId) -> BoxStream<NodeData> {
    stream.inspect(move |item| {
        let summary = summarize(&node_id, item);
        tracing::info!(node_id = %node_id, summary = %summary, "节点输出");
        // 前端通过 node-output event 看到 Error
    }).boxed()
}
```

## Related Issues

- ADR-0022: NodeProcessor stream-to-stream + NodeData::Error variant
- `docs/solutions/integration-issues/legado-engine-full-chain-integration.md` — Bug 1 Error 吞没修复
