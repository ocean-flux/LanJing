//! 节点能力 descriptor：编辑器渲染、port 与默认值的唯一来源。
//!
//! 新增规则能力只需在 [`node_descriptors`] 登记一条声明：compiler port、编辑器字段、
//! 字段标签与默认值都由同一份声明派生, 不需要新的前端页面分支或 runtime dispatch 分支。
//! 本模块只出稳定 code 与 raw 文本, 不出本地化文案（视图层查 `messages/*/rules.json`）。

use serde::{Serialize, Serializer};
use serde_json::Value;

use crate::budget::JsBudget;
use crate::definition::FlowNodeKind;
use crate::plan::{
    CONDITION_INPUT_HANDLE, LINEAR_INPUT_HANDLE, LINEAR_OUTPUT_HANDLE, LOOP_BODY_HANDLE,
    LOOP_COLLECTION_HANDLE, LOOP_DONE_HANDLE, LOOP_YIELD_HANDLE, MERGE_OUTPUT_HANDLE,
    PortValueKind, PortValueType,
};

/// port 在规则图中的结构角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PortRole {
    /// 承载数据值。
    Data,
    /// 承载控制流。
    Control,
    /// 承载 loop binding。
    Binding,
}

/// port 在节点边界上的语义侧位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PortSide {
    /// 左侧。
    Left,
    /// 右侧。
    Right,
    /// 上侧。
    Top,
    /// 下侧。
    Bottom,
}

/// port 展示标签声明。
///
/// `key` 为 `messages/*/rules.json` 的稳定 key；`literal` 为真时 `key` 是 compiler
/// 术语原文，不翻译。`numbered` 为真时视图补 1-based 序号。
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PortLabelDescriptor {
    /// message key（`literal` 为假）或原文（`literal` 为真）。
    pub key: &'static str,
    /// 是否不翻译。
    pub literal: bool,
    /// 是否按位置编号。
    pub numbered: bool,
}

impl PortLabelDescriptor {
    const fn message(key: &'static str) -> Self {
        Self {
            key,
            literal: false,
            numbered: false,
        }
    }

    const fn numbered_message(key: &'static str) -> Self {
        Self {
            key,
            literal: false,
            numbered: true,
        }
    }

    const fn literal(text: &'static str) -> Self {
        Self {
            key: text,
            literal: true,
            numbered: false,
        }
    }
}

/// 一个 port 的 handle 来源。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum PortHandleSource {
    /// 固定 handle。
    Fixed {
        /// handle 名。
        handle: &'static str,
    },
    /// config 中的字符串数组，每项一个 handle。
    Items {
        /// config 字段名。
        field: &'static str,
    },
    /// config 中的对象数组，每项取该字段作为 handle。
    ItemField {
        /// config 字段名。
        field: &'static str,
        /// 数组元素里承载 handle 的字段名。
        handle_field: &'static str,
    },
}

/// 一个 port 的 value type 来源。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum PortValueSource {
    /// 固定单一 kind。
    Kind {
        /// closed kind。
        kind: PortValueKind,
    },
    /// 固定 closed union。
    Union {
        /// union 成员（canonical sort 由调用方保证）。
        kinds: &'static [PortValueKind],
    },
    /// 由 config 中的枚举字段选择 kind。
    FieldVariant {
        /// config 字段名。
        field: &'static str,
        /// wire 值 → kind 映射；未命中时取第一项。
        variants: &'static [VariantKind],
    },
}

/// `FieldVariant` 的一项映射。
#[derive(Debug, Clone, Copy, Serialize)]
pub struct VariantKind {
    /// config 字段的 wire 值。
    pub value: &'static str,
    /// 对应的 closed kind。
    pub kind: PortValueKind,
    /// 该变体的 port 标签 message key。
    pub label_key: &'static str,
}

/// 节点上的一个声明式 port。
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PortDescriptor {
    /// handle 来源。
    pub handle: PortHandleSource,
    /// value type 来源。
    pub value: PortValueSource,
    /// 编辑器结构角色。
    pub role: PortRole,
    /// 编辑器侧位。
    pub side: PortSide,
    /// 编辑器标签。
    pub label: PortLabelDescriptor,
}

