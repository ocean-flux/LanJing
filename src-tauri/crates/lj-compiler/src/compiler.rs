//! Definition → immutable Execution Plan 的确定性编译。
//!
//! 本模块只处理作者合同的规范化、校验与编译；不解析来源专有格式，也不依赖
//! runtime、存储或 Tauri。closed value-kind compatibility matrix 与节点生成端口只在这里
//! 定义；Plan writer 始终由 `lj-rule-model` 的 current 构造器封存。本 crate 不维护第二套
//! 节点/端口清单，也不做 schema 版本分支或 legacy 读取。

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

use lj_rule_model::definition::{
    CollectionSelector, ConditionConfig, ConditionPredicate, ControlExpression, ControlledMapper,
    FlowEdge, FlowNode, FlowNodeConfig, FlowPortRef, LoopIterationLimit, MAX_LOOP_ITERATIONS,
    MergeConfig, RuleDefinition, SourceSpan,
};
use lj_rule_model::literal::TypedLiteral;
use lj_rule_model::plan::{
    CONDITION_INPUT_HANDLE, ControlRegion, EffectDeclaration, EffectKind, ExecutionPlan,
    ExecutionPlanParts, IntentEntry, LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE, LOOP_BODY_HANDLE,
    LOOP_COLLECTION_HANDLE, LOOP_DONE_HANDLE, LOOP_YIELD_HANDLE, LoopControlRegion,
    MERGE_OUTPUT_HANDLE, PlanEdge, PlanForEachConfig, PlanNode, PlanNodeConfig, PlanPort,
    PortValueKind, PortValueType,
};
use lj_rule_model::{Diagnostic, DiagnosticSeverity, definition_hash};
use uuid::Uuid;

use crate::error::CompilerError;

/// 默认 compiler 身份；其值参与 Plan hash。
pub const DEFAULT_COMPILER_VERSION: &str = concat!("lj-compiler@", env!("CARGO_PKG_VERSION"));

const NETWORK_CAPABILITY: &str = "network";

/// 纯 Definition compiler。
#[derive(Debug, Clone)]
pub struct Compiler {
    version: String,
}

impl Compiler {
    /// 使用显式 compiler 身份创建 compiler。
    #[must_use]
    pub fn with_version(version: String) -> Self {
        Self { version }
    }

    /// 返回本 compiler 的稳定身份。
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// 校验并将 current Definition 编译成 immutable Plan。
    ///
    /// # Errors
    ///
    /// Definition 不满足 config、typed handle、端口兼容、structured Loop、意图可达性与
    /// 能力合同时返回 [`CompilerError::Validation`]；Plan canonical hash 无法生成时返回
    /// [`CompilerError::Serialization`]。
    pub fn compile(&self, definition: &RuleDefinition) -> Result<ExecutionPlan, CompilerError> {
        let definition = canonicalize(definition);
        let analysis = analyze(&definition);
        if analysis
            .diagnostics
            .iter()
            .any(|item| item.severity == DiagnosticSeverity::Error)
        {
            return Err(CompilerError::validation(analysis.diagnostics));
        }

        build_plan(&definition, &self.version, analysis.control_regions)
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::with_version(DEFAULT_COMPILER_VERSION.to_string())
    }
}

/// 将 current 作者 Definition 规范化为与物理声明顺序无关的形式。
///
/// 节点按 id、语义边按四元组 identity 排序；Merge input 按显式 `order` 排序；
/// Condition branch 按原始 UTF-8 bytes 排序。Mapper identity fields 等真正有序字段保持
/// 原样。物理数组排列与 editor layout 永不进入 hash。
#[must_use]
pub fn canonicalize(definition: &RuleDefinition) -> RuleDefinition {
    let mut canonical = definition.clone();
    let flow = canonical.flow_mut();
    for node in &mut flow.nodes {
        match &mut node.config {
            FlowNodeConfig::Merge(config) => {
                config.inputs.sort_by_key(|input| input.order);
            }
            FlowNodeConfig::Condition(config) => {
                config
                    .branches
                    .sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
            }
            FlowNodeConfig::Http(_)
            | FlowNodeConfig::Js(_)
            | FlowNodeConfig::Extract(_)
            | FlowNodeConfig::Mapper(_)
            | FlowNodeConfig::Loop(_) => {}
        }
    }
    flow.nodes.sort_by_key(|node| node.id);
    flow.edges.sort();
    canonical
}

/// 返回 Definition 的全部稳定、可定位诊断。
///
/// 本函数不丢弃后续错误，并先 canonicalize 声明顺序，因此同一非法语义不会因编辑器
/// 数组顺序不同而改变诊断顺序。
#[must_use]
pub fn validate(definition: &RuleDefinition) -> Vec<Diagnostic> {
    analyze(&canonicalize(definition)).diagnostics
}

struct Analysis {
    diagnostics: Vec<Diagnostic>,
    control_regions: Vec<ControlRegion>,
}

#[derive(Debug, Clone)]
struct NodePorts {
    inputs: Vec<PlanPort>,
    outputs: Vec<PlanPort>,
}

#[derive(Debug, Clone)]
struct ValidatedLoopRegion {
    region: LoopControlRegion,
    backedge: FlowEdge,
    body_nodes: BTreeSet<Uuid>,
}

