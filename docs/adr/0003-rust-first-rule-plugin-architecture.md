# ADR 0003：规则插件采用 Rust-first、QuickJS host API 和分域扩展契约

- 状态：已采纳
- 影响范围：规则模型、规则编译与运行时、Native Rule Document、Source Adapter、QuickJS plugin runtime

## 背景

LanJing 的规则系统需要支持高内聚、低耦合和丰富的插件 API。当前实现对固定能力进行了强校验，但扩展边界仍是闭集：

- `FlowNodeConfig` 和 `PlanNodeConfig` 是闭集 enum。
- `EffectKind` 只有 HTTP、QuickJS、Extract。
- #35 已删除 `EffectHandlers`, 但 registry 的 `EffectHandler` 仍只有三类 concrete handler.
- `RuleSystem` 已通过 frozen registry 装配内置 adapter; 编译, value 和前端目录仍依赖能力闭集.

新增一个规则节点或 effect 会同时修改 model、compiler、Plan hash、runtime dispatch、archive 和 adapter。只增加 QuickJS host call 虽然入口很小，但不能支持新 node、value type、effect 或 source adapter，也不能满足真正的插件扩展需求。

Cordis 的显式依赖、scope-owned disposer、生命周期状态和 typed events 对本项目有参考价值。其 JavaScript `Proxy Context`、动态 service lookup、函数 identity 和隐式事件改写不适合 Rust、移动端和可验证的规则执行边界。

## 决定

### 1. 分层所有权

规则扩展采用以下层次：

```text
Plugin Contract
        ↓
Plugin Host
  dependency / lifecycle / policy / registry
        ↓
Rule Kernel
  definition / revision / validation / compilation invariants
        ↓
Rule Runtime
  immutable plan / execution / cancellation / capture / replay
```

各层所有权固定如下：

- `Plugin Host` 负责 plugin manifest、依赖解析、注册、scope、生命周期、权限 broker 和 runtime adapter。
- `Rule Kernel` 负责 Definition、Rule Draft Revision、Effective Rule Revision、credential ownership、校验不变量、编译编排和事务端口。
- `Rule Runtime` 只执行 immutable Plan，负责 scheduler、取消、execution event、durable capture 和 replay。
- `RuleSystem` 只作为 application composition facade，不拥有上述领域语义。

插件永远不能直接修改 revision、credential owner、capability grant、event sequence、archive 或 storage transaction。

### 2. Rust-first 实现方式

第一阶段只支持静态链接的 Rust native plugin：

- Rust plugin 使用 typed Rust SDK 和 trait 实现。
- Rust trait 只作为同一构建产物内的实现 API，不承诺 Rust ABI 稳定性。
- 不使用 Rust `dylib` 或 `cdylib` 作为公开插件 ABI。
- plugin host 的可替换边界是 contract 和 adapter，不是 Rust trait object 的内存布局。

### 3. JS plugin 使用 Rust 内嵌 QuickJS API

后续 JS plugin 运行在 Rust 内嵌的 QuickJS 中，通过 Rust 注册的窄 host API 使用能力：

- 每个 JS plugin 使用独立的 QuickJS `Runtime`、`Context`、module loader、scope 和资源预算。
- JS 只能看到稳定的 host functions/modules 和语义 DTO，不看到 `rquickjs` 内部对象、Rust reference、trait object、Tauri handle 或 React state。
- 网络、凭证、文件、时间、随机数、日志、取消和 capture/replay 都由 Rust host API 代理。
- JS plugin 不进入 Tauri WebView。
- 当前 QuickJS rule node 与 JS plugin 分开管理。可以共享底层 engine adapter，但不能共享 plugin registry、权限 scope、生命周期状态或持久化状态。
- 首期不引入 WASM、独立进程或其他脚本 runtime 作为 plugin 基础设施。
- 完整系统支持随应用分发和从本地文件导入的 QuickJS plugin package. 导入先校验 manifest, 依赖, 平台, 预算和权限, 再运行受限注册代码; artifact digest 只证明内容绑定, 不代表可信作者或授权.
- 本地安装, 配置, 启停, 版本更新, 卸载和重启恢复属于同一系统. 在线 marketplace, 自动下载, 动态 native binary 与开发文件监听 HMR 不在范围内.

### 4. 插件 API 采用分域 contract

不设计一个包办所有能力的 `PluginContext`。Rust SDK 通过显式、分域 registry 提供 API：

