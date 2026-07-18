//! 图执行器 — stream-to-stream 管道执行。
//!
//! 按标准意图执行：IPC 传 `intent`，executor 按 `Graph.intent_exports` 查 Flow 入口节点
//! 和 Mapper 输出节点，裁剪入口到 Mapper 的执行路径。执行器只按标准意图入口表运行，不猜来源端点。

use std::collections::{HashMap, HashSet, VecDeque};

use crate::graph::{Edge, Graph, Node, NodeId, NodeKind};
use crate::node_data::NodeData;
use crate::processor::{ExecutionContext, NodeProcessor, SegmentSpec};
use futures::stream::{self, BoxStream, StreamExt};
use lj_capability::{IntentInput, StandardIntent};
use tracing;
use uuid;

use crate::mapper::MapperContext;
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

    /// 子图裁剪：按 `intent` 查 `intent_exports` 表找入口和 Mapper 输出 → 收集执行路径。
    ///
    /// 1. 查 `graph.intent_exports` 获取 `IntentExport`。
    /// 2. 将 `flow_entry` 与 `mapper_output` 转成 `NodeId`。
    /// 3. 从入口沿有向 Edge BFS，找到第一条到 Mapper 输出的路径。
    /// 4. 按原图顺序返回路径节点列表。
    ///
    /// 若 `intent_exports` 中无该能力，返回空 Vec。
    #[must_use]
    pub fn select_subgraph<'g>(&self, graph: &'g Graph, intent: &StandardIntent) -> Vec<&'g Node> {
        // 1. 查 intent_exports 表获取入口声明
        let Some(entry) = graph.intent_exports.get(intent) else {
            return Vec::new();
        };

        // 构建 Uuid → &NodeId 查找表
        let uuid_to_node_id: HashMap<uuid::Uuid, &NodeId> = graph
            .nodes
            .iter()
            .map(|n| (n.node_id.0, &n.node_id))
            .collect();

        let Some(&flow_entry) = uuid_to_node_id.get(&entry.flow_entry) else {
            return Vec::new();
        };
        let Some(&mapper_output) = uuid_to_node_id.get(&entry.mapper_output) else {
            return Vec::new();
        };

        // 3. 从入口沿有向边查找到声明 Mapper 的路径，避免串入同连通分量的其他意图。
        let mut adj: HashMap<&NodeId, Vec<&NodeId>> = HashMap::new();
        for edge in &graph.edges {
            adj.entry(&edge.from).or_default().push(&edge.to);
        }

        let mut visited = HashSet::new();
        let mut queue = VecDeque::from([flow_entry]);
        let mut previous: HashMap<&NodeId, &NodeId> = HashMap::new();

        while let Some(id) = queue.pop_front() {
            if id == mapper_output {
                break;
            }
            if !visited.insert(id) {
                continue;
            }
            if let Some(neighbors) = adj.get(id) {
                for &next in neighbors {
                    if !visited.contains(next) && !previous.contains_key(next) {
                        previous.insert(next, id);
                        queue.push_back(next);
                    }
                }
            }
        }

        if flow_entry != mapper_output && !previous.contains_key(mapper_output) {
            return Vec::new();
        }

        let mut path_ids: HashSet<&NodeId> = HashSet::new();
        let mut current = mapper_output;
        path_ids.insert(current);
        while current != flow_entry {
            let Some(&prev) = previous.get(current) else {
                return Vec::new();
            };
            current = prev;
            path_ids.insert(current);
        }

        // 4. 按原图顺序返回选中节点
        graph
            .nodes
            .iter()
            .filter(|n| path_ids.contains(&n.node_id))
            .collect()
    }

    /// 拓扑排序（Kahn 算法）。
    ///
    /// 对选中节点按 from→to 边关系排序，确保每个节点在其下游之前处理。
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

        // Kahn：从入度 0 的节点开始
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
                    // next 已在入度表中（建表时确保），unwrap 安全
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

    /// 为 entry 节点创建输入 stream（按 `IntentInput` 注入）。
    ///
    /// - `Query(s)` → 单次 `NodeData::Raw(s)`
    /// - `ItemId(s)` → 单次 `NodeData::Raw(s)`
    /// - `UnitId` / `ActionId` / `Page` → 单次 `NodeData::Raw`
    /// - `Opaque(v)` → 单次 `NodeData::Json(v)`
    /// - `None` → 单次空 `Raw`（触发首个 processor 自产，如 Js 产分类列表）
    #[must_use]
    pub fn entry_input(segment: &SegmentSpec) -> BoxStream<'static, NodeData> {
        match &segment.input {
            IntentInput::Query(s) => {
                let val = s.clone();
                stream::once(async move { NodeData::Raw(val) }).boxed()
            }
            IntentInput::ItemId(s) => {
                let val = s.clone();
                stream::once(async move { NodeData::Raw(val) }).boxed()
            }
            IntentInput::UnitId(s) | IntentInput::ActionId(s) | IntentInput::Page(s) => {
                let val = s.clone();
                stream::once(async move { NodeData::Raw(val) }).boxed()
            }
            IntentInput::Opaque(v) => {
                let val = v.clone();
                stream::once(async move { NodeData::Json(val) }).boxed()
            }
            IntentInput::None => stream::once(async { NodeData::Raw(String::new()) }).boxed(),
        }
    }
}

