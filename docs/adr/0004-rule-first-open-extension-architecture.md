# ADR 0004：规则优先的来源扩展架构

- 状态：已采纳
- 取代：ADR 0003
- 影响范围：规则编辑、来源安装、规则编译、受控脚本、Source 生命周期和 capture/replay

## 背景

LanJing 是规则驱动的本地媒体发现与阅读工作台。用户增加来源和媒体处理能力的主要方式应当是定义规则, 而不是开发、安装和管理通用插件。

通用 plugin system 会重复规则的职责, 并引入 PluginHost、插件目录、跨插件 service、独立状态、动态加载、前端扩展和多 runtime 的长期成本。当前产品没有足够证据证明这些成本是必要的。

Legado 的主要扩展模型是来源实体、规则 DSL 和来源脚本。新版把书源脚本与前端插件分成两条独立路线。LanJing 借鉴其来源文件、选择器、导入改写和作者工具, 但保留自己的结构化 Definition、revision、权限、immutable Plan 和 capture/replay 不变量。

## 决定

### 1. 用户规则是一期扩展边界

一期对外提供的是规则定义和可导入的 Rule Package, 不是通用 executable plugin package。

用户可以通过规则定义:

- 请求和分页。
- HTML/XML/JSON 内容提取。
- XPath、CSS、Regex 和 JSON selector。
- 字段映射、清洗、条件、循环和合并。
- 标准媒体模型的字段输出。
- 受控 JS 变换节点。

Rule Package 是规则、来源 metadata、版本声明和可选脚本配置组成的可导入/导出文件。它不是动态二进制, 不能声明任意宿主 service, 也不能注入前端代码。

### 2. Rule Contract 服务于规则编辑和编译

Rule Contract 提供稳定的 namespaced identity、可扩展 envelope、descriptor schema、port、配置 canonicalization 和诊断。

规则节点可以声明:

1. 输入和输出 port。
2. value type 和兼容关系。
3. 配置 schema、默认值和 canonicalization。
4. compiler lowering 到 immutable Plan operation。
5. 所需的受控 host capability。
6. 用于编辑器渲染的声明式 metadata。

这些 contract 首先服务 LanJing 自己的规则编辑器、导入器和 compiler。它们不承诺第三方实现 Rust trait、动态 Rust ABI 或任意 runtime module。

新增规则能力应通过通用节点和规则配置表达, 不增加核心能力专属的前端页面和 runtime dispatch 分支。确实需要新增宿主能力时, 另行设计窄的 host adapter。

### 3. 来源扩展经过既有 Source 生命周期

来源规则的完整路径为:

```text
Adaptive Source Input
  -> Source Definition
  -> Rule Package / Native Rule Document
  -> Install Candidate
  -> 审阅和授权
  -> Source Revision
  -> immutable Plan
  -> 标准媒体模型
```

source input adapter 只能解析、转换和验证输入, 不能直接创建 Source Revision 或绕过安装事务。

一期兼容 Legado JSON, 并可支持自包含规则文件。Legado 的规则字符串、XPath/JSON/CSS/regex selector、替换规则和来源仓库索引可以作为导入和作者工作流的参考, 但 LanJing 内部仍使用结构化 Definition、编译和 immutable Plan。

### 4. JS 是受控变换节点, 不是通用插件 runtime

规则无法表达局部转换时, 使用受控 JS 节点。首期 JS 只处理当前节点输入、规则变量和结构化中间值, 返回结构化结果。

JS 运行在 Rust 管理的 QuickJS 边界内, 受限于超时、取消、内存/输出预算和稳定错误映射。首期网络请求优先使用独立 HTTP 规则节点, 以保持权限、凭证和 capture/replay 语义一致。

JS 不能访问 SQLite、Tauri、React state、任意文件系统、环境变量、进程、event sequence、revision、archive 或明文 credential。

未来如果有明确需求, 可以为单项 host capability 增加受控 API, 但不因此恢复通用 plugin system。

### 5. 前端实现规则工作流, 不实现插件管理

前端一期实现:

- Rule Package 导入、导出、审阅和验证。
- Source、Source Revision、授权和诊断管理。
- Native Rule Document 的编辑、Explicit Rule Save、诊断和 Recovery Draft。
- 根据规则 descriptor 渲染字段、端口、默认值和诊断。
- unknown node、value type 和 payload 的 unavailable 展示与安全 round-trip。
- 规则预览、执行、取消、capture/replay 结果和失败状态。

规则文件不能注入 React component、路由、Tauri command、WebView script 或 App Surface 布局。

### 6. 版本和安全不变量保持不变

- 已发布的 identity 不被覆盖。
- 项目仍在初期, 允许直接进行破坏性 contract、Definition、Plan、storage DTO 和 typed wire 变更。
- 不建设 v2 并存模型、旧 reader、兼容 adapter 或旧本地数据 migration。schema/version 只用于识别当前形状和拒绝不兼容输入。
- unknown payload 可以展示、保存和 round-trip, 但不能 validate、compile 或 execute。
- Source Update 必须经过 Install Candidate 和 stale 检查, 不能覆盖式静默更新。
- Rule Draft Revision 校验失败不能替换 Effective Rule Revision。
- execution 绑定 Source Revision、descriptor digest 和 immutable Plan。
- capability 取 host policy、Source grant、规则声明和 invocation grant 的交集, 只覆盖应用暴露的系统 API（fs / env / process）；网络不受 capability 控制, 不在交集中。
- plaintext credential 不进入普通 rule input、脚本 state、日志、诊断、事件或历史记录。
- 所有受控外部 effect 都必须 capture, replay 禁止 live fallback。

### 7. 一期明确不建设

- 动态 Rust ABI、dylib、cdylib 或独立进程 plugin。
- 通用 PluginHost、plugin catalog、跨插件 service 和 registration lease。
- 第三方 executable plugin SDK 和任意 plugin package runtime。
- 任意前端 UI plugin、页面路由、WebView 注入和自定义 App Surface。
- 在线 marketplace、自动下载和云端分发。
- 任意文件系统、环境变量、进程和原生平台调用。
- 无沙箱任意 JavaScript、浏览器自动化和反爬绕过。

未来如果用户规则和受控 JS 确实无法表达某种稳定平台能力, 再为该能力设计窄的 host adapter, 不把通用 plugin system 作为默认扩展入口。

## 验收

一期必须由用户可创建或导入的 fixture Rule Package 完成:

1. 新规则节点、value type、port 和 editor metadata 的导入、编辑、保存和诊断。
2. 来源规则的 prepare、Install Candidate、授权、Source Revision 和标准媒体输出。
3. 受控 JS 节点的正常完成、错误、超时、取消、权限拒绝和资源超限。
4. Definition canonicalization、immutable Plan、descriptor digest、Definition hash 和 unknown payload round-trip。
5. capture/replay、禁止 live fallback 和凭证脱敏。
6. Rule Draft Revision、Effective Rule Revision 和 Recovery Draft 的失败隔离。
7. Legado JSON 导入覆盖规则转换、凭证脱敏、失败不污染和 Source Update 审阅。
8. 桌面和移动目标使用相同 typed wire contract 完成核心规则旅程。

## 后续扩展

后续可以新增规则节点、选择器、来源输入适配器、受控 host capability 和 editor field, 但必须保持 Rule Contract 和 Kernel/Runtime 所有权边界。

## 参考

- [Legado](https://github.com/LegadoTeam/legado)
- [Legado docs](https://docs.legadoteam.org/guide/introduction.html)
- [Legado rule parser](https://github.com/LegadoTeam/legado-rule)
- ADR 0002: versioned source and rule lifecycle
