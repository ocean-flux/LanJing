//! 规则运行时引擎 crate。
//!
//! 负责规则图的执行调度、生命周期管理（RuntimeProfile 三档配置）、
//! 节点间数据流编排、并发控制与错误恢复。

pub mod executor;
pub(crate) mod mapper;
pub(crate) mod mapper_fields;
pub mod tap;
pub mod tracing_mod;
