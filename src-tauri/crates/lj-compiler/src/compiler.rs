//! Definition → immutable Execution Plan 的确定性编译。
//!
//! 本模块只处理作者合同的规范化、校验与编译；不解析来源专有格式，也不依赖
//! runtime、存储或 Tauri。所有可执行语义必须先经本模块写入 `ExecutionPlan`。

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

use blake3::Hasher;
use lj_rule_model::{
    CapabilityManifest, ControlledMapper, Diagnostic, DiagnosticSeverity, EffectDeclaration,
    EffectKind, ExecutionPlan, FlowNode, FlowNodeKind, IntentEntry, PlanNode, PlanNodeKind,
    PlanPort, RuleDefinition, SourceSpan, canonical_json, definition_hash,
};
use uuid::Uuid;

use crate::error::CompilerError;

/// 默认 compiler 身份；其值参与 Plan hash。
pub const DEFAULT_COMPILER_VERSION: &str = concat!("lj-compiler@", env!("CARGO_PKG_VERSION"));

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

    /// 校验并将 Definition 编译成 immutable Plan。
    ///
    /// # Errors
    ///
    /// 当 Definition 不满足端口、可达性、能力或有界控制流合同，或序列化 Plan
    /// 失败时返回 [`CompilerError`]。
    pub fn compile(&self, definition: &RuleDefinition) -> Result<ExecutionPlan, CompilerError> {
        let definition = canonicalize(definition);
        let diagnostics = validate(&definition);
        if diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        {
            return Err(CompilerError::validation(diagnostics));
        }

        build_plan(&definition, &self.version)
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::with_version(DEFAULT_COMPILER_VERSION.to_string())
    }
}

/// 将作者 Definition 规范化为与节点、边声明顺序无关的形式。
///
/// `FlowNode` 与 `FlowEdge` 的排列仅是编辑器展示细节，不能影响产物 hash；字段内
/// 本身有语义顺序的列表则保持原样。
#[must_use]
pub fn canonicalize(definition: &RuleDefinition) -> RuleDefinition {
    let mut canonical = definition.clone();
    canonical.flow.nodes.sort_by_key(|node| node.id);
    canonical.flow.edges.sort_by(|left, right| {
        (left.from, left.to, &left.condition_branch).cmp(&(
            right.from,
            right.to,
            &right.condition_branch,
        ))
    });
    canonical
}