fn analyze(definition: &RuleDefinition) -> Analysis {
    let mut diagnostics = Vec::new();
    let mut nodes = BTreeMap::<Uuid, &FlowNode>::new();
    let mut ports = BTreeMap::<Uuid, NodePorts>::new();

    validate_definition_header(definition, &mut diagnostics);
    for node in &definition.flow().nodes {
        if nodes.contains_key(&node.id) {
            diagnostics.push(node_diagnostic(
                "DUPLICATE_NODE_ID",
                format!("节点 {} 重复声明", node.id),
                node,
                "",
            ));
            continue;
        }
        validate_node_configuration(node, definition, &mut diagnostics);
        ports.insert(node.id, ports_for_node(node));
        nodes.insert(node.id, node);
    }

    let valid_edges = validate_edges(definition, &nodes, &ports, &mut diagnostics);
    validate_input_and_control_handles(&nodes, &valid_edges, &mut diagnostics);

    let mut loop_regions =
        validate_loop_regions(definition, &nodes, &valid_edges, &mut diagnostics);
    validate_loop_region_overlap(&mut loop_regions, &nodes, &mut diagnostics);

    let accepted_backedges = loop_regions
        .iter()
        .map(|region| region.backedge.clone())
        .collect::<BTreeSet<_>>();
    let acyclic_adjacency = adjacency(&valid_edges, &accepted_backedges);
    if has_cycle(nodes.keys().copied(), &acyclic_adjacency) {
        diagnostics.push(diagnostic(
            "FLOW_CYCLE_UNSTRUCTURED",
            "Flow 含有非 structured Loop yield 的裸循环",
            schema_span("/flow/edges"),
        ));
    }

    let full_adjacency = adjacency(&valid_edges, &BTreeSet::new());
    validate_intent_exports(
        definition,
        &nodes,
        &ports,
        &full_adjacency,
        &mut diagnostics,
    );

    sort_diagnostics(&mut diagnostics);
    let mut control_regions = loop_regions
        .into_iter()
        .map(|region| ControlRegion::Loop(region.region))
        .collect::<Vec<_>>();
    control_regions.sort_by_key(|region| match region {
        ControlRegion::Loop(region) => region.loop_node,
    });

    Analysis {
        diagnostics,
        control_regions,
    }
}

fn validate_definition_header(definition: &RuleDefinition, diagnostics: &mut Vec<Diagnostic>) {
    if definition.source_identity().id.trim().is_empty() {
        diagnostics.push(diagnostic(
            "SOURCE_IDENTITY_REQUIRED",
            "来源稳定身份不能为空",
            schema_span("/source_identity/id"),
        ));
    }
    if definition.source_id_rules().is_empty() {
        diagnostics.push(diagnostic(
            "SOURCE_ID_RULES_REQUIRED",
            "Definition 必须声明来源持有的稳定 ID 规则",
            schema_span("/source_id_rules"),
        ));
    }
    for (index, rule) in definition.source_id_rules().iter().enumerate() {
        if rule.trim().is_empty() {
            diagnostics.push(diagnostic(
                "SOURCE_ID_RULE_INVALID",
                "来源稳定 ID 规则不能为空",
                schema_span(format!("/source_id_rules/{index}")),
            ));
        }
    }
    if definition.intent_exports().is_empty() {
        diagnostics.push(diagnostic(
            "INTENT_EXPORT_REQUIRED",
            "Definition 至少需要一个标准意图入口",
            schema_span("/intent_exports"),
        ));
    }
}

fn validate_node_configuration(
    node: &FlowNode,
    definition: &RuleDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match &node.config {
        FlowNodeConfig::Http(config) => {
            if config.url.trim().is_empty() {
                diagnostics.push(node_diagnostic(
                    "NODE_CONFIG_INVALID",
                    "HTTP 节点 URL 不能为空",
                    node,
                    "/url",
                ));
            }
            require_network(node, definition, diagnostics);
        }
        FlowNodeConfig::Js(config) => {
            if config.code.trim().is_empty() {
                diagnostics.push(node_diagnostic(
                    "NODE_CONFIG_INVALID",
                    "JS 节点脚本不能为空",
                    node,
                    "/code",
                ));
            }
            require_network(node, definition, diagnostics);
        }
        FlowNodeConfig::Extract(_) => {}
        FlowNodeConfig::Mapper(mapper) => validate_mapper(node, mapper, diagnostics),
        FlowNodeConfig::Merge(config) => validate_merge_config(node, config, diagnostics),
        FlowNodeConfig::Condition(config) => {
            validate_condition_config(node, config, diagnostics);
            if matches!(config.expression, ControlExpression::Js { .. }) {
                require_network(node, definition, diagnostics);
            }
        }
        FlowNodeConfig::Loop(config) => {
            match &config.collection {
                CollectionSelector::Typed { pointer } => {
                    if !is_valid_json_pointer(pointer) {
                        diagnostics.push(node_diagnostic(
                            "LOOP_COLLECTION_SELECTOR_INVALID",
                            "Loop typed collection selector 必须是 RFC 6901 JSON Pointer",
                            node,
                            "/collection/pointer",
                        ));
                    }
                }
                CollectionSelector::Js { code } => {
                    if code.trim().is_empty() {
                        diagnostics.push(node_diagnostic(
                            "LOOP_COLLECTION_SELECTOR_INVALID",
                            "Loop JS collection selector 脚本不能为空",
                            node,
                            "/collection/code",
                        ));
                    }
                    require_network(node, definition, diagnostics);
                }
            }
            if config.item_binding.trim().is_empty()
                || config.index_binding.trim().is_empty()
                || config.item_binding == config.index_binding
            {
                diagnostics.push(node_diagnostic(
                    "LOOP_BINDING_INVALID",
                    "Loop item/index binding 必须非空且互不相同",
                    node,
                    "/bindings",
                ));
            }
            let max_iterations = u32::from(config.max_iterations);
            if !(1..=MAX_LOOP_ITERATIONS).contains(&max_iterations) {
                diagnostics.push(node_diagnostic(
                    "LOOP_MAX_ITERATIONS_INVALID",
                    format!("Loop max_iterations 必须在 1..={MAX_LOOP_ITERATIONS}"),
                    node,
                    "/max_iterations",
                ));
            }
        }
    }
}

fn validate_mapper(node: &FlowNode, mapper: &ControlledMapper, diagnostics: &mut Vec<Diagnostic>) {
    if mapper.identity_fields.is_empty() {
        diagnostics.push(node_diagnostic(
            "MAPPER_IDENTITY_REQUIRED",
            "Mapper 必须声明至少一个稳定身份字段",
            node,
            "/identity_fields",
        ));
        return;
    }

    let mut fields = BTreeSet::new();
    for (index, field) in mapper.identity_fields.iter().enumerate() {
        if field.trim().is_empty() || !fields.insert(field.as_str()) {
            diagnostics.push(node_diagnostic(
                "MAPPER_IDENTITY_INVALID",
                "Mapper identity field 必须非空且唯一",
                node,
                &format!("/identity_fields/{index}"),
            ));
        }
    }
}