/// 字段的编辑器种类（闭集）。
///
/// `Specialized` 按 `editor_kind` 选择视图层已注册的结构化子编辑器，仍然与具体节点
/// kind 无关：新增能力复用已有 `editor_kind` 即可。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "editor", rename_all = "snake_case")]
pub enum FieldEditor {
    /// 单行文本。
    Text,
    /// 多行源码 / 表达式。
    Code,
    /// 数值输入。
    Number {
        /// 允许下界。
        min: Option<i64>,
        /// 允许上界。
        max: Option<i64>,
    },
    /// 一组带上下界的命名数值子字段（资源预算这类 object 字段）。
    ///
    /// 子字段名字、上下界都来自声明，视图不做任何节点专属分支。
    Numbers {
        /// 子字段声明，顺序即渲染顺序。
        fields: &'static [NumberFieldDescriptor],
    },
    /// 枚举下拉。
    Select {
        /// 选项（值是 Rust 侧枚举变体名，标签为原文，不本地化）。
        options: &'static [SelectOptionDescriptor],
    },
    /// 字符串列表。
    StringList {
        /// 单项 aria-label 的 message key。
        item_label_key: &'static str,
        /// 新增按钮的 message key。
        add_label_key: &'static str,
        /// 删除按钮的 message key。
        remove_label_key: &'static str,
        /// 少于该数量时禁止删除。
        min_items: u32,
    },
    /// key/value 对列表。
    PairList {
        /// key 列标题的 message key。
        key_label_key: &'static str,
        /// value 列标题的 message key。
        value_label_key: &'static str,
        /// 新增按钮的 message key。
        add_label_key: &'static str,
        /// 删除按钮的 message key。
        remove_label_key: &'static str,
    },
    /// 视图层已注册的结构化子编辑器。
    Specialized {
        /// 子编辑器名。
        editor_kind: &'static str,
    },
}

/// 下拉选项。
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SelectOptionDescriptor {
    /// 写入 config 的值。
    pub value: &'static str,
    /// 展示文本（原文，不本地化）。
    pub label: &'static str,
}

/// [`FieldEditor::Numbers`] 里的一个数值子字段。
#[derive(Debug, Clone, Copy, Serialize)]
pub struct NumberFieldDescriptor {
    /// config 对象里的字段名。
    pub name: &'static str,
    /// 标签的 message key。
    pub label_key: &'static str,
    /// 允许下界。
    pub min: i64,
    /// 允许上界。
    pub max: i64,
}

/// 节点上的一个声明式配置字段。
#[derive(Debug, Clone, Copy, Serialize)]
pub struct FieldDescriptor {
    /// config 字段名。
    pub name: &'static str,
    /// 标签的 message key。
    pub label_key: &'static str,
    /// 编辑器种类。
    pub editor: FieldEditor,
    /// 是否必填（视图层用于提示，不代替 Rust 校验）。
    pub required: bool,
}

/// 新节点的默认 config：唯一来源，且必须是本 kind 的合法 typed config。
#[derive(Clone, Copy)]
pub struct DefaultConfig(pub fn() -> Value);

impl std::fmt::Debug for DefaultConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DefaultConfig")
    }
}

impl DefaultConfig {
    /// 取一份新的默认 config。
    #[must_use]
    pub fn value(self) -> Value {
        (self.0)()
    }
}

impl Serialize for DefaultConfig {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0().serialize(serializer)
    }
}

/// 一个规则能力的完整声明。
#[derive(Debug, Clone, Copy, Serialize)]
pub struct NodeDescriptor {
    /// 节点判别值。
    pub kind: FlowNodeKind,
    /// 节点类型标签的 message key。
    pub label_key: &'static str,
    /// 节点说明的 message key。
    pub description_key: &'static str,
    /// 图标白名单名。
    pub icon: &'static str,
    /// 声明式输入 port。
    pub inputs: &'static [PortDescriptor],
    /// 声明式输出 port。
    pub outputs: &'static [PortDescriptor],
    /// 声明式配置字段。
    pub fields: &'static [FieldDescriptor],
    /// 新节点默认 config。
    pub default_config: DefaultConfig,
}