```text
PluginManifest
PluginRegistrar
  ├─ DefinitionRegistry
  ├─ CompilerRegistry
  ├─ RuntimeRegistry
  ├─ SourceRegistry
  ├─ EditorRegistry
  ├─ ServiceRegistry
  └─ EventRegistry
InvocationHost
  ├─ NetworkPort
  ├─ CredentialBroker
  ├─ ArtifactStore
  ├─ Clock / Random
  ├─ Cancellation
  ├─ PluginStateStore
  └─ DiagnosticSink
```

首批扩展 contract 包括：

1. `NodeDescriptor`、`ValueTypeDescriptor` 和 `PortDescriptor`。
2. `NodeCompiler`、`EffectCompiler`，以及由 Kernel 管理调度不变量的可选 control operator。
3. `EffectHandler`，其输入输出、权限、取消、capture 和 replay 规则由 host 约束。
4. `SourceAdapter`，只能产出标准规则 package draft，不能创建或提交 Source Revision。
5. `EditorDescriptor`，只提供声明式配置、port metadata 和诊断映射，不注入 React component。
6. 显式依赖的 typed service contract. service 使用稳定 identity, 版本和输入输出 schema, 在 generation 构建时绑定 provider; 不提供动态字符串 service locator.

command/query 使用显式方法. typed event 只观察生命周期, 诊断和进度; 需要 transform 的流程使用单独声明并校验的 command contract, 不能通过监听器隐式改变核心结果.

### 5. Definition、Plan 和 identity

规则 Definition 使用可扩展 envelope，不继续扩大核心闭集 enum：

```text
Node {
  node_id,
  type_id,
  config_schema_version,
  canonical_config
}
```

Plan 使用 immutable compiled operation：

```text
Operation {
  plugin_id,
  operation_id,
  contract_version,
  canonical_payload,
  typed_ports,
  effect_declarations
}
```

`NodeTypeId`、`ValueTypeId` 和 `EffectTypeId` 使用 namespaced stable identity。registry freeze 后，execution 绑定不可变 registry snapshot 和 plugin lock。

未知 plugin node 可以 round-trip 和展示为 unavailable，但不能 validate、compile 或 execute。插件缺失不能静默替换或删除 Effective Rule Revision。

默认只允许新增，不允许同 identity 覆盖。破坏性变更使用新的 descriptor identity，不能依赖注册顺序抢占 handler。

项目处于初期，新的 descriptor envelope 直接替换当前 Definition、Plan、storage DTO 和 TypeScript wire。不开辟 v2 并存模型，不读取旧 shape，不迁移旧本地开发数据，也不保留旧 fixtures。contract/schema version 只标识当前协议并用于拒绝不兼容输入，不代表承诺向后兼容。

### 6. 权限、凭证和执行安全

有效 capability 是多个边界的交集：

```text
host policy ∩ source grant ∩ plugin manifest ∩ invocation grant
```

插件只能声明和使用 host 授予的 capability。网络必须继续通过现有 SSRF、scheme、redirect、body limit 和 capture 规则。凭证使用 opaque `CredentialSlotRef` 或 host-managed request injection，插件不能读取明文 credential。

所有外部 effect 在 host port 边界 capture。插件不能实现自己的 archive，replay 也不能回退到 live execution。capture 至少绑定 plugin identity、operation、contract version、输入输出 hash、错误、调用顺序和取消结果。

### 7. 生命周期和可逆注册

plugin host 使用显式状态机：

```text
discovered
  → validated
  → dependencies-resolved
  → admitted
  → registered
  → frozen
  → active
  → draining
  → disposed
```

注册返回可撤销的 `RegistrationLease`。激活失败时逆序回滚已注册项；dispose 必须 exactly once。active execution 持有 lease 时，handler 不能被卸载。停止流程先拒绝新调用，再等待已有调用结束，最后释放 scope-owned disposer。

生命周期转移串行化, 同一实例只允许一个在途转移. 并发停止等待同一次清理结果. 清理按依赖逆拓扑与同 scope 的资源逆获取顺序完成; 一个 disposer 失败仍继续清理其余资源, 汇总错误而非只记录日志. exactly once 指每个清理动作最多尝试一次, 不承诺失败的 disposer 成功; 无法终止的 native 调用不能被假装释放.

新组合在 staging generation 校验, 注册和初始化, 全部成功后原子发布. execution 准入原子获得 generation, 确切 plugin lock, policy context 与 lease; 旧 execution 不混用新 handler. 正常 drain 等待在途工作, 权限撤销则立即禁止新的受影响 host call 并触发取消, 两者不混用.

Manifest 至少声明：

