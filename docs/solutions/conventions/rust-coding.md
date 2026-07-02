---
title: Rust 编码规范
date: '2026-07-01'
category: conventions
module: rust
problem_type: convention
component: development_workflow
severity: high
applies_when:
  - '编写 Rust 代码'
  - 'Code review'
  - '配置 clippy lint'
tags:
  - rust
  - coding-conventions
  - clippy
  - naming
---

# Rust 编码规范

LanJing 项目的 Rust 编码规范，融合 Rust API Guidelines、Clippy pedantic lints 及项目实践经验。

## Context

10 crate 项目需要一致的编码规范来保证代码质量和可维护性。

## Guidance

### 命名规范

**Crate 命名**：`snake_case`，优先单词而非缩写（`lj-core` 而非 `ljc`）

**类型命名**：Struct / Enum / Trait → `UpperCamelCase`；Type alias 同；Generic 参数单字母大写

**函数与方法**：

- 函数名：`snake_case`，动词开头
- 构造器：`new` / `from_*` / `with_*`
- 转换方法：`as_*`（借用）、`to_*`（拷贝/新建）、`into_*`（消耗 self）
- 布尔谓词：`is_*` / `has_*`
- getter：不推荐 `get_` 前缀，直接用名词（`spec()` 而非 `get_spec()`）

**变量与常量**：局部变量 `snake_case`，常量/static `SCREAMING_SNAKE_CASE`

### Clippy Pedantic 规则

workspace 配置 `clippy::all` + `clippy::pedantic` 全 deny。关键规则：

| 规则                      | 要求                                                    |
| ------------------------- | ------------------------------------------------------- |
| `cast_lossless`           | `as` 转换不丢失精度时用 `from()` / `into()`             |
| `cast_sign_loss`          | 禁止有符号到无符号的隐式转换                            |
| `doc_markdown`            | 文档中的标识符必须用反引号包裹                          |
| `if_not_else`             | 交换分支避免否定                                        |
| `items_after_statements`  | 所有定义在文件顶部                                      |
| `match_same_arms`         | 相同 match arm 合并                                     |
| `module_name_repetitions` | 模块名不要再出现在内部类型名中                          |
| `must_use_candidate`      | 无副作用的纯函数/返回 `Result` 的函数标记 `#[must_use]` |
| `needless_pass_by_value`  | 不需要所有权的参数用引用传递                            |
| `similar_names`           | 避免差异仅在下划线位置的变量名                          |
| `too_many_lines`          | 函数体不超过 100 行                                     |
| `unreadable_literal`      | 长数字字面量加下划线分隔                                |
| `unused_self`             | 方法不使用 `&self` → 改为关联函数                       |
| `wildcard_imports`        | 禁止 `use crate::*`                                     |

**项目特殊约束**：

- 单个 `.rs` 文件不超过 400 行
- 不允许用 `#[allow(clippy::xxx)]` 绕过 lint

### API 设计指南

- 优先 newtype 包装原始类型
- `Option<T>` 优于 sentinel value；`Result<T, E>` 优于 error code
- 闭集 enum（closed-set enum）：variants 固定，编译期穷尽匹配
- 优先 `pub(crate)` 而非 `pub`

### 文档

- 所有公开 API 必须有文档注释 `///`
- 文档语言：中文
- 示例代码应可编译

### 代码组织

```text
src/
  lib.rs        # crate 根：pub mod + re-export
  error.rs      # Error enum + Result alias
  types.rs      # 核心类型定义
  traits.rs     # trait 定义
  model/        # 领域模型
  service/      # 业务逻辑
  util.rs       # 内部辅助函数
```

import 顺序：std/core/alloc → 第三方 crate → 本 crate → 父模块，每组间空行分隔。

## Why This Matters

一致的编码规范降低 code review 认知负担，clippy pedantic 全 deny 确保自动检查。

## When to Apply

- 编写任何 Rust 代码时
- Code review 时检查规范合规性

## Related

- `docs/solutions/design-patterns/rust-patterns.md` — Rust 设计模式
- `docs/solutions/conventions/rust-async.md` — 异步模式
- `docs/solutions/conventions/rust-error-handling.md` — 错误处理
