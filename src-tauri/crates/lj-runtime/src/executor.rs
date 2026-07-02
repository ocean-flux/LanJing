//! 图执行器 — stream-to-stream 管道执行(ADR-0022, ADR-0025)
//!
//! 按段执行:IPC 传 `endpoint_kind`,executor 按 `HttpSpec.endpoint_kind` 选 entry 节点
//! + 保留子图内部边,跨端点边因 target 不在选中集合被丢弃。

use std::collections::{HashMap, HashSet, VecDeque};

use futures::stream::{self, BoxStream, StreamExt};
use lj_core::endpoint::EndpointKind;
use lj_core::node::{Edge, Graph, Node, NodeId, NodeKind};
use lj_core::node_data::NodeData;
use lj_core::traits::{ExecutionContext, Executor, NodeProcessor, SegmentSpec};
use tracing;
use uuid;

use crate::tap::tap_stream;
use crate::tracing_mod::node_span;

/// 图执行器。
pub struct GraphExecutor;

impl GraphExecutor {
    /// 创建新的 `GraphExecutor`。
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// 子图裁剪:按 `endpoint_kind` 选 entry 节点 + BFS 收集可达节点。
    ///
    /// 1. 找 entry 节点:Http 节点(按 `HttpSpec.endpoint_kind`)或 Js 节点(按 `JsSpec.endpoint_kind`)。
    /// 2. 从 entry 出发,沿 Edge BFS 收集所有可达节点,跳过跨端点边。
    /// 3. 按原图顺序返回选中节点列表。
    #[must_use]
    pub fn select_subgraph<'g>(
        &self,
        graph: &'g Graph,
        endpoint_kind: &EndpointKind,
    ) -> Vec<&'g Node> {
        // 1. 找 entry 节点:Http 节点(按 HttpSpec.endpoint_kind) + Js 节点(按 JsSpec.endpoint_kind)
        let entry_ids: HashSet<&NodeId> = graph
            .nodes
            .iter()
            .filter(|n| {
                (n.spec.kind == NodeKind::Http
                    && n.spec
                        .http
                        .as_ref()
                        .is_some_and(|h| h.endpoint_kind == *endpoint_kind))
                    || (n.spec.kind == NodeKind::Js
                        && n.spec
                            .js
                            .as_ref()
                            .is_some_and(|j| j.endpoint_kind.as_ref() == Some(endpoint_kind)))
            })
            .map(|n| &n.node_id)
            .collect();

        if entry_ids.is_empty() {
            return Vec::new();
        }

        // 构建 NodeId → &Node 查找表(用于 BFS 时判断目标节点兼容性)
        let node_map: HashMap<&NodeId, &Node> =
            graph.nodes.iter().map(|n| (&n.node_id, n)).collect();

        // 2. 从 entry 出发,沿 Edge BFS 收集可达节点,跳过跨端点边
        //    ponytail: 预建邻接表避免 O(V×E),降到 O(V+E)
        let mut adj: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();
        for edge in &graph.edges {
            adj.entry(&edge.from).or_default().push(&edge.to);
        }

        let mut visited: HashSet<&NodeId> = HashSet::new();
        let mut queue: Vec<&NodeId> = entry_ids.into_iter().collect();

        while let Some(id) = queue.pop() {
            if visited.insert(id)
                && let Some(neighbors) = adj.get(id)
            {
                for &to_id in neighbors {
                    if !visited.contains(to_id) {
                        // 跳过跨端点边(如 search Extract→detail Http),
                        // 避免将其他端点的节点误纳入当前子图。
                        if let Some(target) = node_map.get(to_id)
                            && Self::is_same_endpoint(target, endpoint_kind)
                        {
                            queue.push(to_id);
                        }
                    }
                }
            }
        }

        // 3. 按原图顺序返回选中节点
        graph
            .nodes
            .iter()
            .filter(|n| visited.contains(&n.node_id))
            .collect()
    }

    /// 判断节点是否属于指定端点(Http/Js 节点按 `endpoint_kind` 匹配,其余总是兼容)。
    #[must_use]
    fn is_same_endpoint(node: &Node, endpoint_kind: &EndpointKind) -> bool {
        match &node.spec.kind {
            NodeKind::Http => node
                .spec
                .http
                .as_ref()
                .is_some_and(|h| &h.endpoint_kind == endpoint_kind),
            NodeKind::Js => {
                node.spec.js.as_ref().and_then(|j| j.endpoint_kind.as_ref()) == Some(endpoint_kind)
            }
            // Extract/Merge/Condition/Loop 没有独立端点,总是允许(从上游 Http 继承)
            _ => true,
        }
    }

    /// 拓扑排序(Kahn 算法)。
    ///
    /// 对选中节点按 from→to 边关系排序,确保每个节点在其下游之前处理。
    /// 首刀图是线性链(Http→Extract),简单实现即可。
    ///
    /// # Panics
    ///
    /// 不会 panic。
    #[must_use]
    pub fn topological_sort<'g>(&self, nodes: &[&'g Node], edges: &[Edge]) -> Vec<&'g Node> {
        let node_ids: HashSet<&NodeId> = nodes.iter().map(|n| &n.node_id).collect();

        // 只考虑选中节点间的边
        let mut in_degree: HashMap<&NodeId, usize> = HashMap::new();
        let mut adj: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();

        for node in nodes {
            in_degree.entry(&node.node_id).or_insert(0);
            adj.entry(&node.node_id).or_default();
        }

        for edge in edges {
            if node_ids.contains(&edge.from) && node_ids.contains(&edge.to) {
                adj.entry(&edge.from).or_default().push(&edge.to);
                *in_degree.entry(&edge.to).or_insert(0) += 1;
            }
        }

        // Kahn:从入度 0 的节点开始
        let mut queue: VecDeque<&NodeId> = VecDeque::new();
        for (&id, &deg) in &in_degree {
            if deg == 0 {
                queue.push_back(id);
            }
        }

        let id_to_node: HashMap<&NodeId, &Node> = nodes.iter().map(|n| (&n.node_id, *n)).collect();
        let mut sorted: Vec<&Node> = Vec::with_capacity(nodes.len());

        while let Some(id) = queue.pop_front() {
            if let Some(&node) = id_to_node.get(id) {
                sorted.push(node);
            }
            if let Some(neighbors) = adj.get(id) {
                for &next in neighbors {
                    // next 已在入度表中(建表时确保),unwrap 安全
                    if let Some(deg) = in_degree.get_mut(next) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(next);
                        }
                    }
                }
            }
        }

        sorted
    }

    /// 为 entry 节点创建输入 stream(按端点类型注入段参数)。
    ///
    /// - Search → `stream::once(NodeData::Raw(query))`
    /// - Discover → `stream::empty()`(Js 节点是源头,无输入)
    /// - Detail/Toc → `stream::once(NodeData::Raw(book_url))`
    /// - Content → `stream::once(NodeData::Raw(chapter_url))`
    #[must_use]
    pub fn entry_input(segment: &SegmentSpec) -> BoxStream<'static, NodeData> {
        match segment.endpoint_kind {
            EndpointKind::Search => {
                let query = segment.query.clone().unwrap_or_default();
                stream::once(async move { NodeData::Raw(query) }).boxed()
            }
            EndpointKind::Discover => {
                // Discover 两阶段:
                // - 无 query → 前端调 Js 节点获取分类列表,返回空 stream(由 Js 节点产出)
                // - 有 query → 前端传入分类 URL,注入为 Http 节点的 {{key}}
                if let Some(query) = &segment.query {
                    let q = query.clone();
                    stream::once(async move { NodeData::Raw(q) }).boxed()
                } else {
                    stream::empty().boxed()
                }
            }
            EndpointKind::Detail | EndpointKind::Toc => {
                // 视频 Detail 优先用 vod_id,fallback 到 book_url(KTD1)
                let val = segment
                    .vod_id
                    .clone()
                    .or_else(|| segment.book_url.clone())
                    .unwrap_or_default();
                stream::once(async move { NodeData::Raw(val) }).boxed()
            }
            EndpointKind::Content => {
                let chapter_url = segment.chapter_url.clone().unwrap_or_default();
                stream::once(async move { NodeData::Raw(chapter_url) }).boxed()
            }
        }
    }
}