```text
plugin_id
plugin_version
host_api / contract version
provided descriptors
required / optional dependencies
conflicts
requested capabilities and scopes
lifecycle policy
supported platforms
resource limits
plugin state policy
capture/replay compatibility
artifact digest
```

### 8. 完整系统的交付边界

#22 的完整规格取代仅完成内置 handler 查表的验收上限. 内置 HTTP, QuickJS rule node, Extract, Mapper, Merge, Condition, Loop 与来源格式 adapter 全部走公开 SDK; Kernel 保留可验证的通用控制 IR 与标准媒体出口, Runtime 不按内置 node identity 调度.

Definition, Plan, Value, operation, source detection 和前端 descriptor projection 都必须开放. 新增插件不得增加核心 enum 变体, lowering/dispatch 分支, 内置 ID 映射, 静态 ports/defaults 或专属 React component. 真实的插件节点编辑, 保存, 生效, 执行, 取消和 replay 是交付要求, 不以只读 catalog 代替.

资源所有权区分 app, plugin generation, source/document, execution 和 invocation scope. 注册, 读取, 调用, 订阅和释放绑定同一 scope. 跨插件 service 调用继承原调用者的更窄授权并同时受 callee manifest 限制, 不能代调用扩权; 关闭后的 opaque handle 和迟到结果不能继续发起 effect.

capture 同时记录 operation completion 与 operation 内每个受控 host effect, 含嵌套 service, network, clock/random 和 state 读写. 完整封存 trace 不依赖当前插件激活即可回放; replay 不运行 live handler, 不读取当前插件 state, 不产生持久写入, 缺失或损坏的描述与材料只返回错误.

package catalog 区分 installed, desired-enabled, active, failed, draining 和 unavailable. 提交前更新失败保留上一 active 组合; 提交后发布故障报告 committed-but-unavailable 并拒绝新准入, 不能谎称回滚. 持久 selection 与内存发布使用明确提交点和恢复记录, 提交后不再执行可失败的 plugin callback. staging state write 隔离, 初始化不允许外部网络副作用. 卸载保留规则, 来源和 capture, 插件 state 的清除单独确认.

state schema 升级只在 staging copy 做受限确定性转换, 绑定原 revision/epoch 并用 CAS 防止覆盖并发写入. 不兼容转换先 drain 旧 state lease; 失败保留原 state, 句柄不因新组合发布而重定向 namespace. 不把开发规则数据无迁移的决定扩大成插件升级可丢失用户状态.

插件更新不能静默重绑 Source Revision 或 Effective Rule Revision. 采纳新 lock 仍需 Kernel 重新校验/晋升或来源候选审阅. artifact 回收只被真实 lease 与显式保留的可执行版本 pin 阻止, 历史 lock/descriptor/trace 不永久 pin 代码. 旧 live 绑定无法使用时标 unavailable, 历史快照不改写.

候选 origin 区分 adapter 转换与封存 Native/Source Revision. 转换 provenance 和执行所需 lock 分开, 从封存 package 回滚不要求原 adapter. 最终候选校验到领域提交持有短 admission guard, 与 generation 更新和撤权排序; lease 只保证实现存活, 不能替代准入校验. Host 不获取 Kernel 事务所有权.

来源输入在普通 adapter detection/conversion 前完成可信摄入, 抽取已识别凭证并只交付公开 projection 与 opaque slots. extraction 使用开放的声明式合同与受审计的可信 provider, 不是格式闭集; JS manifest 不能自授可信资格. 未能分类的敏感输入拒绝送给不受信任 adapter, 不声称能发现任意未知文本秘密. 历史 replay 读取敏感 capture 仍校验当前访问政策与 owner, 不凭历史 lock 恢复已撤销授权.

Rust native plugin 是可信同进程代码, SDK 合同不等于安全沙箱. CPU deadline 与取消对 native callback 是协作式; 仅可捕获 unwind panic. QuickJS 的 host module, 每次入口检查, heap/stack, interrupt/deadline, 输出与异步任务预算由宿主强制, 但不承诺 engine 严重故障不会影响宿主.

验收必须由核心外的 Rust fixture plugin 与本地 QuickJS fixture package 完成陌生 node, value, operation, service, editor 和 SourceAdapter 的端到端流程, 证明无需修改核心. SDK 模板, package validator, 权限/生命周期失败矩阵和本地管理恢复均为必需交付. 五目标平台需编译/打包与设备或模拟器的核心旅程 smoke; 未实跑项单列 human validation 并保持父 effort open, 不能以 desktop 结果替代.

