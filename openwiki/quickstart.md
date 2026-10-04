---
type: "参考"
title: "Quickstart"
openwiki_generated: true
sources:
  - id: openwiki-source-6d4b4e707b8d60b6ccfa3425
    resource: repo://.github/workflows/openwiki-update.yml
  - id: openwiki-source-8037e2358a2c4f9b2c722a11
    resource: repo://AGENTS.md
  - id: openwiki-source-0bce0323701d8f46f6287199
    resource: repo://commitlint.config.js
  - id: openwiki-source-39c3295efc089133e87a9c80
    resource: repo://CONTEXT.md
  - id: openwiki-source-f317ee207e1653d2033c81a4
    resource: repo://CONTRIBUTING.md
  - id: openwiki-source-415c14fc16b9bb2d14d082c5
    resource: repo://docs/adr/0004-app-surfaces-and-workbench-layers.md
  - id: openwiki-source-e706cdf6ed71c3ed5f88e79f
    resource: repo://docs/agents/domain.md
  - id: openwiki-source-13dd5d4bbac0d2d6ce41730f
    resource: repo://docs/agents/issue-tracker.md
  - id: openwiki-source-72b816a72bb5f72d95b334ea
    resource: repo://lefthook.yml
  - id: openwiki-source-5b54a58d1b51cd490b0e7162
    resource: repo://package.json
  - id: openwiki-source-23775c3de52f3ab95a13cb8b
    resource: repo://README.md
  - id: openwiki-source-8fb4609cef6e3bffc73c48ee
    resource: repo://src-tauri/src/lib.rs
  - id: openwiki-source-54631e6ebf1d3b815c4a5eed
    resource: repo://src/App.tsx
  - id: openwiki-source-b3eeee8972a1c1a0ed428993
    resource: repo://src/features/apps/AppsHome.tsx
  - id: openwiki-source-a0309a4f66f33e6814b5c22e
    resource: repo://src/features/rules/EditorToolbar.tsx
  - id: openwiki-source-ee130a462d446f70c12f0143
    resource: repo://src/features/rules/model/core.ts
  - id: openwiki-source-bfa866dbb93b4c8108ffdf7f
    resource: repo://src/shared/tauri/rules/wire.ts
