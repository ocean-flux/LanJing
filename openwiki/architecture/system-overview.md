---
type: "参考"
title: "System overview"
openwiki_generated: true
sources:
  - id: openwiki-source-6ae244f79c5e27a2b1f08014
    resource: repo://components.json
  - id: openwiki-source-39c3295efc089133e87a9c80
    resource: repo://CONTEXT.md
  - id: openwiki-source-19477f037e206dc41a7b54b8
    resource: repo://docs/adr/0001-base-ui-and-base-lyra.md
  - id: openwiki-source-9faf227efd814ea6139f8535
    resource: repo://docs/adr/0002-versioned-source-and-rule-lifecycle.md
  - id: openwiki-source-415c14fc16b9bb2d14d082c5
    resource: repo://docs/adr/0004-app-surfaces-and-workbench-layers.md
  - id: openwiki-source-f70faf819fb2edbb8e236f45
    resource: repo://docs/adr/0004-rule-first-open-extension-architecture.md
  - id: openwiki-source-05b14d59724ec4fa57d1c0cc
    resource: repo://docs/adr/0005-turn-engine-and-pagesource.md
  - id: openwiki-source-ca67060e890937010b96de80
    resource: repo://src-tauri/Cargo.toml
  - id: openwiki-source-f11b1c19bf21ece4e75e90c0
    resource: repo://src-tauri/crates/lj-importer/fixtures/legado_synthetic_source.json
  - id: openwiki-source-2e5dc1f7878d17b2b880b035
    resource: repo://src-tauri/crates/lj-importer/src/lib.rs
  - id: openwiki-source-d50d2b8ac95f0636ecbbb93c
    resource: repo://src-tauri/crates/lj-importer/src/maccms/mod.rs
  - id: openwiki-source-06153a1cffee823fcf0f4b22
    resource: repo://src-tauri/crates/lj-importer/tests/legado_test.rs
  - id: openwiki-source-a7f973b529835a5d2bbe2a82
    resource: repo://src-tauri/crates/lj-rule-model/src/hash.rs
  - id: openwiki-source-18e2cd19f7a3c23ecb7a2d80
    resource: repo://src-tauri/crates/lj-rule-model/src/lib.rs
  - id: openwiki-source-f77c1b2f9f427db1e6d94a57
    resource: repo://src-tauri/crates/lj-rule-system/src/lib.rs
  - id: openwiki-source-aa1c93c7ddf499b5db0c1905
    resource: repo://src-tauri/crates/lj-rule-system/src/system.rs
  - id: openwiki-source-f4d8e498f8a27d9242bfdc4b
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/document.rs
  - id: openwiki-source-970f295896afa9156178188f
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/execution.rs
  - id: openwiki-source-80e8762cff162b2ecd16e129
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/prepare_install.rs
  - id: openwiki-source-985022f215d1a8fb48d686c6
    resource: repo://src-tauri/crates/lj-rule-system/src/system/query_adapter.rs
  - id: openwiki-source-ab202b94798a4c907cf938e5
    resource: repo://src-tauri/crates/lj-rule-system/src/system/session_delivery.rs
  - id: openwiki-source-66845dd8b3f72696149d5d09
    resource: repo://src-tauri/crates/lj-rule-system/src/types/document.rs
  - id: openwiki-source-8ece8d8ea6055cf2f800dcb4
    resource: repo://src-tauri/crates/lj-runtime/src/effect_registry.rs
  - id: openwiki-source-424726f325963a46b4c3301e
    resource: repo://src-tauri/crates/lj-runtime/src/effect/archive.rs
  - id: openwiki-source-60db92730bd8b30a2a14ff4f
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler.rs
  - id: openwiki-source-34d489227f93cd0425da2fa7
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/live_capture.rs
  - id: openwiki-source-c33d51acf739e25ca33c8b1d
    resource: repo://src-tauri/crates/lj-storage-migration/src/lib.rs
  - id: openwiki-source-b3eeee8972a1c1a0ed428993
    resource: repo://src/features/apps/AppsHome.tsx