fn validate_merge_config(node: &FlowNode, config: &MergeConfig, diagnostics: &mut Vec<Diagnostic>) {
    if config.inputs.is_empty() {
        diagnostics.push(node_diagnostic(
            "MERGE_INPUT_REQUIRED",
            "Merge 至少需要一个命名 input",
            node,
            "/inputs",
        ));
        return;
    }

    let mut input_ids = BTreeSet::new();
    let mut handles = BTreeSet::new();
    let mut orders = BTreeSet::new();
    let input_count = config.inputs.len();

    for input in &config.inputs {
        if input.input_id.trim().is_empty() {
            diagnostics.push(node_diagnostic(
                "MERGE_INPUT_ID_INVALID",
                "Merge input_id 不能为空",
                node,
                "/inputs",
            ));
        } else if !input_ids.insert(input.input_id.as_str()) {
            diagnostics.push(node_diagnostic(
                "MERGE_INPUT_ID_DUPLICATE",
                format!("Merge input_id {} 重复声明", input.input_id),
                node,
                &format!("/inputs/{}", pointer_token(&input.input_id)),
            ));
        }

        if input.handle.trim().is_empty() {
            diagnostics.push(node_diagnostic(
                "MERGE_INPUT_HANDLE_INVALID",
                "Merge input handle 不能为空",
                node,
                "/inputs",
            ));
        } else if !handles.insert(input.handle.as_str()) {
            diagnostics.push(node_diagnostic(
                "MERGE_INPUT_DUPLICATE",
                format!("Merge input handle {} 重复声明", input.handle),
                node,
                &format!("/inputs/{}", pointer_token(&input.handle)),
            ));
        }

        if !orders.insert(input.order) {
            diagnostics.push(node_diagnostic(
                "MERGE_ORDER_INVALID",
                format!("Merge input order {} 重复声明", input.order),
                node,
                &format!("/inputs/{}", pointer_token(&input.handle)),
            ));
        }
    }

    let expected_orders =
        (0..u32::try_from(input_count).unwrap_or(u32::MAX)).collect::<BTreeSet<_>>();
    if orders != expected_orders {
        diagnostics.push(node_diagnostic(
            "MERGE_ORDER_INVALID",
            "Merge input order 必须唯一且连续覆盖 0..n-1",
            node,
            "/inputs",
        ));
    }
}

fn validate_condition_config(
    node: &FlowNode,
    config: &ConditionConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if config.branches.len() < 2 {
        diagnostics.push(node_diagnostic(
            "CONDITION_BRANCH_MISSING",
            "Condition 至少需要两个已声明 branch",
            node,
            "/branches",
        ));
    }

    let mut branches = BTreeSet::new();
    for branch in &config.branches {
        if branch.trim().is_empty() {
            diagnostics.push(node_diagnostic(
                "CONDITION_BRANCH_INVALID",
                "Condition branch handle 不能为空",
                node,
                "/branches",
            ));
        } else if !branches.insert(branch.as_str()) {
            diagnostics.push(node_diagnostic(
                "CONDITION_BRANCH_DUPLICATE",
                format!("Condition branch {branch} 重复声明"),
                node,
                &format!("/branches/{}", pointer_token(branch)),
            ));
        }
    }

    match &config.expression {
        ControlExpression::Typed {
            predicate,
            true_branch,
            false_branch,
        } => {
            if !is_valid_json_pointer(predicate.pointer()) {
                diagnostics.push(node_diagnostic(
                    "CONDITION_POINTER_INVALID",
                    "Condition predicate 必须使用 RFC 6901 JSON Pointer",
                    node,
                    "/expression/predicate/pointer",
                ));
            }
            if !branches.contains(true_branch.as_str()) || !branches.contains(false_branch.as_str())
            {
                diagnostics.push(node_diagnostic(
                    "CONDITION_BRANCH_MISSING",
                    "Condition true/false 必须映射到已声明 branch",
                    node,
                    "/expression/branches",
                ));
            }
            if true_branch == false_branch {
                diagnostics.push(node_diagnostic(
                    "CONDITION_BRANCH_DUPLICATE",
                    "Condition true/false branch 必须互不相同",
                    node,
                    "/expression/branches",
                ));
            }
            if matches!(
                predicate,
                ConditionPredicate::Lt { value, .. }
                    | ConditionPredicate::Lte { value, .. }
                    | ConditionPredicate::Gt { value, .. }
                    | ConditionPredicate::Gte { value, .. }
                    if !matches!(value, TypedLiteral::Number(_))
            ) {
                diagnostics.push(node_diagnostic(
                    "CONDITION_OPERATOR_INVALID",
                    "Condition 顺序比较只接受 number literal",
                    node,
                    "/expression/predicate/value",
                ));
            }
        }
        ControlExpression::Js { code } => {
            if code.trim().is_empty() {
                diagnostics.push(node_diagnostic(
                    "CONDITION_JS_INVALID",
                    "Condition JS 表达式脚本不能为空",
                    node,
                    "/expression/code",
                ));
            }
        }
    }
}

fn require_network(
    node: &FlowNode,
    definition: &RuleDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !definition.capability_manifest().required.network {
        diagnostics.push(node_diagnostic(
            "CAPABILITY_MISMATCH",
            format!(
                "节点 {} 需要 network 能力，但 capability manifest 未声明",
                node.id
            ),
            node,
            "",
        ));
    }
}