/// 已解析的一个 port：handle 与 value type 已落定，编辑器元数据一并带出。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedPort {
    /// 落定后的 handle。
    pub handle: String,
    /// 落定后的 closed value type。
    pub value_type: PortValueType,
    /// 编辑器结构角色。
    pub role: PortRole,
    /// 编辑器侧位。
    pub side: PortSide,
    /// 编辑器标签。
    pub label: ResolvedPortLabel,
}

/// 落定后的 port 标签。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedPortLabel {
    /// message key 或原文。
    pub key: String,
    /// 是否不翻译。
    pub literal: bool,
    /// 1-based 序号；`None` 表示标签不带序号。
    pub index: Option<usize>,
}

/// 节点已解析的 port 集合。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ResolvedPorts {
    /// 输入 port。
    pub inputs: Vec<ResolvedPort>,
    /// 输出 port。
    pub outputs: Vec<ResolvedPort>,
}

/// 按 config 解析一条 port: handle 展开与 value kind 落定。
fn resolve_port(port: &PortDescriptor, config: &Value, position: usize) -> Vec<ResolvedPort> {
    let (value_type, variant_label) = resolve_value(port.value, config);
    let label_key = variant_label.unwrap_or(port.label.key);
    let label = |index: Option<usize>| ResolvedPortLabel {
        key: label_key.to_owned(),
        literal: port.label.literal,
        index: if port.label.numbered {
            index.or(Some(position + 1))
        } else {
            None
        },
    };
    let handles = resolve_handles(port.handle, config);
    match handles {
        Some(handles) if !handles.is_empty() => handles
            .into_iter()
            .enumerate()
            .map(|(offset, handle)| ResolvedPort {
                handle,
                value_type: value_type.clone(),
                role: port.role,
                side: port.side,
                label: label(Some(position + offset + 1)),
            })
            .collect(),
        Some(_) => Vec::new(),
        None => vec![ResolvedPort {
            handle: fixed_handle(port.handle),
            value_type,
            role: port.role,
            side: port.side,
            label: label(None),
        }],
    }
}

/// `Some(handles)` 表示 handle 由 config 派生（可能为空，表示无 port）。
fn resolve_handles(source: PortHandleSource, config: &Value) -> Option<Vec<String>> {
    match source {
        PortHandleSource::Fixed { .. } => None,
        PortHandleSource::Items { field } => Some(
            config
                .get(field)
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(ToOwned::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
        ),
        PortHandleSource::ItemField {
            field,
            handle_field,
        } => Some(
            config
                .get(field)
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| {
                            item.get(handle_field)
                                .and_then(Value::as_str)
                                .map(ToOwned::to_owned)
                        })
                        .collect()
                })
                .unwrap_or_default(),
        ),
    }
}

fn fixed_handle(source: PortHandleSource) -> String {
    match source {
        PortHandleSource::Fixed { handle } => handle.to_owned(),
        PortHandleSource::Items { .. } | PortHandleSource::ItemField { .. } => String::new(),
    }
}

fn resolve_value(source: PortValueSource, config: &Value) -> (PortValueType, Option<&'static str>) {
    match source {
        PortValueSource::Kind { kind } => (PortValueType::kind(kind), None),
        PortValueSource::Union { kinds } => (PortValueType::union(kinds.iter().copied()), None),
        PortValueSource::FieldVariant { field, variants } => {
            let selected = config.get(field).and_then(Value::as_str);
            let variant = variants
                .iter()
                .find(|variant| Some(variant.value) == selected)
                .or_else(|| variants.first());
            match variant {
                Some(variant) => (PortValueType::kind(variant.kind), Some(variant.label_key)),
                None => (PortValueType::kind(PortValueKind::Json), None),
            }
        }
    }
}

/// 按节点 typed config 解析全部 port；未安装能力返回 `None`。
///
/// config 的 wire envelope（`kind`/`value`）只在本 crate 解一次，调用方不需要知道。
#[must_use]
pub fn resolve_config_ports(config: &crate::definition::FlowNodeConfig) -> Option<ResolvedPorts> {
    let descriptor = descriptor_for_wire(config.kind()?.wire_name())?;
    let wire = serde_json::to_value(config).ok()?;
    let payload = wire.get("value").cloned().unwrap_or(Value::Null);
    Some(resolve_ports(descriptor, &payload))
}

