---
title: Tracing 链路追踪
date: '2026-07-01'
category: design-patterns
module: tracing
problem_type: design_pattern
component: development_workflow
severity: medium
applies_when:
  - '添加新的 tracing span'
  - '调试执行链路问题'
  - '配置 tracing-subscriber'
tags:
  - tracing
  - observability
  - span
  - ipc-channel
---

# Tracing 链路追踪

LanJing Rust 端使用 `tracing` crate 实现全链路可观测性。

## Context

规则执行涉及多个节点、多个 crate，需要端到端的可观测性来调试执行问题。

## Guidance

### Span 层级

```
root span (用户请求)
  ├─ child span: Http node (请求源站)
  ├─ child span: Extract node (CSS/Regex 提取)
  └─ child span: Transform node (清洗/格式化)
```

- **Root Span**：以请求入口为 root span——用户触发搜索/详情/目录浏览时创建
- **Child Span**：各节点处理生成 child span，自动挂载在 root span 下
- **父子关联**：tracing 自动维护 span 的 parent-child 关系，无需手动传递 context

### IPC 通道可观测

自定义 `tracing-subscriber` layer 将 spans 通过 IPC 通道流向前端 devtools 视图：

- **IPC Channel Layer**：拦截 span 事件（enter/exit/close），序列化后经 Tauri IPC event 推送前端
- **前端 devtools**：Svelte 组件接收 `trace-event`、实时渲染 span 时间线与父子关系
- **Error Debug**：错误发生时前端通过 `trace_id` 回溯完整调用链

### trace_id 链路追踪

由 tracing 为每个 root span 自动分配的唯一 ID，贯穿整个执行链路：

- **自动生成**：tracing 在 root span 创建时自动分配 UUID trace_id
- **全链路穿透**：从 Tauri command 入口 → GraphExecutor → 各 NodeProcessor → HTTP 请求，全部共享同一 trace_id
- **前端可观测**：前端按 trace_id 回放完整请求路径

### 设计原则

- **零侵入**：业务代码只需 `#[tracing::instrument]` attribute
- **前端可观测**：后端日志不只存文件——IPC 推前端，非开发者也可见执行流
- **ID 贯穿**：trace_id 从 root span 到最深层 child span 保持一致

### OTLP 可选升级路径

首刀不引入 OpenTelemetry 导出。当前 tracing-subscriber 架构仅写本地日志 + IPC channel 推前端。设计上支持后期升级：

- **Layer 架构**：tracing-subscriber 的 layer 模式允许在不修改现有代码的情况下添加 OTLP layer
- **零侵入升级**：新增 `tracing-opentelemetry` layer 即可将 spans 导出到 OTLP collector

### 监控工具

- **`tokio-console`**：实时监控 task 状态、waker 唤醒频率、poll 耗时
- **`tracing` + `tokio-console` subscriber**：在 console 中关联 span 与 task

## Why This Matters

全链路可观测性使开发者可以在前端 devtools 中实时看到后端执行流，无需查看日志文件。trace_id 贯穿使错误定位无需手动拼接上下文。

## When to Apply

- 新增节点处理器：添加 `#[tracing::instrument]`
- 调试执行问题：通过 trace_id 回溯完整调用链
- 配置 tracing-subscriber：添加或移除 layer

## Related

- `docs/solutions/architecture-patterns/tauri-ipc.md` — IPC 通道推送 tracing span
- `docs/solutions/conventions/rust-async.md` — 异步监控工具
