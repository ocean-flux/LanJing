---
date: 2026-06-26
topic: legado-rule-engine
---

## Summary

10 crate workspace 骨架完善(7 domain crate + src-tauri + 新建 `lj-compiler` + `lj-node-extract`)+ Legado 5 端点完整规则引擎,从粘贴真实源规则到搜索一本书、读完整章节内容,完整 fan-out 链路在前端流式渲染。支持 HTML/XML/JSON 多数据类型 + CSS/XPath/JSONPath/正则多选择器。双轨验收:wiremock 回放验证逻辑链路 + 真源站 QA 最终确认。

## Problem Frame

21 ADR 已定架构方向,但 ADR 体系留了三处空白。HTML/CSS 选择器提取能力无 crate 归属——8 crate 列表中没有内容提取 crate。`NodeKind` enum 从未定义完整 variant——ADR-0011 把端点变节点但没定义"HTML 解析节点"或"字段提取节点"。`SearchSpec`/`DiscoverSpec`/`DetailSpec`/`TocSpec`/`ContentSpec` 内部字段从未定义——ADR-0007 只定义了外层 struct 形状。

原 tracer-bullet 计划(已随 `.omo/` 删除)基于两个错误假设。`NodeProcessor::process` 签名无输入参数,无法支持 fan-out——真实 Legado 链是 search 返 N 本 → 每本 detail → toc 返 M 章 → 每章 content。`Sandbox` 字段用全 `bool`,违反 ADR-0008 的 `Option<bool>` partial override 语义。

用户决定从 tracer-bullet 跃迁到完整规则引擎。规则升级算法、缓存重试层、并行分支节点从推迟改实现。HTML/CSS 选择器能力扩展为多数据类型(HTML/XML/JSON)+ 多选择器(CSS/XPath/JSONPath/正则)。grill-with-docs 会话定了核心架构:NodeProcessor stream-to-stream 签名、NodeData enum、端点子图模板、10 crate 划分、执行分段、子例程表、ConditionBranch enum、节点执行流转可观测。新增 ADR-0022 到 ADR-0026 记录这些决策。

## Key Decisions

**NodeProcessor stream-to-stream 签名(ADR-0022)。** `process` 同步接收 `BoxStream<NodeData>` 输入,同步返回 `BoxStream<NodeData>` 输出,非 async_trait。搭管道是同步操作,管道内部流动才是异步。节点自主控制 fan-out 并发。

**NodeData 闭集 enum(ADR-0022)。** `Raw(String)` / `HttpResponse`(lj-core 自有 struct,不绑 reqwest)/ `Media` / `Json(serde_json::Value)`。XML 不单独设 variant——xmloxide::Document 是 lj-node-extract 内部解析细节。

**端点子图模板 + 节点 I/O 类型声明(ADR-0023)。** 端点不是 NodeKind,是合法子图模板(search = `Http→Extract`)。模板定义放 lj-core 作 GraphSchema,验证逻辑放 lj-importer。本体规则也经验证。NodeKind 6 种全是执行节点(Http/Js/Extract/Merge/Condition/Loop)。

**10 crate 划分(ADR-0024)。** 新增 `lj-compiler`(规则语法解析器,字符串→ExtractRule IR)和 `lj-node-extract`(提取节点处理器)。ExtractRule IR 类型放 lj-core(闭集 enum),lj-compiler 依赖 lj-core(方向正确)。

**执行分段:前端控制(ADR-0025)。** 5 端点链被用户交互切 3 段,分段逻辑在前端 + IPC `execute_segment`,Graph 不含段边界标记。

**子例程表 + ConditionBranch enum(ADR-0026)。** Loop 子图以子例程形式存 `Graph.subroutines: HashMap<SubroutineId, Graph>`。Condition 出边用 `ConditionBranch` enum(True/False/Case),Edge 加 `condition_branch: Option<ConditionBranch>`。Merge 用 mpsc 多 producer 单 consumer,首刀 stub。

**节点执行流转可观测。** executor 对每节点 output stream 包装 tap,每个 NodeData item 流过时 emit Tauri event `node-output`,前端实时看到数据流过。首刀 IN——推迟后面补要改 trait + executor + stream 中间层,容易走歪。

