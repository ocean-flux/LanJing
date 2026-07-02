---
title: rquickjs Runtime 非 Send — spawn_blocking + channel 模式集成 async Rust
date: 2026-06-27
category: docs/solutions/tooling-decisions/
module: node-js
problem_type: tooling_decision
component: service_object
severity: high
applies_when:
  - "在 async Rust (tokio) 中嵌入非 Send 的 JS 引擎(rquickjs/QuickJS)"
  - "JS 执行需要超时控制(watchdog)和资源限制(memory/stack)"
  - "JS 执行结果需要流入 async stream pipeline"
tags:
  - rquickjs
  - quickjs
  - spawn-blocking
  - tokio
  - non-send
  - js-engine
  - sandbox
---

# rquickjs Runtime 非 Send — spawn_blocking + channel 模式集成 async Rust

## Context

LanJing 规则引擎需要执行 Legado 规则中的 `@js:` 块(JavaScript 代码)。选择 rquickjs(QuickJS 的 Rust 绑定)作为 JS 引擎。QuickJS 是单线程引擎,rquickjs 的 `Runtime` 内部使用 `Rc`(非 `Arc`),因此 `Runtime` 不实现 `Send`。但规则引擎的 NodeProcessor 是 async stream pipeline(基于 tokio),需要跨 await 点传递数据。

## Guidance

**不要尝试让 Runtime 跨 await 点。** 用 `tokio::task::spawn_blocking` + channel 模式:JS 代码在 blocking 线程执行,结果通过 channel 传回 async stream。

```rust
use tokio::sync::mpsc;
use tokio::task::spawn_blocking;

fn process(&self, ctx: &ExecutionContext, spec: &NodeSpec, input: BoxStream<NodeData>) -> BoxStream<NodeData> {
    let (tx, rx) = mpsc::channel::<NodeData>(32);

    spawn_blocking(move || {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();

        // 资源限制
        runtime.set_memory_limit(1024 * 1024 * 16); // 16MB
        runtime.set_max_stack_size(1024 * 512);     // 512KB

        // watchdog 超时中断
        let runtime_handle = runtime.handle();
        let watchdog = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(5));
            runtime_handle.interrupt();
        });

        // 执行 JS
        while let Some(item) = input.blocking_recv() {
            let result = context.eval::<String>(js_code);
            let _ = tx.blocking_send(result_to_node_data(result));
        }

        // watchdog 不需要 join(它会自行结束)
        // ponytail: watchdog 线程不 join,Runtime drop 时 interrupt handler 触发
    });

    ReceiverStream::new(rx).boxed()
}
```

### 关键约束

1. **`Runtime` 不跨 await 点**:`Runtime` 在 `spawn_blocking` 闭包内创建和销毁,不 move 出闭包
2. **`Mutex<Runtime>` 也不自动 Send**:`Rc` 内部使 `Runtime` 非 Send,`Mutex` 包装不改变这一点
3. **`spawn_blocking` 闭包是 `Send + 'static`**:闭包内不能捕获非 Send 的值,Runtime 在闭包内创建
4. **input stream 用 `blocking_recv`**:`spawn_blocking` 内是同步上下文,用 `mpsc::Receiver::blocking_recv`
5. **watchdog 用独立线程**:`Runtime::handle()` 是 `Send`(它只是指针),可以 move 到 watchdog 线程调 `interrupt()`

### 资源限制 API

```rust
let runtime = Runtime::new()?;

// 内存上限(超出抛 JS 异常)
runtime.set_memory_limit(16 * 1024 * 1024);

// 栈深度上限(超出触发段错误,需 watchdog 兜底)
runtime.set_max_stack_size(512 * 1024);

// 中断处理器(watchdog 超时后调用)
runtime.set_interrupt_handler(Some(Box::new(|_| {
    Err(QuickJsError::new("执行超时".to_string()))
})));
```

## Why This Matters

- **直接 move Runtime 跨 await 会编译失败**:`Runtime` 非 `Send`,tokio 要求 future 是 `Send`
- **`Mutex<Runtime>` + async 锁是陷阱**:`tokio::sync::Mutex` 要求内部值 `Send`,`std::sync::Mutex` 持锁跨 await 会阻塞 runtime
- **spawn_blocking 是标准解法**:blocking 线程池专为非 Send/阻塞操作设计,不阻塞 async runtime
- **watchdog 独立线程是必须的**:JS 死循环不会自行让出执行权,需要外部线程调 `interrupt()` 打断

## When to Apply

- 在 async Rust 中嵌入任何非 Send 的 C/Rust 绑定库(QuickJS、某些 GUI 库、某些数据库驱动)
- JS 引擎需要资源限制和超时控制的沙箱场景
- 需要将同步执行结果流入 async stream pipeline 的场景

## Examples

### 错误:直接在 async 上下文中使用 Runtime

```rust
// 编译错误:Runtime 非 Send,不能跨 await 点
async fn execute_js(code: &str) -> Result<String> {
    let runtime = Runtime::new()?;  // Runtime: !Send
    let ctx = Context::full(&runtime)?;
    let result = ctx.eval::<String>(code)?;
    // await 点在这里 — Runtime 跨越了 await,编译失败
    some_async_op().await;
    Ok(result)
}
```

### 错误:Mutex<Runtime> + async

```rust
// 运行时陷阱:std::sync::Mutex 持锁跨 await 阻塞 runtime 线程
async fn execute_js(runtime: &Mutex<Runtime>, code: &str) -> Result<String> {
    let runtime = runtime.lock().unwrap();  // 持锁
    let ctx = Context::full(&runtime)?;
    let result = ctx.eval::<String>(code)?;
    drop(runtime);  // 必须在 await 前释放
    some_async_op().await;
    Ok(result)
}
// 看似可行,但容易忘记 drop,且无法处理并发请求
```

### 正确:spawn_blocking + channel

```rust
// Runtime 在 blocking 线程内创建和销毁,结果通过 channel 传回
let result = tokio::task::spawn_blocking(move || {
    let runtime = Runtime::new()?;
    let ctx = Context::full(&runtime)?;
    ctx.eval::<String>(code)
}).await?;
```

## Related Issues

- ADR-0002: rquickjs 作为 JS 运行时
- ADR-0008: JS sandbox 分层配置
- spike 验证: `.tmp/spike/rquickjs-spike/`(Windows MSVC 编译成功 + watchdog 中断验证)
