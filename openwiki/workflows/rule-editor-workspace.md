---
type: "参考"
title: "Rule editor workspace"
openwiki_generated: true
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T10:14:16.110Z
sources:
  - id: openwiki-source-d27d10d277ef08477509b505
    resource: repo://src-tauri/crates/lj-compiler/src/compiler/validation/definition.rs
  - id: openwiki-source-98cb793e96999b4a33b247e5
    resource: repo://src/features/rules/DefinitionPreview.tsx
  - id: openwiki-source-c20aac7f1f6ba9a345db52a3
    resource: repo://src/features/rules/EditorSurface.tsx
  - id: openwiki-source-a0309a4f66f33e6814b5c22e
    resource: repo://src/features/rules/EditorToolbar.tsx
  - id: openwiki-source-2e772d3194b52450a587222b
    resource: repo://src/features/rules/FlowTopBar.tsx
  - id: openwiki-source-1b5330715de2cac0cb733fdd
    resource: repo://src/features/rules/inspector/Inspector.tsx
  - id: openwiki-source-f43b35ba31e46f98d3039124
    resource: repo://src/features/rules/inspector/node-panels.tsx
  - id: openwiki-source-79acc6cd41696fa55762baad
    resource: repo://src/features/rules/model/connection-gate.ts
  - id: openwiki-source-ee130a462d446f70c12f0143
    resource: repo://src/features/rules/model/core.ts
  - id: openwiki-source-b4c477b8156c887a7c0a2a4f
    resource: repo://src/features/rules/model/flow-adapter.ts
  - id: openwiki-source-df07a4012e3080352b9d1935
    resource: repo://src/features/rules/model/flow-layout.ts
  - id: openwiki-source-c103bd2ca7ee51fbcc90fdbb
    resource: repo://src/features/rules/model/layout-json.ts
  - id: openwiki-source-cf08c1737c34782425dd0f98
    resource: repo://src/features/rules/model/native-import.ts
  - id: openwiki-source-4501a731e7c124a791a77f66
    resource: repo://src/features/rules/model/ports.test.ts
  - id: openwiki-source-62d5fcc1f6508dad1c97ad3b
    resource: repo://src/features/rules/model/ports.ts
  - id: openwiki-source-e523845a56feec75d438c920
    resource: repo://src/features/rules/model/session.ts
  - id: openwiki-source-c8d42342a2b428624a25bb88
    resource: repo://src/features/rules/RuleFlowCanvas.tsx
  - id: openwiki-source-e09aeeb6133666577e5f07d8
    resource: repo://src/features/rules/RuleWorkspace.tsx
  - id: openwiki-source-8780adbf81fc42a7d70c2315
    resource: repo://src/features/rules/use-session.tsx
generated: { by: "pi", at: "2026-10-04T10:14:16.110Z" }
---


## 工作区的组成与所有权

`RuleWorkspace` 是**文档切换宿主**: 它列出全部文档, 深链未指定文档时重定向到第一份, 并按 `documentId` 作为 React key 重挂载编辑器 session——注释写明这样「切换文档不会继承上一份的撤销历史」(`src/features/rules/RuleWorkspace.tsx#L1-L4`)。

打开的文档交给 `EditorSurface`, 它是一个可拖拽两栏(画布 + 检查器)加诊断区(`src/features/rules/EditorSurface.tsx#L1-L17`)。两处所有权细节值得注意:

- `ReactFlowProvider` 放在 `EditorSurface` 里而不是更外层, 因为需要 flow 上下文的组件(`FlowTopBar` / `NodePalette` / `NodeShell`)都在画布子树内(`EditorSurface.tsx#L3-L5`);
- 分栏宽度用 `useDefaultLayout`(react-resizable-panels)持久化, 注释明确「它是编辑器偏好, 不进文档 layout: 它跟着人走, 不跟着规则走」(`EditorSurface.tsx#L35-L40`)。

层次是: **纯 TS 模型层**(无 React 依赖, 可在 Vitest 里直接测) ← **zustand vanilla store** ← **React context 绑定**:

| 文件 | 角色 |
| --- | --- |
| `model/core.ts` | 不可变 reducer (`reduce(state, action)`) + 全部作者状态与不变量 |
| `model/session.ts` | 用 zustand 包一层 core, 提供 `loadDocument` / `save` / `validate` / `dispatch` 等 action 与选择器 |
| `model/flow-adapter.ts` | 语义定义 ↔ 画布形状的**纯投影** |
| `model/ports.ts` / `connection-gate.ts` / `flow-layout.ts` | 端口合同 / 连接门禁 / ELK 布局, 全部纯函数 |
| `use-session.tsx` | 只做 context 与选择器订阅, 让高频变更(拖拽/连线)不重渲染整棵树(`src/features/rules/use-session.tsx#L1-L13`) |

core 的状态形状(`model/core.ts#L223-L255`)包含: 当前 `definition`、不透明 `layout`、两个已保存 revision、分域 `dirty`、`epoch`、`selection`、`intentFocus`、`validation`、`conflict`、`recoveryDraft`、`history` / `redo`、`pendingCredentialMutations`、`inFlightSave`。

## 语义定义到画布的投影

`semanticToFlow` 是**只读投影**, 模块头列了四条约束(`model/flow-adapter.ts#L1-L13`):

1. 七类节点 kind 同名映射到画布自定义节点类型;
2. 节点位置优先取 layout, 未保存的节点按 kind 做**确定性默认网格**排布, **绝不写回 definition hash**;
3. 边 id 由语义 identity `from.node_id:from.handle->to.node_id:to.handle` 确定性派生, 与 core 的边去重 identity 一致, **不产生每意图副本**;
4. `intentFocus` 只影响 `focused` / `dimmed` 标记(可达性投影), 不复制节点/边/配置。

投影结果用 `WeakMap` 按 core 引用缓存, 注释解释了必要性: 「selector 每次返回新对象会让 zustand 的 `Object.is` 判定为变化, 陷入无限重渲染」(`model/session.ts#L478-L499`)。这条约束是「core 不可变」这一设计能成立的前提。

节点数据里同时携带诊断与聚焦标记(`flow-adapter.ts#L44-L66`), 因此画布可以把「哪个节点/端口有问题」和「哪些节点在焦点子图外」直接渲染出来, 而不需要另建一份状态。

## 连接门禁

所有连接在派发前都过 `validateConnection`, 它把拒绝原因收敛成十种稳定码(`model/connection-gate.ts#L37-L58`):

```text
missing-handle / unknown-node / unknown-handle / self-loop / incompatible-ports
duplicate-edge / entry-occupied / back-edge / loop-yield-occupied / intent-mismatch
```

判定顺序与语义(`connection-gate.ts#L136-L207`):

1. 句柄完整性 → 端点存在性 → 自环检查;
2. 端口必须属于该节点类型(按 config 推导的动态端口)的合同, 否则 `unknown-handle`;
3. 类型矩阵 `portCompatible`(`ports.ts#L394-L396`: `input.accepts.includes(out.emits)`);
4. 语义边 identity 已存在 → `duplicate-edge`(重连时会先排除被替换的旧边);
5. Loop 的 `yield` 只能有一条 structured return;
6. HTTP 入口输入单入边(意图入口语义);
7. 回边拒绝: 把新边加进去后若目标可达源则拒绝, **唯一例外**是 Loop 的 structured yield backedge;
8. 焦点激活时, 源节点必须已在焦点子图内(构建方向是「焦点内 → 外扩」)。

模块头写明这条门禁是**纯 TS 且无副作用**: 「组件层在 dispatch typed action 前调用 `validateConnection`; 校验失败不产生任何 semantic 变更」(`connection-gate.ts#L1-L5`)。同一模块的 `reachableNodes` 还被 adapter 的聚焦投影复用, 以保证「门禁对意图子图的判定」和「画布高亮」不会出现两套答案(`#L4-L5`)。

