//! trait 边界 — 核心抽象接口(ADR-0012, ADR-0022)。

use std::collections::HashMap;

use futures::stream::BoxStream;

use crate::endpoint::EndpointKind;
use crate::graph_schema::GraphSchema;
use crate::node::{Graph, NodeKind, NodeSpec, SourceId};
use crate::node_data::{NodeData, NodeDataVariant};
use crate::sandbox::Sandbox;

/// 导入器接口(泛型 over 输入类型)。
pub trait Importer<Opts>: Send + Sync {
    /// 解析规则，返回预览(不含真实源站访问)。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::Import` 当规则解析或翻译失败。
    fn import(&self, opts: Opts) -> Result<ImportPreview, crate::error::CoreError>;
}

/// 导入预览。
#[derive(Debug, Clone)]
pub struct ImportPreview {
    /// 源站 URL。
    pub source_url: String,
    /// 节点数量。
    pub node_count: usize,
    /// 边数量。
    pub edge_count: usize,
    /// JS 块数量。
    pub js_block_count: usize,
    /// 沙箱能力配置。
    pub sandbox: Sandbox,
    /// 所有 HTTP 目标 URL 模板(SSRF 审计)。
    pub http_target_urls: Vec<String>,
    /// JS 块源码(用户审计)。
    pub js_sources: Vec<String>,
    /// 生成的图。
    pub graph: Graph,
}

/// 导入结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportOutcome {
    /// 新建。
    New,
    /// 更新(stub 首刀只返 New)。
    Updated,
    /// 替换(stub 首刀不使用)。
    Replaced,
}

/// 节点处理器接口(ADR-0022 stream-to-stream)。
///
/// `process` 同步返回 `BoxStream`，非 `async_trait`。
/// 生命周期绑定 `self`/`ctx`/`spec`/`input`(借用语义)。
pub trait NodeProcessor: Send + Sync {
    /// 节点类型。
    fn kind(&self) -> NodeKind;
    /// 输入数据类型(静态声明，图构建时验证边类型匹配)。
    fn input_type(&self) -> Option<NodeDataVariant>;
    /// 输出数据类型。
    fn output_type(&self) -> NodeDataVariant;
    /// 处理节点:消费 input stream 产出 output stream。
    ///
    /// sync 返回 stream，管道内部异步流动。
    fn process<'a>(
        &'a self,
        ctx: &'a ExecutionContext,
        spec: &'a NodeSpec,
        input: BoxStream<'a, NodeData>,
    ) -> BoxStream<'a, NodeData>;
}

/// 执行上下文(节点执行时的共享状态)。
pub struct ExecutionContext {
    /// cookie jar(同源共享)。
    pub cookies: HashMap<String, String>,
    /// 沙箱能力。
    pub caps: Sandbox,
    /// tracing span 上下文。
    pub trace_id: String,
    /// 源站基础 URL(用于相对路径→绝对路径拼接)。
    pub base_url: String,
}

/// 执行器接口。
pub trait Executor: Send + Sync {
    /// 按段执行图(ADR-0025 前端控制分段)。
    ///
    /// `ctx` 提供执行上下文(`cookie`/`cap`/`trace_id`),由调用方(`Tauri` 命令)创建。
    fn execute<'a>(
        &'a self,
        graph: &'a Graph,
        segment: SegmentSpec,
        ctx: &'a ExecutionContext,
        processors: &'a HashMap<NodeKind, Box<dyn NodeProcessor>>,
    ) -> BoxStream<'a, (crate::node::NodeId, NodeData)>;
}

/// 段执行规格(ADR-0025, 2026-06-29 修订:新增 `vod_id` 支持视频详情段,KTD1)。
#[derive(Debug, Clone)]
pub struct SegmentSpec {
    /// 要执行的端点类型(子图裁剪:按 `HttpSpec.endpoint_kind` 选节点)。
    pub endpoint_kind: EndpointKind,
    /// 搜索关键词(search 段)。
    pub query: Option<String>,
    /// 选中的书 URL(detail/toc 段)。
    pub book_url: Option<String>,
    /// 选中的章节 URL(content 段)。
    pub chapter_url: Option<String>,
    /// 视频 `vod_id(视频` Detail 段,KTD1)。
    pub vod_id: Option<String>,
}

/// 持久层 CRUD 泛型(ADR-0012)。
pub trait Repository<T>: Send + Sync {
    /// 根据 ID 获取实体。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::Storage` 当数据库查询失败。
    fn get(&self, id: &RepoId<T>) -> Result<Option<T>, crate::error::CoreError>;
    /// 保存实体。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::Storage` 当数据库写入失败。
    fn save(&self, id: &RepoId<T>, value: &T) -> Result<(), crate::error::CoreError>;
    /// 删除实体。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::Storage` 当数据库删除失败。
    fn delete(&self, id: &RepoId<T>) -> Result<(), crate::error::CoreError>;
    /// 列出所有实体。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::Storage` 当数据库查询失败。
    fn list(&self) -> Result<Vec<(RepoId<T>, T)>, crate::error::CoreError>;
}

/// 类型隔断 ID(防 ID 跨类型混用)。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepoId<T> {
    /// ID 字符串。
    pub id: String,
    /// PhantomData 标记。
    #[doc(hidden)]
    pub _marker: std::marker::PhantomData<T>,
}

impl<T> RepoId<T> {
    /// 创建新的 `RepoId`。
    #[must_use]
    pub fn new(id: String) -> Self {
        Self {
            id,
            _marker: std::marker::PhantomData,
        }
    }
}

/// 能力装载器。
pub trait CapabilityLoader: Send + Sync {
    /// 根据源 ID 加载沙箱能力。
    fn load_capabilities(&self, source: &SourceId) -> Sandbox;
}

/// 图 schema 验证(导入器调用)。
pub trait GraphValidator: Send + Sync {
    /// 验证图结构是否符合 schema。
    ///
    /// # Errors
    ///
    /// 返回 `CoreError::GraphValidation` 当图结构不符合模板要求。
    fn validate(&self, graph: &Graph, schema: &GraphSchema) -> Result<(), crate::error::CoreError>;
}
