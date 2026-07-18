//! 图结构校验 — 三层校验：结构 + I/O + 标准意图契约。

use std::collections::HashSet;

use lj_rule_model::Error;
use lj_runtime::NodeDataVariant;
use lj_runtime::{Graph, MapperOutputKind, Node, NodeId, NodeKind};

/// 验证图结构是否合法（三层校验）。
///
/// 校验：
/// 1. **结构校验** — 所有边 from/to 节点存在；无子例程递归引用。
/// 2. **I/O 校验** — 边类型匹配（`from.output_type` == `to.input_type`）。
/// 3. **契约校验** — `intent_exports` 声明的 Flow 入口和 Mapper 输出节点存在且可达。
///
/// # Errors
///
/// 返回 `Error::GraphValidation` 当图结构不合法。
pub fn validate_graph(graph: &Graph) -> Result<(), Error> {
    // ===== 1. 结构校验 =====
    validate_structure(graph)?;

    // ===== 2. I/O 校验 =====
    validate_io(graph)?;

    // ===== 3. 能力契约校验 =====
    validate_contract(graph)?;

    Ok(())
}

/// 结构校验：边目标节点存在 + 子例程无递归引用。
fn validate_structure(graph: &Graph) -> Result<(), Error> {
    // 构建节点 ID 集合，O(1) 查找
    let node_ids: HashSet<&NodeId> = graph.nodes.iter().map(|n| &n.node_id).collect();

    // 边 from/to 节点必须存在
    for edge in &graph.edges {
        if !node_ids.contains(&edge.from) {
            return Err(Error::GraphValidation(format!(
                "边 from 节点 {} 不存在",
                edge.from.0
            )));
        }
        if !node_ids.contains(&edge.to) {
            return Err(Error::GraphValidation(format!(
                "边 to 节点 {} 不存在",
                edge.to.0
            )));
        }
    }

    // 子例程递归引用检查（简单：子图不能含与父同 ID 的子例程）
    for (sub_id, sub_graph) in &graph.subroutines {
        if sub_graph.subroutines.contains_key(sub_id) {
            return Err(Error::GraphValidation("子例程不能递归引用自身".to_string()));
        }
    }

    Ok(())
}

/// I/O 校验：边类型匹配（`from.output_type` == `to.input_type`）。
fn validate_io(graph: &Graph) -> Result<(), Error> {
    for edge in &graph.edges {
        // 结构校验已保证节点存在，这里安全 unwrap 不可能触发；
        // 但为防御性编程仍用 find + ok_or_else
        let from_node = graph
            .nodes
            .iter()
            .find(|n| n.node_id == edge.from)
            .ok_or_else(|| {
                Error::GraphValidation(format!("边 from 节点 {} 不存在", edge.from.0))
            })?;
        let to_node = graph
            .nodes
            .iter()
            .find(|n| n.node_id == edge.to)
            .ok_or_else(|| Error::GraphValidation(format!("边 to 节点 {} 不存在", edge.to.0)))?;

        check_edge_type(from_node, to_node)?;
    }

    Ok(())
}

/// 标准意图契约校验：`intent_exports` 声明的 Flow 入口和 Mapper 输出节点存在且可达。
fn validate_contract(graph: &Graph) -> Result<(), Error> {
    let node_ids: HashSet<&NodeId> = graph.nodes.iter().map(|n| &n.node_id).collect();

    for (intent, entry) in &graph.intent_exports {
        let flow_entry = NodeId(entry.flow_entry);
        if !node_ids.contains(&flow_entry) {
            return Err(Error::GraphValidation(format!(
                "标准意图 {:?} 的 Flow 入口节点 {} 不存在于图中",
                intent, entry.flow_entry
            )));
        }

        let mapper_output = NodeId(entry.mapper_output);
        let Some(mapper_node) = graph
            .nodes
            .iter()
            .find(|node| node.node_id == mapper_output)
        else {
            return Err(Error::GraphValidation(format!(
                "标准意图 {:?} 的 Mapper 输出节点 {} 不存在于图中",
                intent, entry.mapper_output
            )));
        };
        validate_mapper_spec(*intent, mapper_node)?;

        let reachable = bfs_reachable(graph, &flow_entry);
        if !reachable.contains(&mapper_output) {
            return Err(Error::GraphValidation(format!(
                "标准意图 {:?} 的 Mapper 输出节点 {} 无法从 Flow 入口 {} 到达",
                intent, entry.mapper_output, entry.flow_entry
            )));
        }
    }

    Ok(())
}