impl Default for GraphExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl Executor for GraphExecutor {
    fn execute<'a>(
        &'a self,
        graph: &'a Graph,
        segment: SegmentSpec,
        ctx: &'a ExecutionContext,
        processors: &'a HashMap<NodeKind, Box<dyn NodeProcessor>>,
    ) -> BoxStream<'a, (NodeId, NodeData)> {
        let nodes = self.select_subgraph(graph, &segment.endpoint_kind);

        if nodes.is_empty() {
            tracing::warn!("按 {:?} 未匹配子图,返回空 stream", segment.endpoint_kind);
            return futures::stream::empty::<(NodeId, NodeData)>().boxed();
        }

        // 拓扑排序
        let sorted = self.topological_sort(&nodes, &graph.edges);

        // Discover 两阶段(Legado 图书源):有真实 Js 节点时按 query 分阶段。
        // - 无 query + 真实 Js → 阶段 1:只执行 Js(产出分类列表 JSON)
        // - 有 query → 阶段 2:跳过 Js,执行 Http→Extract
        // Maccms 视频源:Js 为空占位(schema 要求),无 query 时直接执行 Http→Extract,
        // 注入空 Raw 触发 Http({{pg}} 默认 1),不进阶段 1。
        let has_real_js = sorted.iter().any(|n| {
            matches!(n.spec.kind, NodeKind::Js)
                && n.spec.js.as_ref().is_some_and(|j| !j.code.is_empty())
        });
        let is_discover_phase1 = matches!(segment.endpoint_kind, EndpointKind::Discover)
            && segment.query.is_none()
            && has_real_js;

        // 创建 entry 输入 stream
        let entry_stream: BoxStream<'static, NodeData> =
            if matches!(segment.endpoint_kind, EndpointKind::Discover)
                && segment.query.is_none()
                && !has_real_js
            {
                // Maccms:无真实 Js,Http 为 entry,空 Raw 触发 Http
                stream::once(async { NodeData::Raw(String::new()) }).boxed()
            } else {
                Self::entry_input(&segment)
            };

        // 首刀图为线性链,不支持 fan-out。
        // 每个节点的 output 串联为下游 input,NodeData::Error 透传。
        // 消费 sorted 所有权:&'a Node 直接来自 graph,不依赖局部 Vec 的生命周期
        let mut stream: BoxStream<'a, NodeData> = entry_stream;
        let mut last_node_id: Option<NodeId> = None;

        for node in sorted {
            // 跳过控制流 stub
            if matches!(
                node.spec.kind,
                NodeKind::Merge | NodeKind::Condition | NodeKind::Loop
            ) {
                tracing::warn!(
                    "{}: {:?} 节点首刀未实现,跳过执行",
                    node.node_id.0,
                    node.spec.kind
                );
                continue;
            }

            // Discover 两阶段过滤:
            // - 阶段 1(无 query):只执行 Js
            // - 阶段 2(有 query):跳过 Js
            if is_discover_phase1 && node.spec.kind != NodeKind::Js {
                continue;
            }
            if !is_discover_phase1
                && matches!(segment.endpoint_kind, EndpointKind::Discover)
                && node.spec.kind == NodeKind::Js
            {
                continue;
            }

            // 创建 tracing span
            let span = node_span(node, Some(&segment.endpoint_kind), &ctx.trace_id);
            let _guard = span.enter();

            // 获取 processor
            let Some(processor) = processors.get(&node.spec.kind) else {
                tracing::error!("{}: 未找到 {:?} processor", node.node_id.0, node.spec.kind);
                stream = futures::stream::once(async move {
                    NodeData::Error(format!("未找到 {:?} processor", node.spec.kind))
                })
                .boxed();
                continue;
            };

            // 处理节点:上游 output → 下游 input
            stream = processor.process(ctx, &node.spec, stream);

            // tap wrapper:emits 日志,传真实 NodeId
            let node_id = node.node_id.clone();
            let trace_id = ctx.trace_id.clone();
            stream = tap_stream(stream, node_id.clone(), move |summary| {
                let trace_id = trace_id.clone();
                async move {
                    tracing::info!(
                        trace_id = %trace_id,
                        node_id = %summary.node_id,
                        variant = %summary.variant,
                        summary = %summary.summary,
                        "节点输出"
                    );
                }
            });
            last_node_id = Some(node_id);
        }

        // 线性链:最终 stream 的所有 item 来自最后一个节点
        let node_id = last_node_id.unwrap_or_else(|| NodeId(uuid::Uuid::nil()));
        stream.map(move |item| (node_id.clone(), item)).boxed()
    }
}