/// 按 config 解析一个节点的全部 port。
#[must_use]
pub fn resolve_ports(descriptor: &NodeDescriptor, config: &Value) -> ResolvedPorts {
    let resolve = |ports: &'static [PortDescriptor]| {
        ports
            .iter()
            .enumerate()
            .flat_map(|(position, port)| resolve_port(port, config, position))
            .collect()
    };
    ResolvedPorts {
        inputs: resolve(descriptor.inputs),
        outputs: resolve(descriptor.outputs),
    }
}

/// 全部内置节点能力的声明表。
#[must_use]
pub fn node_descriptors() -> &'static [NodeDescriptor] {
    DESCRIPTORS
}

/// IPC 载荷：规则编辑器一次取回全部能力声明。
#[derive(Debug, Clone, Copy, Serialize)]
pub struct NodeDescriptorSet {
    /// 全部声明。
    pub descriptors: &'static [NodeDescriptor],
}

/// 构造 IPC 载荷。
#[must_use]
pub const fn node_descriptor_set() -> NodeDescriptorSet {
    NodeDescriptorSet {
        descriptors: DESCRIPTORS,
    }
}

/// 按判别值查声明。
///
/// # Panics
///
/// 表中缺少该 kind 时 panic；覆盖性由 `every_builtin_kind_has_exactly_one_descriptor`
/// 测试保证，调用方不需要重复处理缺失。
#[must_use]
pub fn descriptor_for(kind: FlowNodeKind) -> &'static NodeDescriptor {
    node_descriptors()
        .iter()
        .find(|descriptor| descriptor.kind == kind)
        .expect("每个内置节点 kind 都必须登记 descriptor")
}

/// 按 wire 名称查声明；未安装能力返回 `None`。
#[must_use]
pub fn descriptor_for_wire(kind: &str) -> Option<&'static NodeDescriptor> {
    node_descriptors()
        .iter()
        .find(|descriptor| descriptor.kind.wire_name() == kind)
}

const CONTROLLED_INPUT: PortValueSource = PortValueSource::Union {
    kinds: &[
        PortValueKind::IntentInput,
        PortValueKind::Raw,
        PortValueKind::Json,
        PortValueKind::LoopBinding,
    ],
};

const LINEAR_INPUT: PortDescriptor = PortDescriptor {
    handle: PortHandleSource::Fixed {
        handle: LINEAR_INPUT_HANDLE,
    },
    value: CONTROLLED_INPUT,
    role: PortRole::Data,
    side: PortSide::Left,
    label: PortLabelDescriptor::literal("input"),
};

/// HTTP 节点是规则入口: 输入承接 intent entry、循环体绑定与透传原文, 但不接受
/// Json 数据连边。
const HTTP_INPUT_VALUE: PortValueSource = PortValueSource::Union {
    kinds: &[
        PortValueKind::IntentInput,
        PortValueKind::Raw,
        PortValueKind::LoopBinding,
    ],
};

const HTTP_INPUT: PortDescriptor = PortDescriptor {
    handle: PortHandleSource::Fixed {
        handle: LINEAR_INPUT_HANDLE,
    },
    value: HTTP_INPUT_VALUE,
    role: PortRole::Data,
    side: PortSide::Left,
    label: PortLabelDescriptor::literal("input"),
};

const HTTP_INPUTS: &[PortDescriptor] = &[HTTP_INPUT];
const HTTP_OUTPUTS: &[PortDescriptor] = &[PortDescriptor {
    handle: PortHandleSource::Fixed {
        handle: LINEAR_OUTPUT_HANDLE,
    },
    value: PortValueSource::Kind {
        kind: PortValueKind::HttpResponse,
    },
    role: PortRole::Data,
    side: PortSide::Right,
    label: PortLabelDescriptor::message("rules_port_label_http_response"),
}];

