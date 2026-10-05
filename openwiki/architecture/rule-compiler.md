---
type: "参考"
title: "Rule compiler"
openwiki_generated: true
sources:
  - id: openwiki-source-6dd3778e30e69901511a90eb
    resource: repo://src-tauri/crates/lj-compiler/src/compiler.rs
  - id: openwiki-source-4ff03ff2d47317cfa9ac4904
    resource: repo://src-tauri/crates/lj-compiler/src/compiler/analysis.rs
  - id: openwiki-source-08cd7a55f146f80c64882d95
    resource: repo://src-tauri/crates/lj-compiler/src/compiler/canonicalize.rs
  - id: openwiki-source-eb031d6187de241112ff410e
    resource: repo://src-tauri/crates/lj-compiler/src/compiler/diagnostics.rs
  - id: openwiki-source-f6036fbecfaefaf9ee945b5c
    resource: repo://src-tauri/crates/lj-compiler/src/compiler/lowering.rs
  - id: openwiki-source-692ddc65a04b0175c68366fb
    resource: repo://src-tauri/crates/lj-compiler/src/compiler/ports.rs
  - id: openwiki-source-dd16adecb23cf3bd901202ec
    resource: repo://src-tauri/crates/lj-compiler/src/compiler/validation/control.rs
  - id: openwiki-source-d27d10d277ef08477509b505
    resource: repo://src-tauri/crates/lj-compiler/src/compiler/validation/definition.rs
  - id: openwiki-source-817dc201106794f0ddfac379
    resource: repo://src-tauri/crates/lj-compiler/src/compiler/validation/edges.rs
  - id: openwiki-source-56ab91a0968241e6d0b8f3f9
    resource: repo://src-tauri/crates/lj-compiler/src/compiler/validation/intents.rs
  - id: openwiki-source-d041b5c64c26ec960e037424
    resource: repo://src-tauri/crates/lj-compiler/src/compiler/validation/mod.rs
  - id: openwiki-source-4caca92d4e21267e01fb63e8
    resource: repo://src-tauri/crates/lj-compiler/src/error.rs
  - id: openwiki-source-f1968d8e9f0190b5f35679bb
    resource: repo://src-tauri/crates/lj-compiler/src/lib.rs
  - id: openwiki-source-012692312089ec8143764f4d
    resource: repo://src-tauri/crates/lj-compiler/tests/plan_compiler_test.rs
  - id: openwiki-source-bdf4f88ce68a67aa50cdcf1f
    resource: repo://src-tauri/crates/lj-rule-model/src/descriptor.rs
  - id: openwiki-source-568a13748f69a4d9b494241b
    resource: repo://src-tauri/crates/lj-rule-model/src/plan/contract.rs
  - id: openwiki-source-f4d8e498f8a27d9242bfdc4b
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/document.rs
  - id: openwiki-source-80e8762cff162b2ecd16e129
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/prepare_install.rs
generated: { by: "pi", at: "2026-10-05T08:37:16.392Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-05T08:37:16.392Z
---


## 职责与依赖边界

`lj-compiler` 只做三件事: 规范化作者合同、校验并产出可定位诊断、把 `RuleDefinition` 编译成不可变的 `ExecutionPlan`。它不解析来源专有格式(Legado 语法只由 importer adapter 处理), 也不依赖 runtime、存储或 Tauri(`src-tauri/crates/lj-compiler/src/lib.rs#L1-L4`)。

crate 明确不维护第二套节点/端口清单, 不做 schema 版本分支, 不读取 legacy 定义; 节点端口**不在这里声明**, 而是读 `lj-rule-model::descriptor` 的唯一声明表, 这里只留 closed value-kind compatibility matrix, 而 Plan 的写入始终由 `lj-rule-model` 的 current 构造器封存(`src-tauri/crates/lj-compiler/src/compiler/ports.rs#L1-L8`, `src-tauri/crates/lj-compiler/src/compiler.rs#L1-L6`)。

`DEFAULT_COMPILER_VERSION` 是 `lj-compiler@<CARGO_PKG_VERSION>`, **它参与 Plan hash**, 因此升级 compiler 版本会让同一 Definition 产出不同的 plan hash(`src-tauri/crates/lj-compiler/src/compiler.rs#L53-L54`)。

## 编译流水线

`Compiler::compile` 的顺序是固定的四步(`src-tauri/crates/lj-compiler/src/compiler.rs#L84-L96`):

```text
RuleDefinition
  → canonicalize()      语义无损排序, 与物理声明顺序解耦     canonicalize.rs
  → analyze()           单遍构建 nodes/ports/edges/regions + 诊断   analysis.rs
       ├─ 存在 Error 级诊断 → CompilerError::Validation(全部诊断)
  → build_plan()        定义 hash + effects + intent entries + regions → ExecutionPlan::new() 封存 plan_hash
```

`validate()` 是同一分析的只读入口: 它同样先 `canonicalize`, 返回全部诊断而不短路, 因此「同一非法语义不会因编辑器数组顺序不同而改变诊断顺序」(`src-tauri/crates/lj-compiler/src/compiler.rs#L105-L112`)。

`CompilerError::Validation { diagnostics }` 携带**完整**错误列表而非第一个错误, 并通过 `diagnostics()` 暴露给上层, 目标是让作者一次修完而不是逐个试(`src-tauri/crates/lj-compiler/src/error.rs#L21-L55`)。

## canonicalize: 语义无损排序

规范化只调整三类真正无序的字段(`src-tauri/crates/lj-compiler/src/compiler/canonicalize.rs#L5-L33`):

- 节点按 `id` 排序, 语义边整体 `sort`(四元组 identity)。
- `Merge` 的 inputs 按显式 `order` 排序——这是**语义**字段。
- `Condition` 的 branches 按原始 UTF-8 bytes 排序。

`Mapper` 的 identity fields 等真正有序的字段保持原样; 物理数组排列与 editor layout 永不进入 hash。这条规则是「拖动节点、重排声明不改变规则身份」的实现基础。

## analyze: 单遍分析

`analyze()` 只走一遍, 收集诊断并顺带产出可复用的中间结构(`NodePorts`、`ValidatedLoopRegion`)(`src-tauri/crates/lj-compiler/src/compiler/analysis.rs#L29-L92`):

1. `validate_definition_header` — 来源身份与 intent exports 的存在性。
2. 逐节点: 重复 id 记为 `DUPLICATE_NODE_ID` 并跳过该节点, 否则 `validate_node_configuration` 与 `ports_for_node`。
3. `validate_edges` 返回**通过端点与句柄检查的边**集合, 后续控制区与意图分析只在这个集合上做。
4. `validate_input_and_control_handles` — 输入句柄歧义与控制句柄计数。
5. `validate_loop_regions` 求出 Loop 控制区与 backedge, 再 `validate_loop_region_overlap` 拒绝区域重叠。
6. 把已接受的 backedge 从邻接表里排除后检查是否仍有环, 有则报 `FLOW_CYCLE_UNSTRUCTURED`(裸循环)。
7. 用**完整**邻接表 `validate_intent_exports`(意图入口必须可达其 Mapper)。
8. `sort_diagnostics`, 并把 Loop 区按 `loop_node` 排序后作为 `ControlRegion::Loop` 交给 lowering。

顺序本身是设计: 边校验的结果决定后续分析的可信输入域, 避免在已失效的图上产生二次误报。

## 端口声明与兼容矩阵

端口不再由 compiler 声明: `ports_for_node` 调 `lj_rule_model::descriptor::resolve_config_ports(&node.config)`, 把 descriptor 的声明解析成 Plan 端口; 解析不出(节点能力未安装)返回空端口。compiler 与规则编辑器因此读同一份声明, 新增一个节点能力不需要改这里, 只需在 descriptor 里加一条(`src-tauri/crates/lj-compiler/src/compiler/ports.rs#L1-L29`)。

解析出的端口形状:

| 节点 | 输入 handle | 输出 handle |
| --- | --- | --- |
| Http | linear | linear(HttpResponse) |
| Js | linear | linear(Json 或 Raw, 由 `JsOutputKind` 决定) |
| Extract | linear(HttpResponse) | linear(Json) |
| Mapper | linear(Json) | linear(Delta) |
| Merge | 各 `input.handle` 的并集(Json) | merge output(Json) |
| Condition | condition input(Json) | 每个 branch 一个句柄(Json) |
| Loop | collection(Json), yield(Json) | body(LoopBinding), done(Json) |

除 Merge(顺序由显式 `order` 决定)外, 端口按 handle 的 bytes 排序, 只为稳定展示, 不承载语义。

兼容规则不对称: `ports_are_compatible(output, input)` 里 **Union 类型的输出一律不被接受**, 只有 `Kind` 输出才能匹配; 而输入侧既可以是 `Kind` 也可以是 `Union`(二分查找)(`src-tauri/crates/lj-compiler/src/compiler/ports.rs#L30-L47`)。linear 入口的值类型是 `IntentInput|Raw|Json|LoopBinding` 的 union, 声明在 descriptor 的 `LINEAR_INPUT`; Http 入口单独收窄成 `IntentInput|Raw|LoopBinding`(`HTTP_INPUT_VALUE`), 即只去掉 `Json` —— 既保留既有 compiler 连边(`LOOP body → Http`), 又让 `Extract(Json) → Http` 在编辑器里判不兼容(`src-tauri/crates/lj-rule-model/src/descriptor.rs`)。

## 四类校验分区

`validation/` 把检查按关注点分成四组, 共用同一份分析结果(`src-tauri/crates/lj-compiler/src/compiler/validation/mod.rs#L1-L11`):

| 分区 | 关注点 | 代表性诊断 code |
| --- | --- | --- |
| `definition.rs` | 来源身份、节点配置、Mapper/Merge/Condition 语义、未安装能力 | `SOURCE_IDENTITY_REQUIRED`, `NODE_CONFIG_INVALID`, `MAPPER_IDENTITY_INVALID`, `MERGE_INPUT_DUPLICATE`, `MERGE_ORDER_INVALID`, `CONDITION_BRANCH_DUPLICATE`, `NODE_CAPABILITY_UNAVAILABLE` |
| `edges.rs` | 端点存在性、句柄存在性、端口类型、输入歧义、控制句柄计数 | `EDGE_SOURCE_MISSING`, `DUPLICATE_EDGE`, `SOURCE_HANDLE_MISSING`, `PORT_TYPE_MISMATCH`, `INPUT_HANDLE_AMBIGUOUS`, `LOOP_BODY_INVALID` |
| `control.rs` | Loop 区的 body/yield/done、跨区边、嵌套与重叠 | `LOOP_BODY_BYPASS`, `LOOP_YIELD_UNREACHABLE`, `LOOP_CROSS_REGION_EDGE`, `LOOP_NESTING_UNSUPPORTED`, `LOOP_REGION_OVERLAP` |
| `intents.rs` | 标准 intent 入口、Mapper 归属与可达性 | `INTENT_ENTRY_MISSING`, `INTENT_ENTRY_PORT_MISMATCH`, `INTENT_MAPPER_INVALID`, `MAPPER_UNREACHABLE` |

网络不再是编译期能力: 网络不受 capability 控制, compiler 既没有 `require_network`, 也不再产出 `network` 能力声明。编译期剩下的能力类检查只有「节点声明了未安装的能力」, 由 `unavailable_capability_diagnostic` 产出 `NODE_CAPABILITY_UNAVAILABLE` 并阻止它被降成 Plan(`src-tauri/crates/lj-compiler/src/compiler/validation/definition.rs#L73-L78`, `src-tauri/crates/lj-compiler/src/compiler/diagnostics.rs#L32-L40`)。系统能力(fs/env/process)是安装期合同, 不在编译期逐节点检查。

`intents.rs` 还反向检查: 任何 Mapper 节点若未被任何一个可达的标准意图导出使用, 也会报 `MAPPER_UNREACHABLE`——即「图里存在无主的 Mapper」是错误而不是警告(`src-tauri/crates/lj-compiler/src/compiler/validation/intents.rs#L72-L92`)。

## 诊断为什么必须稳定可寻址

诊断的 `span.path` 是 JSON Pointer, 由两类构造器生成(`src-tauri/crates/lj-compiler/src/compiler/diagnostics.rs#L39-L70`):

- 节点类: `/flow/nodes/<uuid>/config<suffix>`, suffix 指向具体字段(如 `/input`)。
- 边类: `/flow/edges/<from-node>/<from-handle>/<to-node>/<to-handle>`, handle 经过 `~` → `~0`、`/` → `~1` 转义。

对合同本身的检查没有源码位置, 用 `schema_span` 产出 `start = end = 0` 但带 path 的 span, 因此**前端可以只靠 path 定位到表单字段**; 节点类诊断若节点带 `SourceSpan`, 则保留原始 start/end 并覆盖 path(`src-tauri/crates/lj-compiler/src/compiler/diagnostics.rs#L18-L52`)。

排序键是 `path → span.start → code → message`(`src-tauri/crates/lj-compiler/src/compiler/diagnostics.rs#L72-L80`), 加上入口处先 canonicalize 声明顺序, 保证同一非法语义在不同编辑器状态下列出同样的顺序——UI 高亮与测试断言都依赖这一点。

诊断消息只引用节点 id、意图名与 code, 不把 Js 控制脚本源码回显进诊断(`plan_compiler_test.rs` 的 `control_script_source_is_not_echoed_in_diagnostics` 断言该行为, `src-tauri/crates/lj-compiler/tests/plan_compiler_test.rs#L448`)。

## lowering 与 hash

`build_plan` 负责把已验证的定义降成 Plan IR(`src-tauri/crates/lj-compiler/src/compiler/lowering.rs#L10-L59`):

- 先算 `definition_hash`(来自 `lj-rule-model`, 失败映射为 `CompilerError::Serialization`)。
- `lower_nodes` 同时产出 `PlanNode` 与 `EffectDeclaration`; effect 由节点类型推导, 且 `Js` 节点、Js 条件表达式、Js 循环集合三者都会产生 `EffectKind::QuickJs`(`#L95-L113`)。
- `PlanNode` 不再声明所需能力(`required_capabilities: Vec::new()`); Plan 的 `capability_requirements` 只由 effect 声明的所需能力去重排序而来, 而内置 Http/QuickJs effect 已不再声明 `network`, 因此当前恒为空(`#L19-L24`, `#L68-L72`)。
- effect 按 `(node_id, kind rank)` 排序, 保证 Plan 内部顺序确定(`#L87-L91`)。
- `IntentEntry` 从 `definition.intent_exports()` 逐项搬运, 边的四元组直接映射为 `PlanEdge`。
- Loop 的 `max_iterations` 在这里被转成受限类型 `LoopIterationLimit::new(...)`, 越界即 `Serialization` 失败(`#L128-L137`)。

最后 `ExecutionPlan::new(compiler_version, definition_hash, parts)` 在构造器末尾封存 `plan_hash` 与 `descriptor_digest`: plan hash 覆盖 typed config、端口、边与 control region, descriptor digest 取自 `descriptor_set_digest()`, 执行方在跑之前按同一份声明表重算并与 Plan 对比(`src-tauri/crates/lj-rule-model/src/plan/contract.rs#L28-L52`)。

## 失败如何被调用方接住

编译失败不是内部错误, 而是三种不同语义的对外结果:

- **保存规则文档**: 保存路径先 `canonicalize` + `validate`; 校验失败保留 Draft Revision、不替换 Effective Rule Revision(`src-tauri/crates/lj-rule-system/src/system/lifecycle/document.rs#L400-L417`; 对应测试 `invalid_save_preserves_effective_rule_and_records_draft`, `#L1532`)。
- **只读预览**: `validate_native_rule_document` 在 `CompilerError::Validation` 时返回 `valid: false`、`plan_hash: None` 并附全部诊断, 而不是抛错(`src-tauri/crates/lj-rule-system/src/system/lifecycle/document.rs#L417-L446`)。
- **准备来源安装**: `stage_prepared_candidate` 把 `canonicalize` + `validate` + `compile` 串起来, 编译错误经 `compiler_error(..)` 映射成带 stage `Candidate` 的 `RuleError` 并附带诊断; 随后还要过 `runtime.validate_plan(&plan)` 才能生成候选(`src-tauri/crates/lj-rule-system/src/system/lifecycle/prepare_install.rs#L159-L190`)。

也就是说: 规则文档场景失败是**可保存的草稿**, 来源安装场景失败是**不可继续的错误**。同一份 Definition 在两条链路上的后果不同, 这是有意的产品语义。

## 测试契约

`tests/plan_compiler_test.rs`(883 行)是 compiler 的行为契约, 断言的可观察结果集中在 hash 稳定性与诊断可定位性:

| 测试 | 断言 |
| --- | --- |
| `base_linear_graph_writes_only_current_typed_plan` | 只产出 current typed plan |
| `good_seven_node_graph_compiles_typed_configs_edges_ports_and_loop_region` | 七节点图的 config/边/端口/Loop 区完整 |
| `typed_and_js_condition_and_typed_and_js_loop_emit_explicit_quickjs_effects` | Js 条件与 Js 循环集合都产出 QuickJS effect |
| `control_script_source_is_not_echoed_in_diagnostics` | 控制脚本源码不进入诊断 |
| `declaration_reordering_and_source_spans_do_not_change_definition_or_plan_hash` | 重排声明与改 span 不改两种 hash |
| `every_node_config_and_semantic_edge_change_changes_plan_hash` | 每种 config 与语义边改动都改 plan hash |
| `bad_config_handle_port_intent_and_capability_contracts_have_stable_paths` | 非法合同的诊断 path 稳定 |
| `bad_condition_and_merge_contracts_are_locatable` / `bad_loop_boundaries_cross_region_cycles_and_nesting_are_locatable` | Condition/Merge 与 Loop 边界诊断可定位 |
| `canonical_number_literal_semantics_are_preserved_in_condition_hash_material` | 数值字面量按十进制语义参与 hash, 不经 f64 |

前两类(重排不改 hash、任一语义改动改 hash)共同把 plan hash 钉成「语义指纹」: 它既要对编辑器噪声免疫, 又必须对任何会影响执行的改动敏感。
