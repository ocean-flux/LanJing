---
type: "参考"
title: "Testing and gates"
openwiki_generated: true
sources:
  - id: openwiki-source-6d4b4e707b8d60b6ccfa3425
    resource: repo://.github/workflows/openwiki-update.yml
  - id: openwiki-source-bbf3d58bf1c2c539fc59e66c
    resource: repo://.oxlintrc.json
  - id: openwiki-source-0bce0323701d8f46f6287199
    resource: repo://commitlint.config.js
  - id: openwiki-source-f317ee207e1653d2033c81a4
    resource: repo://CONTRIBUTING.md
  - id: openwiki-source-72b816a72bb5f72d95b334ea
    resource: repo://lefthook.yml
  - id: openwiki-source-5b54a58d1b51cd490b0e7162
    resource: repo://package.json
  - id: openwiki-source-cb1f9cbde660f6cd0ce1fd99
    resource: repo://src-tauri/.clippy.toml
  - id: openwiki-source-ca67060e890937010b96de80
    resource: repo://src-tauri/Cargo.toml
  - id: openwiki-source-012692312089ec8143764f4d
    resource: repo://src-tauri/crates/lj-compiler/tests/plan_compiler_test.rs
  - id: openwiki-source-b64b6ab843d2592264a06eb9
    resource: repo://src-tauri/crates/lj-integration-tests/Cargo.toml
  - id: openwiki-source-4866e15f73219bace3482e99
    resource: repo://src-tauri/crates/lj-integration-tests/tests/legado_rule_system.rs
  - id: openwiki-source-980fd459aadb6afbaa667438
    resource: repo://src-tauri/crates/lj-integration-tests/tests/maccms_json_rule_system.rs
  - id: openwiki-source-9c10ac9b7d289d7fe8e6de00
    resource: repo://src-tauri/crates/lj-node-extract/tests/processor_test.rs
  - id: openwiki-source-c9a97bdea04d62635b90bf91
    resource: repo://src-tauri/crates/lj-node-http/tests/processor_test.rs
  - id: openwiki-source-4faec7944a92fbaedeeffea2
    resource: repo://src-tauri/crates/lj-node-js/tests/processor_test.rs
  - id: openwiki-source-5ded4b6721374179194a354c
    resource: repo://src-tauri/crates/lj-rule-model/tests/model_contract_test.rs
  - id: openwiki-source-32d4001fffac613566845710
    resource: repo://src-tauri/crates/lj-rule-system/src/system/error_mapping.rs
  - id: openwiki-source-f4d8e498f8a27d9242bfdc4b
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/document.rs
  - id: openwiki-source-00fc7c09c51d967590463257
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/tests.rs
  - id: openwiki-source-985022f215d1a8fb48d686c6
    resource: repo://src-tauri/crates/lj-rule-system/src/system/query_adapter.rs
  - id: openwiki-source-d52076293fecd21d5b53a29e
    resource: repo://src-tauri/crates/lj-runtime/tests/effect_registry_test.rs
  - id: openwiki-source-a93b7dfdb8c469a921b0d6ba
    resource: repo://src-tauri/crates/lj-runtime/tests/plan_runtime_test.rs
  - id: openwiki-source-115e591341fdf411553c3c50
    resource: repo://src-tauri/crates/lj-runtime/tests/plan_runtime_test/scheduling_contract.rs
  - id: openwiki-source-3a3fdfb5a00399ef7859b900
    resource: repo://src-tauri/crates/lj-storage/src/keyring_init.rs
  - id: openwiki-source-91742d0fdf22103ae9601de4
    resource: repo://src-tauri/crates/lj-storage/tests/event_projection_storage_test.rs
  - id: openwiki-source-f7e1959d32ecb943411c3a10
    resource: repo://src-tauri/crates/lj-storage/tests/event_projection_storage_test/archive_contract.rs
  - id: openwiki-source-2d345f6b900b5259dfd538b3
    resource: repo://src-tauri/crates/lj-storage/tests/native_rule_document_test.rs
  - id: openwiki-source-451254818f9278942b21492c
    resource: repo://src-tauri/rustfmt.toml
  - id: openwiki-source-68e2dde2c84dfbdac9ee9fd2
    resource: repo://src-tauri/src/commands/execution.rs
  - id: openwiki-source-8fb4609cef6e3bffc73c48ee
    resource: repo://src-tauri/src/lib.rs
  - id: openwiki-source-3f29fd103f5b0c1546441d4a
    resource: repo://src/shared/i18n/messages.ts
  - id: openwiki-source-b3a7e56ded1ac0e23164f7e0
    resource: repo://src/shared/tauri/rules/wire.test.ts
  - id: openwiki-source-b416070b252080078d4165de
    resource: repo://src/shared/tauri/sources.test.ts
  - id: openwiki-source-991a3aaa436bd565cf973c8d
    resource: repo://src/test/setup.ts
  - id: openwiki-source-581dc5746c844c4ee0b781c7
    resource: repo://vite.config.js
