---
title: Tauri 后端与 IPC 模式
date: '2026-07-01'
category: architecture-patterns
module: src-tauri
problem_type: architecture_pattern
component: development_workflow
severity: medium
applies_when:
  - '新增 Tauri command'
  - '修改前后端通信协议'
  - '调试 IPC 问题'
tags:
  - tauri
  - ipc
  - command
  - event
---

# Tauri 后端与 IPC 模式

LanJing 基于 Tauri v2 的桌面应用后端模式：command 设计、State 管理、IPC 通道、事件系统。

## Context

Tauri v2 应用的前后端通信需要明确的 IPC 契约。Rust 后端通过 command 响应前端请求，通过 event 向前端推送流式数据。

## Guidance

### Tauri Command 设计

```rust
#[tauri::command]
fn execute_rule(app: tauri::AppHandle, rule_id: String) -> anyhow::Result<Vec<Media>> {
    let state = app.state::<AppState>();
    let repo = state.media_repo.lock().unwrap();
    Ok(results)
}
```

- 返回值：`Result<T, String>` 或 `anyhow::Result<T>`（IPC 边界自动转 String）
- 参数：基本类型自动反序列化，复杂类型定义 `#[derive(Deserialize)]` struct
- 特殊注入：`tauri::AppHandle`、`tauri::Window`、`tauri::State<T>` 运行时自动注入

### State 管理

```rust
struct AppState {
    config: Config,
    rules_repo: Arc<RwLock<dyn Repository<Graph>>>,
    media_repo: Arc<RwLock<dyn Repository<Media>>>,
}
```

- `tauri::State<'_, T>`：command 参数中注入，只读引用
- 可变状态：`Arc<RwLock<T>>` 或 `Arc<Mutex<T>>` 包裹
- 避免在 command 中长时间持有 Mutex 锁——Tauri command 在主线程执行

### IPC 通道

| 方向              | 机制                           | 场景                       |
| ----------------- | ------------------------------ | -------------------------- |
| 前端→后端         | `invoke` + `#[tauri::command]` | 用户操作触发（导入、执行） |
| 后端→前端（单次） | command 返回值                 | 请求-响应模式              |
| 后端→前端（持续） | Tauri event                    | 流式媒体结果、tracing span |

```typescript
// 前端 invoke
const media = await invoke<Media[]>('execute_rule', { ruleId: 'xxx' });

// 前端 listen event
const unlisten = await listen<MediaEvent>('media-stream', (event) => {
  console.log('收到媒体数据:', event.payload);
});
```

### Event Payload Schema 稳定性

Tauri event 的 payload 需要稳定的消息契约：

- 每个 event 类型定义对应的 Rust struct，加 `#[derive(Serialize)]`
- 前端 TypeScript 侧定义对应的 discriminated union type
- `#[serde(tag = "kind")]` 保证 Rust enum ↔ TypeScript discriminated union 的 JSON 结构一致性

### Error Serialization

IPC 边界上 `anyhow::Error` 被映射为 `String`（通过 `.to_string()`）供前端展示。前端收到 String 格式的错误消息，不再尝试解析结构化错误。

### 设计原则

- **单向推送**：Rust → 前端（event-based），前端 → Rust（command + invoke）
- **稳定 schema**：event payload 经 serde 序列化，前后端类型一一对应
- **错误扁平化**：IPC 层面只传 String
- **零轮询**：所有数据推送基于 Tauri event，前端不需要定时器轮询后端状态

## Why This Matters

稳定的 IPC schema 是前后端编译期类型安全的基础——不在运行时靠字符串匹配判断消息类型。流式 event 推送使前端可以渐进式渲染媒体结果。

## When to Apply

- 新增 Tauri command：定义 command 函数 + 前端 invoke 调用
- 新增 event 类型：定义 Rust struct + TypeScript type + `#[serde(tag = "kind")]`
- 调试 IPC 问题：检查 event payload schema 一致性

## Related

- `docs/solutions/conventions/rust-coding.md` — Rust 编码规范
- `docs/solutions/design-patterns/tracing.md` — tracing span 通过 IPC 推前端
