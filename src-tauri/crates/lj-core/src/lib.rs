//! 核心域模型 crate。
//!
//! 定义标准媒体资源图、节点规格、提取规则和沙箱等核心类型。
//! 所有其他 crate 直接或间接依赖本 crate。

pub mod endpoint;
pub mod error;
pub mod extract_rule;
pub mod mapper_vocab;
pub mod media;
pub mod node;
pub mod node_data;
pub mod sandbox;
pub mod traits;

pub use endpoint::{HttpMethod, HttpSpec};
pub use error::CoreError;
pub use extract_rule::{
    ExpectedDataType, ExtractRule, ExtractSpec, ExtractType, OutputTarget, RegexClean,
};
pub use media::{
    MediaAction, MediaAsset, MediaAssetKind, MediaAssetLocator, MediaCollection, MediaGraphDelta,
    MediaItem, MediaKind, MediaRelation, MediaRelationKind, MediaResourceId, MediaUnit,
    PresentationHint, ResourceCompleteness, SourceProfile,
};
pub use node::{
    ConditionBranch, Edge, Graph, JsSpec, MapperOutputKind, MapperSpec, Node, NodeId, NodeKind,
    NodeSpec, SourceId, SubroutineId,
};
pub use node_data::{HttpResponse, NodeData, NodeDataVariant};
pub use sandbox::{Capability as SandboxCapability, CapabilityError, Sandbox, SystemCapabilities};
pub use traits::{
    ExecutionContext, Executor, GraphValidator, ImportOutcome, ImportPreview, Importer,
    NodeProcessor, RepoId, Repository, SandboxLoader, SegmentSpec,
};