画布侧有两个调用点(`src/features/rules/RuleFlowCanvas.tsx#L286-L289`、`#L486-L492`): `isValidConnection` 只返回布尔(不落状态), 而落边前的那次会记录拒绝原因用于提示。位置变更经 `layoutNodes` 写入(`#L253-L268`)。

**门禁不做的事**: 它只管单条边能否成立。跨边的结构合法性(意图可达性、Merge 必需输入、Loop 区域完整性、capability)不在前端判定——那是编译器的职责。

## 前端端口矩阵与编译器 ports 的关系

`ports.ts` 是编译期端口合同的 **TS 镜像**(`model/ports.ts#L1-L12`): `PortType` 五值 `http_response | json | raw | delta | loop_binding`「与 compiler 的 `PortValueKind` 对齐」, 另有 `PortRole`(data/control/binding)与 `PortSide` 用于渲染。

静态合同 `PORT_CONTRACT` 覆盖七类节点的空配置回退(`#L74` 起), 动态端口由 `getNodePorts(kind, config)` 推导(`#L270` 起):

- Js 节点的输出类型随 `config.output`(json/raw)变化;
- Merge 的输入来自声明的 `inputs`(按显式 order), `input_id` 缺省写 `input_${index+1}`、`handle` 缺省写 `in:${index}`(`#L253-L268`);
- Condition 的每个分支产生一个 `branch:<index>` 输出;
- Loop 暴露 `collection` 入、`body` 出、`yield` 入与 `done` 出四个句柄。

**这是两份实现**: 编译器从 Definition 里的显式 config 推导同一组端口并校验结构(输入 handle 必须存在、order 唯一连续、input_id 唯一等), 前端则为了**即时反馈**必须能在没有往返的情况下告诉用户「这条边能不能连」。分工是:

- **前端拥有句柄命名**: 检查器新增 Merge 输入时写死 `input_id: input_<n>` 与 `handle: in:<n-1>`(`src/features/rules/inspector/node-panels.tsx#L340-L345`), 与端口推导的缺省值一致; 注释还说明「编辑过程中 input_id 与 handle 都可以临时重复, 只有下标是唯一的」(`#L353`), 也就是说中间态的非法结构是允许存在的;
- **编译器是权威**: 保存时由 Rust 校验并给出诊断, 前端矩阵只决定 UI 是否允许连边。

两份实现的漂移风险是真实的: 仓库里没有跨语言断言两侧矩阵一致的测试(`ports.test.ts` 只对 TS 侧断言)。两侧是否仍然同步 未验证。

## Loop 区域在 TS 里的重复推导

`projectLoopRegion` / `projectLoopRegions`(`model/flow-adapter.ts#L375-L495`)用邻接关系在 TS 里重新推导区域成员与问题:

- 区域由 `collection` 入边、唯一的 `body` 出边、唯一的 `yield` 入边与至少一条 `done` 出边界定;
- body 成员是「从 body 入口出发、排除 yield backedge 可达」∩「能回到 yield 源」两个集合的交;
- 诊断码与编译器同名前缀: `LOOP_NOT_FOUND`、`LOOP_ENTRY_INVALID`、`LOOP_BODY_INVALID`、`LOOP_YIELD_INVALID`、`LOOP_DONE_INVALID`、`LOOP_BODY_BYPASS`、`LOOP_CROSS_REGION_EDGE`(`#L401-L460`)。

用法是**只读投影**: `LoopRegionView` 带 `status: 'valid' | 'invalid'` 与逐条诊断, 画布据此画区域框(`LoopRegionOverlay.tsx`), 不写回任何语义。与端口矩阵同理, 这份逻辑在编译器里有一份权威实现(它决定保存是否通过), 前端这份只为了不等待往返。

## 布局与 layout_json

布局是**文档的一部分**, 但被当作不透明值处理:

