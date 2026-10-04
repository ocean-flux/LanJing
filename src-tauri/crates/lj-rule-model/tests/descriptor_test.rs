//! descriptor 是编辑器渲染、port 与默认值的唯一来源：本文件锁住该不变量。

use std::path::PathBuf;

use lj_rule_model::budget::JsBudget;
use lj_rule_model::definition::FlowNodeConfig;
use lj_rule_model::descriptor::{
    DefaultConfig, FieldDescriptor, FieldEditor, NodeDescriptor, PortDescriptor, PortHandleSource,
    PortLabelDescriptor, PortRole, PortSide, PortValueSource, VariantKind, descriptor_for,
    descriptor_for_wire, node_descriptor_set, node_descriptors, resolve_config_ports,
    resolve_ports,
};
use lj_rule_model::plan::{LINEAR_INPUT_HANDLE, PortValueKind, PortValueType};
use serde_json::{Value, json};

/// 内置 kind 的 wire 名；覆盖表完整性用。
const BUILTIN_KINDS: [&str; 7] = [
    "http",
    "js",
    "extract",
    "mapper",
    "merge",
    "condition",
    "loop",
];

#[test]
fn every_builtin_kind_has_exactly_one_descriptor() {
    assert_eq!(node_descriptors().len(), BUILTIN_KINDS.len());
    for name in BUILTIN_KINDS {
        let descriptor = descriptor_for_wire(name).expect("内置 kind 必须有 descriptor");
        assert_eq!(descriptor.kind.wire_name(), name);
        assert_eq!(
            node_descriptors()
                .iter()
                .filter(|candidate| candidate.kind == descriptor.kind)
                .count(),
            1
        );
        assert_eq!(descriptor_for(descriptor.kind).kind, descriptor.kind);
        assert!(!descriptor.fields.is_empty(), "{name} 必须声明字段");
    }
    assert!(descriptor_for_wire("legacy_reader").is_none());
}

#[test]
fn descriptor_defaults_are_valid_typed_configs() {
    for descriptor in node_descriptors() {
        let default = descriptor.default_config.value();
        let wire = json!({
            "kind": descriptor.kind.wire_name(),
            "value": default,
        });
        let config: FlowNodeConfig =
            serde_json::from_value(wire).expect("descriptor 默认值必须是合法 typed config");
        assert_eq!(config.kind(), Some(descriptor.kind));
        assert_eq!(serde_json::to_value(&config).unwrap()["value"], default);
    }
}

#[test]
fn descriptor_is_the_only_port_source_for_dynamic_ports() {
    // Merge：input handle 由 config 的 inputs[].handle 派生。
    let merge = descriptor_for_wire("merge").unwrap();
    let resolved = resolve_ports(merge, &merge.default_config.value());
    let handles: Vec<&str> = resolved
        .inputs
        .iter()
        .map(|port| port.handle.as_str())
        .collect();
    assert_eq!(handles, ["in:0", "in:1"]);
    assert_eq!(resolved.inputs[0].label.index, Some(1));
    assert_eq!(resolved.inputs[1].label.index, Some(2));

    // Condition：output handle 由 config 的 branches 派生。
    let condition = descriptor_for_wire("condition").unwrap();
    let resolved = resolve_ports(condition, &json!({ "branches": ["alpha", "beta"] }));
    let handles: Vec<&str> = resolved
        .outputs
        .iter()
        .map(|port| port.handle.as_str())
        .collect();
    assert_eq!(handles, ["alpha", "beta"]);
    assert_eq!(
        resolved.outputs[1].label.key,
        "rules_port_label_condition_branch"
    );

    // Js：output value kind 由 config 的 output 字段派生。
    let js = descriptor_for_wire("js").unwrap();
    for (wire, kind) in [("json", PortValueKind::Json), ("raw", PortValueKind::Raw)] {
        let resolved = resolve_ports(js, &json!({ "output": wire }));
        assert_eq!(resolved.outputs[0].value_type, PortValueType::kind(kind));
    }

    // Http：线性 input 的 closed union 与 kind 判别都来自声明。
    let http = descriptor_for_wire("http").unwrap();
    let resolved = resolve_ports(http, &http.default_config.value());
    assert_eq!(resolved.inputs[0].handle, LINEAR_INPUT_HANDLE);
}

#[test]
fn resolve_config_ports_reads_the_same_source_as_the_compiler() {
    for descriptor in node_descriptors() {
        let wire = json!({
            "kind": descriptor.kind.wire_name(),
            "value": descriptor.default_config.value(),
        });
        let config: FlowNodeConfig = serde_json::from_value(wire).unwrap();
        let resolved = resolve_config_ports(&config).expect("内置 kind 必须可解析 port");
        assert_eq!(
            resolved,
            resolve_ports(descriptor, &descriptor.default_config.value())
        );
        assert!(
            !resolved.inputs.is_empty() || !resolved.outputs.is_empty(),
            "{} 必须声明 port",
            descriptor.kind.wire_name()
        );
    }
    let unavailable: FlowNodeConfig = serde_json::from_value(json!({
        "kind": "legacy_reader",
        "value": { "whatever": true },
    }))
    .unwrap();
    assert!(resolve_config_ports(&unavailable).is_none());
}

#[test]
fn descriptor_for_wire_is_a_generic_lookup() {
    assert_eq!(
        descriptor_for_wire("http").map(|descriptor| descriptor.icon),
        Some("broadcast")
    );
}