generated: { by: "pi", at: "2026-10-04T13:54:24.186Z" }
---


## 两个层, 不是一层

产品是双层结构(ADR 0004 已采纳):

- **工作台层**: 境场、应用入口、来源与规则、资料库、设置。使用 base-lyra 母版, 目标是克制与效率。
- **应用面层**: `/apps` 下的沉浸式体验面, 进入后隐藏工作台 chrome, 不受直角/`text-xs`/32px 控件约束, 唯一共享约束是色值只来自四个 appearance pack。

工作台层已经实现; 应用面层目前只有 `AppsHome` 的占位列表(五个 surface 全部 `enabled: false`, 无子路由), TurnEngine / PageSource / `lanjing://` 资产网关都还只有 ADR。详细对照见「应用面、翻页引擎与资产网关蓝图」。

## 仓库拓扑

```text
src/                 React 工作台外壳(features/{realm,apps,sources,rules,library,settings})
src-tauri/           Tauri 壳 + Rust workspace(14 个 crate)
  src/               main.rs / lib.rs / commands/ / deeplink/
  crates/lj-*        领域 crate, 见下表
messages/            Paraglide 翻译源(en / zh-CN)
docs/adr/            7 份已采纳的架构决定
```

Tauri 根 package 只依赖 `lj-rule-system` 与 Tauri 插件, 不直接依赖 storage/importer/runtime(`src-tauri/Cargo.toml#L97-L113`; `src-tauri/crates/lj-rule-system/src/lib.rs#L1-L5`)。这是「门面之外不越层」的编译期体现。

## crate 依赖方向

| crate | 依赖 | 说明 |
| --- | --- | --- |
| `lj-capability` | 无内部依赖 | 标准意图契约, 最底层 |
| `lj-rule-model` | `lj-capability` | 定义/计划/策略 DTO, 刻意不引入 ORM/Tokio/Tauri |
| `lj-media` | `lj-capability` | 标准媒体模型 |
| `lj-compiler` | `lj-rule-model` | 校验与降低 |
| `lj-runtime` | `lj-rule-model`, `lj-capability`, `lj-media` | 执行引擎, **不依赖 storage** |
| `lj-node-http` / `lj-node-js` / `lj-node-extract` | `lj-runtime` (+ `lj-rule-model`) | effect 适配层, 位于 runtime 之上 |
| `lj-importer` | `lj-rule-model`, `lj-capability` | 来源格式解析与翻译 |
| `lj-storage-entity` | 无内部依赖 | 关系模型 |
| `lj-storage-migration` | `lj-compiler`, `lj-rule-model`, `lj-storage-entity` | 迁移与 schema 自检 |
| `lj-storage` | `lj-rule-model`, `lj-runtime`, `lj-media`, `lj-storage-entity`, `lj-storage-migration` | 持久化 |
| `lj-rule-system` | 上面几乎全部 | 唯一 composition facade |

两条被刻意维持的「不依赖」值得单独指出:

1. `lj-rule-model` 不依赖 compiler/runtime/storage, 因此规则合同可以被任何一层引用而不引入运行时假设。
2. `lj-runtime` 不依赖任何存储 crate: 它只要求一个 `EffectArchive` trait(`src-tauri/crates/lj-runtime/src/effect/archive.rs#L336-L352`), 由门面层在调用时注入。这把「执行需要持久化」从编译期依赖降级为运行期参数, 使 runtime 可以在测试里用内存 archive 跑。

## 所有权分层

ADR 0004(规则优先的来源扩展架构, 已采纳并取代 ADR 0003)保留了 Kernel / Runtime 的所有权边界: 后续可以新增规则节点、选择器、来源输入适配器、受控 host capability 与 editor field, 但「必须保持 Rule Contract 和 Kernel/Runtime 所有权边界」(`docs/adr/0004-rule-first-open-extension-architecture.md#L129-L131`)。今天实际存在的所有权是三层:

