---
title: Rust 异步模式
date: '2026-07-01'
category: conventions
module: rust
problem_type: convention
component: development_workflow
severity: high
applies_when:
  - '编写异步 Rust 代码'
  - '使用 Tokio 运行时'
  - '调试异步问题（死锁、内存泄漏、取消安全）'
tags:
  - rust
  - async
  - tokio
  - concurrency
---

# Rust 异步模式

Rust 异步运行时的生产级模式：Tokio 为基础，涵盖 graceful shutdown、背压控制、结构化并发、spawn_blocking、取消安全。

## Context

LanJing 使用 Tokio 异步运行时，规则执行涉及大量 IO 操作（HTTP 请求、SQLite 读写、JS 执行），需要正确的异步模式来避免死锁、内存泄漏和取消安全问题。

## Guidance

### Graceful Shutdown

使用 `watch::channel` 广播关闭信号，每个任务持有独立 `Receiver` clone，通过 `tokio::select!` 同时等待关闭信号与正常 work。

### Backpressure

有界 channel 天然背压：`mpsc::channel(capacity)`，发送方在 channel 满时 `.await`，自然降速。容量过大 → OOM 风险；容量过小 → 吞吐瓶颈。

Semaphore 限流：`Arc::new(Semaphore::new(10))` 限制最大并发数。

### 结构化并发

- **JoinSet**：自动管理任务生命周期，`join_next()` 按完成顺序返回
- **TaskTracker**（tokio-util）：`tracker.close()` 标记不再有新任务，`tracker.wait().await` 等待完成

### spawn_blocking 桥接

同步 IO 密集型代码（如 rusqlite）在 `spawn_blocking` 中执行。非 Send 类型可通过 `spawn_blocking` + `std::sync::mpsc::channel` 传递。

### 取消安全

Future 在 `.await` 点被 drop 时，已执行的状态可能丢失。安全模式：

- `tokio::sync::oneshot`：取消不丢失数据
- `tokio::sync::mpsc`：`send` 是 cancel-safe
- `tokio::io::AsyncWriteExt::write_all`：部分写入后取消 = 数据可能已写入

### 常见陷阱

1. **阻塞异步运行时**：`std::thread::sleep` → 用 `tokio::time::sleep`
2. **`std::sync::Mutex` 跨 `.await`**：持有 MutexGuard 跨 .await → 在 .await 前释放锁
3. **非 Send Future**：`Rc<T>` 不能跨 `.await` 持有 → 改用 `Arc<T>`
4. **无限 stream**：`StreamExt::collect()` 在无限 stream 上永远阻塞 → 始终添加 `take()`/`timeout()`/shutdown 信号

## Why This Matters

错误的异步模式会导致死锁、内存泄漏、取消安全问题。这些模式是从生产环境中总结的最佳实践。

## When to Apply

- 编写异步函数时：选择正确的同步原语
- 配置 channel 容量时：按实测负载设定
- 调试异步问题时：检查常见陷阱

## Related

- `docs/solutions/conventions/rust-coding.md` — Rust 编码规范
- `docs/solutions/design-patterns/rust-patterns.md` — mpsc Backpressure 模式
