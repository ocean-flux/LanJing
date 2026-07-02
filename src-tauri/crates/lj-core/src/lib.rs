//! 核心域模型 crate。
//!
//! 定义整个系统的标准化媒体数据模型(`Media`、`BookMedia`、`BookChapter` 等)，
//! 节点规格(`NodeSpec`、`NodeData`)、提取规则(`ExtractRule`)，
//! 以及沙箱、端点、图谱 schema 等基础类型。
//! 所有其他 crate 直接或间接依赖本 crate。

pub mod endpoint;
pub mod error;
pub mod extract_rule;
pub mod graph_schema;
pub mod media;
pub mod node;
pub mod node_data;
pub mod sandbox;
pub mod traits;

pub use endpoint::{EndpointKind, EndpointSpec, HttpMethod, HttpSpec};
pub use error::CoreError;
pub use extract_rule::{
    ExpectedDataType, ExtractRule, ExtractSpec, ExtractType, PlayUrlParserSpec, RegexClean,
};
pub use graph_schema::{ConditionBranch, EndpointTemplate, GraphSchema};
pub use media::{AudioMedia, BookChapter, BookMedia, Media, PlayLine, VideoEpisode, VideoMedia};
pub use node::{Edge, Graph, JsSpec, Node, NodeId, NodeKind, NodeSpec, SourceId, SubroutineId};
pub use node_data::{HttpResponse, NodeData, NodeDataVariant};
pub use sandbox::{Capability, CapabilityError, Sandbox, SystemCapabilities};
pub use traits::{
    CapabilityLoader, ExecutionContext, Executor, GraphValidator, ImportOutcome, ImportPreview,
    Importer, NodeProcessor, RepoId, Repository, SegmentSpec,
};