fn validate_edges<'a>(
    definition: &'a RuleDefinition,
    nodes: &BTreeMap<Uuid, &'a FlowNode>,
    ports: &BTreeMap<Uuid, NodePorts>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<&'a FlowEdge> {
    let mut seen = BTreeSet::new();
    let mut valid_edges = Vec::new();

    for edge in &definition.flow().edges {
        if !seen.insert(edge.clone()) {
            diagnostics.push(edge_diagnostic(
                "DUPLICATE_EDGE",
                "语义边 identity 重复",
                edge,
            ));
            continue;
        }

        let from = nodes.get(&edge.from.node_id);
        let to = nodes.get(&edge.to.node_id);
        match (from, to) {
            (None, None) => {
                diagnostics.push(edge_diagnostic(
                    "EDGE_ENDPOINT_MISSING",
                    "语义边的起点和终点节点都不存在",
                    edge,
                ));
                continue;
            }
            (None, Some(_)) => {
                diagnostics.push(edge_diagnostic(
                    "EDGE_SOURCE_MISSING",
                    format!("边起点节点 {} 不存在", edge.from.node_id),
                    edge,
                ));
                continue;
            }
            (Some(_), None) => {
                diagnostics.push(edge_diagnostic(
                    "EDGE_TARGET_MISSING",
                    format!("边终点节点 {} 不存在", edge.to.node_id),
                    edge,
                ));
                continue;
            }
            (Some(_), Some(_)) => {}
        }

        let source_port = ports
            .get(&edge.from.node_id)
            .and_then(|ports| find_port(&ports.outputs, &edge.from.handle));
        let target_port = ports
            .get(&edge.to.node_id)
            .and_then(|ports| find_port(&ports.inputs, &edge.to.handle));
        if source_port.is_none() {
            diagnostics.push(edge_diagnostic(
                "SOURCE_HANDLE_MISSING",
                format!(
                    "节点 {} 未声明 output handle {}",
                    edge.from.node_id, edge.from.handle
                ),
                edge,
            ));
        }
        if target_port.is_none() {
            diagnostics.push(edge_diagnostic(
                "TARGET_HANDLE_MISSING",
                format!(
                    "节点 {} 未声明 input handle {}",
                    edge.to.node_id, edge.to.handle
                ),
                edge,
            ));
        }
        let (Some(source_port), Some(target_port)) = (source_port, target_port) else {
            continue;
        };
        if !ports_are_compatible(&source_port.value_type, &target_port.value_type) {
            diagnostics.push(edge_diagnostic(
                "PORT_TYPE_MISMATCH",
                format!(
                    "节点 {} 的 output {} 不能连接到节点 {} 的 input {}",
                    edge.from.node_id, edge.from.handle, edge.to.node_id, edge.to.handle
                ),
                edge,
            ));
        }
        valid_edges.push(edge);
    }

    valid_edges
}

fn validate_input_and_control_handles(
    nodes: &BTreeMap<Uuid, &FlowNode>,
    edges: &[&FlowEdge],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let incoming = edges_by_input(edges);
    let outgoing = edges_by_output(edges);

    for node in nodes.values() {
        match &node.config {
            FlowNodeConfig::Merge(config) => {
                let mut checked = BTreeSet::new();
                for input in &config.inputs {
                    if input.handle.trim().is_empty() || !checked.insert(input.handle.as_str()) {
                        continue;
                    }
                    let reference = FlowPortRef::new(node.id, input.handle.clone());
                    if incoming.get(&reference).map_or(0, Vec::len) != 1 {
                        diagnostics.push(node_diagnostic(
                            "MERGE_INPUT_POLICY_INVALID",
                            format!(
                                "Merge input {} 必须有唯一 producer，并保留声明的 activation policy",
                                input.handle
                            ),
                            node,
                            &format!("/inputs/{}", pointer_token(&input.handle)),
                        ));
                    }
                }
                let output = FlowPortRef::new(node.id, MERGE_OUTPUT_HANDLE);
                if outgoing.get(&output).map_or(0, Vec::len) == 0 {
                    diagnostics.push(node_diagnostic(
                        "MERGE_OUTPUT_UNCONNECTED",
                        "Merge output 必须连接到下游",
                        node,
                        "/output",
                    ));
                }
            }
            FlowNodeConfig::Condition(config) => {
                let input = FlowPortRef::new(node.id, CONDITION_INPUT_HANDLE);
                if incoming.get(&input).map_or(0, Vec::len) != 1 {
                    diagnostics.push(node_diagnostic(
                        "CONDITION_INPUT_INVALID",
                        "Condition input 必须有唯一 producer",
                        node,
                        "/input",
                    ));
                }
                let mut checked = BTreeSet::new();
                for branch in &config.branches {
                    if branch.trim().is_empty() || !checked.insert(branch.as_str()) {
                        continue;
                    }
                    let output = FlowPortRef::new(node.id, branch.clone());
                    if outgoing.get(&output).map_or(0, Vec::len) == 0 {
                        diagnostics.push(node_diagnostic(
                            "CONDITION_BRANCH_UNCONNECTED",
                            format!("Condition branch {branch} 未连接到下游"),
                            node,
                            &format!("/branches/{}", pointer_token(branch)),
                        ));
                    }
                }
            }
            FlowNodeConfig::Loop(_) => {
                validate_loop_handle_counts(node, &incoming, &outgoing, diagnostics);
            }
            FlowNodeConfig::Http(_)
            | FlowNodeConfig::Js(_)
            | FlowNodeConfig::Extract(_)
            | FlowNodeConfig::Mapper(_) => {
                let input = FlowPortRef::new(node.id, LINEAR_INPUT_HANDLE);
                if incoming.get(&input).map_or(0, Vec::len) > 1 {
                    diagnostics.push(node_diagnostic(
                        "INPUT_HANDLE_AMBIGUOUS",
                        "线性节点 input 不能有多个 producer",
                        node,
                        "/input",
                    ));
                }
            }
        }
    }
}

fn validate_loop_handle_counts(
    node: &FlowNode,
    incoming: &BTreeMap<FlowPortRef, Vec<&FlowEdge>>,
    outgoing: &BTreeMap<FlowPortRef, Vec<&FlowEdge>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let collection = FlowPortRef::new(node.id, LOOP_COLLECTION_HANDLE);
    if incoming.get(&collection).map_or(0, Vec::len) != 1 {
        diagnostics.push(node_diagnostic(
            "LOOP_ENTRY_INVALID",
            "Loop collection 必须有唯一外部 producer",
            node,
            "/collection",
        ));
    }
    let body = FlowPortRef::new(node.id, LOOP_BODY_HANDLE);
    if outgoing.get(&body).map_or(0, Vec::len) != 1 {
        diagnostics.push(node_diagnostic(
            "LOOP_BODY_INVALID",
            "Loop body 必须有唯一 structured entry",
            node,
            "/body",
        ));
    }
    let yield_input = FlowPortRef::new(node.id, LOOP_YIELD_HANDLE);
    if incoming.get(&yield_input).map_or(0, Vec::len) != 1 {
        diagnostics.push(node_diagnostic(
            "LOOP_YIELD_INVALID",
            "Loop yield 必须有唯一 structured return",
            node,
            "/yield",
        ));
    }
    let done = FlowPortRef::new(node.id, LOOP_DONE_HANDLE);
    if outgoing.get(&done).map_or(0, Vec::len) == 0 {
        diagnostics.push(node_diagnostic(
            "LOOP_DONE_INVALID",
            "Loop done 必须连接到 region 外下游",
            node,
            "/done",
        ));
    }
}