```text
Effect Registry        内置 effect handler 的注册与冻结(键是 Rule Contract 的 EffectKind)
      ↓
Rule Kernel            Definition、Draft/Effective Revision、credential ownership、校验不变量、编译编排
      ↓
Rule Runtime           immutable Plan 的执行、调度、取消、execution event、durable capture、replay
```

原来的 Plugin Contract / Plugin Host 两层已随 ADR 0003 一起废弃: 通用 plugin system 属一期明确不建设, `lj-plugin-contract` 与 `PluginHost` / `FrozenRegistry` 已删除, 内置能力改用同一 Rule Contract 注册(见「规则模型与合同」与「规则运行时」页)。

| 层 | 状态与证据 |
| --- | --- |
| Effect Registry | 按 `EffectKind` 原子批量注册、`freeze()` 冻结、未注册 kind 稳定失败(`src-tauri/crates/lj-runtime/src/effect_registry.rs#L61-L107`) |
| Rule Kernel | Definition/校验/编译/凭证 owner 与分域 revision 全在跑(`lj-rule-model` + `lj-compiler` + `lj-rule-system`) |
| Rule Runtime | 调度、取消、capture、replay 全部在跑, 且不依赖 storage(`lj-runtime`) |

`RuleSystem` 自身「只作为 application composition facade, 不拥有上述领域语义」。这条声明在代码里体现为: 它不缓存 Definition/Plan, 不自己写 SQLite, 只把请求翻译成 storage/compiler/runtime 的调用。

## 三条跨 crate 主链路

### 一、来源安装与更新

```text
粘贴文本 / 本地文件 / legado|yuedu 深链
  → src/features/sources/** 与 src/shared/tauri/{sources,catalog,deep-link-parse}.ts
  → command: fetch_import_src / prepare_install / install
  → RuleSystem::prepare_install
      → lj-importer         Legado/Maccms 解析与翻译成 RuleDefinition
      → lj-compiler         canonicalize + validate + compile
      → lj-runtime          validate_plan(编译通过 ≠ 运行时可接受)
      → lj-storage          candidate staging(不透明、有 TTL)
  → 前端展示候选与所需能力 → 用户确认 grant
  → command: install
  → lj-storage             追加 source_version, 写 package/Plan artifact 与 secret
```

关键点: 「准备候选」与「安装」是两次独立调用, 中间隔着用户对能力的确认; 候选带 TTL 且会因目标来源更新而 stale。详见「来源安装、更新与回退」。

### 二、规则保存

```text
规则编辑器(前端语义图)
  → src/shared/tauri/rules/wire.ts
  → command: save_native_rule_document
  → RuleSystem::save_native_rule_document
      → lj-compiler::validate        诊断决定 Draft 还是 Effective
      → definition_hash              canonical BLAKE3
      → lj-storage                  语义域 + 布局域各自带 expected_revision 的一次事务
```

保存是分域的: 语义域与布局域独立乐观并发, 冲突只影响该域并产生 `RevisionConflict`; 校验失败不替换 Effective Rule Revision。详见「Native Rule Document 生命周期」。

### 三、规则执行

```text
command: execute
  → RuleSystem::execute
      → lj-storage   持久化 execution 起点(先落库再起执行)
  → lj-runtime::PlanRuntime::execute
      → validate_plan + execution_path(intent 子图)
      → scheduler 逐节点 → 控制流/Loop
      → effect seam → lj-node-http / lj-node-js / lj-node-extract
      → 同一 storage 作为 EffectArchive: durable capture → receipt 校验
  → session_delivery
      → Delta 先 commit_event+projection, 再投递
      → terminal 收敛, 取消只阻后续 effect
  → 前端经 command: catch_up_execution 与 query 系列读取投影
```

这条链路的可恢复性由两个「先持久后推进」保证: effect 走 durable-before-advance, Delta 走先提交后投递。详见「执行、捕获与重放」。

## ADR 与实现状态对照