**完整 fan-out over 线性单链。** 验收要求 search 返多本 + toc 返多章。

**双轨验收 over BLOCKED 即停。** wiremock 回放验证逻辑链路,真源站 QA 作为最终确认。

**全局默认 sandbox over 源级 partial override。** 首刀只验证全局默认 + JS capability gating。`Sandbox` 保持全 `bool`。

**真实源测试产物隔离在集成测试 crate 内部。** 防版权触碰,录制响应/截图/tracing 日志不进版本控制。

**导入支持本体和第三方规则。** 统一 `Importer<Opts>` trait,本体规则直通(反序列化),第三方规则翻译(调 lj-compiler)。前端 IPC 统一 `import_rule_with_preview`,内部按格式分发。

**规则升级算法/缓存重试层/并行分支节点从推迟改实现。** ADR-0019 升级算法完整实现。ADR-0009 缓存重试纳入。ADR-0013 Merge/Condition/Loop impl(Merge stub 预留)。

**技术栈已确认。** `scraper 0.27.0`(HTML/CSS)、`xmloxide 0.4.3`(XML/XPath)、`jsonpath-rust 1.0.4`(JSON/JSONPath)、`regex 1.12.4`(正则)。URL 模板用 `str::replace` 零依赖。

## Requirements

### Workspace skeleton

R1. Cargo workspace 含 10 个成员 crate(9 个 domain crate + src-tauri 本包):`lj-core`/`lj-compiler`/`lj-runtime`/`lj-node-http`/`lj-node-js`/`lj-node-extract`/`lj-sandbox`/`lj-importer`/`lj-storage` + `src-tauri` 本包。每 crate 立 trait/泛型模块/`thiserror` 错误类型,即便 impl 是 stub。

R2. 集成测试独立 crate(不进 workspace members),含跨 crate 集成测试与真源站 QA。真实源测试产物隔离在该 crate 内部,通过该 crate `.gitignore` 排除出版本控制。

R3. `AGENTS.md` 提交作用域清单补 `core`/`compiler`/`sandbox`/`node-http`/`node-js`/`node-extract`/`ui-rs` 七项。

### Core types and traits

R4. `lj-core` 全类型冻结:`Media`/`Book`/`Video`/`Audio`/`Sandbox`/`Capabilities`/`HttpSpec`(含 `endpoint_kind`)/`EndpointKind`(5 variant)/5 `EndpointSpec`/`NodeKind`(6 variant:Http/Js/Extract/Merge/Condition/Loop)/`NodeSpec`/`Node`/`Edge`(含 `condition_branch`)/`Graph`(含 `subroutines`)/`NodeId`/`SourceId`/`SubroutineId`。

R5. `lj-core` 定义 `NodeData` enum(`Raw`/`HttpResponse`/`Media`/`Json`),`HttpResponse` 是 lj-core 自有 struct(status/headers/body/charset),不绑 reqwest。

R6. `lj-core` 定义 `ExtractRule` 闭集 enum(`CssSelector`/`XPath`/`JsonPath`/`Regex` + 提取类型 + 正则清理),作为规则语法解析的 IR。`ExtractSpec.rules: Vec<ExtractRule>`。

R7. `lj-core` 定义 `GraphSchema`/`EndpointTemplate`(端点子图模板纯数据结构),`ConditionBranch` enum(`True`/`False`/`Case(String)`)。

R8. `lj-core` 全 trait 边界冻结:`Importer<Opts>`/`NodeProcessor`/`Executor`/`Repository<T>`/`RepoId<T>`/`CapabilityLoader`。`NodeProcessor::process` 签名:`fn process(&self, ctx, spec, input: BoxStream<NodeData>) -> BoxStream<NodeData>`,sync 返回 stream,非 async_trait。每节点声明 `input_type`/`output_type`。

R9. `Node` struct 含双 ID 字段(`node_id: NodeId` UUIDv4 恒久 + `import_hash: String` 64 字符 hex sha256 canonical json spec)。

R10. 每 domain crate 独立 `thiserror` 错误类型,domain crate 公共 trait API 不 leak `anyhow`。

### Compiler

R11. `lj-compiler` 含 Legado 规则语法解析器,将 `@text`/`@href`/`@src` 后缀、`||` 多选回退、`##regex##replacement` 正则清理解析成 `ExtractRule` IR。纯函数字符串→IR,无 IO。

