---
type: "参考"
title: "Rule model and contracts"
openwiki_generated: true
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T13:54:24.186Z
sources:
  - id: openwiki-source-f70faf819fb2edbb8e236f45
    resource: repo://docs/adr/0004-rule-first-open-extension-architecture.md
  - id: openwiki-source-88eb5697f2554e4827380829
    resource: repo://src-tauri/crates/lj-capability/src/lib.rs
  - id: openwiki-source-f37a0f7edcf7d757acfa95d3
    resource: repo://src-tauri/crates/lj-media/src/lib.rs
  - id: openwiki-source-b84a408efd38365db5465652
    resource: repo://src-tauri/crates/lj-rule-model/src/definition/contract.rs
  - id: openwiki-source-a7f973b529835a5d2bbe2a82
    resource: repo://src-tauri/crates/lj-rule-model/src/hash.rs
  - id: openwiki-source-18e2cd19f7a3c23ecb7a2d80
    resource: repo://src-tauri/crates/lj-rule-model/src/lib.rs
  - id: openwiki-source-ad5b7bbe3cd66d46d2915258
    resource: repo://src-tauri/crates/lj-rule-model/src/literal.rs
  - id: openwiki-source-568a13748f69a4d9b494241b
    resource: repo://src-tauri/crates/lj-rule-model/src/plan/contract.rs
  - id: openwiki-source-de4ea1ad9081d5dca7a0cfc8
    resource: repo://src-tauri/crates/lj-rule-model/src/plan/types.rs
  - id: openwiki-source-cd54ecbc688a6cd58fc32385
    resource: repo://src-tauri/crates/lj-rule-model/src/policy.rs
  - id: openwiki-source-8c969793b07c46132d5a3d59
    resource: repo://src-tauri/crates/lj-rule-model/src/sensitive.rs
  - id: openwiki-source-8ece8d8ea6055cf2f800dcb4
    resource: repo://src-tauri/crates/lj-runtime/src/effect_registry.rs
generated: { by: "pi", at: "2026-10-04T13:54:24.186Z" }
---


## 这一层是什么

`lj-rule-model` 只承载可序列化合同: `Definition`、`Plan`、`EventEnvelope`、`Diagnostic`、`Policy` DTO 与节点配置 IR; 它**刻意不引入** ORM、Tokio、Tauri、HTTP 或 QuickJS 实现(`src-tauri/crates/lj-rule-model/src/lib.rs#L1-L4`)。这使它可以被 compiler、runtime、storage、importer 与前端 wire 层共同依赖, 而不把任何一方的运行时假设带进合同。

同一层里还有另外两个更窄的契约 crate, 分工不重叠:

| crate | 拥有什么 | 不拥有什么 |
| --- | --- | --- |
| `lj-capability` | 6 个 `StandardIntent`、`IntentExport`、7 种 `IntentInput`(`src-tauri/crates/lj-capability/src/lib.rs#L11-L64`) | 来源端点与展示语义 |
| `lj-media` | 标准媒体模型与资源图增量 `MediaGraphDelta`(`src-tauri/crates/lj-media/src/lib.rs#L1-L291`) | 来源专有的抓取与展示布局 |
| `lj-rule-model` | 作者合同与执行计划 IR、hash、策略 DTO | 任何解析、执行或持久化 |

曾经的第四个契约 crate `lj-plugin-contract`(`PluginId` / `OperationId` / `PluginManifest` / `HOST_CONTRACT_VERSION`)及其在 `lj-runtime` 里的 `PluginHost` / `FrozenRegistry` 已**删除**: 通用 plugin system 按 ADR 0004 第 7 节属一期明确不建设, 内置能力改用同一 Rule Contract 的 `EffectKind` 注册(`src-tauri/crates/lj-runtime/src/effect_registry.rs#L1-L5`, `docs/adr/0004-rule-first-open-extension-architecture.md#L104-L114`)。

依赖方向是单向的: `lj-capability` 无内部依赖, `lj-media` 与 `lj-rule-model` 依赖它。反向依赖(规则模型依赖运行时或媒体结果)不成立。

## RuleDefinition: 作者合同

`RuleDefinition` 的字段全部私有, 只能经公开构造器创建: `source_identity`、`base_url`、`intent_exports: BTreeMap<StandardIntent, IntentExport>`、`flow: FlowGraph`、`capability_manifest`、`source_id_rules`(`src-tauri/crates/lj-rule-model/src/definition/contract.rs#L109-L120`)。

`FlowGraph { nodes: Vec<FlowNode>, edges: Vec<FlowEdge> }` 的两个数组都显式标注「声明顺序不承载语义」(`src-tauri/crates/lj-rule-model/src/definition/contract.rs#L99-L107`)。`FlowNode` 带 `id` / `config` / `span`, 其中 `span` 只服务诊断定位, 不参与身份; `FlowEdge` 的身份是两端的 handle 四元组(上面的 `semantic_identity()`)。