fn validate_mapper_spec(
    intent: lj_capability::StandardIntent,
    mapper_node: &Node,
) -> Result<(), Error> {
    if mapper_node.spec.kind != NodeKind::Mapper {
        return Err(Error::GraphValidation(format!(
            "标准意图 {:?} 的 Mapper 输出节点 {} 不是 Mapper 节点",
            intent, mapper_node.node_id.0
        )));
    }
    let Some(spec) = &mapper_node.spec.mapper else {
        return Err(Error::GraphValidation(format!(
            "标准意图 {:?} 的 Mapper 输出节点 {} 缺少 Mapper spec",
            intent, mapper_node.node_id.0
        )));
    };
    if spec.identity_fields.is_empty()
        || spec
            .identity_fields
            .iter()
            .any(|field| field.trim().is_empty())
    {
        return Err(Error::GraphValidation(format!(
            "标准意图 {:?} 的 Mapper 输出节点 {} 缺少稳定资源 ID 字段",
            intent, mapper_node.node_id.0
        )));
    }
    let expected = match intent {
        lj_capability::StandardIntent::Search | lj_capability::StandardIntent::ResolveItem => {
            MapperOutputKind::Items
        }
        lj_capability::StandardIntent::Discover | lj_capability::StandardIntent::ContinueAction => {
            MapperOutputKind::Discovery
        }
        lj_capability::StandardIntent::ListUnits => MapperOutputKind::Units,
        lj_capability::StandardIntent::ResolveAsset => MapperOutputKind::Assets,
    };
    if spec.output != expected {
        return Err(Error::GraphValidation(format!(
            "标准意图 {:?} 的 Mapper 输出类型 {:?} 不匹配，期望 {:?}",
            intent, spec.output, expected
        )));
    }
    Ok(())
}

/// 从指定节点 BFS 收集所有可达节点 ID（含起始节点自身）。
fn bfs_reachable(graph: &Graph, start: &NodeId) -> HashSet<NodeId> {
    let mut visited = HashSet::new();
    let mut queue = std::collections::VecDeque::new();

    visited.insert(start.clone());
    queue.push_back(start.clone());

    while let Some(current) = queue.pop_front() {
        for edge in &graph.edges {
            if edge.from == current && !visited.contains(&edge.to) {
                visited.insert(edge.to.clone());
                queue.push_back(edge.to.clone());
            }
        }
    }

    visited
}

/// 检查边类型匹配：`from.output_type` == `to.input_type`。
fn check_edge_type(from_node: &Node, to_node: &Node) -> Result<(), Error> {
    let (_, from_output) = node_kind_io(&from_node.spec.kind);
    let (to_input, _) = node_kind_io(&to_node.spec.kind);

    // to_input == None 表示不限制输入（源头节点或通配 stub）
    if let Some(to_input) = to_input
        && let Some(from_output) = from_output
        && from_output != to_input
    {
        return Err(Error::GraphValidation(format!(
            "边类型不匹配: {:?}(output={:?}) → {:?}(input={:?})",
            from_node.spec.kind, from_output, to_node.spec.kind, to_input,
        )));
    }

    Ok(())
}

