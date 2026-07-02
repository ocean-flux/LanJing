---
title: Rust 设计模式与惯用法
date: '2026-07-01'
category: design-patterns
module: rust
problem_type: design_pattern
component: development_workflow
severity: high
applies_when:
  - '编写新的 Rust 模块或 crate'
  - '选择类型系统设计策略'
  - '实现跨 crate 错误传播'
tags:
  - rust-patterns
  - enum-dispatch
  - newtype
  - typestate
  - error-handling
---

# Rust 设计模式与惯用法

LanJing Rust 代码库中反复出现的设计模式与惯用法。

## Context

10 crate 架构需要一致的设计模式来确保编译期类型安全、零成本抽象、明确的 trait 边界。

## Guidance

### Enum + Trait Dispatch

核心模式：通过 enum variant 持有具体类型，结合 trait 定义共享行为：

```rust
enum NodeKind {
    Http(HttpSpec),
    Extract(ExtractSpec),
    Condition(ConditionSpec),
}

trait NodeProcessor {
    fn process(&self, input: BoxStream<NodeData>) -> BoxStream<NodeData>;
}
```

各 variant 实现 `NodeProcessor` trait，调用侧通过 match 分发——编译器保证穷尽匹配。避免 `Box<dyn Trait>` 的动态分发开销。

### 闭集 Enum（Closed-set Enum）

variants 固定、不开放外部扩展。LanJing 大量使用：

- **NodeKind**：所有节点类型闭集
- **NodeData**：节点间数据载荷闭集
- **ExtractRule**：提取规则闭集
- **Media**：媒体类型闭集
- **EndpointKind**：端点类型闭集

核心价值：编译期穷尽匹配——新增 variant = 编译错误在所有未覆盖处，强制显式处理。

**被否定的替代方案**：Flat + Kind Tag（所有字段平铺为可选 `Option<T>`，靠 `kind` 标签字符串区分类型）——丢失编译期类型安全检查。

### Newtype Pattern

用单字段 tuple struct 包装原始类型，赋予语义并防止混用：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RepoId<T>(Uuid);

// 编译错误：不能混用 RepoId<Graph> 和 RepoId<Media>
fn find_media(id: RepoId<Media>) -> Option<Media> { ... }
find_media(graph_id); // ❌ 类型不匹配
```

Phantom type 参数 `T` 只在编译期有意义，运行时零开销。

### Builder Pattern

构造器参数过多（>3 个）或大部分参数可选时使用：

```rust
pub struct HttpSpecBuilder {
    url: Option<String>,
    method: Option<Method>,
    headers: Option<HeaderMap>,
    timeout: Option<Duration>,
}

impl HttpSpecBuilder {
    pub fn url(mut self, url: impl Into<String>) -> Self { self.url = Some(url.into()); self }
    pub fn build(self) -> Result<HttpSpec, BuildError> { /* 验证不变量 */ }
}
```

### Typestate Pattern

用类型参数在编译期编码状态转换——`url()` 只能在 `MissingUrl` 状态调用，`build()` 只能在 `Ready` 状态调用。

### Result 类型别名模式

每个 crate 定义自己的 `Result<T>` 类型别名：

```rust
pub type Result<T> = std::result::Result<T, Error>;
```

简化函数签名——`fn process() -> Result<()>` 而非 `fn process() -> std::result::Result<(), Error>`。

### Cross-Crate Error Conversion via From

手写 `From` 实现将跨 crate 边界的 Error enum 串联：

```rust
impl From<lj_core::Error> for lj_runtime::Error {
    fn from(e: lj_core::Error) -> Self { Error::Core(e) }
}
```

每个 crate 的 Error enum 为上游 crate 的 Error 提供 `From` 实现，使 `?` 操作符跨 crate 透明传播。

### 混合错误处理范式

- **thiserror（domain crate）**：`lj-core`、`lj-runtime` 使用 `#[derive(Error)]` 强类型 Error enum
- **anyhow（glue 层）**：`src-tauri` IPC 边界使用 `anyhow::Result<T>` + `.context()`
- **串联**：thiserror Error enum 通过 `From` 向上传播，在 glue 层被 `.context()` 包装为 anyhow error

### Sync Return, Async Flow

管道组装是同步的，但 stream 内部的 item 生产是异步的——避免了 `async fn` 在 trait 中的复杂性（无需 `async_trait` macro）。

### 序列化向后兼容

新增字段一律 `Option<T>` 保证反序列化旧 payload 不中断。

### mpsc Backpressure

`tokio::sync::mpsc` channel 天然提供背压机制：有界 channel，发送方在 channel 满时 `.await`，自然背压。

## Why This Matters

这些模式不是学术练习——每个都有明确的工程价值：闭集 enum 保证编译期穷尽，newtype 防止 ID 混用，From 链保留错误溯源。

## When to Apply

- 新增类型：优先闭集 enum + trait dispatch
- 新增 crate：定义自己的 Error enum + Result alias + From 实现
- 构造器复杂：使用 Builder pattern

## Related

- `docs/solutions/conventions/rust-coding.md` — Rust 编码规范
- `docs/solutions/conventions/rust-async.md` — Rust 异步模式
- `docs/solutions/conventions/rust-error-handling.md` — 错误处理详解
