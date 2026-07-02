//! 规则编译器 crate。
//!
//! 负责将 Legado 书源规则（JSON 格式）解析为内部 AST，
//! 并进行编译、验证与优化，产出可供运行时执行的指令序列。

pub mod error;
pub mod legado_parser;

// 骨架阶段，error 类型待 U3/U4 填充
