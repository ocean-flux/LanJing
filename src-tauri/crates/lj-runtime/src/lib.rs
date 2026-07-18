//! 规则运行时引擎 crate。
//!
//! 负责规则图的执行调度、生命周期管理（RuntimeProfile 三档配置）、
//! 节点间数据流编排、并发控制与错误恢复。
//!
//! C1：临时托管旧 Graph 类型与 `NodeProcessor` seam；C3 起改为 Plan-only。

pub mod capability;
pub mod executor;
pub mod graph;
pub(crate) mod mapper;
pub(crate) mod mapper_fields;
pub mod node_data;
pub mod processor;
pub mod tap;
pub mod tracing_mod;

pub use capability::{check_capability, default_capabilities, merge};
pub use graph::{
    ConditionBranch, Edge, Graph, JsSpec, MapperOutputKind, MapperSpec, Node, NodeId, NodeKind,
    NodeSpec, SourceId, SubroutineId,
};
pub use node_data::{HttpResponse, NodeData, NodeDataVariant};
pub use processor::{ExecutionContext, NodeProcessor, SegmentSpec};
