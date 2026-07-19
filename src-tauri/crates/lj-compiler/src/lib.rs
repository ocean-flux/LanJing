//! 规则 Definition compiler crate。
//!
//! 只负责作者合同的规范化、校验、诊断与 immutable `ExecutionPlan` 编译；
//! 来源专有 Legado 语法只由 importer adapter 解析，不能进入 compiler 或 runtime。

pub mod compiler;
pub mod error;

pub use compiler::{Compiler, DEFAULT_COMPILER_VERSION, canonicalize, validate};
pub use error::CompilerError;