generated: { by: "pi", at: "2026-10-04T13:54:24.186Z" }
---


## 门禁的组成

前端用一个命令收口(`package.json#L12-L21`):

```text
pnpm check = lint:web(oxlint src vite.config.js --ignore-path .gitignore --deny-warnings)
           && typecheck:web(tsc --noEmit)
           && format:check:web(oxfmt ... --check)
           && test:web(vitest run)
```

`--deny-warnings` 让 warning 也阻断; `vitest run` 是一次性执行而非 watch, 因此可以在钩子和 CI 里无副作用地跑。

Rust 侧不用 wrapper, 直接用 Cargo(`CONTRIBUTING.md#L31-L37`):

```text
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --workspace
```

`-D warnings` 不是唯一的 clippy 严格度来源: workspace 级 lint 已经是 `clippy.all = "deny"` 加 `clippy.pedantic = "deny"`(`src-tauri/Cargo.toml#L31-L33`), 各 crate 通过 `[lints] workspace = true` 继承, 所以新增 crate 自动落进同一门禁。两个配套配置:

- `src-tauri/.clippy.toml#L1` 把 `too-many-lines-threshold` 设为 150, 超过这个行数的函数会被 clippy 拦下;
- `src-tauri/rustfmt.toml#L1-L4` 固定 edition 2024、`max_width = 100`、字段初始化与 `try` 简写。

Oxlint 的规则面本身也是门禁的一部分(`.oxlintrc.json#L3-L25`): 除 eslint/oxc/typescript/unicorn/import/react/jsx-a11y/vitest 外挂了 `jsPlugins: ["oxlint-tailwindcss"]` 并把 correctness/suspicious/pedantic/perf/style 全部设为 error, 另有 `max-dependencies: 20` 与 `max-params: 6`(`#L143-L154`)。Tailwind 规则(未知类名、重复类、冲突类、规范写法、排序)是 error/warn 级(`#L88-L108`), 这就是业务代码不能写裸 `z-50` / `rounded-md` 的强制手段。

生成物豁免是**逐规则**而不是整文件: `.oxlintrc.json#L176-L202` 对 `src/components/ui/**`、`src/hooks/use-mobile.ts`、`src/shared/utils.ts` 关掉的只是纯风格规则(`eqeqeq`、`unicorn/max-nested-calls`、`react/hook-use-state`、`jsx-a11y/click-events-have-key-events` 等), `#L203-L211` 只对 `vite.config.js` 放行 node 内置模块, `#L212-L224` 只给测试文件开 vitest env 并允许 warning 注释。`CONTRIBUTING.md#L74` 把这条边界写清楚了: 生成物仍受 lint 与格式化管辖, 目的是保留可被 `shadcn add` 升级的能力。

## lefthook 的三段钩子

`lefthook.yml` 把本地门禁拆成三种触发时机:

| 钩子 | 内容 | 说明 |
| --- | --- | --- |
| pre-commit | oxlint --fix + oxfmt --write(并行组内串行, 只跑暂存文件) / oxfmt 只管仓库根配置 / `cargo fmt --all` | 三个 job 并行, 每个都 `stage_fixed: true` |
| commit-msg | `commitlint --edit {1}` | 规则继承 `@commitlint/config-conventional`(`commitlint.config.js#L1-L3`) |
| pre-push | web: `pnpm check && pnpm test:web` ∥ rust: clippy -D warnings + `cargo test --workspace` | 唯一的全量门禁 |

两处细节值得留意:

- pre-commit 里 oxlint 与 oxfmt 被刻意拆成 piped 的两步(`lefthook.yml#L11-L22`), 注释说明了原因: Oxlint 只认代码文件, oxfmt 还要管 css/html/json, 共用一个 glob 时「只暂存 css 的提交」会让 oxlint 拿到空输入并以 1 退出。
- `cargo fmt` 必须带 `--all`(`#L37-L41`), 否则只格式化根包, 子 crate 与测试会被漏掉。
- pre-push 的 web job 里 `pnpm check` 已经包含 `pnpm test:web`, 再显式 `&& pnpm test:web` 会让前端测试在推送时跑两遍——是重复成本, 不是正确性问题。

`stage_fixed: true` 意味着钩子自动修好的格式会直接回到暂存区, 提交内容与工作区不会分叉; 代价是钩子会改写你正要提交的文件, 所以 pre-commit 之后不该假设 `git diff` 还是原来那份。

## CI 现状

`.github/workflows/` 里只有一个 workflow, 且与质量无关: `openwiki-update.yml` 按 cron(`0 8 * * *`)或手动触发, 用 `fetch-depth: 0` 让 `openwiki code --update` 能把 HEAD 与上次记录的提交做 diff, 然后以 `contents: write` / `pull-requests: write` 权限提交 wiki 更新(`.github/workflows/openwiki-update.yml#L3-L67`)。

**没有 lint / typecheck / test / clippy job。** 后果是: pre-push 是唯一一次真正执行全量门禁的时刻, 而它由本地钩子实现, 需要开发者执行过 `lefthook install` 且不写 `--no-verify`; 服务端无法复验一条「门禁通过」的声明, 也无法阻止绕过。仓库里那份 `pnpm check` 与 `cargo test --workspace` 因此是**约定**, 不是强制。这是当前质量体系里最大的结构性风险, 也解释了为什么 `pnpm test:web` 的失败只会在本地被发现。

## Rust 测试的六个层次

按「离 IO 多远」排列, 每层断言的是不同种类的契约:

| 层 | 位置 | 规模 | 证明什么 | 不能证明什么 |
| --- | --- | --- | --- | --- |
| 纯契约 | `lj-rule-model/tests/`、`lj-compiler/tests/plan_compiler_test.rs` | 3 + 1 个文件 | hash 归一化、端口/句柄类型矩阵、诊断路径与排序稳定性 | 任何 IO、持久化、并发行为 |
| adapter | `lj-node-http/tests/processor_test.rs`、`lj-node-js/tests/`、`lj-node-extract/tests/`(含 `html_xpath_test.rs`) | 3 个 crate | 单个 effect handler 在 wiremock 前的行为、提取器与 QuickJS 约束 | 调度顺序、捕获持久化、取消语义 |
| runtime | `lj-runtime/tests/plan_runtime_test/{scheduling_contract,control_contract,replay_contract}.rs` + `effect_registry_test.rs` | 6 + 8 + 10 + 5 | 调度/控制流/replay 的不变量, effect 注册的原子性与冻结 | 真实 SQLite 事务下的 durable-before-advance |

