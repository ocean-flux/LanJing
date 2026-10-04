# ADR 0004：规则优先的开放扩展架构

- 状态：已采纳
- 取代：ADR 0003
- 影响范围：规则包、来源输入、规则编译、规则编辑器、脚本节点和 Source 生命周期

## 背景

LanJing 本身是规则驱动的本地媒体工作台。规则已经是用户增加来源和媒体处理能力的主要扩展机制。

如果再把 node、source、editor、service、runtime 和前端页面统一包装为通用 plugin system, 会重复规则的职责, 并引入与当前产品阶段不匹配的安装、升级、隔离、跨插件状态和前端扩展成本。

Legado 的主要扩展模型也是来源定义、规则 DSL 和来源脚本。新版把书源脚本与前端插件分成两条独立路线。LanJing 借鉴其来源包、自包含文件、作者工具和版本声明, 但不把规则 JSON 或来源脚本误称为通用插件, 也不照搬无沙箱脚本和覆盖式更新。

## 决定

### 1. 规则包是一期开放扩展单位

一期对外开放的是 `Rule Package`, 不是通用 plugin package。

Rule Package 可以包含:

- Source Definition metadata
- 规则图 Definition
- node、value type 和 port descriptor
- 配置 schema 和 canonicalization
- 可选的受控 script node 配置
- editor descriptor
- 兼容的 source input adapter metadata
- 版本、来源、摘要和更新声明

第三方作者基于稳定 Rule Contract 制作 Rule Package。用户可以导入、审阅、保存、验证、安装和更新它。

规则包不能直接获得 SQLite、Tauri、React state、文件系统、进程、event sequence、revision、archive 或明文 credential。

### 2. Rule Contract 取代通用 Plugin Contract

规则扩展使用稳定的 namespaced identity、可扩展 envelope、descriptor schema 和明确的 contract version。

开放的能力包括:

1. node descriptor、value type descriptor 和 port descriptor。
2. 规则配置 schema、默认值、canonicalization 和诊断。
3. compiler lowering 到 immutable Plan operation。
4. 受控 effect 和 host capability 声明。
5. Source Definition 到标准 Rule Package Draft 的输入转换。
6. 声明式 EditorDescriptor, 用于配置字段、端口和诊断投影。

核心保留 scheduler、取消、权限交集、credential ownership、revision、事务、capture/replay 和标准媒体模型。

新增规则能力只增加 namespaced descriptor 和 package 内容, 不增加核心能力专属 enum、dispatch 分支或 React 专属组件。

### 3. 来源扩展优先于通用运行时扩展

来源扩展的完整路径为:

```text
Rule Package
  -> Adaptive Source Input
  -> Install Candidate
  -> 审阅和授权
  -> Source Revision
  -> immutable Plan
  -> 标准媒体模型
```

Source input adapter 只能解析、转换和验证输入, 不能直接创建 Source Revision 或绕过安装事务。

一期兼容 Legado JSON 和后续可定义的自包含规则文件。Legado 的字符串规则、XPath/JSON/CSS/regex 选择器、替换规则和来源仓库索引可以作为导入和作者工作流的参考, 但 LanJing 内部仍使用结构化 Definition、编译和 immutable Plan。

### 4. 脚本只作为受控规则节点

需要代码逻辑时, 先提供受控 script node, 而不是独立的通用 plugin runtime。

脚本运行在 Rust 管理的 QuickJS 边界内, 只能通过稳定 host API 使用被授予的 network、opaque credential slot、clock、random、state、logging、diagnostics 和 cancellation。

脚本调用必须遵守资源预算、权限交集、capture/replay 和取消规则。脚本不能注册任意 service、注入前端页面、修改规则生命周期或动态改变 scheduler。

是否增加新的 host capability, 由后续 ADR 或 contract version 单独决定。

### 5. 前端消费规则描述, 不执行插件 UI

前端实现规则和来源工作流:

- Rule Package 导入、审阅、验证、安装和更新。
- Source、Source Revision 和授权管理。
- Native Rule Document 的编辑、保存、诊断和恢复。
- 根据 EditorDescriptor 渲染插件节点的字段、端口和诊断。
- 对未知或缺失 descriptor 的节点进行 unavailable 展示和安全 round-trip。

规则包不能注入 React component、路由、Tauri command、WebView script 或 App Surface 布局。

### 6. 版本演进采用新增优先

- 已发布的 identity 不被覆盖。
- 不兼容的 schema 或 contract 必须显式升级版本。
- 未知 node、value type 和 payload 可以展示、保存和 round-trip, 但不能 validate、compile 或 execute。
- Rule Package 更新必须经过 Install Candidate 和 stale 检查, 不能覆盖式静默更新。
- Rule Draft Revision 失败不能替换 Effective Rule Revision。
- 规则执行使用绑定的 immutable Plan、descriptor digest 和 Source Revision。

### 7. 暂不建设通用插件平台

一期不包含:

- 动态 Rust ABI、dylib、cdylib 或独立进程 plugin。
- 通用 PluginHost、跨插件 service、registration lease 和 plugin catalog。
- 任意前端 UI plugin、页面路由、WebView 注入和自定义 App Surface。
- 在线 marketplace、自动下载和云端分发。
- 任意文件系统、环境变量、进程和原生平台调用。
- 无沙箱任意 JS、浏览器自动化和反爬绕过。

如果未来出现无法由 Rule Package 或受控 script node 表达的稳定平台能力, 再为该能力设计窄的 host adapter, 不恢复通用 plugin system 作为默认扩展入口。

## 验收

一期必须由核心之外的 fixture Rule Package 完成:

1. 新 node、value type、port 和 editor descriptor 的导入、编辑、保存和诊断。
2. 新来源规则的 prepare、Install Candidate、授权、Source Revision 和标准媒体输出。
3. 受控 script node 的正常完成、错误、超时、取消、权限拒绝和资源超限。
4. Definition canonicalization、immutable Plan、descriptor digest 和 unknown payload round-trip。
5. capture/replay、禁止 live fallback 和凭证脱敏。
6. Rule Draft Revision 与 Effective Rule Revision 的失败隔离。
7. 前端管理工作流在桌面和移动目标保持同一 wire contract。

## 后续扩展

后续可以新增规则包、选择器、来源适配器、受控 host capability 和 editor field, 但必须保持上述 Rule Contract 和 Kernel/Runtime 所有权边界。

## 参考

- [Legado](https://github.com/LegadoTeam/legado)
- [Legado docs](https://docs.legadoteam.org/guide/introduction.html)
- [Legado rule parser](https://github.com/LegadoTeam/legado-rule)
- ADR 0002: versioned source and rule lifecycle
- ADR 0003: rust-first rule plugin architecture, superseded by this ADR