/// 能力描述路径不区分内置与新增：这里登记一个「非内置 kind」的能力，走同一套
/// port/字段/默认值解析，证明新增能力不需要新的 enum arm 或新的 dispatch 分支。
#[test]
fn a_new_capability_needs_only_a_descriptor_entry() {
    const NEW_INPUTS: &[PortDescriptor] = &[PortDescriptor {
        handle: PortHandleSource::Items { field: "sources" },
        value: PortValueSource::Kind {
            kind: PortValueKind::Json,
        },
        role: PortRole::Data,
        side: PortSide::Left,
        label: PortLabelDescriptor {
            key: "rules_port_label_json_input",
            literal: false,
            numbered: true,
        },
    }];
    const NEW_OUTPUTS: &[PortDescriptor] = &[PortDescriptor {
        handle: PortHandleSource::Fixed { handle: "feed" },
        value: PortValueSource::FieldVariant {
            field: "flavour",
            variants: &[
                VariantKind {
                    value: "json",
                    kind: PortValueKind::Json,
                    label_key: "rules_port_label_json_output",
                },
                VariantKind {
                    value: "raw",
                    kind: PortValueKind::Raw,
                    label_key: "rules_port_label_raw_output",
                },
            ],
        },
        role: PortRole::Data,
        side: PortSide::Right,
        label: PortLabelDescriptor {
            key: "rules_port_label_json_output",
            literal: false,
            numbered: false,
        },
    }];
    const NEW_FIELDS: &[FieldDescriptor] = &[FieldDescriptor {
        name: "sources",
        label_key: "rules_node_inspector_field_inputs",
        editor: FieldEditor::StringList {
            item_label_key: "rules_node_inspector_field_inputs",
            add_label_key: "rules_node_inspector_identity_field_add",
            remove_label_key: "rules_node_inspector_identity_field_remove",
            min_items: 0,
        },
        required: true,
    }];
    let descriptor = NodeDescriptor {
        kind: lj_rule_model::definition::FlowNodeKind::Merge,
        label_key: "rules_node_inspector_type_merge",
        description_key: "rules_node_desc_merge",
        icon: "git-merge",
        inputs: NEW_INPUTS,
        outputs: NEW_OUTPUTS,
        fields: NEW_FIELDS,
        default_config: DefaultConfig(|| json!({ "sources": ["a", "b"], "flavour": "raw" })),
    };

    let resolved = resolve_ports(&descriptor, &descriptor.default_config.value());
    assert_eq!(
        resolved
            .inputs
            .iter()
            .map(|port| port.handle.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    assert_eq!(resolved.outputs[0].handle, "feed");
    assert_eq!(
        resolved.outputs[0].value_type,
        PortValueType::kind(PortValueKind::Raw)
    );
    assert_eq!(
        serde_json::to_value(descriptor.fields[0].editor).unwrap()["editor"],
        json!("string_list")
    );
}

/// JS 预算字段的编辑器上下界与默认值都是 host policy 上限的投影。
///
/// descriptor 里的数字是字面量（`const` 数组的限制），本测试把它们钉回
/// [`JsBudget::HOST_CEILING`]：两处一旦漂移，编辑器会让人写出必然被后端夹回的值。
#[test]
fn js_budget_field_bounds_are_the_host_ceiling() {
    let descriptor = descriptor_for_wire("js").expect("js 必须有 descriptor");
    let editor = descriptor
        .fields
        .iter()
        .find(|field| field.name == "budgets")
        .expect("js 必须声明 budgets 字段")
        .editor;
    let FieldEditor::Numbers { fields } = editor else {
        panic!("budgets 必须是 numbers 编辑器");
    };
    let bounds: Vec<(&str, i64, i64)> = fields
        .iter()
        .map(|field| (field.name, field.min, field.max))
        .collect();
    assert_eq!(
        bounds,
        vec![
            (
                "timeout_ms",
                1,
                i64::from(JsBudget::HOST_CEILING.timeout_ms)
            ),
            (
                "memory_bytes",
                1,
                i64::try_from(JsBudget::HOST_CEILING.memory_bytes).unwrap()
            ),
            (
                "output_bytes",
                1,
                i64::try_from(JsBudget::HOST_CEILING.output_bytes).unwrap()
            ),
        ]
    );

    let default = descriptor.default_config.value();
    assert_eq!(
        default["budgets"],
        json!({
            "timeout_ms": JsBudget::HOST_CEILING.timeout_ms,
            "memory_bytes": JsBudget::HOST_CEILING.memory_bytes,
            "output_bytes": JsBudget::HOST_CEILING.output_bytes,
        })
    );
}

/// 前端 fixture 与 Rust 声明表同源：Rust 改变声明时该测试失败, 用
/// `UPDATE_NODE_DESCRIPTOR_GOLDEN=1` 重新生成后前端测试也跟着更新。
#[test]
fn node_descriptor_golden_matches_frontend_fixture() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../src/features/rules/model/__fixtures__/node-descriptors.json");
    let produced = serde_json::to_string_pretty(&node_descriptor_set()).unwrap();
    if std::env::var("UPDATE_NODE_DESCRIPTOR_GOLDEN").is_ok_and(|value| value == "1") {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, format!("{produced}\n")).unwrap();
        return;
    }
    let golden = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("缺少前端 fixture {}: {error}", path.display()));
    let golden: Value = serde_json::from_str(&golden).unwrap();
    let produced: Value = serde_json::from_str(&produced).unwrap();
    assert_eq!(produced, golden, "descriptor 声明与前端 fixture 不一致");
}