契约层最近收缩过一次: `lj-plugin-contract` 与其 8 条 identity/manifest 解析用例随 crate 删除; runtime 侧的 `plugin_host_test.rs`(7 条)换成 `effect_registry_test.rs`(5 条, `src-tauri/crates/lj-runtime/tests/effect_registry_test.rs#L73-L174`)。少掉的两条断言的是「重复 plugin identity」与「operation 不在 manifest 里」——这两个命题随「注册键改为 Rule Contract 的 `EffectKind`」一并消失, 不是被放宽。
| storage | `lj-storage/tests/event_projection_storage_test/{archive,credential_writer,media_query,projection_retention,replay}_contract.rs` + `native_rule_document_test.rs` | 12 + 5 + 1 + 7 + 4 + 13 | 真实 SQLite/临时目录/mock keyring 下的追加顺序、乐观并发、vault、投影顺序与保留策略 | runtime 是否发出了正确的事件序列 |
| facade | `lj-rule-system/src/system/lifecycle/document.rs#L1352-L2134`、`error_mapping.rs#L419-L436`、`query_adapter.rs#L447-L477`、`lifecycle/tests.rs#L142` | 19 + 2 + 4 + 1 | 凭据策略、provenance 脱敏、revision 冲突、draft 保留、分页边界、错误码稳定映射 | 跨 crate 的完整生命周期 |
| 跨 crate 集成 | `lj-integration-tests/tests/legado_rule_system.rs`、`maccms_json_rule_system.rs` | 6 + 8 | 唯一覆盖完整链路的层(导入 → candidate → install → live 执行 → 捕获 → replay → 投影 → library 查询) | 真实网络与真实 DNS; 真实浏览器/UI |

集成层的定位写在文件头(`lj-integration-tests/tests/maccms_json_rule_system.rs#L1-L5`): 「只构造真实 SQLite/artifact、wiremock 与 concrete façade; **不组装内部执行编排、handler registry 或 storage transaction**」。这条自限让这层测试验的是公开契约而不是实现细节, 用例名即断言对象, 例如:

- `candidate_boundary_is_opaque_and_rejects_tampering_expiry_and_insufficient_grant`(`#L483`)——篡改 profile、必填 grant、过期、definition_hash、诊断或 artifact 算法都会被拒;
- `candidate_install_revalidates_event_metadata_after_restart`(`#L572`)——重启后安装要重新校验事件元数据, 证明「信任只来自持久化收据」;
- `source_revision_history_and_rollback_require_a_new_reviewed_candidate`(`#L606`)——回滚必须走一个新的待审 candidate, 不能直接改指针;
- `maccms_json_four_intents_live_and_replay_use_only_rule_system`(`#L696`)——四个 intent 在 live 与 replay 两条路径上产出同样的媒体图;
- `stream_drop_does_not_cancel_and_cancel_and_catch_up_are_idempotent_and_contiguous`(`#L786`)——丢弃投递流不取消执行、取消幂等、catch-up 只接受连续序列;
- `library_projection_orders_pinned_favorite_recent_and_id`(`#L995`)与其后的 `safe_query_facade_lists_sources_projects_library_and_catches_up`(`#L1047`)。

runtime 层的 archive fixture 并不是 no-op: 它每次 capture 都写入并 `sync_all` 临时文件, 目的正是验证「只在真实确认收据之后推进下游节点」(`lj-runtime/tests/plan_runtime_test.rs#L1-L5`)。它与集成层的区别在于 fixture 不是真 SQLite 事务, 所以「事务原子性」仍属 storage 层的责任。

Legado 侧同构(`legado_rule_system.rs#L1-L5` 同样的自限声明), 用 `lj_importer::legado::LegadoImporter` 与本地 wiremock 驱动六个标准 intent 的 live/replay 黄金合同。

## 前端测试的真实边界

前端有 14 个测试文件, 配置是 jsdom 环境加 `src/test/setup.ts`(`vite.config.js#L39-L43`, setup 只有一行 `import '@testing-library/jest-dom/vitest'`)。`package.json#L62-L78` 里 RTL / user-event / jest-dom / jsdom 全都装了。

但**没有任何一个测试 import `@testing-library/react`, 也没有任何 `render(` 调用**。也就是说这 14 个文件全部在测纯逻辑:

