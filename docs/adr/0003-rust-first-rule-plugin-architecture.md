# ADR 0003：规则插件采用 Rust-first、QuickJS host API 和分域扩展契约

- 状态：已采纳
- 影响范围：规则模型、规则编译与运行时、Native Rule Document、Source Adapter、QuickJS plugin runtime

## 背景

LanJing 的规则系统需要支持高内聚、低耦合和丰富的插件 API。当前实现对固定能力进行了强校验，但扩展边界仍是闭集：

- `FlowNodeConfig` 和 `PlanNodeConfig` 是闭集 enum。
- `EffectKind` 只有 HTTP、QuickJS、Extract。
- `EffectHandlers` 直接持有三类 concrete handler。
- `RuleSystem` 直接组装 storage、compiler、runtime 和 node adapter。

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

command/query 使用显式方法。事件只用于生命周期、诊断、进度和明确声明的 transform pipeline，不能通过监听器隐式改变核心结果。

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

## 实施顺序

```text
0. plugin-contract：stable IDs、manifest、envelope、错误和 capability contract
1. PluginHost：dependency resolver、scope、lease、lifecycle、frozen registry
2. Definition / Plan / Value / Effect identity 开放化，内置节点先做 Rust adapters
3. compiler/runtime dispatch registry 化，保留 capture/replay 不变量
4. Rule Kernel document lifecycle：Draft Revision → validation/compile → Effective Revision
5. QuickJS plugin runtime 和 Rust host API
6. authoring session、declarative editor descriptor、semantic projection
7. SourceAdapter 与 prepare → candidate → install
```

## 领域词汇范围

`CONTEXT.md` 继续只维护用户可识别的领域概念：Source、Install Candidate、Native Rule Document、Rule Draft Revision、Effective Rule Revision 和标准媒体模型。`PluginHost`、QuickJS host API、registry、loader 和 Rust ABI 属于实现架构术语，不加入 `CONTEXT.md`。
