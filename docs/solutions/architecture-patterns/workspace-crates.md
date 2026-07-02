---
title: Crate 工作区划分
date: '2026-07-01'
category: architecture-patterns
module: workspace
problem_type: architecture_pattern
component: development_workflow
severity: high
applies_when:
  - '理解 LanJing 的 10 crate 架构'
  - '新增 crate 或修改 crate 依赖'
  - '优化编译时间'
tags:
  - workspace
  - crate-organization
  - cargo
---

# Crate 工作区划分

LanJing Rust workspace 的 10 个 crate 及其依赖关系。

## Context

大型 Rust 项目需要合理的 crate 划分来隔离编译时间、分离关注点、稳定公开 API。

## Guidance

### Crate 列表

| Crate             | 职责                        | 依赖                               |
| ----------------- | --------------------------- | ---------------------------------- |
| `lj-core`         | 纯类型+trait 定义           | 无                                 |
| `lj-runtime`      | 图执行器 `GraphExecutor`    | lj-core                            |
| `lj-node-http`    | HTTP 节点处理器             | lj-core, reqwest, hickory-resolver |
| `lj-node-js`      | JS 节点处理器               | lj-core, rquickjs, lj-sandbox      |
| `lj-node-extract` | 提取节点处理器              | lj-core, scraper                   |
| `lj-sandbox`      | JS 沙箱能力                 | lj-core, rquickjs                  |
| `lj-compiler`     | 规则字符串 → ExtractRule IR | lj-core                            |
| `lj-importer`     | 异构规则 → Graph 翻译       | lj-core, lj-compiler               |
| `lj-storage`      | `Repository<T>` 持久层      | lj-core, rusqlite                  |
| `src-tauri`       | Tauri IPC 胶水 + main       | 以上所有                           |

### 依赖方向

```
src-tauri (顶层装配)
  └── lj-runtime / lj-importer / lj-storage / lj-node-*
        └── lj-core (纯类型+trait，零业务逻辑)
```

依赖方向严格向下：`lj-core` 在最底层，不依赖任何 LanJing crate；`src-tauri` 在最顶层，只做装配。

### 核心 Trait 边界

四个核心 trait 定义 crate 间边界：

- **Importer trait**：导入器接口，泛型 over 输入源类型
- **NodeProcessor trait**：节点处理器接口，`process` 同步返回 `BoxStream<NodeData>`
- **Repository<T> trait**：持久层泛型 CRUD，newtype `RepoId<T>` 防跨类型 ID 混用
- **Executor trait**：执行器接口，可 mock 注入

每个 trait 定义在 `lj-core`，实现分布在对应 crate——trait 不绑定实现细节，crate 之间仅通过 trait 通信。

### Crate 边界原则

**何时拆出独立 crate**：

- 编译时间隔离：修改后不影响其他 crate 的重新编译
- 关注点分离：不同类型的逻辑不应混在同一 crate
- API 稳定性：稳定公开 API 应独立于内部实现变化频繁的代码

**何时不拆**：

- 过早抽象：一个 crate 只有一个 struct → 合并回父 crate
- 依赖环：Cargo 禁止循环依赖
- 单文件 crate：< 200 行且无独立编译价值

### 编译时间优化

- 减少 crate 间的依赖链深度 → 更多 crate 可并行编译
- 稳定的 crate（如 `lj-core`）改动少 → 下游 crate 缓存命中率高
- 频繁改动的 crate（如 `lj-importer`）放在依赖树末端 → 重编译范围小
- `cargo check` 比 `cargo build` 快 2-3 倍——开发时用 check 验证类型

### 集成测试独立 workspace

`.integration-tests/` 是独立 workspace crate（非 `src-tauri` workspace 成员）：

- 测试依赖不影响主 crate 的 `Cargo.lock`
- 测试不阻塞主 crate 编译
- `.gitignore` 忽略整个目录，本地维护

## Why This Matters

10 crate 划分使 `lj-core`（纯类型）改动不会触发下游重编译，`src-tauri`（装配层）不含核心业务逻辑。trait 边界确保 crate 间松耦合。

## When to Apply

- 新增功能模块：评估是否需要独立 crate
- 优化编译时间：调整 crate 依赖深度
- 添加外部依赖：在 workspace 级别统一管理版本

## Related

- `docs/solutions/conventions/rust-coding.md` — Rust 编码规范
- `docs/solutions/design-patterns/rust-patterns.md` — Rust 设计模式