`RulePackage` 是 Definition 加上安装元数据 `source_identity` + `version`(`#L210-L218`)。读写走手写的 current wire: `read_rule_definition` / `read_rule_package` 只接受唯一 current shape, contract tag 不匹配、未知 schema、未知字段或 shape 损坏都返回 `SchemaReadError`, 不存在 legacy 读取分支(`#L254-L394`)。

这套「私有字段 + 公开构造器 + 手写 current wire」的组合有一个直接后果: 任何绕过构造器的字段组合都进不了系统, hash 与校验面对的状态空间是封闭的。

## definition_hash: 为什么重排定义不改 hash

hash 由两步组成。第一步 `canonical_json` 把值序列化为确定性 JSON: object key 在**每一层**按字节序排序, 数组顺序保持不变(`src-tauri/crates/lj-rule-model/src/hash.rs#L8-L20`, `#L71-L93`)。第二步用一个「语义投影」拷贝出待 hash 的定义, 显式清掉/排序所有不承载语义的部分(`#L37-L69`):

| 处理 | 对象 | 理由 |
| --- | --- | --- |
| `span = None` | 每个节点 | 源码位置不是语义 |
| `identity_fields.sort()` | Mapper | 身份字段集合无序 |
| 按显式 `order` 排序 | Merge inputs | `order` 是语义, 物理存储顺序不是 |
| 按 UTF-8 bytes 排序 | Condition branches | 分支集合无序 |
| 按 id 排序 | nodes | 声明顺序不承载语义 |
| `edges.sort()` | 语义边 | 同上 |
| 按 bytes 排序 | `source_id_rules` | 规则集合无序 |

投影后的 JSON 再过 BLAKE3 取 hex(`#L30-L35`)。因此「拖动节点、换编辑器的数组顺序、改源码 span」都不改变 `definition_hash`, 而「改显式 Merge order、改分支集合、改任一节点配置」一定改变它。

## ExecutionPlan: 不可变封存

`ExecutionPlan` 的字段同样全部私有: `compiler_version`、`definition_hash`、`plan_hash`、`nodes`、`edges`、`intent_entries`、`effects`、`capability_requirements`、`control_regions`(`src-tauri/crates/lj-rule-model/src/plan/contract.rs#L24-L35`)。`ExecutionPlan::new` 在构造末尾调用 `execution_plan_hash(&plan)` 并把结果写进 `plan_hash`, 调用方无法自行指定 hash(`#L43-L61`)。

`execution_plan_hash` 覆盖全部 typed config、端口、边与 control region, 计算前把 `plan_hash` 清空; 它对 hash 材料也做规范化: 节点端口 union 与句柄排序、`nodes.sort_by_key(id)`、`edges.sort()`、每个 effect 的 `required_capabilities` 排序、`effects` 按 node_id 排序、`capability_requirements` 排序、Loop region 的 `body_nodes` 排序与 region 按 loop_node 排序(`#L140-L182`)。结论与 definition hash 一致: 物理数组排列不进 hash, 显式 Merge `order` 与全部控制语义进 hash。

`has_control_flow()` 用来回答「这份 Plan 是否用到当前 runtime 阶段尚未开放的控制节点/region」: 只要有 control region, 或存在 Merge/Condition/Loop 节点即为真(`#L113-L125`)。它是 runtime 能力协商的输入, 而不是编译期校验。

Plan 的读取同样是唯一 current wire: `read_execution_plan` 只认当前 shape，`plan_hash` 由构造器或读取校验路径重新确认(见「运行时」页)。

Plan IR 的值类型是闭集: `EffectKind` 只有 `Http` / `QuickJs` / `Extract`; 端口值类型只有 `PortValueType::{Kind, Union}`; 固定 handle 常量(`LINEAR_INPUT_HANDLE`、`CONDITION_INPUT_HANDLE`、`MERGE_OUTPUT_HANDLE`、`LOOP_*`)在这一层定义, 编译器的端口矩阵引用它们(`src-tauri/crates/lj-rule-model/src/plan/types.rs#L17-L31`)。

`EffectKind` 还是运行时 effect registry 的注册与查找键(registry 内部是 `BTreeMap<EffectKind, EffectHandler>`), 因此它在 `Clone` / `PartialEq` / `Eq` 之外还 derive 了 `PartialOrd` / `Ord`; 这只决定键序, 不改变 serde 表示(`src-tauri/crates/lj-rule-model/src/plan/types.rs#L124-L133`, `src-tauri/crates/lj-runtime/src/effect_registry.rs#L98-L107`)。

## 敏感名策略: 唯一 owner

`SensitiveNamePolicy` 是跨来源共享的**唯一**敏感名称表, 注释明确要求 Legado/Maccms/HTTP witness 都调用它, 来源 adapter 不得复制 `Authorization`/`Cookie`/token 名称表(`src-tauri/crates/lj-rule-model/src/sensitive.rs#L1-L5`)。