- `src/features/rules/model/` 下 7 个文件(端口矩阵、连接门、撤销重做、ELK 布局、导入适配)是最大的一块;
- `src/features/{library,sources}/workflow.test.ts` 与 `src/shared/navigation.test.ts` 测状态机与路由判定;
- `src/shared/tauri/*.test.ts` 测 wire 层。

`jsdom` 与 jest-dom 目前是**已装未用**的配置。按 `AGENTS.md` 的测试边界(UI 单测只在用户可观察合同无法由逻辑或公共集成边界证明时才用)这是有意为之, 但它意味着: 组件的可访问名称、focus-visible、路由渲染、Toast 提示、空态与错误态**完全没有测试**。前端改动只能靠 `lint` / `typecheck` / 生产构建与人工 Tauri 冒烟来判定。

## wire 测试证明的是名字与形状

`src/shared/tauri/rules/wire.test.ts` 与 `sources.test.ts` 的做法是 `vi.mock('@tauri-apps/api/core')` 把 `invoke`/`isTauri` 换成 spy, 然后断言 command 名与包络形状, 例如 `prepareSourceInstall({kind:'maccms_json', url})` 必须落到 `invoke('prepare_install', { request: { kind: 'maccms_json', url } })`, `'{"bookSourceName":...}'` 必须被包成 `{kind:'legado', source_json}`(`src/shared/tauri/sources.test.ts#L12-L40`)。

这类测试能抓的是**前后端契约的漂移**: 命令改名、请求包络从 `{request}` 变成平铺参数、字段从 snake_case 变成 camelCase。它证明不了任何 Rust 侧行为——`invoke` 是 mock, Rust 从未被调用。真正的端到端保证在 `lj-integration-tests` 里, 而它不经过 Tauri 命令层。

`vi.mock('@tauri-apps/api/core')` 在 4 个文件里出现(`library.test.ts`、`sources.test.ts`、`rules/wire.test.ts`、`features/rules/model/session.test.ts`, 见各文件 `vi.mock` 行), 这也是前端测试里唯一被 mock 的模块边界。

## 不可证明的清单

把上面各层的空白合起来, 当前**没有任何自动化测试**覆盖:

- **Tauri 命令层的行为**: 25 个命令没有 Rust 测试, 唯一的覆盖是 `lib.rs` 里对命令**名单**的断言(数量 25、无重复、无旧 `source_document` 名)。`execute` 的取消注册表登记、`forward_execution_events` 的失败继续语义、`catch_up_execution` 的终态清理都只靠代码审查。
- **真实网络与 DNS/SSRF**: 所有 HTTP 断言走 wiremock; `HttpEffectAdapter::new_test()` 还显式把 `ssrf_enabled` 设为 false 以便打回环。真实 pinning 行为、TLS、真实响应体积都未验证。
- **真实 keyring**: 测试用 `keyring_core::mock` 预置 store; 平台安全存储不可用/被锁时的降级路径(`KeyringUnavailable`/`KeyringLocked`)只有类型存在, 没有测试。
- **任何 UI 渲染**。
- **迁移的历史路径**: `lj-storage-migration` 的基线测试断言 33 张表与 down/up 重建, 但没有跨版本升级的端到端 fixture(见存储页)。
- **i18n 的 key parity**: `CONTRIBUTING.md#L60` 要求两个 locale 的 key 集合保持一致, 但仓库里没有对应脚本或测试, Paraglide 的构建步骤是否会让缺 key 失败**未验证**; `src/shared/i18n/messages.ts` 只是对生成物的再导出。

## 未强制的要求

- 没有 CI 复验, 所以「门禁通过」不可被服务端证明(见上文)。
- `pnpm check` 与 pre-push 的 web job 重复跑一次前端测试。
- 集成测试自限于 concrete façade, 因此内部编排(handler registry、storage transaction 的组装)只有各自层的单测保护, 没有一层同时验内部与外部。