| ADR | 主题 | 状态 | 落地证据 |
| --- | --- | --- | --- |
| 0001 | UI primitive 选 Base UI, 母版选 base-lyra | 已实现 | `components.json` 的 `style: base-lyra`; 业务代码用 `render={<Link/>}` |
| 0002 | 来源与规则采用版本化审阅式生命周期 | 大部分已实现 | candidate/revision/rollback、Draft/Effective、凭证 owner 均在代码中(见 `lj-rule-system` 与 `lj-storage`) |
| 0004(应用面分层) | 双层架构(工作台 + 应用面) | 已决定未实现 | `AppsHome` 五个 surface `enabled: false`, 无 `/apps/<surface>` 路由 |
| 0004(规则优先扩展) | 规则优先的来源扩展架构(取代已删除的 ADR 0003) | 已采纳, 规则路径已实现 | 编辑器/候选安装/编译/immutable Plan/capture-replay 均在代码中; 通用 plugin system 按第 7 节一期不建设, 原 plugin/effect contract 层已删除(`docs/adr/0004-rule-first-open-extension-architecture.md#L104-L114`) |
| 0005 | TurnEngine 与 PageSource | 已决定未实现 | 无 `src/features/turn/**`, 全仓检索不到 `PageSource`/`TurnEngine`/`TurnIntent` |
| 0006 | 仿真卷页空闲帧预渲染纹理缓存 | 已决定未实现 | 依赖 0005, 无 `CurlTransition` 与 `TextPageSource::snapshotFor()` |
| 0007 | `lanjing://` 资产网关 | 已决定未实现 | Rust 未注册自定义协议, CSP 无 `lanjing:`, 无网关 crate; 但 SSRF 防护已存在于 `lj-node-http/src/ssrf.rs` |

`docs/adr/` 现在有两个编号 `0004` 的文件: `0004-app-surfaces-and-workbench-layers.md`(双层架构)与 `0004-rule-first-open-extension-architecture.md`(规则优先扩展)。引用时看文件名而不是编号。

`docs/reference/reader-architecture.md` 是比 ADR 更早的独立设计文档(Rust 归一化 Book AST + Web Worker 分页 + Mode Dispatcher), 与 ADR 0005 的术语与模式清单尚未收敛; 当前前端没有任何对应实现。

## 领域词汇的单一来源

产品概念(Source Revision、Effective Rule Revision、Recovery Draft、Library Entry、Turn Mode 等)的规范定义在根 `CONTEXT.md`, 采用「术语 + 定义 + `_Avoid_`」的写法。它与代码的关系是: 词汇表定义用户可识别的概念, 类型系统与 DTO 定义这些概念的机器表示, 两者同名但职责不同; 当某个术语在代码里没有对应类型时(例如 `Rule Revision` 只在 storage/DTO 侧), 说明该概念还没有被提升为领域合同。

## 当前边界

- 前端只有工作台层; 应用面、翻页引擎、资产网关均未实现。
- Rust 侧不再有 plugin 机制: 通用 plugin system(动态 Rust ABI、通用 PluginHost、plugin catalog、跨插件 service、registration lease、第三方 executable plugin SDK、在线 marketplace)按 ADR 0004 第 7 节属**一期明确不建设**, 原有的 plugin/effect contract 层(`lj-plugin-contract` 与 `PluginHost` / `FrozenRegistry`)已删除(`docs/adr/0004-rule-first-open-extension-architecture.md#L104-L114`)。运行时只注册内置的三种 effect handler, 未注册 kind 由 dispatch 转成稳定失败。
- Maccms 来源在 importer crate 内没有单元测试(`lj-importer/tests/` 只有 `legado_test.rs` 与 Legado fixture), 它只被跨 crate 集成测试覆盖(`lj-integration-tests/tests/maccms_json_rule_system.rs` 的 8 个用例, 其中 `maccms_json_four_intents_live_and_replay_use_only_rule_system` 走完整门面); 也就是说 importer 的 Maccms 翻译逻辑本身缺细粒度回归, 但线路层不缺口。
