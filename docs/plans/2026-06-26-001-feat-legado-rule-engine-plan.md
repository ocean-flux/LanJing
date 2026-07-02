---
date: 2026-06-26
type: feat
origin: docs/brainstorms/2026-06-26-legado-rule-engine-requirements.md
status: draft
---

## Summary

10 crate workspace 骨架 + Legado 5 端点完整规则引擎,从粘贴真实源规则到搜索一本书、读完整章节内容,完整 fan-out 链路在前端流式渲染。支持 HTML/XML/JSON 多数据类型 + CSS/XPath/JSONPath/正则多选择器。双轨验收:wiremock 回放 + 真源站 QA。

## Problem Frame

21 ADR 定了架构方向但留了三处空白(HTML/CSS 选择器无 crate 归属、NodeKind variant 未定义、EndpointSpec 内部字段未定义)。grill-with-docs 会话定了核心架构(stream-to-stream 签名、NodeData enum、端点子图模板、10 crate 划分、执行分段、子例程表、ConditionBranch),新增 ADR-0022~0026。ce-doc-review 7 persona 审查后做了优先级重排:升级算法/缓存重试/控制流节点降为 stub,补用户价值门控标准,加 SSRF 防护,补 discover 验收。

**注**: origin 文档 Key Decisions(line 44)与 Requirements(R14/R19/R23)存在内部矛盾——Key Decisions 说"完整实现",Requirements 说"stub/推迟"。本 plan 遵循 Requirements 版本(stub),因 ce-doc-review 已确认优先级重排。

## Requirements

从 origin 文档携带,ce-plan 阶段无新增无删除:

- R1-R10: workspace skeleton + core types/traits(详见 origin)
- R11: lj-compiler 规则语法解析器
- R12-R14: lj-importer(翻译+模板验证+升级 stub)
- R15-R17: lj-node-extract(多数据类型多选择器+URL 模板)
- R18-R19: lj-node-http(SSRF 校验+cookie jar;缓存重试推迟)
- R20: lj-node-js(rquickjs+sandbox gating;目标源 @js: 子集)
- R21: lj-sandbox(全局默认,partial override 推迟)
- R22-R25: lj-runtime(stream pipeline+tracing+tap 可观测;Condition/Loop/Merge stub)
- R26: lj-storage(SQLite Repository)
- R27-R28: Tauri IPC(preview+execute_segment)
- R29-R30: Frontend(import 预览+执行 witness)
- R31-R32: Verification(双轨验收+真源站 QA)

## Key Technical Decisions

