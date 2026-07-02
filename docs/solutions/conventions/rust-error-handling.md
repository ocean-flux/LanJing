---
title: Rust 错误处理
date: '2026-07-01'
category: conventions
module: rust
problem_type: convention
component: development_workflow
severity: high
applies_when:
  - '定义新的 Error 类型'
  - '跨 crate 传播错误'
  - '在 Tauri IPC 边界处理错误'
tags:
  - rust
  - error-handling
  - thiserror
  - anyhow
---

# Rust 错误处理

Rust 错误处理的分层策略：从 domain crate 的强类型 Error enum 到 glue 层的 `anyhow` 收口，再到 stream 管道的错误传播。

## Context

10 crate 架构需要一致的错误处理策略，确保错误信息在跨 crate 传播时不丢失类型信息，在 IPC 边界能被前端理解。

## Guidance

### 分层策略

| 层                | 使用                 | 原因                             |
| ----------------- | -------------------- | -------------------------------- |
| 库 crate 公开 API | `thiserror`          | 调用方需要 match 处理具体错误    |
| 应用程序 glue 层  | `anyhow`             | 只关心"是否成功"，不关心具体类型 |
| 跨 crate 调用     | `thiserror` + `From` | 保留类型信息穿越 crate 边界      |

### thiserror（Domain Crate）

```rust
#[derive(Error, Debug)]
pub enum Error {
    #[error("HTTP 请求失败: {0}")]
    Http(#[from] reqwest::Error),

    #[error("规则解析错误 at line {line}: {msg}")]
    Parse { line: usize, msg: String },

    #[error("节点执行超时: node_id={node_id}, timeout={timeout:?}")]
    Timeout { node_id: Uuid, timeout: Duration },

    #[error("SSRF 拦截: {url}")]
    SsrfBlocked { url: String },
}
```

设计原则：

- 每个 variant 携带足够的上下文供调试
- `#[from]` 自动生成 `From` 实现，使 `?` 透明传播
- 避免 `Box<dyn Error>` 在 domain crate 中出现

### anyhow（Glue 层）

```rust
#[tauri::command]
fn execute_rule(rule_id: String) -> anyhow::Result<Vec<Media>> {
    let rule = rules_repo.find(&rule_id)
        .context("获取规则失败")?;
    let media = executor.execute(rule)
        .context("规则执行失败")?;
    Ok(media)
}
```

`.context()` 在最外层包装人类可读的上下文——错误链从底层到顶层逐层追加信息。

### 跨 Crate 错误转换

手写 `From` 实现串联 domain crate 的错误链：

```rust
impl From<lj_core::Error> for lj_runtime::Error {
    fn from(e: lj_core::Error) -> Self { Error::Core(e) }
}
```

每个 crate 的 Error enum 包含上游 crate 的 Error variant——`?` 操作符跨 crate 透明传播，不压缩为字符串。

### Stream 管道中的错误处理

- **错误作为一等公民**：`stream::once(async { Err(Error::Timeout { ... }) })`
- **错误不中止管道**：一个 item 失败不影响后续 item 处理
- **错误携带上下文**：`NodeData` 的 Error variant 包含 node_id + 时间戳 + 错误详情
- **前端可观测**：Tauri event 实时推送错误信息

### 何时 Panic

**应该 panic**：不可恢复的不变量违反（`unreachable!()`、`todo!()`）、初始化失败无法继续

**不应该 panic**：IO 操作失败、网络请求超时、用户输入格式错误、Library 代码中的任何可恢复错误

**核心原则**：library 代码永远不 panic，application 代码在无法继续时 panic

## Why This Matters

分层错误处理使 domain crate 保留类型信息供 match 处理，glue 层用 anyhow 简化错误传播，stream 管道确保错误不被静默吞掉。

## When to Apply

- 定义新的 Error 类型：使用 thiserror，每个 variant 携带上下文
- 跨 crate 传播错误：手写 From 实现
- IPC 边界：使用 anyhow + .context()

## Related

- `docs/solutions/design-patterns/rust-patterns.md` — Result 类型别名、Cross-Crate Error Conversion
- `docs/solutions/conventions/rust-coding.md` — Rust 编码规范