fn validate_loop_regions(
    definition: &RuleDefinition,
    nodes: &BTreeMap<Uuid, &FlowNode>,
    edges: &[&FlowEdge],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<ValidatedLoopRegion> {
    let incoming = edges_by_input(edges);
    let outgoing = edges_by_output(edges);
    let mut regions = Vec::new();

    for node in definition
        .flow()
        .nodes
        .iter()
        .filter(|node| matches!(node.config, FlowNodeConfig::Loop(_)))
    {
        let before = diagnostics.len();
        let collection_ref = FlowPortRef::new(node.id, LOOP_COLLECTION_HANDLE);
        let body_ref = FlowPortRef::new(node.id, LOOP_BODY_HANDLE);
        let yield_ref = FlowPortRef::new(node.id, LOOP_YIELD_HANDLE);
        let done_ref = FlowPortRef::new(node.id, LOOP_DONE_HANDLE);

        let collection_edges = incoming.get(&collection_ref).map_or(&[][..], Vec::as_slice);
        let body_edges = outgoing.get(&body_ref).map_or(&[][..], Vec::as_slice);
        let yield_edges = incoming.get(&yield_ref).map_or(&[][..], Vec::as_slice);
        let done_edges = outgoing.get(&done_ref).map_or(&[][..], Vec::as_slice);
        if collection_edges.len() != 1
            || body_edges.len() != 1
            || yield_edges.len() != 1
            || done_edges.is_empty()
        {
            continue;
        }

        let body_edge = body_edges[0];
        let yield_edge = yield_edges[0];
        let body_entry = body_edge.to.clone();
        let yield_source = yield_edge.from.clone();
        if body_entry.node_id == node.id || yield_source.node_id == node.id {
            diagnostics.push(node_diagnostic(
                "LOOP_BODY_INVALID",
                "Loop body entry/yield source 必须是 region 内的其他节点",
                node,
                "/body",
            ));
            continue;
        }

        let forward = reachable_loop_body_nodes(body_entry.node_id, node.id, yield_edge, edges);
        let reverse = nodes_reaching_loop_yield(yield_source.node_id, node.id, yield_edge, edges);
        if !forward.contains(&yield_source.node_id) {
            diagnostics.push(node_diagnostic(
                "LOOP_YIELD_UNREACHABLE",
                "Loop body entry 无法到达唯一 yield source",
                node,
                "/yield",
            ));
            continue;
        }

        let body_nodes = forward
            .intersection(&reverse)
            .copied()
            .collect::<BTreeSet<_>>();
        if forward
            .iter()
            .any(|candidate| !body_nodes.contains(candidate))
        {
            diagnostics.push(node_diagnostic(
                "LOOP_BODY_BYPASS",
                "Loop body 存在不能恰好到达 yield 的旁路或终点",
                node,
                "/body",
            ));
        }
        if body_nodes.iter().any(|node_id| {
            nodes
                .get(node_id)
                .is_some_and(|candidate| matches!(candidate.config, FlowNodeConfig::Loop(_)))
        }) {
            diagnostics.push(node_diagnostic(
                "LOOP_NESTING_UNSUPPORTED",
                "首期不支持 nested Loop region",
                node,
                "/body",
            ));
        }

        for edge in edges {
            let from_inside = body_nodes.contains(&edge.from.node_id);
            let to_inside = body_nodes.contains(&edge.to.node_id);
            let valid_entry = *edge == body_edge;
            let valid_yield = *edge == yield_edge;
            if (!from_inside && to_inside && !valid_entry)
                || (from_inside && !to_inside && !valid_yield)
            {
                diagnostics.push(edge_diagnostic(
                    "LOOP_CROSS_REGION_EDGE",
                    format!("边跨越 Loop {} region 边界", node.id),
                    edge,
                ));
            }
        }
        if collection_edges
            .iter()
            .any(|edge| body_nodes.contains(&edge.from.node_id))
            || done_edges
                .iter()
                .any(|edge| body_nodes.contains(&edge.to.node_id))
        {
            diagnostics.push(node_diagnostic(
                "LOOP_CROSS_REGION_EDGE",
                "Loop collection 必须来自 region 外，done 必须离开 region",
                node,
                "/region",
            ));
        }

        if diagnostics.len() == before {
            regions.push(ValidatedLoopRegion {
                region: LoopControlRegion {
                    loop_node: node.id,
                    body_entry,
                    yield_source,
                    body_nodes: body_nodes.iter().copied().collect(),
                },
                backedge: (*yield_edge).clone(),
                body_nodes,
            });
        }
    }

    regions
}

fn validate_loop_region_overlap(
    regions: &mut Vec<ValidatedLoopRegion>,
    nodes: &BTreeMap<Uuid, &FlowNode>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut invalid = BTreeSet::new();
    for left_index in 0..regions.len() {
        for right_index in (left_index + 1)..regions.len() {
            let left = &regions[left_index];
            let right = &regions[right_index];
            if left.body_nodes.is_disjoint(&right.body_nodes) {
                continue;
            }
            invalid.insert(left.region.loop_node);
            invalid.insert(right.region.loop_node);
        }
    }
    for loop_node in &invalid {
        if let Some(node) = nodes.get(loop_node) {
            diagnostics.push(node_diagnostic(
                "LOOP_REGION_OVERLAP",
                "Loop body regions 不得重叠或嵌套",
                node,
                "/body",
            ));
        }
    }
    regions.retain(|region| !invalid.contains(&region.region.loop_node));
}

fn reachable_loop_body_nodes(
    start: Uuid,
    loop_node: Uuid,
    backedge: &FlowEdge,
    edges: &[&FlowEdge],
) -> BTreeSet<Uuid> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([start]);
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current) {
            continue;
        }
        for edge in edges {
            if *edge == backedge || edge.from.node_id != current || edge.to.node_id == loop_node {
                continue;
            }
            queue.push_back(edge.to.node_id);
        }
    }
    seen
}