KTD1. **NodeProcessor stream-to-stream 签名(ADR-0022)**: `fn process<'a>(&'a self, ctx: &'a ExecutionContext, spec: &'a NodeSpec, input: BoxStream<'a, NodeData>) -> BoxStream<'a, NodeData>`,sync 返回 stream,非 async_trait。生命周期绑定 self/ctx/spec/input(借用语义,非 'static owned),减少 clone。搭管道同步,管道内部异步。首刀 spike 验证 Http+Extract 最小例实现负担 + 借用场景。

KTD2. **NodeData 闭集 enum(ADR-0022)**: `Raw(String)`/`HttpResponse`(lj-core 自有 struct)/`Media`/`Json`。XML 不跨节点传递(xmloxide::Document 是 Extract 内部细节)。

KTD3. **端点子图模板 + I/O 类型声明(ADR-0023)**: 端点不是 NodeKind,是合法子图模板。GraphSchema/EndpointTemplate 放 lj-core,验证放 lj-importer。NodeKind 6 种(Http/Js/Extract/Merge/Condition/Loop)。

KTD4. **10 crate 划分(ADR-0024)**: 新增 lj-compiler(规则语法→IR)+ lj-node-extract(提取执行)。ExtractRule IR 放 lj-core。

KTD5. **执行分段:前端控制(ADR-0025)**: execute_segment IPC 按段执行,Graph 不含段边界。子图裁剪:IPC 传 endpoint_kind,executor 按 HttpSpec.endpoint_kind 选 entry 节点 + 保留子图内部边。

KTD6. **子例程表 + ConditionBranch(ADR-0026)**: Graph.subroutines: HashMap<SubroutineId, Graph>。Edge.condition_branch: Option<ConditionBranch>。Merge mpsc 多 producer 单 consumer。三者首刀 stub。

KTD7. **节点执行流转可观测**: executor tap stream emit `node-output` Tauri event。tap 用 async adapter(`then` + async 闭包 emit 后 yield item),非同步 `inspect`(inspect 闭包无法 await Tauri emit)。tap 实现成独立 wrapper 层(非焊进 stream 中间层),降低签名逆转连带撕扯。`node-output` payload 含 `node_id` + `data_summary`(按 NodeData variant 给摘要:HttpResponse→status+body 长度,Media→title,Raw/Json→截断 200 字符)。

KTD8. **SSRF 防护(完整模型)**: lj-node-http 统一出口加目标主机校验,覆盖 IPv4(RFC1918/环回/链路本地/169.254.169.254/100.100.100.200 阿里云)+ IPv6(::1/fe80::/10/fc00::/7/::ffff:0:0/96 IPv4-mapped)。**DNS rebinding 防护**:自行 DNS 解析得 IpAddr → SSRF 校验 → 用解析出的 IP 直连(配合 Host header 保 SNI),禁止 reqwest 二次解析。**Redirect 每跳校验**:自定义 redirect::Policy,每跳重跑 SSRF 校验,最大 5 跳。导入预览列所有 HTTP 目标 URL 模板 + JS 源码可读 + 危险调用扫描(fetch/eval/外部 URL 高亮标注)。

KTD9. **JS 资源上限 + 执行模型**: rquickjs `set_memory_limit` + `set_max_stack_size` + `set_interrupt_handler`(watchdog 中断,spike 已验证)。防恶意 @js: 块 DoS。**Runtime 非 Send**(内部含 Rc),JsNodeProcessor 用 `tokio::task::spawn_blocking + channel` 模式:JS 代码在 blocking 线程执行,结果通过 channel 传回 async stream,不跨 await 点 move Runtime。

KTD10. **控制流节点首刀 stub**: Condition/Loop/Merge 首刀 stub(NodeKind variant 保留,processor 返 NotImplemented)。目标源线性链不用控制流。完整 impl 推迟到有真实消费者。

KTD11. **升级算法首刀 stub**: 返回 ImportOutcome::New(全量替换 + 提示)。import_hash 双 ID schema 到位,完整差异合并推迟。

KTD12. **技术栈**: scraper 0.27.0(HTML/CSS)、xmloxide 0.4.3(XML/XPath)、jsonpath-rust 1.0.4(JSON/JSONPath)、regex 1.12.4(正则)。URL 模板 str::replace 零依赖。

KTD13. **代码质量与注释规范**: 代码不能过长堆积屎山——每文件控制在合理行数(建议 Rust ≤400 行,Svelte ≤300 行),超出则拆分模块。注释完善:每个 pub fn/struct/enum/trait 必须有 `///` doc comment(中文,解释意图/约束/非显然行为)。模块级 `//!` doc 说明模块职责。复杂逻辑(分支/循环/解析/安全路径)必须有行内注释解释 why。遵循 AGENTS.md 已有规范。clippy 启用 `missing_docs`(pub 项),首刀不启用 `missing_docs_in_private_items`(骨架阶段摩擦过高,实现稳定后再收紧)。文件行数限制用自定义 pnpm script(`lint:filelen`)守卫。

KTD14. **CSP 收紧 + 远程内容安全渲染**: 引入远程源站内容后收紧 CSP:`default-src 'self'; img-src 'self' data: https:; script-src 'self'`(禁止 inline)。正文默认纯文本渲染(textContent),需 HTML 时用 DOMPurify 白名单。评估 `withGlobalTauri: true` 是否必要(可改显式 import @tauri-apps/api)。

KTD15. **HttpResponse body 大小上限**: HttpNodeProcessor 设 body 最大 16 MiB,reqwest 用 `.bytes_stream()` + 累计计数,超限即截断并返 `CoreError::BodyTooLarge`。防恶意源站 OOM。

KTD16. **Cookie 存储保护**: cookie 表不纳入 webdav 同步范围(同步白名单排除 cookies 表),或用 OS 密钥链加密 cookie 值。防 session token 经 webdav 外泄。

## High-Level Technical Design

```
┌─────────────────────────────────────────────────────────┐
│                    Frontend (Svelte)                     │
│  Import Preview │ Execution Witness │ Rule List         │
│       │                │                                 │
│       ▼                ▼                                 │
│  import_rule_with   execute_segment                      │
│  preview/confirm    /list_rules                          │
│       │                │                                 │
├───────┴────────────────┴─────────────────────────────────┤
│                  src-tauri (IPC 胶水)                     │
│       │                │                                 │
│       ▼                ▼                                 │
│  lj-importer      lj-runtime                             │
│  (翻译+验证)      (GraphExecutor)                        │
│       │                │                                 │
│       ▼                ├──→ lj-node-http (HTTP+SSRF)     │
│  lj-compiler      │    ├──→ lj-node-js (rquickjs+gating) │
│  (语法→IR)        │    ├──→ lj-node-extract (多选择器)    │
│       │           │    └──→ [stub: Condition/Loop/Merge] │
│       ▼           │                                      │
│  lj-core          │    tap → node-output event           │
│  (类型+trait+     │    tracing → span tree               │
│   NodeData+IR+    │                                      │
│   GraphSchema)    │                                      │
│                   │                                      │
│  lj-sandbox       │    lj-storage                        │
│  (Capabilities)   │    (SQLite Repository)               │
└───────────────────────────────────────────────────────────┘
```

执行流(stream-to-stream 管道):

```
search 段:
  Http(search) ──BoxStream<HttpResponse>──→ Extract ──BoxStream<Media>──→ emit rule-output

detail_toc 段:
  Http(detail) ──BoxStream<HttpResponse>──→ Extract ──BoxStream<Media>──→ emit rule-output
  Http(toc)    ──BoxStream<HttpResponse>──→ Extract ──BoxStream<Media>──→ emit rule-output

content 段:
  Http(content) ──BoxStream<HttpResponse>──→ Extract ──BoxStream<Media>──→ emit rule-output

每节点 output stream 经 tap wrapper → emit node-output event
```

## Output Structure

```
src-tauri/
├── Cargo.toml                    # [workspace] members + [workspace.dependencies]
├── crates/
│   ├── lj-core/                  # 纯类型+trait
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── media.rs          # Media/Book/Video/Audio
│   │       ├── node.rs           # Node/NodeKind/NodeSpec/Edge/Graph/SubroutineId
│   │       ├── node_data.rs      # NodeData/HttpResponse
│   │       ├── extract_rule.rs   # ExtractRule IR
│   │       ├── endpoint.rs       # EndpointKind/EndpointSpec/HttpSpec
│   │       ├── sandbox.rs        # Sandbox/Capabilities/Capability
│   │       ├── graph_schema.rs   # GraphSchema/EndpointTemplate/ConditionBranch
│   │       ├── traits.rs         # Importer/NodeProcessor/Executor/Repository/CapabilityLoader
│   │       └── error.rs          # CoreError
│   ├── lj-compiler/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── legado_parser.rs  # @text/@href/||/##regex## → ExtractRule
│   │       └── error.rs
│   ├── lj-runtime/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── executor.rs       # GraphExecutor stream pipeline
│   │       ├── tap.rs            # TapStream wrapper (node-output event)
│   │       ├── tracing.rs        # span tree + trace_id
│   │       └── error.rs
│   ├── lj-node-http/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── processor.rs      # HttpNodeProcessor
│   │       ├── ssrf.rs           # 目标主机校验
│   │       └── error.rs
│   ├── lj-node-js/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── processor.rs      # JsNodeProcessor
│   │       ├── host_api.rs       # Legado 宿主 API bridge
│   │       └── error.rs
│   ├── lj-node-extract/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── processor.rs      # ExtractNodeProcessor
│   │       ├── html.rs           # scraper (CSS)
│   │       ├── xml.rs            # xmloxide (XPath)
│   │       ├── json.rs           # jsonpath-rust (JSONPath)
│   │       ├── regex_extract.rs  # regex
│   │       └── error.rs
│   ├── lj-sandbox/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── capabilities.rs   # Capabilities + default + merge
│   │       └── error.rs
│   ├── lj-importer/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── legado.rs         # LegadoImporter
│   │       ├── native.rs         # NativeImporter
│   │       ├── validate.rs       # GraphSchema::validate
│   │       ├── upgrade.rs        # stub: ImportOutcome::New
│   │       └── error.rs
│   └── lj-storage/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── repository.rs     # Repository<T> impl
│           ├── schema.rs         # SQLite schema
│           └── error.rs
├── src/
│   └── lib.rs                    # Tauri commands (import/execute/list)
└── tests/
    └── integration/              # 独立 crate (不进 workspace)
        ├── Cargo.toml
        ├── .gitignore            # 排除 recordings/ 等产物
        └── tests/
            ├── wiremock_replay.rs
            └── real_source_qa.rs

src/lib/
├── stores/
│   ├── rules.svelte.ts           # 规则列表 store
│   └── execution.svelte.ts       # 执行状态 store
├── components/
│   ├── import-preview.svelte     # 导入预览面板
│   └── execution-witness.svelte  # 执行 witness 页
└── routes/
    └── +page.svelte              # 主页(替换欢迎页)
```

## Implementation Units

### U1. rquickjs 编译 spike + stream-to-stream spike

**Goal:** 验证两个最高风险前置项:rquickjs Windows MSVC 编译 + NodeProcessor stream-to-stream 签名可行性。

**Files:**
- `.tmp/spike/rquickjs/Cargo.toml`(临时 crate,dep rquickjs)
- `.tmp/spike/rquickjs/src/main.rs`(调 `Runtime::new()` 验证编译)
- `.tmp/spike/stream/Cargo.toml`(临时 crate,dep futures/async-stream)
- `.tmp/spike/stream/src/main.rs`(mock Http+Extract 2-node stream-to-stream 管道)

**Approach:**
- rquickjs spike:最小 crate 只 dep rquickjs,调 `Runtime::new()`,跑 `cargo build` 验证 Windows MSVC 编译。**额外验证**:Runtime 是否 Send(跨 await 点);`set_memory_limit`/`set_max_stack_size` API 是否存在;`set_interrupt_handler` 跨线程触发(Arc<AtomicBool> + tokio task)。spike 应在 workspace lints 环境下验证(临时加入 workspace members 或复制 lints 配置)。失败则触发 ADR-0002 复议。
- stream spike:mock 2 节点(Http 产 3 个 mock HttpResponse → Extract 消费产 3 个 mock Media),用 `async_stream` 宏 + `BoxStream` 验证 stream-to-stream 管道可行。**用真实借用场景验证**(非纯 owned mock):process 签名生命周期绑定 self/ctx/spec/input。评估实现负担。

**Test scenarios:**
- rquickjs: `cargo build` 成功(0 exit code)
- stream: mock 管道产出 3 个 Media,顺序正确

**Verification:** `cargo build` in `.tmp/spike/rquickjs/` + `cargo run` in `.tmp/spike/stream/`

**Depends on:** 无(最先执行)

### U2. Workspace skeleton

**Goal:** 10 crate workspace 骨架 + workspace.dependencies + AGENTS.md 作用域扩充。

**Files:**
- `src-tauri/Cargo.toml`(改:加 members + workspace.dependencies)
- `src-tauri/crates/lj-core/Cargo.toml`(新建)
- `src-tauri/crates/lj-compiler/Cargo.toml`(新建)
- `src-tauri/crates/lj-runtime/Cargo.toml`(新建)
- `src-tauri/crates/lj-node-http/Cargo.toml`(新建)
- `src-tauri/crates/lj-node-js/Cargo.toml`(新建)
- `src-tauri/crates/lj-node-extract/Cargo.toml`(新建)
- `src-tauri/crates/lj-sandbox/Cargo.toml`(新建)
- `src-tauri/crates/lj-importer/Cargo.toml`(新建)
- `src-tauri/crates/lj-storage/Cargo.toml`(新建)
- 每 crate `src/lib.rs`(新建,空 `pub mod xxx;` 骨架)
- `AGENTS.md`(改:作用域补 6 项 core/sandbox/node-http/node-js/node-extract/ui-rs;compiler 已存在)

**Approach:**
- `[workspace]` 加 `members = ["crates/lj-core", ..., "crates/lj-storage"]`
- `[workspace.dependencies]` 加 rusqlite(bundled)/reqwest(cookies)/cookie_store/rquickjs/sha2/async-trait/wiremock/tokio/tracing/tracing-subscriber/async-stream/futures/uuid/scraper 0.27/xmloxide 0.4/jsonpath-rust 1.0/regex 1.12/encoding_rs/thiserror。版本号在 U1 spike 中实际锁定验证后 pin。
- 每 crate `[dependencies]` 按需 `{ workspace = true }` 引用
- 每 crate `[lints] workspace = true` 继承 clippy
- 每 crate `src/lib.rs` 含 `pub mod` 声明 + `thiserror` 错误类型骨架

**Test scenarios:**
- `cargo build --manifest-path src-tauri/Cargo.toml --workspace` 成功
- `cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets -- -D warnings` 0 warning

**Verification:** `pnpm lint:rs`

**Depends on:** U1(spike 确认 rquickjs 可编译后才加进 workspace.dependencies)

### U3. lj-core 类型冻结

**Goal:** lj-core 全类型 + NodeData + ExtractRule + GraphSchema + ConditionBranch 定义。

**Files:**
- `src-tauri/crates/lj-core/src/media.rs`
- `src-tauri/crates/lj-core/src/node.rs`
- `src-tauri/crates/lj-core/src/node_data.rs`
- `src-tauri/crates/lj-core/src/extract_rule.rs`
- `src-tauri/crates/lj-core/src/endpoint.rs`
- `src-tauri/crates/lj-core/src/sandbox.rs`
- `src-tauri/crates/lj-core/src/graph_schema.rs`
- `src-tauri/crates/lj-core/src/lib.rs`
- `src-tauri/crates/lj-core/tests/types_test.rs`

**Approach:**
- Media enum(Book/Video/Audio)+ Book struct(title/author/cover_url/description/chapters)
- NodeData enum(Raw/HttpResponse/Media/Json),HttpResponse 自有 struct(status/headers/body: Vec<u8>/charset: Option<String>)
- NodeKind enum 6 variant,NodeSpec 鉴别 union,Node 含双 ID(node_id + import_hash)
- Edge 含 condition_branch: Option<ConditionBranch>,Graph 含 subroutines: HashMap<SubroutineId, Graph>
- ExtractRule enum(CssSelector/XPath/JsonPath/Regex + 提取类型 + 正则清理)
- EndpointKind 5 variant,HttpSpec 含 endpoint_kind + method/url/headers/body/charset/expected_type
- GraphSchema/EndpointTemplate 纯数据结构
- Sandbox/Capabilities 全 bool

**Test scenarios:**
- Node/Edge/Graph serde 序列化/反序列化 round-trip
- Graph 含 subroutines 的序列化(递归类型验证)
- NodeData variant match 穷尽性
- ExtractRule variant 覆盖

**Verification:** `pnpm test:rs`

**Depends on:** U2

### U4. lj-core trait 边界冻结

**Goal:** lj-core 全 trait 定义 + error 类型。

**Files:**
- `src-tauri/crates/lj-core/src/traits.rs`
- `src-tauri/crates/lj-core/src/error.rs`
- `src-tauri/crates/lj-core/tests/traits_test.rs`

**Approach:**
- Importer<Opts>: `fn import(&self, opts: Opts) -> Result<ImportPreview, ImportError>`
- NodeProcessor: `fn kind(&self) -> NodeKind` + `fn input_type(&self) -> NodeDataVariant` + `fn output_type(&self) -> NodeDataVariant` + `fn process(&self, ctx: &ExecutionContext, spec: &NodeSpec, input: BoxStream<NodeData>) -> BoxStream<NodeData>`
- Executor: `fn execute(&self, graph: &Graph, segment: SegmentSpec) -> BoxStream<NodeData>`
- Repository<T>: `fn get/save/delete` + RepoId<T> newtype
- CapabilityLoader: `fn load_capabilities(&self, source: &SourceId) -> Capabilities`
- ExecutionContext: 含 cookie_jar + caps + tracing span
- CoreError thiserror,不 leak anyhow

**Test scenarios:**
- trait object 安全(`Box<dyn NodeProcessor>` 可构造)
- NodeProcessor::process 签名编译通过(sync 返回 BoxStream)
- RepoId<T> 类型隔断(不同 T 的 RepoId 编译期不兼容)

**Verification:** `pnpm test:rs` + `pnpm lint:rs`

**Depends on:** U3

### U5. lj-compiler 规则语法解析器

**Goal:** Legado 规则字符串→ExtractRule IR 解析器。

**Files:**
- `src-tauri/crates/lj-compiler/src/legado_parser.rs`
- `src-tauri/crates/lj-compiler/src/error.rs`
- `src-tauri/crates/lj-compiler/tests/parser_test.rs`

**Approach:**
- 解析 `@text`/`@href`/`@src`/`@html`/`@ownText` 后缀→提取类型
- 解析 `||` 多选回退→Vec<ExtractRule>
- 解析 `##regex##replacement` 正则清理→Regex variant + replacement
- 解析 `@css:`/`@xpath:`/`@json:`/`@regex:` 前缀→选择器类型
- 纯函数,无 IO

**Test scenarios:**
- `h2[itemprop='name']@text` → CssSelector + Text
- `a[itemprop='url']@href` → CssSelector + Attr("href")
- `.title@text||.fallback@text` → Vec[CssSelector+Text, CssSelector+Text]
- `.price@text##\d+\.\d+` → CssSelector + Text + RegexClean
- 无效语法 → CompilerError

**Verification:** `pnpm test:rs`

**Depends on:** U3(ExtractRule 类型) + U17(目标源语法审计,gate 编译器覆盖范围)

### U6. lj-sandbox

**Goal:** Capabilities + default + merge。

**Files:**
- `src-tauri/crates/lj-sandbox/src/capabilities.rs`
- `src-tauri/crates/lj-sandbox/tests/capabilities_test.rs`

**Approach:**
- Capabilities struct(network/fs/env/process 全 bool)
- default_capabilities(): network=true, fs/env/process=false
- merge(global, source): 全 bool 合并(首刀无 partial override)

**Test scenarios:**
- default 返 network=true,其余 false
- merge(global, source) 正确合并

**Verification:** `pnpm test:rs`

**Depends on:** U3

### U7. lj-storage

**Goal:** SQLite Repository impl。

**Files:**
- `src-tauri/crates/lj-storage/src/repository.rs`
- `src-tauri/crates/lj-storage/src/schema.rs`
- `src-tauri/crates/lj-storage/tests/repository_test.rs`

**Approach:**
- rusqlite 连接管理
- schema: rules 表(id/source_url/graph_json/import_hash)、media 表、cookie 表
- Repository<Graph/Media/Cookie> impl Repository trait
- RepoId<T> newtype 类型隔断

**Test scenarios:**
- save → get round-trip
- delete 后 get 返 None
- RepoId<Graph> ≠ RepoId<Media>(编译期)

**Verification:** `pnpm test:rs`

**Depends on:** U4(Repository trait)

### U8. lj-node-http

**Goal:** HttpNodeProcessor + SSRF 校验 + cookie jar。

**Files:**
- `src-tauri/crates/lj-node-http/src/processor.rs`
- `src-tauri/crates/lj-node-http/src/ssrf.rs`
- `src-tauri/crates/lj-node-http/tests/processor_test.rs`

**Approach:**
- impl NodeProcessor,kind() 返 Http
- reqwest 请求 + cookie jar 同源共享
- reqwest::Response → HttpResponse 转换(status/headers/body: Vec<u8>/charset)
- SSRF 校验:阻断 RFC1918/环回/链路本地/169.254.169.254
- stream-to-stream:input(空或 Loop 驱动)→ async_stream 产 HttpResponse stream

**Test scenarios:**
- 正常 URL 请求返 HttpResponse
- 127.0.0.1 被 SSRF 校验阻断
- 169.254.169.254 被阻断
- cookie jar 同源共享

**Verification:** `pnpm test:rs`

**Depends on:** U4(NodeProcessor trait)+ U3(HttpSpec/HttpResponse)

### U9. lj-node-extract

**Goal:** ExtractNodeProcessor + 多数据类型多选择器。

**Files:**
- `src-tauri/crates/lj-node-extract/src/processor.rs`
- `src-tauri/crates/lj-node-extract/src/html.rs`
- `src-tauri/crates/lj-node-extract/src/xml.rs`
- `src-tauri/crates/lj-node-extract/src/json.rs`
- `src-tauri/crates/lj-node-extract/src/regex_extract.rs`
- `src-tauri/crates/lj-node-extract/tests/processor_test.rs`

**Approach:**
- impl NodeProcessor,kind() 返 Extract
- 消费 HttpResponse/Json,按 ExtractRule IR 执行提取,产 Media
- HTML:scraper(cssparser 选择器 + element.text()/attr())
- XML:xmloxide(XPath 1.0+ 查询)
- JSON:jsonpath-rust(RFC 9535 JSONPath)
- 正则:regex crate
- charset 解码:encoding_rs 按 HttpResponse.charset 字段解码 Vec<u8>→String
- URL 模板:str::replace `{{key}}`/`{{page}}`

**Test scenarios:**
- HTML + CSS 选择器提取文本/属性
- XML + XPath 提取
- JSON + JSONPath 提取
- 正则清理
- charset 解码(GBK/UTF-8)
- `||` 多选回退(第一个非空结果)

**Verification:** `pnpm test:rs`

**Depends on:** U4(NodeProcessor trait)+ U3(ExtractRule/NodeData)+ U5(编译器产 IR)

### U10. lj-node-js

**Goal:** JsNodeProcessor + rquickjs + sandbox gating + Legado 宿主 API bridge。

**Files:**
- `src-tauri/crates/lj-node-js/src/processor.rs`
- `src-tauri/crates/lj-node-js/src/host_api.rs`
- `src-tauri/crates/lj-node-js/tests/processor_test.rs`

**Approach:**
- impl NodeProcessor,kind() 返 Js
- rquickjs Runtime + Context
- 资源上限:set_memory_limit + set_max_stack_size + watchdog 超时
- sandbox gating:caps.fs=false 时 fs_read 返 CapabilityBlocked
- Legado 宿主 API bridge:枚举 目标源用到的子集(java.net.URLEncoder.encode/result.put/get/baseUrl)
- stream-to-stream:input → JS 执行 → 产 NodeData stream

**Test scenarios:**
- 简单 JS 表达式执行(`1+1` → 2)
- caps.fs=false 时 fs_read 返 CapabilityBlocked
- 内存上限触发时中断
- Legado URLEncoder.encode bridge
- exploreUrl @js: 块产 URL 列表

**Verification:** `pnpm test:rs`

**Depends on:** U4(NodeProcessor trait)+ U6(Capabilities)+ U1(rquickjs spike 确认编译)+ U17(目标源宿主 API 枚举,gate JS bridge 范围)

### U11. lj-importer

**Goal:** LegadoImporter + NativeImporter + GraphSchema 验证 + 升级 stub。

**Files:**
- `src-tauri/crates/lj-importer/src/legado.rs`
- `src-tauri/crates/lj-importer/src/native.rs`
- `src-tauri/crates/lj-importer/src/validate.rs`
- `src-tauri/crates/lj-importer/src/upgrade.rs`
- `src-tauri/crates/lj-importer/tests/legado_test.rs`
- `src-tauri/crates/lj-importer/tests/native_test.rs`
- `src-tauri/crates/lj-importer/fixtures/legado_source.json`(目标源 fixture)

**Approach:**
- LegadoImporter:调 lj-compiler 解析规则字符串→ExtractRule,翻译 5 端点为子图(Http→Extract),生成 Graph + 端点间 Edge + 双 ID 补全
- NativeImporter:节点图 JSON 直通反序列化
- validate:GraphSchema::validate 检查边类型匹配 + 子图结构合法
- upgrade:stub 返 ImportOutcome::New
- 目标源语法审计:对 fixture 做语法面审计,产出覆盖子集

**Test scenarios:**
- 目标源 JSON → 5 端点子图 + 双 ID
- 本体 JSON 直通反序列化
- GraphSchema::validate 通过/失败
- 升级 stub 返 ImportOutcome::New
- 目标源语法审计:覆盖子集清单

**Verification:** `pnpm test:rs`

**Depends on:** U5(编译器)+ U3(Graph/GraphSchema)+ U7(storage)

### U12. lj-runtime

**Goal:** GraphExecutor stream pipeline + tap 可观测 + tracing + 控制流 stub。

**Files:**
- `src-tauri/crates/lj-runtime/src/executor.rs`
- `src-tauri/crates/lj-runtime/src/tap.rs`
- `src-tauri/crates/lj-runtime/src/tracing.rs`
- `src-tauri/crates/lj-runtime/tests/executor_test.rs`

**Approach:**
- GraphExecutor:按图拓扑序取节点,按 NodeKind 路由到 NodeProcessor,process(input)→output,喂给下游
- 子图裁剪:按 SegmentSpec.endpoint_kind 选 entry 节点 + 保留子图内部边
- tap:TapStream wrapper(独立层,非焊进 stream),inspect 每个 NodeData item emit node-output event
- tracing:每节点一层 span,trace_id 贯穿,中文 message + 英文 snake_case field key
- Condition/Loop/Merge:stub processor 返 NotImplemented

**Test scenarios:**
- 2-node 线性图(Http→Extract)stream pipeline 跑通
- tap emit node-output event(每 item)
- tracing span tree 含 trace_id
- 子图裁剪:只跑 search 段不跑 detail
- Condition stub 返 NotImplemented

**Verification:** `pnpm test:rs`

**Depends on:** U4(Executor trait)+ U8/U9/U10(三个 NodeProcessor impl)

### U13. Tauri IPC

**Goal:** import_rule_with_preview + confirm_import + list_rules + execute_segment。

**Files:**
- `src-tauri/src/lib.rs`(改:加命令)
- `src-tauri/capabilities/default.json`(改:加权限)

**Approach:**
- import_rule_with_preview:检测格式(本体/第三方)→ Importer.import() → 返 ImportPreview(source URL + 节点数 + JS 块数 + sandbox 声明 + HTTP 目标 URL 列表 + JS 源码)
- confirm_import:落库到 RulesRepository
- list_rules:返规则列表
- execute_segment:按 SegmentSpec 调 GraphExecutor,流式 emit rule-output + node-output event
- capabilities 加 http 权限

**Test scenarios:**
- import Legado JSON → preview 含节点数/JS 块数/HTTP 目标 URL
- confirm → 落库成功
- list_rules → 返列表
- execute_segment search → 流式 emit rule-output

**Verification:** `pnpm test:rs` + manual IPC 测试

**Depends on:** U11(importer)+ U12(runtime)+ U7(storage)

### U14. Frontend import 预览面板

**Goal:** 粘贴规则 JSON → 解析预览 → 确认导入 → 落库成功。

**Files:**
- `src/lib/components/import-preview.svelte`
- `src/lib/stores/rules.svelte.ts`
- `src/routes/+page.svelte`(改:加 import 入口)

**Approach:**
- Textarea 粘贴 JSON
- 调 import_rule_with_preview IPC
- 渲染预览:source URL / 节点数 / 边数 / JS 块数 / sandbox caps / HTTP 目标 URL 列表 / JS 源码可读
- loading/error/empty 状态(解析中/JSON 无效/验证失败/落库失败/成功)
- confirm 按钮调 confirm_import
- store 遵循 theme.svelte.ts 模式(.svelte.ts + 模块级 $state + 函数导出)

**Test scenarios:**
- 粘贴有效 JSON → 预览渲染
- 粘贴无效 JSON → error 状态
- confirm → 落库成功 toast

**Verification:** `pnpm check` + manual UI 测试

**Depends on:** U13(IPC)

### U15. Frontend 执行 witness 页

**Goal:** 按段执行(search → 选书 → detail+toc → 选章 → content),流式渲染。

**Files:**
- `src/lib/components/execution-witness.svelte`
- `src/lib/stores/execution.svelte.ts`
- `src/routes/+page.svelte`(改:加执行入口)

**Approach:**
- 规则列表(从 rules store)+ 搜索关键词输入 + **发现/分类浏览入口**(无关键词,与 search 并列)
- 段 1:search 路径(调 execute_segment search → listen rule-output → 流式渲染 BookMedia cards)或 discover 路径(调 execute_segment discover → Js 执行 exploreUrl → 流式渲染)
- 用户选 1 本(card click 即确认,返回按钮可回退)→ 段 2:调 execute_segment detail_toc → 流式渲染章节列表
- 用户选 1 章 → 段 3:调 execute_segment content → 流式渲染正文(默认纯文本 textContent,KTD14)
- listen node-output → 节点流转可观测(TreeView 只读 + 当前节点高亮 + data_summary 行内显示)
- loading/empty(0 结果,文案 + 换关键词入口)/error(按类型分类:SSRF 阻断/网络/解码,各给文案 + 重试/换规则)状态
- 段间过渡:选书/选章后自动推进下一段,execution.svelte.ts 持久化已选 book/chapter
- 选书后停止 listen rule-output(丢弃后续 card)
- **中途取消**:首刀不支持 cancel IPC,用户可关闭页面/换规则绕行(显式声明推迟)

**Test scenarios:**
- search → 3+ BookMedia cards 流式渲染
- discover → Js 执行 exploreUrl → BookMedia cards 流式渲染
- 选书 → 章节列表渲染
- 选章 → 正文渲染(纯文本)
- 0 结果 empty state(文案 + 换关键词入口)
- error state(SSRF 阻断/网络不通分类展示)
- node-output event 实时展示(TreeView 高亮 + data_summary)

**Verification:** `pnpm check` + manual UI 测试

**Depends on:** U13(IPC)+ U14(规则列表 store)

### U16. 集成测试 crate

**Goal:** wiremock 回放层 + 真源站 QA,独立 crate 不进 workspace。

**Files:**
- `src-tauri/tests/integration/Cargo.toml`(独立,不进 workspace members)
- `src-tauri/tests/integration/.gitignore`(排除 recordings/截图/tracing 日志)
- `src-tauri/tests/integration/tests/wiremock_replay.rs`
- `src-tauri/tests/integration/tests/real_source_qa.rs`
- `src-tauri/tests/integration/recordings/`(gitignore,录制响应 fixture)
- `package.json`(改:加 `test:rs:integration` script)

**Approach:**
- wiremock 回放:录制 目标源站一次成功响应作 fixture,wiremock mock 5 端点(含 discover),跑完整链路
- 真源站 QA:连 目标源站 跑完整链路(search + discover → 选书 → detail → toc → 选章 → content),产物存 recordings/(gitignore)
- package.json 加 `test:rs:integration: "cargo test --manifest-path src-tauri/tests/integration/Cargo.toml"`

**Test scenarios:**
- wiremock 回放:5 端点链路在 fixture 上跑通
- 真源站 QA:import → search → 选书 → detail → toc → 选章 → content

**Verification:** `pnpm test:rs:integration`

**Depends on:** U12(runtime)+ U13(IPC)+ U11(importer)

### U17. 目标源语法审计 + 宿主 API 枚举

**Goal:** 对 目标源实际规则做语法面审计 + 枚举宿主 API。

**Files:**
- `.tmp/star-source-audit.md`(审计报告,不进版本控制)

**Approach:**
- 获取 目标源规则 JSON(从 Legado 社区或源站)
- 审计选择器语法:列出 目标源用到的所有选择器语法,对照 R11/R16 声明覆盖子集与未覆盖子集
- 枚举宿主 API:列出 目标源 @js: 块用到的所有 Legado 宿主 API
- 产出覆盖清单,反馈到 U5(编译器)/U9(提取器)/U10(JS bridge)

**Test scenarios:**
- 审计报告含:选择器语法清单 + 覆盖/未覆盖子集 + 宿主 API 清单

**Verification:** 审计报告完整性检查

**Depends on:** U2(workspace 骨架,能编译 lj-compiler)

## Risks & Dependencies

| 风险 | 严重度 | 缓解 |
|---|---|---|
| rquickjs Windows MSVC 编译失败 | 高 | U1 spike 前置验证(含 Send/API/interrupt),失败触发 ADR-0002 复议。**回滚策略**:U1 spike 通过后再开 U2;U2 骨架合入作为独立可回退 commit;rquickjs 后续失败则 fallback(改用 boa / 移除 Js 节点首刀范围) |
| stream-to-stream 签名在真实节点上过重 | 中 | U1 spike 评估(含借用场景),tap 独立 wrapper 层降低逆转连带。**回滚策略**:改 per-item dispatch,重写 NodeProcessor trait + 3 个 impl |
| 目标源选择器语法未覆盖 | 中 | U17 审计前置(gate U5/U9),反馈到编译器/提取器 |
| 目标源 @js: 宿主 API 未覆盖 | 中 | U17 枚举前置(gate U10),反馈到 JS bridge |
| 目标源站 QA 日不通 | 中 | 双轨验收,wiremock 回放 fallback |
| Graph.subroutines 递归序列化 | 低 | U3 测试验证 serde 递归 derive。GraphSchema::validate 加无环校验 |
| 集成测试 crate 不进 workspace 导致 CI 漏跑 | 低 | package.json 加 test:rs:integration script。集成 crate 自带 clippy pedantic deny(本地 [lints]) |
| CSP null + 远程内容 XSS | 高 | KTD14 CSP 收紧 + 正文默认纯文本 + DOMPurify |
| Cookie 明文 + webdav 外泄 | 高 | KTD16 cookie 表排除 webdav 同步 |

## System-Wide Impact

- **Cargo workspace**:从单 crate 变 10 member,全 workspace clippy/test 覆盖
- **Tauri capabilities**:加 http 权限(网络请求)
- **前端路由**:+page.svelte 从欢迎页变 import+执行入口
- **AGENTS.md**:提交作用域从 7 项增到 13 项(补 core/sandbox/node-http/node-js/node-extract/ui-rs;compiler 已存在)
- **package.json**:加 test:rs:integration script
- **CONTEXT.md**:glossary 已更新(10 crate/NodeData/ExtractRule/端点子图模板/控制流节点/子例程/Merge/ConditionBranch/执行分段/流转可观测)
- **ADR**:新增 0022-0026(已写)

## Test Strategy

- **单元测试**:每 domain crate 内 tests/ 目录,`pnpm test:rs` 覆盖
- **集成测试**:独立 crate(wiremock 回放 + 真源站 QA),`pnpm test:rs:integration` 覆盖
- **前端测试**:vitest + @testing-library/svelte(需补 test:web script)
- **lint**:clippy all+pedantic deny + eslint + svelte-check
- **TDD 信号**:requirements doc 强调测试先行,每 unit 先写 test scenarios 再实现
- **手动 QA**:真源站端到端(用户价值门控项)
- **代码质量守卫**:每 PR 检查文件行数(Rust ≤400 行,Svelte ≤300 行,超出需拆分理由)+ pub 项 doc comment 覆盖率(clippy `missing_docs_in_private_items` + `missing_docs` pedantic lint)+ 复杂逻辑行内注释抽查

## Sources & Research

- Origin: `docs/brainstorms/2026-06-26-legado-rule-engine-requirements.md`
- ADR-0001~0026(26 个 ADR,其中 0022-0026 为 grill 产出)
- CONTEXT.md glossary
- librarian 确认技术栈:scraper 0.27.0 / xmloxide 0.4.3 / jsonpath-rust 1.0.4 / regex 1.12.4
- explorer 代码库核对:Cargo.toml 现状 / lib.rs 现状 / 前端结构 / capabilities
- ce-doc-review 7 persona 审查 findings(已 Apply/Defer)