generated: { by: "pi", at: "2026-10-04T13:54:24.186Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T13:54:24.186Z
---


## 这是什么

LanJing / 览境 是**本地优先的跨媒体发现与阅读工作台**: 没有云端后端、登录或同步假设, 用户的规则、历史和资料库都留在本机(`README.md#L17-L19`)。

架构是**两层**(ADR 0004 已采纳, `docs/adr/0004-app-surfaces-and-workbench-layers.md#L20-L23`):

| 层 | 组成 | 交互语言 |
| --- | --- | --- |
| 工作台层 | 境场、来源、规则、资料库、设置 | base-lyra 母版, 克制、效率优先 |
| 应用面层 | `/apps` 下的沉浸式体验面 | 另一套交互语言, 进入后隐藏工作台 chrome |

两层的唯一共享约束是色值只能来自 `src/index.css` 的四个 appearance pack(`docs/adr/0004-app-surfaces-and-workbench-layers.md#L25-L26`); 工作台层的「克制」**不延伸到应用面**。

端到端链路一句话: 媒体源规则 → 标准媒体模型 → React 模板 → 阅读 / 播放 / 收藏 / 管理(`README.md#L21-L23`)。规则侧只输出标准媒体模型与 `PresentationHint`, **不定义 UI 布局或来源专属页结构**(`AGENTS.md#L30`)。

## 当前实现状态

工作台六个工作区都已接上路由(`src/App.tsx#L94-L106`):

- 入口 `index` → RealmHome, `apps` → AppsHome, `sources` → SourcesHome, `sources/rules[/:documentId]` → RuleWorkspace(懒加载 `@xyflow/react` / CodeMirror / elkjs), `library` 与 `library/item/:resourceId` → LibraryHome, `settings` → SettingsHome, 其余 → NotFound。

已经打通的链路:

- **来源安装 / 更新 / 回退**(`/sources`): Adaptive Source Input、Install Candidate 的 TTL 与 stale 判定、授权确认、追加式 Source Revision 与由历史 revision 构造新候选的软回退。见 [来源安装、更新与回退](workflows/source-install-and-update.md)。
- **Native Rule Document 生命周期**(`/sources/rules`): 草稿 / 生效分域乐观并发保存、校验失败保留草稿、Effective Revision 历史与恢复。见 [Native Rule Document 生命周期](workflows/rule-document-lifecycle.md)。
- **规则编辑器**: 节点配置、连线、删除、粘贴、撤销重做都在本地 reducer 里, 只有「保存 / 校验 / provenance / 新建导入」写回后端(`src/features/rules/model/core.ts#L258-L278`)。见 [规则编辑器前端工作区](workflows/rule-editor-workspace.md)。
- **Rust 侧完整内核**: 编译器 → 不可变 Plan → 运行时逐节点推进 → live 捕获 / replay 逐字段比对 → 事件账本与投影。见 [整体架构与分层所有权](architecture/system-overview.md)。
- **资料库读侧**: 列表 + Sheet Inspector, 收藏 / 置顶不变量由存储层写入事务强制。见 [资料库状态与媒体查询](workflows/library-and-media-query.md)。

明确还没做或只做了一半的(不要按 ADR 的字面当成已实现):

- 五个应用面全是 `enabled: false` 的禁用列表, 没有 `src/features/turn/`(`src/features/apps/AppsHome.tsx#L15-L20`)。见 [应用面、翻页引擎与资产网关蓝图](reference/app-surfaces-and-asset-gateway.md)。
- 资料库**不能真的打开媒体**: 没有阅读器、没有跳转应用面的路由, 也没有任何 UI 写进度。
- `execute` / `cancel_execution` / `catch_up_execution` 与 `get_media_item` / `list_media_units` / `list_media_assets` **没有前端 wrapper**, 所以界面上还没有任何路径能启动、取消或续接一次执行(`src-tauri/src/lib.rs#L14-L60` 是 25 个命令的唯一声明处, 前端 `src/shared/tauri/rules/wire.ts#L301-L364` 只覆盖规则文档那 10 个)。
- 图片渲染与 `lanjing://` 资产网关未实现, deep link 只有 `legado` / `yuedu` / `lanjing` 三个 scheme 的解析。
- 保存冲突是正常返回值而不是抛错, 工具栏会在写入被拒时也提示「已保存」(`src/features/rules/EditorToolbar.tsx#L33-L42`); Recovery Draft 只活在内存里。

两处**文档与实现不符**, 读到时以代码为准:

- `README.md#L25` 说规则编辑器「目前是语义图的只读投影加布局保存, 完整编辑能力仍在迁移中」, 但实现已支持配置编辑、连线、删除、粘贴与撤销重做(`src/features/rules/model/core.ts#L258-L278`)。
- `README.md#L57` 把 `pnpm test:web` 描述为 "React Testing Library + Vitest", 但 `src/` 下没有任何文件 import `@testing-library/react`, 该命令实际只证明纯逻辑。见 [测试分层与质量门禁](quality/testing-and-gates.md)。

## 常用命令

环境要求: Rust stable、Node.js 20+、pnpm 12(通过 `corepack enable` 启用)、以及目标平台的 Tauri 工具链(`CONTRIBUTING.md#L7-L16`)。

前端与壳(`package.json#L7-L22`):

```bash
corepack enable
pnpm install
pnpm dev              # Vite 开发服务器, 端口 1420
pnpm tauri dev        # 启动 Tauri 应用
pnpm check            # lint:web + typecheck:web + format:check:web + test:web
pnpm build            # React 生产构建, 输出 build/(Tauri 的 frontendDist)
pnpm tauri build      # 打包 Tauri 应用
```

Rust workspace 直接走 Cargo(`CONTRIBUTING.md#L31-L37`):

```bash
cargo check  --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --all-features -- -D warnings
cargo test   --manifest-path src-tauri/Cargo.toml --workspace
```

门禁由 lefthook 挂在 git hooks 上(`lefthook.yml#L6-L58`): pre-commit 并行跑 oxlint+oxfmt(串行两步, 避免 css-only 提交让 oxlint 拿到空输入)与 `cargo fmt --all` 并对暂存文件 `stage_fixed`; commit-msg 跑 commitlint, 约定是 Conventional Commits 的 `<type>(<scope>): <subject>`(`commitlint.config.js#L1-L3`、`CONTRIBUTING.md#L76-L84`); pre-push 并行跑 `pnpm check && pnpm test:web` 与 clippy + `cargo test --workspace`。

CI 里**只有一个 OpenWiki 更新 workflow**(定时 + 手动, `openwiki code --update`), 没有 lint / test job(`.github/workflows/openwiki-update.yml#L1-L10`)——因此上面这些门禁只在本地 pre-push 与人工执行时生效。

## 任务路由表

| 想改什么 | 先读 | 关键落点 |
| --- | --- | --- |
| 外壳、导航、主题色、i18n | [React 工作台外壳](architecture/frontend-shell.md) | `src/app/**`、`src/index.css`、`src/shared/theme`、`src/shared/i18n` |
| 规则节点 / 端口 / 画布交互 | [规则编辑器前端工作区](workflows/rule-editor-workspace.md) | `src/features/rules/model/{ports,connection-gate,flow-adapter,core}.ts` |
| 规则定义与 Plan 的数据形状 | [规则模型与契约层](architecture/rule-model-and-contracts.md) | `src-tauri/crates/lj-rule-model`、`lj-media` |
| 校验规则与诊断定位 | [规则编译与校验](architecture/rule-compiler.md) | `src-tauri/crates/lj-compiler` |
| 节点执行、循环、effect 适配与 effect 注册 | [规则运行时与 effect 注册](architecture/rule-runtime.md) | `src-tauri/crates/lj-runtime` |
| 门面 API、生命周期编排、脱敏收敛 | [RuleSystem 门面与生命周期编排](architecture/rule-system-facade.md) | `src-tauri/crates/lj-rule-system` |
| 规则文档保存 / 晋升 / 历史 / 凭据 owner | [Native Rule Document 生命周期](workflows/rule-document-lifecycle.md) | `lj-storage/src/transaction/document.rs`、`types/document.rs` |
| 来源安装 / 更新 / 回退 / importer | [来源安装、更新与回退](workflows/source-install-and-update.md) | `lj-importer`、`candidate_source/**`、`src/features/sources/**` |
| 执行、捕获、重放、取消 | [执行、捕获与重放](workflows/execution-capture-and-replay.md) | `lj-runtime/src/plan_runtime/scheduler/**`、`lj-storage/src/repository/execution/**` |
| 事件账本、迁移、投影、凭证存储 | [存储层与事件账本](architecture/storage-and-event-ledger.md) | `lj-storage/**`、`lj-storage-migration/**` |
| 资料库读侧与媒体查询 | [资料库状态与媒体查询](workflows/library-and-media-query.md) | `lj-media`、`lj-storage/src/storage/query.rs`、`src/features/library/**` |
| Tauri 命令与前端 wire 层 | [Tauri 壳与 IPC 边界](architecture/tauri-ipc-boundary.md) | `src-tauri/src/commands/**`、`src/shared/tauri/**` |
| SSRF / 凭据 / CSP / 资源上限 | [信任边界与安全不变量](security/trust-boundaries.md) | `lj-node-http/src/{ssrf,processor/request,processor/redirect}.rs`、`lj-storage/src/artifact.rs` |
| 测试该放哪一层、门禁是什么 | [测试分层与质量门禁](quality/testing-and-gates.md) | `lefthook.yml`、`lj-*-tests/**`、`src/**/*.test.ts` |
| 应用面 / 翻页引擎 / 资产网关(蓝图) | [应用面、翻页引擎与资产网关蓝图](reference/app-surfaces-and-asset-gateway.md) | ADR 0004-0007、`docs/reference/reader-architecture.md` |
| 分层与 crate 依赖方向 | [整体架构与分层所有权](architecture/system-overview.md) | `src-tauri/Cargo.toml`、`src-tauri/src/lib.rs` |

## 仓库约定

- **事项与规格**记录在 GitHub Issues(`ocean-flux/LanJing`), 统一用 `gh` 操作: `gh issue create` / `view` / `comment` / `edit --add-label` / `close`; effort 用父 issue, 每个事项是它的 sub-issue, 依赖用 GitHub 原生 blocked-by, 不把状态写进正文(`docs/agents/issue-tracker.md#L1-L27`)。
- **领域词表**是根 `CONTEXT.md`, 事项标题、规格、测试名与设计讨论必须用它定义的词, 不使用其中 `_Avoid_` 列出的同义词; 与既有 ADR 冲突时必须显式说明冲突与重新审议的理由(`docs/agents/domain.md#L5-L19`)。
- **硬边界**: 本地-only(不新增登录 / 云同步 / 云端后端假设); 规则只出标准媒体模型 + `PresentationHint`(`AGENTS.md#L30`)。
- **UI 原语**是 Base UI(`@base-ui/react`), 组件从 shadcn `base-lyra` registry 取, 禁止引入 `radix-ui`; 图标只能走 `src/components/Icon.tsx` 的字面量白名单(`AGENTS.md#L7`、`CONTRIBUTING.md#L64-L72`)。
- **生成物不手改**: `src/shared/paraglide/**`; registry 产物 `src/components/ui/**` 允许项目级魔改但仍纳入 lint, 且每次 `shadcn add` 后要把 `@phosphor-icons/react` import 换成 `@/components/ui/icon-glyphs`(`CONTRIBUTING.md#L74`)。
- **文案与注释用中文**, 技术标识符保留英文; 生成物(如 `src/shared/paraglide/**`)不手改; 两个 locale 的 key 集合必须一致(`AGENTS.md#L31`、`CONTRIBUTING.md#L58-L60`)。

## 本 wiki 怎么用

页面按四类组织, 每一页都显式标注 已实现 / 已决定未实现 / 占位, 并对每条事实结论给出 `相对路径#Lx-Ly` 证据:

| 目录 | 回答什么 |
| --- | --- |
| `architecture/` | 分层、模型契约、编译、运行时、门面、存储、前端外壳、IPC 边界(现状) |
| `workflows/` | 来源安装、规则文档生命周期、编辑器、执行与重放、资料库(端到端链路) |
| `reference/` | ADR 0004-0007 与 reader-architecture 的蓝图和落地状态对照 |
| `security/`、`quality/` | 信任边界与不变量、测试分层与门禁 |

不确定从哪读时按「任务路由表」取页; 只看机制与所有权, 不要把这套 wiki 当成新仓库事实的来源——它只是当前 commit 的注脚, 与代码冲突时以代码为准。