fn nodes_reaching_loop_yield(
    target: Uuid,
    loop_node: Uuid,
    backedge: &FlowEdge,
    edges: &[&FlowEdge],
) -> BTreeSet<Uuid> {
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::from([target]);
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current) {
            continue;
        }
        for edge in edges {
            if *edge == backedge || edge.to.node_id != current || edge.from.node_id == loop_node {
                continue;
            }
            queue.push_back(edge.from.node_id);
        }
    }
    seen
}

fn validate_intent_exports(
    definition: &RuleDefinition,
    nodes: &BTreeMap<Uuid, &FlowNode>,
    ports: &BTreeMap<Uuid, NodePorts>,
    adjacency: &BTreeMap<Uuid, Vec<Uuid>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (intent, export) in definition.intent_exports() {
        let entry = nodes.get(&export.flow_entry);
        let mapper = nodes.get(&export.mapper_output);
        if entry.is_none() {
            diagnostics.push(diagnostic(
                "INTENT_ENTRY_MISSING",
                format!("{intent:?} 的入口节点 {} 不存在", export.flow_entry),
                schema_span(format!("/intent_exports/{intent:?}/flow_entry")),
            ));
        }
        if let Some(entry) = entry {
            let accepts_intent = ports
                .get(&entry.id)
                .and_then(|ports| find_port(&ports.inputs, LINEAR_INPUT_HANDLE))
                .is_some_and(|port| accepts_kind(&port.value_type, PortValueKind::IntentInput));
            if !accepts_intent {
                diagnostics.push(node_diagnostic(
                    "INTENT_ENTRY_PORT_MISMATCH",
                    format!("{intent:?} 入口不接受 IntentInput"),
                    entry,
                    "/input",
                ));
            }
        }
        match mapper {
            Some(node) if matches!(node.config, FlowNodeConfig::Mapper(_)) => {}
            Some(node) => diagnostics.push(node_diagnostic(
                "INTENT_MAPPER_INVALID",
                format!("{intent:?} 的输出节点 {} 不是 Mapper", node.id),
                node,
                "",
            )),
            None => diagnostics.push(diagnostic(
                "INTENT_MAPPER_MISSING",
                format!("{intent:?} 的 Mapper 节点 {} 不存在", export.mapper_output),
                schema_span(format!("/intent_exports/{intent:?}/mapper_output")),
            )),
        }
        if entry.is_some()
            && mapper.is_some()
            && !is_reachable(adjacency, export.flow_entry, export.mapper_output)
        {
            diagnostics.push(diagnostic(
                "MAPPER_UNREACHABLE",
                format!(
                    "{intent:?} 的入口 {} 无法到达 Mapper {}",
                    export.flow_entry, export.mapper_output
                ),
                mapper.map_or_else(
                    || schema_span("/intent_exports"),
                    |node| semantic_span(node.span.as_ref(), node_path(node.id, "")),
                ),
            ));
        }
    }

    for mapper in nodes
        .values()
        .filter(|node| matches!(node.config, FlowNodeConfig::Mapper(_)))
    {
        let declared = definition
            .intent_exports()
            .values()
            .any(|export| export.mapper_output == mapper.id);
        let reachable = definition
            .intent_exports()
            .values()
            .any(|export| is_reachable(adjacency, export.flow_entry, mapper.id));
        if !declared || !reachable {
            diagnostics.push(node_diagnostic(
                "MAPPER_UNREACHABLE",
                format!("Mapper 节点 {} 未被可达的标准意图导出使用", mapper.id),
                mapper,
                "",
            ));
        }
    }
}