### Importer

R12. `lj-importer` 调 `lj-compiler` 解析 Legado 规则字符串→`ExtractRule`,翻译 5 端点为子图(`Http→Extract`),生成 Graph + 端点间 Edge。模板验证(`GraphSchema::validate`)。本体规则(节点图 JSON 直通)也经验证。

R13. `LegadoImporter` impl `Importer<LegadoSourceJson>`,fixture 目标源 JSON → 5 端点子图 + 双 ID 补全。`NativeImporter` impl `Importer<GraphJson>`,本体规则直通反序列化。

R14. 规则升级算法 stub 实现:二次导入同 `source_url` 时返回 `ImportOutcome::New`(全量替换 + 提示用户编辑将丢失)。`import_hash` 双 ID schema 到位,完整差异合并算法推迟到二次导入成为真实用户场景时实现(ADR-0019 原立场)。

### Content extraction

R15. `lj-node-extract` impl `NodeProcessor`,`kind()` 返 `NodeKind::Extract`。消费 `HttpResponse`/`Json`,按 `ExtractRule` IR 执行提取,产 `Media`。支持 HTML(`scraper`)、XML(`xmloxide`)、JSON(`jsonpath-rust`)三种数据类型。

R16. 选择器引擎支持 CSS selector(`scraper`)、XPath(`xmloxide`)、JSONPath(`jsonpath-rust`)、正则表达式(`regex`)四种提取方式。

R17. URL 模板渲染(`{{key}}`/`{{page}}`)用 `str::replace` 零依赖实现。

### HTTP node

R18. `lj-node-http` impl `NodeProcessor`,`kind()` 返 `NodeKind::Http`。`reqwest` + cookie jar 同源共享 + Tauri Store UA 接线。内部做 `reqwest::Response → HttpResponse` 转换。**目标主机校验**:默认阻断 RFC1918/环回/链路本地/云元数据地址段,防 SSRF。

R19. 缓存重试层推迟到下一刀(首刀单源单会话无需缓存重试)。`lj-node-http` 用 reqwest 默认 redirect on 即可。

### JS node

R20. `lj-node-js` 用 `rquickjs` 集成 + sandbox capability gating + Legado `exploreUrl` `@js:` 块执行。`caps.fs=false` 时 `fs_read` 返 `CapabilityBlocked(Capability::Fs)`。**前置:枚举 目标源实际使用的 Legado 宿主 API 清单**(如 `java.net.URLEncoder`、`result.push/get`、`baseUrl` 等),R20 范围声明为"目标源用到的 @js: 子集"而非"Legado 完整集成"。

### Sandbox

R21. `lj-sandbox` 实现 `Capabilities` + `default_capabilities()`(network=true,fs/env/process=false)+ `merge(global, source)` 合并。首刀只验证全局默认,源级 partial override 推迟。

### Runtime execution

R22. `lj-runtime` `GraphExecutor` stream-to-stream 管道执行:每节点 `process(input: BoxStream) -> BoxStream`,executor 把上游 output 喂给下游 input,形成 stream pipeline 链。tracing span 树 `trace_id` 贯穿。

R23. `Condition`/`Loop`/`Merge` 节点首刀 stub 预留(NodeKind variant 保留,processor 返 NotImplemented)。目标源线性链不用控制流,与 Merge stub 标准统一。完整 impl 推迟到有真实消费者时。

R24. tracing span tree 含中文 message + 英文 snake_case field key(ADR-0016)。

R25. 节点执行流转可观测:executor 对每节点 output stream 包装 tap(`inspect`/`TapStream`),每个 NodeData item 流过时 emit Tauri event `node-output { node_id, data_summary }`。前端实时看到数据流过。

### Storage

R26. `lj-storage` SQLite `rusqlite` 集成 + `Repository<Graph/Media/Cookie>` CRUD + `RepoId<T>` 类型隔断。

### Tauri IPC

R27. Tauri IPC `import_rule_with_preview`/`confirm_import`/`list_rules`。import 前必经 preview 中介步(source URL + 节点数 + JS 块数 + sandbox 声明 + **所有 HTTP 目标 URL 模板列表** + **JS 块源码可读**),用户 confirm 才落库。统一入口,内部按规则格式分发(本体直通/第三方翻译)。