impl Default for GraphExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphExecutor {
    /// 按段执行图。
    pub fn execute<'a>(
        &'a self,
        graph: &'a Graph,
        segment: &'a SegmentSpec,
        ctx: &'a ExecutionContext,
        processors: &'a HashMap<NodeKind, Box<dyn NodeProcessor>>,
    ) -> BoxStream<'a, (NodeId, NodeData)> {
        let nodes = self.select_subgraph(graph, &segment.intent);

        if nodes.is_empty() {
            tracing::warn!("按 {:?} 未匹配子图,返回空 stream", segment.intent);
            return futures::stream::empty::<(NodeId, NodeData)>().boxed();
        }

        // 拓扑排序
        let sorted = self.topological_sort(&nodes, &graph.edges);

        // 创建 entry 输入 stream
        let entry_stream: BoxStream<'static, NodeData> = Self::entry_input(segment);

        // 线性链，每个节点的 output 串联为下游 input，NodeData::Error 透传。
        // 消费 sorted 所有权：'a Node 直接来自 graph，不依赖局部 Vec 的生命周期
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

            if node.spec.kind == NodeKind::Mapper {
                let Some(mapper_spec) = node.spec.mapper.clone() else {
                    let node_kind = node.spec.kind.clone();
                    stream = futures::stream::once(async move {
                        NodeData::Error(format!("未找到 {node_kind:?} spec"))
                    })
                    .boxed();
                    last_node_id = Some(node.node_id.clone());
                    continue;
                };
                let mapper = MapperContext::new(graph, ctx);
                let intent = segment.intent;
                let input = segment.input.clone();
                stream = stream
                    .map(move |item| mapper.map_node_data(&mapper_spec, intent, &input, item))
                    .boxed();
                let node_id = node.node_id.clone();
                let trace_id = ctx.trace_id.clone();
                stream = tap_stream(node_id.clone(), stream, move |summary| {
                    tracing::info!(
                        trace_id = %trace_id,
                        node_id = %summary.node_id.0,
                        variant = %summary.variant,
                        summary = %summary.summary,
                        "节点输出"
                    );
                });
                last_node_id = Some(node_id);
                continue;
            }

            // 创建 tracing span
            let span = node_span(node, Some(&segment.intent), &ctx.trace_id);
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

            // 处理节点：上游 output → 下游 input
            stream = processor.process(ctx, &node.spec, stream);

            // tap wrapper：emits 日志，传真实 NodeId
            let node_id = node.node_id.clone();
            let trace_id = ctx.trace_id.clone();
            stream = tap_stream(node_id.clone(), stream, move |summary| {
                tracing::info!(
                    trace_id = %trace_id,
                    node_id = %summary.node_id.0,
                    variant = %summary.variant,
                    summary = %summary.summary,
                    "节点输出"
                );
            });
            last_node_id = Some(node_id);
        }

        // 线性链：最终 stream 的所有 item 来自最后一个节点。
        let node_id = last_node_id.unwrap_or_else(|| NodeId(uuid::Uuid::nil()));
        stream.map(move |item| (node_id.clone(), item)).boxed()
    }
}