/// 返回 Definition 的全部可定位诊断。
///
/// 本函数不丢弃后续错误，以便调用方一次修复所有作者合同问题。
#[must_use]
pub fn validate(definition: &RuleDefinition) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut nodes = BTreeMap::new();

    if definition.source_identity.id.trim().is_empty() {
        diagnostics.push(error(
            "SOURCE_IDENTITY_REQUIRED",
            "来源稳定身份不能为空",
            None,
        ));
    }
    if definition
        .source_id_rules
        .iter()
        .all(|rule| rule.trim().is_empty())
    {
        diagnostics.push(error(
            "SOURCE_ID_RULES_REQUIRED",
            "Definition 必须声明来源持有的稳定 ID 规则",
            None,
        ));
    }
    if definition.intent_exports.is_empty() {
        diagnostics.push(error(
            "INTENT_EXPORT_REQUIRED",
            "Definition 至少需要一个标准意图入口",
            None,
        ));
    }

    for node in &definition.flow.nodes {
        if nodes.insert(node.id, node).is_some() {
            diagnostics.push(error(
                "DUPLICATE_NODE_ID",
                format!("节点 {} 重复声明", node.id),
                node.span.clone(),
            ));
        }
        validate_node_configuration(node, &definition.capability_manifest, &mut diagnostics);
    }

    let adjacency = adjacency(&definition.flow.edges);
    for edge in &definition.flow.edges {
        let from = nodes.get(&edge.from);
        let to = nodes.get(&edge.to);
        match (from, to) {
            (Some(from), Some(to)) => {
                let (_, output) = ports(&from.kind);
                let (input, _) = ports(&to.kind);
                if !ports_are_compatible(output, input) {
                    diagnostics.push(error(
                        "PORT_TYPE_MISMATCH",
                        format!(
                            "节点 {} 的 {output} 输出不能连接到节点 {} 的 {input} 输入",
                            from.id, to.id
                        ),
                        to.span.clone(),
                    ));
                }
            }
            (None, Some(to)) => diagnostics.push(error(
                "EDGE_SOURCE_MISSING",
                format!("边起点 {} 不存在", edge.from),
                to.span.clone(),
            )),
            (Some(from), None) => diagnostics.push(error(
                "EDGE_TARGET_MISSING",
                format!("边终点 {} 不存在", edge.to),
                from.span.clone(),
            )),
            (None, None) => diagnostics.push(error(
                "EDGE_ENDPOINT_MISSING",
                format!("边 {} → {} 的两端都不存在", edge.from, edge.to),
                None,
            )),
        }
    }

    if has_cycle(&nodes, &adjacency) {
        diagnostics.push(error(
            "FLOW_UNBOUNDED",
            "Flow 含有循环；当前 Definition 没有可验证的循环上界",
            None,
        ));
    }

    for node in nodes.values() {
        if node.kind == FlowNodeKind::Loop {
            diagnostics.push(error(
                "FLOW_UNBOUNDED",
                "Loop 节点必须提供 compiler 可验证的上界；当前模型不支持无界 Loop",
                node.span.clone(),
            ));
        }
    }

    for (intent, export) in &definition.intent_exports {
        let entry = nodes.get(&export.flow_entry);
        let mapper = nodes.get(&export.mapper_output);
        if entry.is_none() {
            diagnostics.push(error(
                "INTENT_ENTRY_MISSING",
                format!("{intent:?} 的入口节点 {} 不存在", export.flow_entry),
                None,
            ));
        }
        match mapper {
            Some(node) if node.kind == FlowNodeKind::Mapper => {}
            Some(node) => diagnostics.push(error(
                "INTENT_MAPPER_INVALID",
                format!("{intent:?} 的输出节点 {} 不是 Mapper", node.id),
                node.span.clone(),
            )),
            None => diagnostics.push(error(
                "INTENT_MAPPER_MISSING",
                format!("{intent:?} 的 Mapper 节点 {} 不存在", export.mapper_output),
                None,
            )),
        }
        if entry.is_some()
            && mapper.is_some()
            && !is_reachable(&adjacency, export.flow_entry, export.mapper_output)
        {
            diagnostics.push(error(
                "MAPPER_UNREACHABLE",
                format!(
                    "{intent:?} 的入口 {} 无法到达 Mapper {}",
                    export.flow_entry, export.mapper_output
                ),
                mapper.and_then(|node| node.span.clone()),
            ));
        }
    }

    for mapper in nodes
        .values()
        .filter(|node| node.kind == FlowNodeKind::Mapper)
    {
        let declared = definition
            .intent_exports
            .values()
            .any(|export| export.mapper_output == mapper.id);
        let reachable = definition
            .intent_exports
            .values()
            .any(|export| is_reachable(&adjacency, export.flow_entry, mapper.id));
        if !declared || !reachable {
            diagnostics.push(error(
                "MAPPER_UNREACHABLE",
                format!("Mapper 节点 {} 未被可达的标准意图导出使用", mapper.id),
                mapper.span.clone(),
            ));
        }
    }

    diagnostics
}