- wire 侧 `layout_json` 是字符串, `layout-json.ts` 只承担「字符串 → unknown」这一步, 注释明确「坏 JSON 视为无布局, 让画布退回默认排布, 而不是让整个文档加载失败」(`src/features/rules/model/layout-json.ts#L1-L20`);
- core 只对 layout 做快照比较与回传(`model/core.ts#L225-L229` 注释「不透明」);
- 自动布局用 ELK: `elk.algorithm: layered`、方向 `RIGHT`、正交路由、层间距 168、同层节点间距 72、`feedbackEdges: true` 等(`model/flow-layout.ts#L5-L19`);
- `layoutNativeRuleFlow` 是**动态 import**(`await import('elkjs/lib/elk.bundled.js')`)并在三种失败情形下回退到确定性网格: 无节点、ELK 抛错、ELK 返回的坐标数不等于节点数(`#L193-L208`)。这既是代码分割也是健壮性兜底。

自动布局的结果通过 `layoutNodes(positions)` 写进 core(因此进 dirty.layout 并可撤销), 随后等一帧再 `fitView`(`src/features/rules/FlowTopBar.tsx#L76-L86`)。也就是说**布局会被保存到后端**, 但它永远不参与 definition hash——这是「编辑器布局不进入规则语义」这条不变量在前后端各自成立的方式。

## 撤销重做与状态边界

core 的不变量写在模块头(`model/core.ts#L1-L15`), 并在 `core.test.ts` 里覆盖:

- `history` / `redo` **只记录可回放的 semantic/layout 命令**; `selection` / `viewport` / `intentFocus` 等 transient 状态不进历史;
- **credential 值不进 action/history**: `credentialReplace` / `credentialClear` 携带一次性明文, 入队后在 `saveRequest` 快照进 `inFlightSave` 并立即清空, 不跨 save 驻留;
- `epoch` 在 `saveRequest` 时递增并快照, `saveResponse` 校验快照仍为当前 epoch 才接受(stale response 拒绝), 接受后再递增一次(同一响应重复投递同样被拒);
- `saveRequest` 按当前 dirty 域拍快照, `saveResponse` 按快照分域 merge——**编辑中的域保持 dirty, 未编辑的域收敛为已保存**。

布局命令有合并规则: 连续同节点的 move/collapse 会合成一条历史(`coalesceLayout`, `#L927-L953`), 否则拖动一次会塞进几十条撤销记录。

会话对外提供的能力分成三类(`model/session.ts#L133-L195`): 文档生命周期(`loadDocument` / `createBlank` / `createTemplate` / `createImported` / `save` / `validate`)、状态变更(`dispatch` / `dispatchAll` / `undo` / `redo`)、画布便捷操作(`connect` / `disconnect` / `reconnect` / `moveNode` / `layoutNodes` / `collapse*` / `select*` / `revealNode` / `focusIntent` / `setNodeConfig` / `credential*` / `addNode` / `delete*` / `pasteSubgraph` / `duplicateNodes`)。其中 `revealRequest` 被单独标注为「仅供画布消费的一次定位请求; 不属于文档语义或布局, 也不进入撤销历史」(`#L129-L130`)。

粘贴/复制有一段专门说明: 「id 在这里重新分配并重写边的端点……语义、布局和历史在一次粘贴命令中同时落地, 避免两次撤销才能恢复粘贴前的图」(`session.ts#L187-L193`)。

## 哪些操作真正写回后端

| 用户动作 | 是否写回 | 落到哪 |
| --- | --- | --- |
| 改节点配置 / 连线 / 删节点 / 粘贴 | 否 | 只进 core, 标记 dirty.semantic |
| 拖动节点 / 自动布局 / 折叠 | 否 | 只进 core, 标记 dirty.layout |
| 选中 / 切意图焦点 / 平移缩放 | 否 | transient, 不进历史也不写回 |
| 点击「保存」 | 是 | `save_native_rule_document`(分域, 带各自 expected_revision) |
| 点击「校验」 | 是(只读) | `validate_native_rule_document` |
| 切换预览里的 provenance | 是(只读) | `get_native_rule_provenance` |
| 新建 / 导入 | 是 | `create_native_rule_document`(blank/template/import) |

