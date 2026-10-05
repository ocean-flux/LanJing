# ADR 0008：Rust workspace 的 crate 分层与单向依赖

- 状态：已采纳
- 日期：2026-10-05
- 影响范围：`src-tauri/Cargo.toml`、`src-tauri/crates/*/Cargo.toml`、
  `src-tauri/tests/workspace_layering.rs`（新增）

## 背景

`src-tauri` 是一个 14 个 crate 的 workspace。分层此前只存在于各 `Cargo.toml` 里：
没有文档写出「谁可以依赖谁」，也没有任何检查拦住新增的跨层依赖。要回答
「包边界是否清晰、有没有跨边界」，只能逐个读 `Cargo.toml` 手工拼图 —— 代价高，
而且拼出来的答案会随时过期。`src-tauri/src/**` 不绕过门面这件事同样只有约定。

## 决定

1. **依赖单向无环**，自上而下五层：

   | 层 | crate |
   | --- | --- |
   | 契约叶子 | `lj-capability`、`lj-storage-entity` |
   | 规则合同 | `lj-media`、`lj-rule-model`、`lj-compiler`、`lj-importer` |
   | 执行 | `lj-runtime`、`lj-node-extract`、`lj-node-http`、`lj-node-js` |
   | 持久化 | `lj-storage-migration`、`lj-storage` |
   | 门面 | `lj-rule-system`、`lanjing`（Tauri 二进制） |

   只允许向下的边；同层之间不互相依赖。

2. **应用层不绕过门面**：`src-tauri/src/**` 只依赖 `lj-rule-system`，不直接依赖
   `lj-storage` / `lj-runtime` / `lj-compiler` / `lj-importer`。前端同理只有
   `src/shared/tauri/**` 一个 `invoke()` 出口。

3. **允许的依赖边由 `src-tauri/tests/workspace_layering.rs` 的 `ALLOWED` 表定义**，
   该表与实际 `[dependencies]` 边**完全一致**，并断言整张图无环。因此新增或删除
   一条边都必须改这张表 —— 边界变更是一次显式动作，不会悄悄发生。

4. `lj-integration-tests` 是端到端收尾 crate，允许依赖全部层，在表里显式豁免；
   它的 `[dependencies]` 不代表产品依赖方向。

## 后果

- 这条不变量从「没人检查」变成「`cargo test --workspace` 必查」，也就进了 pre-push 门禁。
- 两条既有边值得单独记下来：它们不是笔误，但也谈不上理想 ——
  `lj-storage-migration -> lj-compiler`（编译器行为变化会牵动迁移）、
  `lj-storage -> lj-runtime`（持久化层依赖执行契约类型）。要改就连表与本节一起改。
- 前端侧的同类边界（feature 之间不互相 import）目前仍靠约定与 review，
  没有机器检查；要补就是这张表在前端的移植版。