fn build_plan(
    definition: &RuleDefinition,
    compiler_version: &str,
    control_regions: Vec<ControlRegion>,
) -> Result<ExecutionPlan, CompilerError> {
    let definition_hash = definition_hash(definition)
        .map_err(|error| CompilerError::Serialization(error.to_string()))?;

    let mut effects = Vec::new();
    let mut nodes = Vec::with_capacity(definition.flow().nodes.len());
    for flow_node in &definition.flow().nodes {
        if let Some(kind) = effect_kind(&flow_node.config) {
            let required_capabilities = if matches!(&kind, EffectKind::Http | EffectKind::QuickJs) {
                vec![NETWORK_CAPABILITY.to_string()]
            } else {
                Vec::new()
            };
            effects.push(EffectDeclaration {
                node_id: flow_node.id,
                kind,
                required_capabilities,
            });
        }
        let NodePorts { inputs, outputs } = ports_for_node(flow_node);
        nodes.push(PlanNode {
            id: flow_node.id,
            inputs,
            outputs,
            config: plan_node_config(&flow_node.config),
        });
    }

    effects.sort_by(|left, right| {
        left.node_id
            .cmp(&right.node_id)
            .then_with(|| effect_kind_rank(&left.kind).cmp(&effect_kind_rank(&right.kind)))
    });
    let capability_requirements = effects
        .iter()
        .flat_map(|effect| effect.required_capabilities.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let intent_entries = definition
        .intent_exports()
        .iter()
        .map(|(intent, export)| {
            (
                *intent,
                IntentEntry {
                    intent: *intent,
                    entry_node: export.flow_entry,
                    mapper_output: export.mapper_output,
                },
            )
        })
        .collect();
    let edges = definition
        .flow()
        .edges
        .iter()
        .map(|edge| PlanEdge::new(edge.from.clone(), edge.to.clone()))
        .collect();

    ExecutionPlan::new(
        compiler_version,
        definition_hash,
        ExecutionPlanParts {
            nodes,
            edges,
            intent_entries,
            effects,
            capability_requirements,
            control_regions,
        },
    )
    .map_err(|error| CompilerError::Serialization(error.to_string()))
}

fn ports_for_node(node: &FlowNode) -> NodePorts {
    let is_merge = matches!(node.config, FlowNodeConfig::Merge(_));
    let (mut inputs, mut outputs) = match &node.config {
        FlowNodeConfig::Http(_) => (
            vec![PlanPort::new(LINEAR_INPUT_HANDLE, controlled_value_type())],
            vec![PlanPort::new(
                LINEAR_OUTPUT_HANDLE,
                PortValueType::kind(PortValueKind::HttpResponse),
            )],
        ),
        FlowNodeConfig::Js(config) => {
            let output_kind = match config.output {
                lj_rule_model::definition::JsOutputKind::Json => PortValueKind::Json,
                lj_rule_model::definition::JsOutputKind::Raw => PortValueKind::Raw,
            };
            (
                vec![PlanPort::new(LINEAR_INPUT_HANDLE, controlled_value_type())],
                vec![PlanPort::new(
                    LINEAR_OUTPUT_HANDLE,
                    PortValueType::kind(output_kind),
                )],
            )
        }
        FlowNodeConfig::Extract(_) => (
            vec![PlanPort::new(
                LINEAR_INPUT_HANDLE,
                PortValueType::kind(PortValueKind::HttpResponse),
            )],
            vec![PlanPort::new(
                LINEAR_OUTPUT_HANDLE,
                PortValueType::kind(PortValueKind::Json),
            )],
        ),
        FlowNodeConfig::Mapper(_) => (
            vec![PlanPort::new(
                LINEAR_INPUT_HANDLE,
                PortValueType::kind(PortValueKind::Json),
            )],
            vec![PlanPort::new(
                LINEAR_OUTPUT_HANDLE,
                PortValueType::kind(PortValueKind::Delta),
            )],
        ),
        FlowNodeConfig::Merge(config) => {
            let mut ordered = config.inputs.iter().collect::<Vec<_>>();
            ordered.sort_by_key(|input| input.order);
            (
                ordered
                    .into_iter()
                    .map(|input| {
                        PlanPort::new(
                            input.handle.clone(),
                            PortValueType::kind(PortValueKind::Json),
                        )
                    })
                    .collect(),
                vec![PlanPort::new(
                    MERGE_OUTPUT_HANDLE,
                    PortValueType::kind(PortValueKind::Json),
                )],
            )
        }
        FlowNodeConfig::Condition(config) => (
            vec![PlanPort::new(
                CONDITION_INPUT_HANDLE,
                PortValueType::kind(PortValueKind::Json),
            )],
            config
                .branches
                .iter()
                .map(|branch| {
                    PlanPort::new(branch.clone(), PortValueType::kind(PortValueKind::Json))
                })
                .collect(),
        ),
        FlowNodeConfig::Loop(_) => (
            vec![
                PlanPort::new(
                    LOOP_COLLECTION_HANDLE,
                    PortValueType::kind(PortValueKind::Json),
                ),
                PlanPort::new(LOOP_YIELD_HANDLE, PortValueType::kind(PortValueKind::Json)),
            ],
            vec![
                PlanPort::new(
                    LOOP_BODY_HANDLE,
                    PortValueType::kind(PortValueKind::LoopBinding),
                ),
                PlanPort::new(LOOP_DONE_HANDLE, PortValueType::kind(PortValueKind::Json)),
            ],
        ),
    };
    // Merge input 顺序由显式 order 决定；其他节点 handle 排序只保证稳定展示，不承载语义。
    if !is_merge {
        inputs.sort_by(|left, right| left.handle.as_bytes().cmp(right.handle.as_bytes()));
    }
    outputs.sort_by(|left, right| left.handle.as_bytes().cmp(right.handle.as_bytes()));
    NodePorts { inputs, outputs }
}

fn controlled_value_type() -> PortValueType {
    let mut kinds = vec![
        PortValueKind::IntentInput,
        PortValueKind::Raw,
        PortValueKind::Json,
        PortValueKind::LoopBinding,
    ];
    kinds.sort_unstable();
    kinds.dedup();
    PortValueType::union(kinds)
}

fn ports_are_compatible(output: &PortValueType, input: &PortValueType) -> bool {
    match output {
        PortValueType::Kind { kind } => accepts_kind(input, *kind),
        PortValueType::Union { .. } => false,
    }
}

fn accepts_kind(input: &PortValueType, output: PortValueKind) -> bool {
    match input {
        PortValueType::Kind { kind } => *kind == output,
        PortValueType::Union { kinds } => kinds.binary_search(&output).is_ok(),
    }
}

fn find_port<'a>(ports: &'a [PlanPort], handle: &str) -> Option<&'a PlanPort> {
    ports.iter().find(|port| port.handle == handle)
}

fn effect_kind(config: &FlowNodeConfig) -> Option<EffectKind> {
    match config {
        FlowNodeConfig::Http(_) => Some(EffectKind::Http),
        FlowNodeConfig::Js(_)
        | FlowNodeConfig::Condition(ConditionConfig {
            expression: ControlExpression::Js { .. },
            ..
        })
        | FlowNodeConfig::Loop(lj_rule_model::definition::ForEachConfig {
            collection: CollectionSelector::Js { .. },
            ..
        }) => Some(EffectKind::QuickJs),
        FlowNodeConfig::Extract(_) => Some(EffectKind::Extract),
        FlowNodeConfig::Mapper(_)
        | FlowNodeConfig::Merge(_)
        | FlowNodeConfig::Condition(_)
        | FlowNodeConfig::Loop(_) => None,
    }
}
fn plan_node_config(config: &FlowNodeConfig) -> PlanNodeConfig {
    match config {
        FlowNodeConfig::Http(config) => PlanNodeConfig::Http(config.clone()),
        FlowNodeConfig::Js(config) => PlanNodeConfig::Js(config.clone()),
        FlowNodeConfig::Extract(config) => PlanNodeConfig::Extract(config.clone()),
        FlowNodeConfig::Mapper(config) => PlanNodeConfig::Mapper(config.clone()),
        FlowNodeConfig::Merge(config) => {
            let mut ordered = config.clone();
            ordered.inputs.sort_by_key(|input| input.order);
            PlanNodeConfig::Merge(ordered)
        }
        FlowNodeConfig::Condition(config) => PlanNodeConfig::Condition(config.clone()),
        FlowNodeConfig::Loop(config) => {
            let max_iterations = LoopIterationLimit::new(u32::from(config.max_iterations))
                .unwrap_or_else(|_| {
                    // validate_node_configuration 已拒绝非法 limit；此处兜底保证 Plan 构造不 panic。
                    LoopIterationLimit::new(1).expect("1 is a valid loop limit")
                });
            PlanNodeConfig::Loop(PlanForEachConfig::new(
                config.collection.clone(),
                config.item_binding.clone(),
                config.index_binding.clone(),
                max_iterations,
            ))
        }
    }
}

