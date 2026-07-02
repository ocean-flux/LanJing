---
title: 规则引擎架构
date: '2026-07-01'
category: architecture-patterns
module: lj-core
problem_type: architecture_pattern
component: development_workflow
severity: high
applies_when:
  - '理解 LanJing 规则系统的整体设计'
  - '新增节点类型或修改执行引擎'
  - '实现新的导入器'
tags:
  - rule-engine
  - node-graph
  - streaming-pipeline
  - dagnodes
---

# 规则引擎架构

LanJing 的核心：规则定义 → 编译 → 节点图 → 执行 → 流式产出媒体模型。

## Context

规则在 LanJing 内部恒为节点图（DAG）。规则引擎是 Rust 端承载规则执行的运行时，前端用 Svelte xyflow 可视化编辑。需要一个统一的内部表示，兼容三类异构源（Legado 图文书源、Maccms10 视频源、LXMusic 音源）的规则格式。

## Guidance

### 架构概览

```
规则定义 (JSON/YAML)
    ↓
[lj-compiler] 编译
    ↓
GraphSpec (节点图)
    ↓
[lj-runtime] 执行
    ↓
Stream<Media> 产出
```

### 节点图（Node Graph）— 一等公民 Schema

所有规则以 DAG 形态存储与执行：

```rust
struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    schema: GraphSchema,
}

struct Node {
    id: NodeId,
    kind: NodeKind,
    spec: NodeSpec,  // 随 kind 变化
}
```

设计要点：

- **导入器自动生成**：Legado/Maccms10/LXMusic 规则文件由导入器自动转换为节点图
- **用户可选手动搭建**：保留手动搭建能力，但非必须
- **GraphSchema 约束**：Discover 端点强制 `Js → Http → Extract` 节点序列
- **占位符 Js 节点**：Maccms 源也必须有占位符 Js 节点，executor 根据代码是否为空决定是否跳过
- **前端可视化**：Svelte xyflow 拖拽编辑

### 节点类型（NodeKind）

闭集 enum，编译期穷尽匹配：

```rust
enum NodeKind {
    Http(HttpSpec),      // url, method, headers, body
    Js(JsSpec),          // code, timeout
    Extract(ExtractSpec), // selector, field_rules
    ConditionBranch,
    Merge,
    Subroutine,
}
```

| 类别         | 类型            | 职责                        | 实现 crate            |
| ------------ | --------------- | --------------------------- | --------------------- |
| Endpoint     | HTTP            | 执行 HTTP 请求              | lj-node-http          |
| Endpoint     | JS              | 执行 JS 代码                | lj-node-js (rquickjs) |
| Endpoint     | Extract         | 从 HTML/JSON 提取结构化数据 | lj-node-extract       |
| Control Flow | ConditionBranch | 条件分支                    | —                     |
| Control Flow | Merge           | 合并多路输入流              | —                     |
| Control Flow | Subroutine      | 调用子图                    | —                     |

### 节点规格（NodeSpec）

端点规格的强类型设计：

```rust
enum NodeSpec {
    Http(HttpSpec),      // url, method, headers, body
    Js(JsSpec),          // code, timeout
    Extract(ExtractSpec), // selector, field_rules
}
```

设计原则：

- **struct + Option**：端点 spec 用 struct，可选字段用 Option
- **序列化向后兼容**：新增字段用 Option，旧数据反序列化不报错
- **exhaustive match**：闭集 enum，编译器强制处理所有 variant

### Stream-to-Stream 管道范式

核心执行模型：每个 `NodeProcessor::process` 同步接收上游 `BoxStream<NodeData>`，同步返回下游 `BoxStream<NodeData>`。

```rust
trait NodeProcessor {
    fn process(&self, input: BoxStream<NodeData>) -> BoxStream<NodeData>;
}
```

关键特性：

- **同步返回，异步流动**：管道组装同步，stream 内 item 生产与消费异步
- **NodeData 闭集枚举**：Raw / HttpResponse / Media / Json / Error
- **Error 一等公民**：`stream::once(Error)` 透传，不可静默吞掉
- **批量即退化流式**：批量结果视为单元素流
- **背压天然支持**：stream-to-stream 模型下背压由 Rust `Stream` trait 的 poll 机制自动传播
- **Fan-out Concurrency**：节点自行控制并发扇出，不需要中心调度器

### 流式规则输出

规则执行通过 Tauri event 将每条媒体结果边产边推送给前端：

|            | 流式                 | 批量             |
| ---------- | -------------------- | ---------------- |
| 首结果延迟 | 低（首个 item 即推） | 高（等所有完成） |
| 内存       | 常数（流）           | O(n)（全量缓存） |
| 用户体验   | 渐进渲染             | 全量等待后渲染   |

### 类型安全

使用 enum + pattern matching 实现穷尽检查：

```rust
impl NodeProcessor for NodeKind {
    async fn process(&self, input: NodeData) -> Result<NodeData> {
        match self {
            NodeKind::Http(spec) => http_processor.process(spec, input),
            NodeKind::Js(spec) => js_processor.process(spec, input),
            // 编译器确保所有分支都被处理
        }
    }
}
```

## Why This Matters

规则引擎的节点图 DAG + stream-to-stream 范式是 LanJing 的核心架构假设。所有规则在内部以统一 DAG 形态存储与执行，使得异构源（书源/视频源/音源）可以通过不同的导入器翻译为同一内部表示，由同一执行引擎统一执行。

## When to Apply

- 新增节点类型时，需修改 `NodeKind` enum 并在所有 match 处添加分支
- 新增导入器时，产出必须是 `Graph`（节点图），不能是其他格式
- 修改执行引擎时，需保证 stream-to-stream 范式不变

## Related

- `docs/solutions/architecture-patterns/import-system.md` — 导入器如何产出节点图
- `docs/solutions/design-patterns/rust-patterns.md#节点双-id-模式` — NodeId 与 ImportHash 的分离
- `docs/solutions/design-patterns/extract-rules.md` — Extract 节点的提取能力
- `docs/solutions/design-patterns/tracing.md` — 运行时追踪