fn build_plan(
    definition: &RuleDefinition,
    compiler_version: &str,
) -> Result<ExecutionPlan, CompilerError> {
    let definition_hash = definition_hash(definition)
        .map_err(|error| CompilerError::Serialization(error.to_string()))?;
    let mut effects = Vec::new();
    let mut nodes = Vec::with_capacity(definition.flow.nodes.len());

    for flow_node in &definition.flow.nodes {
        let kind = plan_kind(&flow_node.kind);
        if let Some(effect_kind) = effect_kind(&kind) {
            let required_capabilities = match effect_kind {
                EffectKind::Http | EffectKind::QuickJs => vec!["network".to_string()],
                EffectKind::Extract => Vec::new(),
            };
            effects.push(EffectDeclaration {
                node_id: flow_node.id,
                kind: effect_kind,
                required_capabilities,
            });
        }
        let (input, output) = ports(&flow_node.kind);
        nodes.push(PlanNode {
            id: flow_node.id,
            kind,
            inputs: vec![PlanPort {
                name: "input".to_string(),
                type_tag: input.to_string(),
            }],
            outputs: vec![PlanPort {
                name: "output".to_string(),
                type_tag: output.to_string(),
            }],
            config: node_config(flow_node)?,
        });
    }

    effects.sort_by_key(|effect| effect.node_id);
    let capability_requirements = effects
        .iter()
        .flat_map(|effect| effect.required_capabilities.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let intent_entries = definition
        .intent_exports
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
    let mut plan = ExecutionPlan {
        schema_version: definition.schema_version,
        compiler_version: compiler_version.to_string(),
        definition_hash,
        plan_hash: String::new(),
        nodes,
        edges: definition
            .flow
            .edges
            .iter()
            .map(|edge| (edge.from, edge.to))
            .collect(),
        intent_entries,
        effects,
        capability_requirements,
    };
    plan.plan_hash = plan_hash(&plan)?;
    Ok(plan)
}

fn node_config(node: &FlowNode) -> Result<serde_json::Value, CompilerError> {
    let missing = || CompilerError::Internal(format!("已校验节点 {} 仍缺少对应配置", node.id));
    let config = match node.kind {
        FlowNodeKind::Http => serde_json::to_value(node.http.as_ref().ok_or_else(missing)?)
            .map_err(|error| CompilerError::Serialization(error.to_string()))?,
        FlowNodeKind::Js => serde_json::json!({
            "code": node.js_code.as_ref().ok_or_else(missing)?,
        }),
        FlowNodeKind::Extract => {
            serde_json::to_value(node.extract.as_ref().ok_or_else(missing)?)
                .map_err(|error| CompilerError::Serialization(error.to_string()))?
        }
        FlowNodeKind::Mapper => serde_json::to_value(node.mapper.as_ref().ok_or_else(missing)?)
            .map_err(|error| CompilerError::Serialization(error.to_string()))?,
        FlowNodeKind::Merge | FlowNodeKind::Condition | FlowNodeKind::Loop => {
            return Err(CompilerError::Internal(format!(
                "未支持的控制流节点 {} 不应通过校验",
                node.id
            )));
        }
    };
    Ok(config)
}

fn plan_hash(plan: &ExecutionPlan) -> Result<String, CompilerError> {
    let canonical =
        canonical_json(plan).map_err(|error| CompilerError::Serialization(error.to_string()))?;
    let mut hasher = Hasher::new();
    hasher.update(canonical.as_bytes());
    Ok(hasher.finalize().to_hex().to_string())
}

fn validate_node_configuration(
    node: &FlowNode,
    manifest: &CapabilityManifest,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let invalid = |message: String, diagnostics: &mut Vec<Diagnostic>| {
        diagnostics.push(error("NODE_CONFIG_MISMATCH", message, node.span.clone()));
    };
    match node.kind {
        FlowNodeKind::Http => {
            if node.http.is_none() {
                invalid(format!("HTTP 节点 {} 缺少 HTTP 配置", node.id), diagnostics);
            }
            require_network(node, manifest, diagnostics);
        }
        FlowNodeKind::Js => {
            if node
                .js_code
                .as_deref()
                .is_none_or(|code| code.trim().is_empty())
            {
                invalid(format!("JS 节点 {} 缺少脚本", node.id), diagnostics);
            }
            require_network(node, manifest, diagnostics);
        }
        FlowNodeKind::Extract => {
            if node.extract.is_none() {
                invalid(
                    format!("Extract 节点 {} 缺少提取配置", node.id),
                    diagnostics,
                );
            }
        }
        FlowNodeKind::Mapper => match node.mapper.as_ref() {
            Some(mapper) => validate_mapper(node, mapper, diagnostics),
            None => invalid(
                format!("Mapper 节点 {} 缺少 mapper 配置", node.id),
                diagnostics,
            ),
        },
        FlowNodeKind::Merge | FlowNodeKind::Condition => diagnostics.push(error(
            "CONTROL_FLOW_UNSUPPORTED",
            format!("节点 {} 的控制流尚无 Plan runtime 语义", node.id),
            node.span.clone(),
        )),
        FlowNodeKind::Loop => {}
    }
}

fn require_network(
    node: &FlowNode,
    manifest: &CapabilityManifest,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !manifest.required.network {
        diagnostics.push(error(
            "CAPABILITY_MISMATCH",
            format!(
                "节点 {} 需要 network 能力，但 capability manifest 未声明该能力",
                node.id
            ),
            node.span.clone(),
        ));
    }
}

fn validate_mapper(node: &FlowNode, mapper: &ControlledMapper, diagnostics: &mut Vec<Diagnostic>) {
    if mapper
        .identity_fields
        .iter()
        .all(|field| field.trim().is_empty())
    {
        diagnostics.push(error(
            "MAPPER_IDENTITY_REQUIRED",
            format!("Mapper 节点 {} 必须声明至少一个稳定身份字段", node.id),
            node.span.clone(),
        ));
    }
}

fn error(code: &str, message: impl Into<String>, span: Option<SourceSpan>) -> Diagnostic {
    Diagnostic {
        code: code.to_string(),
        severity: DiagnosticSeverity::Error,
        message: message.into(),
        span,
    }
}

fn plan_kind(kind: &FlowNodeKind) -> PlanNodeKind {
    match kind {
        FlowNodeKind::Http => PlanNodeKind::Http,
        FlowNodeKind::Js => PlanNodeKind::Js,
        FlowNodeKind::Extract => PlanNodeKind::Extract,
        FlowNodeKind::Mapper => PlanNodeKind::Mapper,
        FlowNodeKind::Merge => PlanNodeKind::Merge,
        FlowNodeKind::Condition => PlanNodeKind::Condition,
        FlowNodeKind::Loop => PlanNodeKind::Loop,
    }
}

fn effect_kind(kind: &PlanNodeKind) -> Option<EffectKind> {
    match kind {
        PlanNodeKind::Http => Some(EffectKind::Http),
        PlanNodeKind::Js => Some(EffectKind::QuickJs),
        PlanNodeKind::Extract => Some(EffectKind::Extract),
        PlanNodeKind::Mapper
        | PlanNodeKind::Merge
        | PlanNodeKind::Condition
        | PlanNodeKind::Loop => None,
    }
}

fn ports(kind: &FlowNodeKind) -> (&'static str, &'static str) {
    match kind {
        FlowNodeKind::Http => ("value", "http_response"),
        FlowNodeKind::Js => ("value", "json"),
        FlowNodeKind::Extract => ("http_response", "json"),
        FlowNodeKind::Mapper => ("json", "delta"),
        FlowNodeKind::Merge | FlowNodeKind::Condition | FlowNodeKind::Loop => ("json", "json"),
    }
}

fn ports_are_compatible(output: &str, input: &str) -> bool {
    output == input || (input == "value" && matches!(output, "raw" | "json"))
}

fn adjacency(edges: &[lj_rule_model::FlowEdge]) -> BTreeMap<Uuid, Vec<Uuid>> {
    let mut result = BTreeMap::<Uuid, Vec<Uuid>>::new();
    for edge in edges {
        result.entry(edge.from).or_default().push(edge.to);
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

fn has_cycle(nodes: &BTreeMap<Uuid, &FlowNode>, adjacency: &BTreeMap<Uuid, Vec<Uuid>>) -> bool {
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    nodes
        .keys()
        .copied()
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