const fn effect_kind_rank(kind: &EffectKind) -> u8 {
    match kind {
        EffectKind::Http => 0,
        EffectKind::QuickJs => 1,
        EffectKind::Extract => 2,
    }
}

fn edges_by_input<'a>(edges: &'a [&'a FlowEdge]) -> BTreeMap<FlowPortRef, Vec<&'a FlowEdge>> {
    let mut result = BTreeMap::<FlowPortRef, Vec<&FlowEdge>>::new();
    for edge in edges {
        result.entry(edge.to.clone()).or_default().push(edge);
    }
    result
}

fn edges_by_output<'a>(edges: &'a [&'a FlowEdge]) -> BTreeMap<FlowPortRef, Vec<&'a FlowEdge>> {
    let mut result = BTreeMap::<FlowPortRef, Vec<&FlowEdge>>::new();
    for edge in edges {
        result.entry(edge.from.clone()).or_default().push(edge);
    }
    result
}

fn adjacency(edges: &[&FlowEdge], excluded: &BTreeSet<FlowEdge>) -> BTreeMap<Uuid, Vec<Uuid>> {
    let mut result = BTreeMap::<Uuid, Vec<Uuid>>::new();
    for edge in edges {
        if excluded.contains(*edge) {
            continue;
        }
        result
            .entry(edge.from.node_id)
            .or_default()
            .push(edge.to.node_id);
    }
    for neighbors in result.values_mut() {
        neighbors.sort_unstable();
        neighbors.dedup();
    }
    result
}

fn is_reachable(adjacency: &BTreeMap<Uuid, Vec<Uuid>>, from: Uuid, to: Uuid) -> bool {
    if from == to {
        return true;
    }
    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([from]);
    while let Some(current) = queue.pop_front() {
        if !seen.insert(current) {
            continue;
        }
        if let Some(neighbors) = adjacency.get(&current) {
            for neighbor in neighbors {
                if *neighbor == to {
                    return true;
                }
                queue.push_back(*neighbor);
            }
        }
    }
    false
}

fn has_cycle(nodes: impl IntoIterator<Item = Uuid>, adjacency: &BTreeMap<Uuid, Vec<Uuid>>) -> bool {
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    nodes
        .into_iter()
        .any(|node| has_cycle_from(node, adjacency, &mut visiting, &mut visited))
}

fn has_cycle_from(
    node: Uuid,
    adjacency: &BTreeMap<Uuid, Vec<Uuid>>,
    visiting: &mut HashSet<Uuid>,
    visited: &mut HashSet<Uuid>,
) -> bool {
    if visited.contains(&node) {
        return false;
    }
    if !visiting.insert(node) {
        return true;
    }
    let cycle = adjacency.get(&node).is_some_and(|neighbors| {
        neighbors
            .iter()
            .any(|next| has_cycle_from(*next, adjacency, visiting, visited))
    });
    visiting.remove(&node);
    visited.insert(node);
    cycle
}

fn is_valid_json_pointer(pointer: &str) -> bool {
    if pointer.is_empty() {
        return true;
    }
    if !pointer.starts_with('/') {
        return false;
    }
    for token in pointer.split('/').skip(1) {
        let bytes = token.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != b'~' {
                index += 1;
                continue;
            }
            if index + 1 >= bytes.len() || !matches!(bytes[index + 1], b'0' | b'1') {
                return false;
            }
            index += 2;
        }
    }
    true
}

fn diagnostic(code: &str, message: impl Into<String>, span: SourceSpan) -> Diagnostic {
    Diagnostic {
        code: code.to_string(),
        severity: DiagnosticSeverity::Error,
        message: message.into(),
        span: Some(span),
    }
}

fn node_diagnostic(
    code: &str,
    message: impl Into<String>,
    node: &FlowNode,
    suffix: &str,
) -> Diagnostic {
    diagnostic(
        code,
        message,
        semantic_span(node.span.as_ref(), node_path(node.id, suffix)),
    )
}

fn edge_diagnostic(code: &str, message: impl Into<String>, edge: &FlowEdge) -> Diagnostic {
    diagnostic(code, message, schema_span(edge_path(edge)))
}

fn semantic_span(source: Option<&SourceSpan>, path: impl Into<String>) -> SourceSpan {
    SourceSpan {
        start: source.map_or(0, |span| span.start),
        end: source.map_or(0, |span| span.end),
        path: Some(path.into()),
    }
}

fn schema_span(path: impl Into<String>) -> SourceSpan {
    semantic_span(None, path)
}

fn node_path(node_id: Uuid, suffix: &str) -> String {
    format!("/flow/nodes/{node_id}/config{suffix}")
}

fn edge_path(edge: &FlowEdge) -> String {
    format!(
        "/flow/edges/{}/{}/{}/{}",
        edge.from.node_id,
        pointer_token(&edge.from.handle),
        edge.to.node_id,
        pointer_token(&edge.to.handle)
    )
}

fn pointer_token(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|left, right| {
        diagnostic_path(left)
            .cmp(diagnostic_path(right))
            .then_with(|| diagnostic_start(left).cmp(&diagnostic_start(right)))
            .then_with(|| left.code.as_bytes().cmp(right.code.as_bytes()))
            .then_with(|| left.message.as_bytes().cmp(right.message.as_bytes()))
    });
}

fn diagnostic_path(diagnostic: &Diagnostic) -> &str {
    diagnostic
        .span
        .as_ref()
        .and_then(|span| span.path.as_deref())
        .unwrap_or("")
}

fn diagnostic_start(diagnostic: &Diagnostic) -> usize {
    diagnostic.span.as_ref().map_or(0, |span| span.start)
}