R28. Tauri IPC `execute_segment { rule_id, segment, ... }`:按段执行(search/discover 段、detail+toc 段、content 段)。每段独立执行,executor 跑完就结束。流式 emit `rule-output` Tauri event。

### Frontend

R29. 前端 import 预览面板 + confirm 落库 flow。粘贴规则 JSON → 解析预览 → 确认导入 → 落库成功。

R30. 前端执行 witness 页:按段执行(search → 用户选书 → detail+toc → 用户选章 → content),流式渲染 `BookMedia` card 列表 + 章节列表 + 正文。`listen('rule-output')` + `listen('node-output')` reactive append。

### Verification

R31. 双轨验收:wiremock 回放层验证 5 端点链路逻辑(录制真实源站一次成功响应作 fixture)+ 真源站 QA 最终确认。源站不通时逻辑层仍可推进,真源站层挂起。

R32. 真实 目标源端到端 manual QA:import → confirm → search → 选书 → detail → toc → 选章 → content,完整 fan-out 链路流式渲染 + tracing 日志含中文 message + `trace_id` 一致 + 节点流转可观测。

## Key Flows

- F1. Import with preview
  - **Trigger:** 用户粘贴规则 JSON 到 import 预览面板
  - **Actors:** 用户,前端,Tauri IPC,Importer,lj-compiler,lj-storage
  - **Steps:** 前端调 `import_rule_with_preview` → IPC 检测格式(本体/第三方)→ 第三方调 lj-compiler 解析规则字符串→ExtractRule + 翻译 5 端点为子图 → `GraphSchema::validate` → 返回 ImportPreview(不含真源站访问)→ 前端渲染预览 → 用户点确认 → `confirm_import` 落库
  - **Covered by:** R11, R12, R13, R27, R29

- F2. Rule execution segmented fan-out
  - **Trigger:** 用户选规则 + 输搜索关键词 + 点执行
  - **Actors:** 用户,前端,Tauri IPC,GraphExecutor,HttpNodeProcessor,ExtractNodeProcessor
  - **Steps:** 段 1:前端调 `execute_segment { segment: "search" }` → GraphExecutor 跑 search 子图(Http→Extract)→ 流式 emit `rule-output` + `node-output` → 前端渲染 N 本 BookMedia → 用户选 1 本 → 段 2:前端调 `execute_segment { segment: "detail_toc", book_url }` → 跑 detail+toc 子图 → 流式 emit M 章 → 用户选 1 章 → 段 3:前端调 `execute_segment { segment: "content", chapter_url }` → 跑 content 子图 → 流式 emit 正文
  - **Covered by:** R15, R18, R22, R25, R28, R30, R32

- F3. Rule upgrade (stub)
  - **Trigger:** 用户二次导入同 `source_url` 的新版规则
  - **Actors:** 用户,Importer,RulesRepository
  - **Steps:** Importer 解析新版 JSON → stub 行为:返回 `ImportOutcome::New`(全量替换 + 提示用户编辑将丢失)→ 前端提示用户
  - **Covered by:** R9, R14

- F4. Discover endpoint (exploreUrl @js:)
  - **Trigger:** 用户选规则 + 点"发现/分类浏览"(无关键词)
  - **Actors:** 用户,前端,Tauri IPC,GraphExecutor,JsNodeProcessor,HttpNodeProcessor,ExtractNodeProcessor
  - **Steps:** 段 1:前端调 `execute_segment { segment: "discover" }` → GraphExecutor 跑 discover 子图(Js 执行 exploreUrl @js: 块产 URL 列表 → Http 请求每个 URL → Extract 提取 BookMedia)→ 流式 emit `rule-output` + `node-output` → 前端渲染 N 本 BookMedia → 用户选 1 本 → 段 2/段 3 同 F2
  - **Covered by:** R15, R18, R20, R22, R25, R28, R30, R32

## Acceptance Examples

- AE1. Real source reachable
  - **Covers R32.**
  - **Given:** 目标源站 可达,已导入 目标源规则
  - **When:** 用户搜索"修罗"并触发段 1 执行
  - **Then:** 流式渲染 3+ BookMedia cards,`node-output` event 实时推送,tracing 日志含中文 message 且 `trace_id` 贯穿 span tree

