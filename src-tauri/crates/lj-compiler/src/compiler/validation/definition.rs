//! Definition header 与节点 config 合同。

use super::super::{
    BTreeSet, CollectionSelector, ConditionConfig, ConditionPredicate, ControlExpression,
    ControlledMapper, Diagnostic, FlowNode, FlowNodeConfig, MAX_LOOP_ITERATIONS, MergeConfig,
    RuleDefinition, TypedLiteral, diagnostic, is_valid_json_pointer, node_diagnostic,
    pointer_token, schema_span, unavailable_capability_diagnostic,
};

pub(in crate::compiler) fn validate_definition_header(
    definition: &RuleDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
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

pub(in crate::compiler) fn validate_node_configuration(
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
        FlowNodeConfig::Unavailable(config) => {
            diagnostics.push(unavailable_capability_diagnostic(node, config));
        }
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

pub(in crate::compiler) fn validate_mapper(
    node: &FlowNode,
    mapper: &ControlledMapper,
    diagnostics: &mut Vec<Diagnostic>,
) {
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

pub(in crate::compiler) fn validate_merge_config(
    node: &FlowNode,
    config: &MergeConfig,
    diagnostics: &mut Vec<Diagnostic>,
) {
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

pub(in crate::compiler) fn validate_condition_config(
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

pub(in crate::compiler) fn require_network(
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