const JS_INPUTS: &[PortDescriptor] = &[PortDescriptor {
    label: PortLabelDescriptor::message("rules_port_label_js_input"),
    ..LINEAR_INPUT
}];
const JS_OUTPUTS: &[PortDescriptor] = &[PortDescriptor {
    handle: PortHandleSource::Fixed {
        handle: LINEAR_OUTPUT_HANDLE,
    },
    value: PortValueSource::FieldVariant {
        field: "output",
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
    label: PortLabelDescriptor::message("rules_port_label_json_output"),
}];

const EXTRACT_INPUTS: &[PortDescriptor] = &[PortDescriptor {
    handle: PortHandleSource::Fixed {
        handle: LINEAR_INPUT_HANDLE,
    },
    value: PortValueSource::Kind {
        kind: PortValueKind::HttpResponse,
    },
    role: PortRole::Data,
    side: PortSide::Left,
    label: PortLabelDescriptor::message("rules_port_label_http_response"),
}];
const EXTRACT_OUTPUTS: &[PortDescriptor] = &[PortDescriptor {
    handle: PortHandleSource::Fixed {
        handle: LINEAR_OUTPUT_HANDLE,
    },
    value: PortValueSource::Kind {
        kind: PortValueKind::Json,
    },
    role: PortRole::Data,
    side: PortSide::Right,
    label: PortLabelDescriptor::message("rules_port_label_extract_result"),
}];

const MAPPER_INPUTS: &[PortDescriptor] = &[PortDescriptor {
    handle: PortHandleSource::Fixed {
        handle: LINEAR_INPUT_HANDLE,
    },
    value: PortValueSource::Kind {
        kind: PortValueKind::Json,
    },
    role: PortRole::Data,
    side: PortSide::Left,
    label: PortLabelDescriptor::message("rules_port_label_json_input"),
}];
const MAPPER_OUTPUTS: &[PortDescriptor] = &[PortDescriptor {
    handle: PortHandleSource::Fixed {
        handle: LINEAR_OUTPUT_HANDLE,
    },
    value: PortValueSource::Kind {
        kind: PortValueKind::Delta,
    },
    role: PortRole::Data,
    side: PortSide::Right,
    label: PortLabelDescriptor::message("rules_port_label_delta_output"),
}];

const MERGE_INPUTS: &[PortDescriptor] = &[PortDescriptor {
    handle: PortHandleSource::ItemField {
        field: "inputs",
        handle_field: "handle",
    },
    value: PortValueSource::Kind {
        kind: PortValueKind::Json,
    },
    role: PortRole::Data,
    side: PortSide::Left,
    label: PortLabelDescriptor::numbered_message("rules_port_label_merge_input"),
}];
const MERGE_OUTPUTS: &[PortDescriptor] = &[PortDescriptor {
    handle: PortHandleSource::Fixed {
        handle: MERGE_OUTPUT_HANDLE,
    },
    value: PortValueSource::Kind {
        kind: PortValueKind::Json,
    },
    role: PortRole::Data,
    side: PortSide::Right,
    label: PortLabelDescriptor::message("rules_port_label_merge_result"),
}];

const CONDITION_INPUTS: &[PortDescriptor] = &[PortDescriptor {
    handle: PortHandleSource::Fixed {
        handle: CONDITION_INPUT_HANDLE,
    },
    value: PortValueSource::Kind {
        kind: PortValueKind::Json,
    },
    role: PortRole::Data,
    side: PortSide::Left,
    label: PortLabelDescriptor::message("rules_port_label_condition_input"),
}];
const CONDITION_OUTPUTS: &[PortDescriptor] = &[PortDescriptor {
    handle: PortHandleSource::Items { field: "branches" },
    value: PortValueSource::Kind {
        kind: PortValueKind::Json,
    },
    role: PortRole::Control,
    side: PortSide::Right,
    label: PortLabelDescriptor::numbered_message("rules_port_label_condition_branch"),
}];

const LOOP_INPUTS: &[PortDescriptor] = &[
    PortDescriptor {
        handle: PortHandleSource::Fixed {
            handle: LOOP_COLLECTION_HANDLE,
        },
        value: PortValueSource::Kind {
            kind: PortValueKind::Json,
        },
        role: PortRole::Data,
        side: PortSide::Left,
        label: PortLabelDescriptor::literal("collection"),
    },
    PortDescriptor {
        handle: PortHandleSource::Fixed {
            handle: LOOP_YIELD_HANDLE,
        },
        value: PortValueSource::Kind {
            kind: PortValueKind::Json,
        },
        role: PortRole::Control,
        side: PortSide::Top,
        label: PortLabelDescriptor::literal("yield(value)"),
    },
];
const LOOP_OUTPUTS: &[PortDescriptor] = &[
    PortDescriptor {
        handle: PortHandleSource::Fixed {
            handle: LOOP_BODY_HANDLE,
        },
        value: PortValueSource::Kind {
            kind: PortValueKind::LoopBinding,
        },
        role: PortRole::Binding,
        side: PortSide::Bottom,
        label: PortLabelDescriptor::literal("body(item,index)"),
    },
    PortDescriptor {
        handle: PortHandleSource::Fixed {
            handle: LOOP_DONE_HANDLE,
        },
        value: PortValueSource::Kind {
            kind: PortValueKind::Json,
        },
        role: PortRole::Control,
        side: PortSide::Right,
        label: PortLabelDescriptor::literal("done(collected)"),
    },
];

const fn field(
    name: &'static str,
    label_key: &'static str,
    editor: FieldEditor,
) -> FieldDescriptor {
    FieldDescriptor {
        name,
        label_key,
        editor,
        required: true,
    }
}

const HTTP_FIELDS: &[FieldDescriptor] = &[
    field(
        "method",
        "rules_node_inspector_field_method",
        FieldEditor::Select {
            options: &[
                SelectOptionDescriptor {
                    value: "Get",
                    label: "GET",
                },
                SelectOptionDescriptor {
                    value: "Post",
                    label: "POST",
                },
            ],
        },
    ),
    field(
        "url",
        "rules_node_inspector_field_url_template",
        FieldEditor::Text,
    ),
    field(
        "expected_type",
        "rules_node_inspector_field_expected_type",
        EXPECTED_TYPE_EDITOR,
    ),
    field(
        "charset",
        "rules_node_inspector_field_charset",
        FieldEditor::Text,
    ),
    field("body", "rules_node_inspector_field_body", FieldEditor::Code),
    field(
        "headers",
        "rules_node_inspector_field_headers",
        FieldEditor::PairList {
            key_label_key: "rules_node_inspector_header_name",
            value_label_key: "rules_node_inspector_header_value",
            add_label_key: "rules_node_inspector_header_add",
            remove_label_key: "rules_node_inspector_header_remove",
        },
    ),
];

const EXPECTED_TYPE_EDITOR: FieldEditor = FieldEditor::Select {
    options: &[
        SelectOptionDescriptor {
            value: "Html",
            label: "HTML",
        },
        SelectOptionDescriptor {
            value: "Xml",
            label: "XML",
        },
        SelectOptionDescriptor {
            value: "Json",
            label: "JSON",
        },
    ],
};

const JS_FIELDS: &[FieldDescriptor] = &[
    field(
        "output",
        "rules_node_inspector_field_output",
        FieldEditor::Select {
            options: &[
                SelectOptionDescriptor {
                    value: "json",
                    label: "JSON",
                },
                SelectOptionDescriptor {
                    value: "raw",
                    label: "RAW",
                },
            ],
        },
    ),
    field(
        "code",
        "rules_node_inspector_field_script",
        FieldEditor::Code,
    ),
    field(
        "budgets",
        "rules_node_inspector_field_budgets",
        FieldEditor::Numbers {
            fields: JS_BUDGET_FIELDS,
        },
    ),
];

/// JS 资源预算的三个子字段：名字与上下界都是合同边界的投影。
///
/// 上下界写成字面量是因为 `const` 数组里没有无 panic 的 `u64` → `i64` 转换；
/// 与 [`JsBudget`] 的一致性由 `js_budget_field_bounds_are_the_host_ceiling` 测试钉住。
const JS_BUDGET_FIELDS: &[NumberFieldDescriptor] = &[
    NumberFieldDescriptor {
        name: "timeout_ms",
        label_key: "rules_node_inspector_budget_timeout_ms",
        min: 1,
        max: 5_000,
    },
    NumberFieldDescriptor {
        name: "memory_bytes",
        label_key: "rules_node_inspector_budget_memory_bytes",
        min: 1_048_576,
        max: 16_777_216,
    },
    NumberFieldDescriptor {
        name: "output_bytes",
        label_key: "rules_node_inspector_budget_output_bytes",
        min: 1,
        max: 1_048_576,
    },
];

const EXTRACT_FIELDS: &[FieldDescriptor] = &[
    field(
        "expected_type",
        "rules_node_inspector_field_expected_type",
        EXPECTED_TYPE_EDITOR,
    ),
    field(
        "output_target",
        "rules_node_inspector_field_output_target",
        FieldEditor::Select {
            options: &[
                SelectOptionDescriptor {
                    value: "Media",
                    label: "Media",
                },
                SelectOptionDescriptor {
                    value: "Units",
                    label: "Units",
                },
                SelectOptionDescriptor {
                    value: "Asset",
                    label: "Asset",
                },
            ],
        },
    ),
    field(
        "rules",
        "rules_node_inspector_field_rules_hint",
        FieldEditor::Specialized {
            editor_kind: "extract_rules",
        },
    ),
];

const MAPPER_FIELDS: &[FieldDescriptor] = &[
    field(
        "output",
        "rules_node_inspector_field_output",
        FieldEditor::Select {
            options: &[
                SelectOptionDescriptor {
                    value: "items",
                    label: "Items",
                },
                SelectOptionDescriptor {
                    value: "discovery",
                    label: "Discovery",
                },
                SelectOptionDescriptor {
                    value: "units",
                    label: "Units",
                },
                SelectOptionDescriptor {
                    value: "assets",
                    label: "Assets",
                },
            ],
        },
    ),
    field(
        "identity_fields",
        "rules_node_inspector_field_identity_fields",
        FieldEditor::StringList {
            item_label_key: "rules_node_inspector_field_identity_fields",
            add_label_key: "rules_node_inspector_identity_field_add",
            remove_label_key: "rules_node_inspector_identity_field_remove",
            min_items: 0,
        },
    ),
];

const MERGE_FIELDS: &[FieldDescriptor] = &[
    field(
        "strategy",
        "rules_node_inspector_field_strategy",
        FieldEditor::Select {
            options: &[
                SelectOptionDescriptor {
                    value: "single_active",
                    label: "Single active",
                },
                SelectOptionDescriptor {
                    value: "collect_array",
                    label: "Collect array",
                },
                SelectOptionDescriptor {
                    value: "concat_arrays",
                    label: "Concat arrays",
                },
                SelectOptionDescriptor {
                    value: "overlay_objects",
                    label: "Overlay objects",
                },
            ],
        },
    ),
    field(
        "inputs",
        "rules_node_inspector_field_inputs",
        FieldEditor::Specialized {
            editor_kind: "merge_inputs",
        },
    ),
];

const CONDITION_FIELDS: &[FieldDescriptor] = &[
    field(
        "branches",
        "rules_node_inspector_field_branches",
        FieldEditor::StringList {
            item_label_key: "rules_node_inspector_branch_handle",
            add_label_key: "rules_node_inspector_branch_add",
            remove_label_key: "rules_node_inspector_branch_remove",
            min_items: 2,
        },
    ),
    field(
        "expression",
        "rules_node_inspector_field_expression",
        FieldEditor::Specialized {
            editor_kind: "condition_expression",
        },
    ),
];

const LOOP_FIELDS: &[FieldDescriptor] = &[
    field(
        "collection",
        "rules_node_inspector_field_collection",
        FieldEditor::Specialized {
            editor_kind: "loop_collection",
        },
    ),
    field(
        "item_binding",
        "rules_node_inspector_field_item_binding",
        FieldEditor::Text,
    ),
    field(
        "index_binding",
        "rules_node_inspector_field_index_binding",
        FieldEditor::Text,
    ),
    field(
        "max_iterations",
        "rules_node_inspector_field_max_iterations",
        FieldEditor::Number {
            min: Some(1),
            max: Some(256),
        },
    ),
];

static DESCRIPTORS: &[NodeDescriptor] = &[
    NodeDescriptor {
        kind: FlowNodeKind::Http,
        label_key: "rules_node_inspector_type_http",
        description_key: "rules_node_desc_http",
        icon: "broadcast",
        inputs: HTTP_INPUTS,
        outputs: HTTP_OUTPUTS,
        fields: HTTP_FIELDS,
        default_config: DefaultConfig(|| {
            serde_json::json!({
                "method": "Get",
                "url": "",
                "headers": {},
                "body": null,
                "charset": null,
                "expected_type": "Html",
            })
        }),
    },
    NodeDescriptor {
        kind: FlowNodeKind::Js,
        label_key: "rules_node_inspector_type_js",
        description_key: "rules_node_desc_js",
        icon: "code",
        inputs: JS_INPUTS,
        outputs: JS_OUTPUTS,
        fields: JS_FIELDS,
        default_config: DefaultConfig(|| {
            serde_json::json!({
                "code": "",
                "output": "json",
                "budgets": {
                    "timeout_ms": JsBudget::HOST_CEILING.timeout_ms,
                    "memory_bytes": JsBudget::HOST_CEILING.memory_bytes,
                    "output_bytes": JsBudget::HOST_CEILING.output_bytes,
                },
            })
        }),
    },
    NodeDescriptor {
        kind: FlowNodeKind::Extract,
        label_key: "rules_node_inspector_type_extract",
        description_key: "rules_node_desc_extract",
        icon: "tree-structure",
        inputs: EXTRACT_INPUTS,
        outputs: EXTRACT_OUTPUTS,
        fields: EXTRACT_FIELDS,
        default_config: DefaultConfig(|| {
            serde_json::json!({
                "rules": [],
                "field_rules": {},
                "expected_type": "Html",
                "output_target": "Media",
            })
        }),
    },
    NodeDescriptor {
        kind: FlowNodeKind::Mapper,
        label_key: "rules_node_inspector_type_mapper",
        description_key: "rules_node_desc_mapper",
        icon: "translate",
        inputs: MAPPER_INPUTS,
        outputs: MAPPER_OUTPUTS,
        fields: MAPPER_FIELDS,
        default_config: DefaultConfig(
            || serde_json::json!({ "output": "items", "identity_fields": [] }),
        ),
    },
    NodeDescriptor {
        kind: FlowNodeKind::Merge,
        label_key: "rules_node_inspector_type_merge",
        description_key: "rules_node_desc_merge",
        icon: "git-merge",
        inputs: MERGE_INPUTS,
        outputs: MERGE_OUTPUTS,
        fields: MERGE_FIELDS,
        default_config: DefaultConfig(|| {
            serde_json::json!({
                "inputs": [
                    {
                        "input_id": "input_1",
                        "handle": "in:0",
                        "order": 0,
                        "activation": "required",
                    },
                    {
                        "input_id": "input_2",
                        "handle": "in:1",
                        "order": 1,
                        "activation": "optional",
                    },
                ],
                "strategy": "single_active",
            })
        }),
    },
    NodeDescriptor {
        kind: FlowNodeKind::Condition,
        label_key: "rules_node_inspector_type_condition",
        description_key: "rules_node_desc_condition",
        icon: "compass",
        inputs: CONDITION_INPUTS,
        outputs: CONDITION_OUTPUTS,
        fields: CONDITION_FIELDS,
        default_config: DefaultConfig(|| {
            serde_json::json!({
                "branches": ["true", "false"],
                "expression": {
                    "mode": "typed",
                    "predicate": { "operator": "exists", "pointer": "" },
                    "true_branch": "true",
                    "false_branch": "false",
                },
            })
        }),
    },
    NodeDescriptor {
        kind: FlowNodeKind::Loop,
        label_key: "rules_node_inspector_type_loop",
        description_key: "rules_node_desc_loop",
        icon: "arrow-counter-clockwise",
        inputs: LOOP_INPUTS,
        outputs: LOOP_OUTPUTS,
        fields: LOOP_FIELDS,
        default_config: DefaultConfig(|| {
            serde_json::json!({
                "collection": { "mode": "typed", "pointer": "" },
                "item_binding": "item",
                "index_binding": "index",
                "max_iterations": 64,
            })
        }),
    },
];