- AE2. Real source unreachable
  - **Covers R31.**
  - **Given:** 目标源站 不通,wiremock 回放层含录制 fixture
  - **When:** 跑集成测试 crate 的回放测试
  - **Then:** 5 端点链路在回放数据上跑通,逻辑层验收通过,真源站层标挂起而非全盘 BLOCKED

- AE3. Rule upgrade stub on re-import
  - **Covers R14.**
  - **Given:** 已导入 目标源规则(首次 `ImportOutcome::New`)
  - **When:** 用户二次导入同 `source_url` 的新版 JSON
  - **Then:** stub 返回 `ImportOutcome::New`(全量替换),前端提示用户编辑将丢失

- AE4. Native rule import
  - **Covers R13.**
  - **Given:** 用户有 LanJing 本体格式规则 JSON(含 `nodes`/`edges`/`subroutines`)
  - **When:** 粘贴到 import 预览面板
  - **Then:** `NativeImporter` 直通反序列化为 Graph,`GraphSchema::validate` 通过,预览显示节点图信息

- AE5. Discover endpoint (exploreUrl @js:)
  - **Covers R20, R32.**
  - **Given:** 目标源站 可达,已导入 目标源规则
  - **When:** 用户点"发现/分类浏览"触发 discover 段执行
  - **Then:** Js 节点执行 exploreUrl @js: 块产 URL 列表,Http 请求每个 URL,Extract 提取 BookMedia,流式渲染 N 本 BookMedia cards

## Success Criteria

- **用户价值门控(最高优先级)**:用户粘贴 目标源 JSON → 搜到 ≥1 本 → 选 1 本 → 读到 ≥1 章正文文本。此标准为门控项,不通过则项目不算完成。
- `cargo test --manifest-path src-tauri/Cargo.toml --workspace` + `cargo clippy --workspace --all-targets --all-features -- -D warnings` + `cargo fmt --check --all` 全 0 exit code。
- 10 crate skeleton 完整:`Get-ChildItem src-tauri/crates/*/Cargo.toml` 计 9 个 domain crate + src-tauri 本包 = 10 members。
- 集成测试 crate 独立于 workspace,单独命令跑通 wiremock 回放层。
- 真实 目标源端到端 manual QA 截图 + tracing 日志存集成测试 crate 内部(gitignore)。
- 节点执行流转可观测:`node-output` event 在每段执行中实时推送。
- 离线护栏 ADR-0018 不变:无自有 backend / 登录 / 云端同步。

## Scope Boundaries

### Deferred for later

- 源级 partial override(ADR-0008 已立能力,首刀只验证全局默认,切通后 grill 边界再补)
- `tauri-plugin-deep-link` 集成(`lanjing://import` 入口,首刀仅 textarea 粘贴)
- xyflow 节点图编辑器含调试/验收能力(ADR-0011 已立 xyflow,首刀前端只显 TreeView 只读,调试/断点/重跑推迟)
- FTS5 全文搜索表与 UI(ADR-0014 已立 FTS5,首刀先空表)
- Tauri Store 全局设置面板 UI(ADR-0017 已立,首刀 UA 用内置默认值)
- 搜索历史 UI / 阅读进度 UI(表 schema 建,前端 UI 暂不接)
- UI 主题切换 / 字体偏好美化
- Merge 节点完整 impl(首刀 stub 预留,Legado 链路无分支合并场景)
- Condition/Loop 节点完整 impl(首刀 stub 预留,与 Merge 标准统一,目标源线性链不用控制流)
- 缓存重试层(首刀单源单会话无需,推迟到下一刀)
- 规则升级算法完整 impl(首刀 stub,完整差异合并推迟到二次导入成为真实用户场景)

### Outside this product's identity

- Maccms10 / LXMusic importer 实现(独立 crate 已建骨架,实现 OUT)
- WebDAV 同步实现(ADR-0020 stub)
- LanJing 自有 backend / 登录认证 / 云端同步(ADR-0018 离线护栏)

## Dependencies and Assumptions