工具栏只调 session 的 action(`src/features/rules/EditorToolbar.tsx#L33-L111`): 保存成功后弹 `rules_saved`, 失败弹 `sessionErrorText`; 校验按钮在 `hasUnsaved` 时禁用, 因为它校验的是**已保存的 revision**(`model/session.ts#L321-L345`: 语义有未保存变更或冲突时直接抛 `document_semantic_unsaved`)。

保存失败与冲突的用户可见状态有一处真实缺口(详见规则文档生命周期页): 冲突是正常返回, 因此工具栏会提示「已保存」而实际写入被拒, 且没有组件读取 `conflict` / `recoveryDraft`。

## 检查器与只读预览

`Inspector` 按 core 的单个 selection token(`node id` / `edge:` / `port:` / `loop-region:` 前缀)分发到四种检查器(`src/features/rules/inspector/Inspector.tsx#L1-L60`): 节点面板、边、端口、Loop 区域。选择 token 的解析在 flow-adapter 里(`parseEditorSelection`), 因此「选了什么」在模型层就有稳定表达, 端口选中不会把节点标记为 selected(`flow-adapter.ts#L62-L63`)。

`DefinitionPreview` 是**只读预览**, 且承担一层脱敏: 序列化 Definition 时把键名匹配 `/credential|secret|password|token/iu` 的值打码, 注释写「宁可多打码也不能漏」; provenance 的 `masked_text` 由 Rust 负责脱敏; 「这里绝不显示凭证明文与执行 Plan」(`src/features/rules/DefinitionPreview.tsx#L1-L17`)。

`NewRuleDialog` 的导入路径先做本地预检: `parseNativeRuleDefinition` 只接受 current `RuleDefinition` 合同, 并有 5 MiB 上限(`src/features/rules/model/native-import.ts#L1-L6`), 因此格式不符在发请求前就会被拒绝。

## 前端校验与后端编译校验的分工

| 判定 | 谁做 | 何时 |
| --- | --- | --- |
| 句柄存在性、端口类型、自环、重复边、回边、单入边、焦点外连接 | 前端门禁(纯 TS) | 连接瞬间, 即时反馈 |
| 端口矩阵的具体内容(哪些类型可接) | 前端矩阵 + 编译器各自一份 | 连接时 / 保存时 |
| Loop 区域成员与越界 | 前端投影(只显示) + 编译器(权威) | 编辑时 / 保存与校验时 |
| Definition 结构、意图导出、必需输入、capability、Loop 嵌套 | **仅编译器** | 保存 / 显式校验 |
| `definition_hash` / `plan_hash` | **仅编译器** | 保存 |

诊断回流是单向的: 后端返回的诊断带 `path`(节点/边/句柄定位), 前端把它们挂到节点数据上由 `DiagnosticList` 与画布渲染(`model/flow-adapter.ts#L52-L53`)。前端**不复制编译器的诊断生成逻辑**——只有 Loop 区域那一段是例外(见上文)。

## 遗留术语与边界

- `flow-adapter.ts`、`connection-gate.ts`、`core.ts` 的注释仍写「Svelte Flow」「无 runes」「.svelte.ts」(`flow-adapter.ts#L1-L4`、`connection-gate.ts#L12-L18`、`core.ts#L1-L4`), 而实现早已切到 `@xyflow/react`(`EditorSurface.tsx#L6`)。这些是 React 迁移的历史残留, 不影响行为, 但会误导读者。
- 编辑器只能创作与保存; **没有从编辑器发起执行或安装的入口**(见 IPC 页与规则文档生命周期页的命令/路径缺口)。
- 连接门禁只作用于画布交互; 通过导入(JSON)得到的定义可以绕过它, 由编译器兜底——这是有意的分层, 不是漏洞。
- 前端矩阵/区域推导与编译器的一致性没有自动化断言, 属 未验证。
