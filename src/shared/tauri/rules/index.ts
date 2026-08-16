//! 原生规则文档 IPC 边界。
//!
//! 这一层只做 Rust DTO 的 TS 镜像与 invoke wrapper，不持有编辑器状态、
//! 不解释布局语义。布局的结构由 `@/features/rules/model/core` 拥有。

export * from './wire';