- rquickjs 跨平台编译:Windows MSVC 下 QuickJS C 编译需提前 spike,不应等到后期才首次验证。
- 目标源站 可达性:真源站 QA 依赖外站,不通时走 wiremock 回放 fallback。
- `scraper 0.27.0` / `xmloxide 0.4.3` / `jsonpath-rust 1.0.4` / `regex 1.12.4` 均兼容 Rust edition 2024 / MSRV 1.96(crates 编译已确认)。**Legado 选择器语法覆盖待审计**:需对 目标源实际规则做语法面审计,产出覆盖子集与未覆盖子集。
- **Legado @js: 宿主 API 待枚举**:rquickjs 纯 JS 引擎,Legado @js: 块依赖 Legado 宿主 API(java.net.URLEncoder 等),需枚举 目标源用到的宿主 API 作为 R20 范围声明。
- ADR-0022 到 ADR-0026 记录 grill 决策,ce-plan 必须遵从。

## Outstanding Questions

### Resolve before planning

- `execute_segment` 子图裁剪机制:Graph 不含段边界(ADR-0025),executor 如何只跑子图一部分?推荐方案:IPC 传 endpoint_kind,executor 按 HttpSpec.endpoint_kind 选 entry 节点 + 保留子图内部边。
- JS 执行资源上限:rquickjs 内存上限/栈深度/执行超时(防 DoS)。
- NodeProcessor stream-to-stream 早期 spike:用 stream-to-stream 实现 Http+Extract 最小例评估实现负担,tap 实现成独立 wrapper 层。

### Deferred to planning

- 集成测试 crate 的测试运行命令(`cargo test --manifest-path` 单独跑 vs pnpm script 封装)。
- `lj-node-extract` 内部模块结构(按数据类型分 HTML/XML/JSON sub-mod vs 按选择器分 CSS/XPath/JSONPath/Regex sub-mod)。
- `execute_segment` IPC 的具体参数形态(每段传什么参数:search 段传 query,detail_toc 段传 book_url,content 段传 chapter_url)。
- HttpResponse body charset 解码归属(lj-node-http 解码存 String vs lj-node-extract 按 charset 字段解码 Vec<u8>)。
- Cookie 存储敏感性(明文存 SQLite vs 加密 vs webdav 同步排除)。
- HttpResponse body 大小上限(防 OOM)。
- 前端 UX 状态(loading/empty/error/partial/段间过渡/选书选章交互/node-output 展示/视图导航)。

## Sources and Research

- ADR-0001 首刀切 Legado 图文书源
- ADR-0002 rquickjs 作 JS 运行时
- ADR-0003 自有 schema + Legado 导入器
- ADR-0007 端点 spec struct + Option
- ADR-0008 JS 沙箱分级可配置
- ADR-0009 HTTP spec 中等字段集
- ADR-0011 节点图一等公民
- ADR-0012 8 crate workspace trait+泛型(被 ADR-0024 覆盖)
- ADR-0013 流式异步执行范式
- ADR-0014 SQLite 统一存储 rusqlite
- ADR-0015 thiserror domain + anyhow glue
- ADR-0016 tracing + trace_id + 中文 message
- ADR-0017 用户配置 Tauri Store
- ADR-0018 离线单机无自有后端
- ADR-0019 节点双 ID UUID + content hash
- ADR-0020 WebDAV 同步 stub
- ADR-0021 多入口 + 导入预览中介步
- ADR-0022 NodeProcessor stream-to-stream 签名与 NodeData enum
- ADR-0023 端点子图模板与节点 I/O 类型声明
- ADR-0024 10 crate 划分(加 lj-compiler + lj-node-extract)
- ADR-0025 执行分段:前端控制
- ADR-0026 子例程表与 ConditionBranch enum
- `CONTEXT.md` glossary 领域术语
- `scraper 0.27.0`:docs.rs/scraper,已启用 edition 2024,Legado 官方 Tauri 项目同用
- `xmloxide 0.4.3`:github.com/jonwiggins/xmloxide,MSRV 1.81,XPath 1.0+ 含 2.0 函数,Send+Sync
- `jsonpath-rust 1.0.4`:RFC 9535 合规,90 天 1200 万下载
- `regex 1.12.4`:Rust 官方正则,MSRV 1.65