分类是三分法(`#L9-L19`, `#L24-L45`):

| 处置 | 名称 |
| --- | --- |
| `Blocked` | `Proxy-Authorization`(改变代理 hop 授权边界)、`Set-Cookie`(只属于 response) |
| `Credential` | `Authorization`、`Cookie`, 以及名称中含 `token` / `secret` / `api-key` / `apikey` |
| `Public` | 其余 |

名称比较对大小写不敏感, 并把 `_` 与 `-` 视为同一分隔符(`equivalent` / `normalize_byte`, `#L74-L85`, `#L145-L152`)。`url_contains_sensitive_query_name` 只解析 query 的参数名(含 %-decode)并且**不保留也不返回 value**(`#L59-L72`)——它是「URL 里是否出现敏感参数名」的判定器, 不是凭据读取器。

策略只分类名称, 从不持有或记录对应值, 这是后续 error_mapping/witness 能够「只谈名字不谈内容」的前提。

## 能力策略

`PolicyCapabilities { network: bool, system: SystemCapabilities }`, 其中 `system` 拆出 `fs` / `env` / `process` 三个 bool(`src-tauri/crates/lj-rule-model/src/policy.rs#L8-L27`)。这份结构同时充当安装 grant 与执行沙箱边界; 被拒绝的能力以 `CapabilityError::Blocked(Capability)` 表达(`#L29-L48`)。

拆分的原因写在注释里: 避免触发 clippy 的 `struct_excessive_bools` 阈值——这是一个契约形状被 lint 规则影响的少见但真实的例子。

## TypedLiteral 与规范数值

`TypedLiteral` 是闭集 `Null/Bool/Number/String/Array/Object`(`src-tauri/crates/lj-rule-model/src/literal.rs#L59-L70`)。数值比较走规范十进制而不是 `f64`: `1`、`1.0`、`1e0` 等价, 负零等于零(`canonical_number_cmp` / `canonical_number_eq`, `#L86-L107`)。

这条规则直接影响 hash: 条件里的数值字面量按十进制语义参与 hash 材料, 所以「同一数字的不同写法」不会产生两种规则身份(`plan_compiler_test.rs` 的 `canonical_number_literal_semantics_are_preserved_in_condition_hash_material` 断言了这一点)。`canonical_json_deep_eq` 与 `typed_literal_matches_json` 也建立在这套比较上(`#L100-L149`)。

## 标准媒体模型: 可逆 ID 与增量

`lj-media` 的 ID 是可逆的: `item_resource_id` / `unit_resource_id` 用 hex 编码组件拼出 `item:<source>:<key>` / `unit:<source>:<item>:<unit>`, `parse_item_resource_id` / `parse_unit_resource_id` 能解回组成(`src-tauri/crates/lj-media/src/lib.rs#L11-L53`)。可逆意味着前端拿到资源 ID 就能本地判断它属于哪个来源与主体, 不需要回查数据库。

模型集合是一套标准媒体词汇: `MediaResourceId`、`MediaKind`、`ResourceCompleteness`、`SourceProfile`、`MediaItem`、`MediaCollection`、`MediaUnit`、`MediaAsset`(含 `MediaAssetKind` 与 `MediaAssetLocator`)、`MediaRelation`、`MediaAction`、`PresentationHint`(`#L7-L258`)。`PresentationHint` 是「规则只出标准模型 + 展示提示、不定义 UI 布局」这条产品硬边界在类型层的位置: 它携带 `card_density`、`cover_ratio`、`dominant_color`、`preferred_template` 之类的线索(`#L240-L245`)。

发现的产物用增量表达: `MediaGraphDelta` 按 `sources/items/collections/units/assets/relations/actions/hints` 分组, `merge` 的语义是「同 ID 后到覆盖」, 只有 `relations` 走去重追加(`#L249-L278`)。这让一次执行可以只投递变化的部分, 而合并顺序无关最终状态(除 relations 的集合语义外)。

## 边界与遗留

- 这一层没有 `Revision` 类型。「草稿 / 生效 revision」的概念只出现在 `lj-rule-system` 的文档 DTO 与存储侧, 不是 rule-model 的合同(`lj-rule-model` re-export 列表里没有任何 revision 类型, `src-tauri/crates/lj-rule-model/src/lib.rs#L21-L51`)。
- `EventEnvelope`、`SecretRef`、`ArtifactRef`、`ControlTrace` / `InvocationPath` / `LoopInvocationSegment`、`ExtractRule` 等类型也在本 crate 的 re-export 里, 它们分别是持久化事件、witness 与重放、抽取规则编译的合同; 这些类型的**使用**方式见「执行、捕获与重放」与「存储层与事件账本」。
- `schema.rs` 的 `RULE_CONTRACT_SCHEMA_VERSION` 参与 Plan hash 材料，因此合同 schema 版本变化同样会改变 plan hash(`src-tauri/crates/lj-rule-model/src/plan/contract.rs#L170-L171`)。