/// 获取 `NodeKind` 的静态输入/输出类型映射。
fn node_kind_io(kind: &NodeKind) -> (Option<NodeDataVariant>, Option<NodeDataVariant>) {
    match kind {
        NodeKind::Http => (None, Some(NodeDataVariant::HttpResponse)),
        NodeKind::Js => (None, Some(NodeDataVariant::Json)),
        NodeKind::Extract => (
            Some(NodeDataVariant::HttpResponse),
            Some(NodeDataVariant::Json),
        ),
        NodeKind::Mapper => (Some(NodeDataVariant::Json), Some(NodeDataVariant::Delta)),
        // Merge/Condition/Loop：任意 → 任意（stub，未钉 IO）
        NodeKind::Merge | NodeKind::Condition | NodeKind::Loop => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lj_capability::{IntentExport, StandardIntent};
    use lj_rule_model::{ExpectedDataType, ExtractSpec};
    use lj_rule_model::{HttpMethod, HttpSpec};
    use lj_runtime::{Edge, MapperSpec, NodeKind, NodeSpec, SourceId, SubroutineId};
    use std::collections::HashMap;
    use uuid::Uuid;

    fn make_node_id() -> NodeId {
        NodeId(Uuid::new_v4())
    }

    fn make_http_node(id: NodeId) -> Node {
        Node {
            node_id: id,
            import_hash: "a".repeat(64),
            spec: NodeSpec {
                kind: NodeKind::Http,
                http: Some(HttpSpec {
                    method: HttpMethod::Get,
                    url: "https://example.com".to_string(),
                    headers: HashMap::new(),
                    body: None,
                    charset: None,
                    expected_type: ExpectedDataType::Html,
                }),
                js: None,
                extract: None,
                mapper: None,
            },
        }
    }

    fn make_extract_node(id: NodeId) -> Node {
        Node {
            node_id: id,
            import_hash: "b".repeat(64),
            spec: NodeSpec {
                kind: NodeKind::Extract,
                http: None,
                js: None,
                extract: Some(ExtractSpec {
                    rules: vec![],
                    field_rules: HashMap::new(),
                    expected_type: ExpectedDataType::Html,
                    output_target: lj_rule_model::OutputTarget::Media,
                }),
                mapper: None,
            },
        }
    }

    fn make_js_node(id: NodeId) -> Node {
        Node {
            node_id: id,
            import_hash: "c".repeat(64),
            spec: NodeSpec {
                kind: NodeKind::Js,
                http: None,
                js: Some(lj_runtime::JsSpec {
                    code: "var x = 1;".to_string(),
                }),
                extract: None,
                mapper: None,
            },
        }
    }

    fn make_mapper_node(id: NodeId, output: MapperOutputKind) -> Node {
        Node {
            node_id: id,
            import_hash: "m".repeat(64),
            spec: NodeSpec {
                kind: NodeKind::Mapper,
                http: None,
                js: None,
                extract: None,
                mapper: Some(MapperSpec {
                    output,
                    identity_fields: vec!["id".to_string()],
                }),
            },
        }
    }

    fn empty_graph() -> Graph {
        Graph {
            nodes: vec![],
            edges: vec![],
            subroutines: HashMap::new(),
            source_id: SourceId(Uuid::new_v4()),
            base_url: String::new(),
            intent_exports: HashMap::new(),
        }
    }

    // ===== 结构校验 =====

    #[test]
    fn empty_graph_passes() {
        assert!(validate_graph(&empty_graph()).is_ok());
    }

    #[test]
    fn valid_graph_passes() {
        let http_id = make_node_id();
        let extract_id = make_node_id();
        let graph = Graph {
            nodes: vec![
                make_http_node(http_id.clone()),
                make_extract_node(extract_id.clone()),
            ],
            edges: vec![Edge {
                from: http_id,
                to: extract_id,
                condition_branch: None,
            }],
            ..empty_graph()
        };
        assert!(validate_graph(&graph).is_ok());
    }

    #[test]
    fn edge_from_not_found_errors() {
        let missing = make_node_id();
        let extract_id = make_node_id();
        let graph = Graph {
            nodes: vec![make_extract_node(extract_id.clone())],
            edges: vec![Edge {
                from: missing,
                to: extract_id,
                condition_branch: None,
            }],
            ..empty_graph()
        };
        let result = validate_graph(&graph);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("from"));
    }

    #[test]
    fn edge_to_not_found_errors() {
        let http_id = make_node_id();
        let missing = make_node_id();
        let graph = Graph {
            nodes: vec![make_http_node(http_id.clone())],
            edges: vec![Edge {
                from: http_id,
                to: missing,
                condition_branch: None,
            }],
            ..empty_graph()
        };
        let result = validate_graph(&graph);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("to"));
    }

    #[test]
    fn subroutine_self_reference_errors() {
        let sub_id = SubroutineId(Uuid::new_v4());
        let inner_subs = {
            let mut m = HashMap::new();
            m.insert(
                sub_id.clone(),
                Graph {
                    nodes: vec![],
                    edges: vec![],
                    subroutines: HashMap::new(),
                    source_id: SourceId(Uuid::new_v4()),
                    base_url: String::new(),
                    intent_exports: HashMap::new(),
                },
            );
            m
        };
        let outer = Graph {
            nodes: vec![],
            edges: vec![],
            subroutines: {
                let mut m = HashMap::new();
                m.insert(
                    sub_id,
                    Graph {
                        nodes: vec![],
                        edges: vec![],
                        subroutines: inner_subs,
                        source_id: SourceId(Uuid::new_v4()),
                        base_url: String::new(),
                        intent_exports: HashMap::new(),
                    },
                );
                m
            },
            source_id: SourceId(Uuid::new_v4()),
            base_url: String::new(),
            intent_exports: HashMap::new(),
        };
        assert!(validate_graph(&outer).is_err());
        assert!(
            validate_graph(&outer)
                .unwrap_err()
                .to_string()
                .contains("递归")
        );
    }

    // ===== I/O 校验 =====

    #[test]
    fn edge_type_match_passes() {
        // Http(output=HttpResponse) → Extract(input=HttpResponse): 匹配
        let http_id = make_node_id();
        let extract_id = make_node_id();
        let graph = Graph {
            nodes: vec![
                make_http_node(http_id.clone()),
                make_extract_node(extract_id.clone()),
            ],
            edges: vec![Edge {
                from: http_id,
                to: extract_id,
                condition_branch: None,
            }],
            ..empty_graph()
        };
        assert!(validate_graph(&graph).is_ok());
    }

    #[test]
    fn edge_type_mismatch_errors() {
        // Js(output=Json) → Extract(input=HttpResponse): 不匹配
        let js_id = make_node_id();
        let extract_id = make_node_id();
        let graph = Graph {
            nodes: vec![
                make_js_node(js_id.clone()),
                make_extract_node(extract_id.clone()),
            ],
            edges: vec![Edge {
                from: js_id,
                to: extract_id,
                condition_branch: None,
            }],
            ..empty_graph()
        };
        let result = validate_graph(&graph);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("类型不匹配"));
    }

    #[test]
    fn merge_condition_loop_no_io_constraint() {
        // Merge/Condition/Loop: (None, None) — 不限制 I/O，任意边都通过
        let http_id = make_node_id();
        let merge_id = make_node_id();
        let graph = Graph {
            nodes: vec![
                make_http_node(http_id.clone()),
                Node {
                    node_id: merge_id.clone(),
                    import_hash: "d".repeat(64),
                    spec: NodeSpec {
                        kind: NodeKind::Merge,
                        http: None,
                        js: None,
                        extract: None,
                        mapper: None,
                    },
                },
            ],
            edges: vec![Edge {
                from: http_id,
                to: merge_id,
                condition_branch: None,
            }],
            ..empty_graph()
        };
        assert!(validate_graph(&graph).is_ok());
    }

    // ===== 标准意图契约校验 =====

    #[test]
    fn empty_intent_exports_passes() {
        // 源可不声明任何能力（结构合法，虽然无意义）
        let http_id = make_node_id();
        let graph = Graph {
            nodes: vec![make_http_node(http_id)],
            edges: vec![],
            ..empty_graph()
        };
        assert!(validate_graph(&graph).is_ok());
    }

    #[test]
    fn entry_point_main_valid_passes() {
        let http_id = make_node_id();
        let extract_id = make_node_id();
        let mapper_id = make_node_id();
        let mut intent_exports = HashMap::new();
        intent_exports.insert(
            StandardIntent::Search,
            IntentExport::new(http_id.0, mapper_id.0),
        );
        let graph = Graph {
            nodes: vec![
                make_http_node(http_id.clone()),
                make_extract_node(extract_id.clone()),
                make_mapper_node(mapper_id.clone(), MapperOutputKind::Items),
            ],
            edges: vec![
                Edge {
                    from: http_id,
                    to: extract_id.clone(),
                    condition_branch: None,
                },
                Edge {
                    from: extract_id,
                    to: mapper_id,
                    condition_branch: None,
                },
            ],
            intent_exports,
            ..empty_graph()
        };
        assert!(validate_graph(&graph).is_ok());
    }

    #[test]
    fn entry_point_main_not_in_graph_errors() {
        let http_id = make_node_id();
        let missing_uuid = Uuid::new_v4();
        let mut intent_exports = HashMap::new();
        intent_exports.insert(
            StandardIntent::Search,
            IntentExport::new(missing_uuid, missing_uuid),
        );
        let graph = Graph {
            nodes: vec![make_http_node(http_id)],
            edges: vec![],
            intent_exports,
            ..empty_graph()
        };
        let result = validate_graph(&graph);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Flow 入口"));
    }

    #[test]
    fn entry_point_mapper_output_not_in_graph_errors() {
        let http_id = make_node_id();
        let missing_mapper = Uuid::new_v4();
        let mut intent_exports = HashMap::new();
        intent_exports.insert(
            StandardIntent::Discover,
            IntentExport::new(http_id.0, missing_mapper),
        );
        let graph = Graph {
            nodes: vec![make_http_node(http_id)],
            edges: vec![],
            intent_exports,
            ..empty_graph()
        };
        let result = validate_graph(&graph);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Mapper 输出"));
    }

    #[test]
    fn entry_point_mapper_output_not_reachable_errors() {
        let http_id = make_node_id();
        let mapper_id = make_node_id();
        let mut intent_exports = HashMap::new();
        intent_exports.insert(
            StandardIntent::Search,
            IntentExport::new(http_id.0, mapper_id.0),
        );
        let graph = Graph {
            nodes: vec![
                make_http_node(http_id),
                make_mapper_node(mapper_id, MapperOutputKind::Items),
            ],
            edges: vec![],
            intent_exports,
            ..empty_graph()
        };
        let result = validate_graph(&graph);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("无法从 Flow 入口"));
    }

    #[test]
    fn mapper_without_identity_fields_errors() {
        let http_id = make_node_id();
        let extract_id = make_node_id();
        let mapper_id = make_node_id();
        let mut mapper = make_mapper_node(mapper_id.clone(), MapperOutputKind::Items);
        mapper.spec.mapper.as_mut().unwrap().identity_fields.clear();
        let mut intent_exports = HashMap::new();
        intent_exports.insert(
            StandardIntent::Search,
            IntentExport::new(http_id.0, mapper_id.0),
        );
        let graph = Graph {
            nodes: vec![
                make_http_node(http_id.clone()),
                make_extract_node(extract_id.clone()),
                mapper,
            ],
            edges: vec![
                Edge {
                    from: http_id,
                    to: extract_id.clone(),
                    condition_branch: None,
                },
                Edge {
                    from: extract_id,
                    to: mapper_id,
                    condition_branch: None,
                },
            ],
            intent_exports,
            ..empty_graph()
        };
        let result = validate_graph(&graph);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("稳定资源 ID"));
    }

    #[test]
    fn entry_point_with_valid_flow_and_mapper_passes() {
        let js_id = make_node_id();
        let mapper_id = make_node_id();
        let mut intent_exports = HashMap::new();
        intent_exports.insert(
            StandardIntent::Discover,
            IntentExport::new(js_id.0, mapper_id.0),
        );
        let graph = Graph {
            nodes: vec![
                make_js_node(js_id.clone()),
                make_mapper_node(mapper_id.clone(), MapperOutputKind::Discovery),
            ],
            edges: vec![Edge {
                from: js_id,
                to: mapper_id,
                condition_branch: None,
            }],
            intent_exports,
            ..empty_graph()
        };
        assert!(validate_graph(&graph).is_ok());
    }

    #[test]
    fn maccms_style_fork_passes() {
        let http_id = make_node_id();
        let extract_id = make_node_id();
        let item_mapper = make_node_id();
        let unit_mapper = make_node_id();
        let mut intent_exports = HashMap::new();
        intent_exports.insert(
            StandardIntent::ResolveItem,
            IntentExport::new(http_id.0, item_mapper.0),
        );
        intent_exports.insert(
            StandardIntent::ListUnits,
            IntentExport::new(http_id.0, unit_mapper.0),
        );
        let graph = Graph {
            nodes: vec![
                make_http_node(http_id.clone()),
                make_extract_node(extract_id.clone()),
                make_mapper_node(item_mapper.clone(), MapperOutputKind::Items),
                make_mapper_node(unit_mapper.clone(), MapperOutputKind::Units),
            ],
            edges: vec![
                Edge {
                    from: http_id,
                    to: extract_id.clone(),
                    condition_branch: None,
                },
                Edge {
                    from: extract_id.clone(),
                    to: item_mapper,
                    condition_branch: None,
                },
                Edge {
                    from: extract_id,
                    to: unit_mapper,
                    condition_branch: None,
                },
            ],
            intent_exports,
            ..empty_graph()
        };
        assert!(validate_graph(&graph).is_ok());
    }

    #[test]
    fn multi_capability_all_valid_passes() {
        let search_http = make_node_id();
        let search_ext = make_node_id();
        let search_mapper = make_node_id();
        let detail_http = make_node_id();
        let detail_ext = make_node_id();
        let detail_mapper = make_node_id();
        let mut intent_exports = HashMap::new();
        intent_exports.insert(
            StandardIntent::Search,
            IntentExport::new(search_http.0, search_mapper.0),
        );
        intent_exports.insert(
            StandardIntent::ResolveItem,
            IntentExport::new(detail_http.0, detail_mapper.0),
        );
        let graph = Graph {
            nodes: vec![
                make_http_node(search_http.clone()),
                make_extract_node(search_ext.clone()),
                make_mapper_node(search_mapper.clone(), MapperOutputKind::Items),
                make_http_node(detail_http.clone()),
                make_extract_node(detail_ext.clone()),
                make_mapper_node(detail_mapper.clone(), MapperOutputKind::Items),
            ],
            edges: vec![
                Edge {
                    from: search_http,
                    to: search_ext.clone(),
                    condition_branch: None,
                },
                Edge {
                    from: search_ext,
                    to: search_mapper,
                    condition_branch: None,
                },
                Edge {
                    from: detail_http,
                    to: detail_ext.clone(),
                    condition_branch: None,
                },
                Edge {
                    from: detail_ext,
                    to: detail_mapper,
                    condition_branch: None,
                },
            ],
            intent_exports,
            ..empty_graph()
        };
        assert!(validate_graph(&graph).is_ok());
    }
}