这个选择吸收 Cordis 与 DeepSeek Harness 的插件组合和 scope-owned registration, 保留 LanJing 的权限交集, 领域提交, 凭证归属与可回放不变量. DeepSeek Harness 的可信 host code 和 Cordis 的 Context 隔离都不是受限 QuickJS 的安全实现模板.

## 后果

### 正面后果

- 新 node、value type、effect 和 source adapter 的变化集中在 plugin registry 与对应 contract，不再扩散到所有 closed match。
- Rule Kernel 保持 revision、credential、授权和事务的不变量，插件不能绕过这些边界。
- Rust native plugin 适用于 Windows、macOS、Linux、iOS 和 Android 的统一构建路径。
- JS plugin 可以复用当前 Rust QuickJS 集成的 blocking lane、watchdog 和 cancellation 思路，但拥有独立的权限和生命周期。
- capture/replay 仍由 Runtime 集中处理，插件不需要重复实现安全 witness 和 archive 逻辑。
- 声明式 editor descriptor 不绑定 React，规则编辑器、CLI 和移动端可以共享规则 contract。

### 代价

- Definition 和 Plan 从闭集 enum 变为 registry-backed envelope，部分 exhaustive match 安全性转移到 descriptor 校验和 contract tests。
- registry generation、plugin lock、schema version 和 descriptor digest 增加了版本管理约束。
- embedded QuickJS 是进程内执行；JS exception 可以转换为 plugin error，但 QuickJS/native runtime 的严重错误仍可能影响宿主。因此 JS plugin 必须使用严格的 host API、资源预算和 watchdog。
- plugin state、editor descriptor、capture/replay 和 source adapter 都需要稳定 contract，不能只实现一个 `invoke` 函数。

## 测试要求

基础 contract tests 必须覆盖：

1. 未知 descriptor 能 round-trip，但不能错误执行。
2. duplicate identity、依赖循环、版本不兼容和权限不足在 active 前失败。
3. plugin 不能修改 revision、credential owner、storage transaction、archive 或 event sequence。
4. normal completion、error、panic、cancel 和 unload 都 exactly-once dispose。
5. permission 只能收紧，不能由 plugin 放宽。
6. capture/replay 绑定 plugin identity 和 operation；replay 缺少 capture 时不能 live fallback。
7. SourceAdapter 只能进入 prepare → candidate → install 流程。
8. QuickJS plugin 的 module loader、host API、quota、watchdog 和 cancellation 在 Rust 与 JS 两侧都有 contract test。
9. 核心外 fixture 新增 node/value/operation/service/editor/source adapter 不修改核心闭集或前端分支.
10. generation 更新, 正常 drain, 权限撤销, 并发清理与 disposer 失败都验证真实资源状态.
11. package 安装, 更新失败, state 保留/清除和崩溃恢复验证持久提交与残留资源.
12. operation 内多个 host effect 的封存 trace 在插件未激活时可 replay, 且不产生 live I/O 或当前 state 访问.

## 实施顺序

以下是依赖方向, 不再是按层完成即验收的清单. 实现用端到端 tracer bullet 穿过开放契约, 内置与外部插件, 规则生命周期和编辑器, 随后补齐 generation/service, 本地管理恢复, QuickJS 与来源适配; 安全准入和 capture 必须随每个切片交付. #22 旧子事项在重新拆分前不代表可直接执行的 frontier.

```text
0. plugin-contract：stable IDs、manifest、envelope、错误和 capability contract
1. PluginHost：dependency resolver、scope、lease、lifecycle、frozen registry
2. Definition / Plan / Value / Effect identity 开放化，内置节点先做 Rust adapters
3. compiler/runtime dispatch registry 化，保留 capture/replay 不变量
4. Rule Kernel document lifecycle：Draft Revision → validation/compile → Effective Revision
5. QuickJS plugin runtime 和 Rust host API
6. authoring session、declarative editor descriptor、semantic projection
7. SourceAdapter 与 prepare → candidate → install
8. 本地 package catalog, 管理界面与重启恢复
9. 核心零修改扩展实验, 旧闭集删除与跨平台完整验收
```

## 领域词汇范围

`CONTEXT.md` 继续只维护用户可识别的领域概念：Source、Install Candidate、Native Rule Document、Rule Draft Revision、Effective Rule Revision 和标准媒体模型。`PluginHost`、QuickJS host API、registry、loader 和 Rust ABI 属于实现架构术语，不加入 `CONTEXT.md`。
