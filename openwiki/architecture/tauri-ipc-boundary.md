---
type: "参考"
title: "Tauri ipc boundary"
openwiki_generated: true
sources:
  - id: openwiki-source-00ff4b2512b6dbfa268cbfa4
    resource: repo://src-tauri/capabilities/default.json
  - id: openwiki-source-ca67060e890937010b96de80
    resource: repo://src-tauri/Cargo.toml
  - id: openwiki-source-164707dfe3596be848b3ec22
    resource: repo://src-tauri/src/commands/delivery.rs
  - id: openwiki-source-a9261f19e98f918d04f73854
    resource: repo://src-tauri/src/commands/document.rs
  - id: openwiki-source-68e2dde2c84dfbdac9ee9fd2
    resource: repo://src-tauri/src/commands/execution.rs
  - id: openwiki-source-52d3b98dec25fcf03e3ee40d
    resource: repo://src-tauri/src/commands/import_install.rs
  - id: openwiki-source-80fc8f51454bf5d5e7fa3f1d
    resource: repo://src-tauri/src/commands/query.rs
  - id: openwiki-source-d15a9d80a96c7517107f4457
    resource: repo://src-tauri/src/commands/state.rs
  - id: openwiki-source-ca3209a9ec95c80c1d596a56
    resource: repo://src-tauri/src/deeplink/mod.rs
  - id: openwiki-source-8fb4609cef6e3bffc73c48ee
    resource: repo://src-tauri/src/lib.rs
  - id: openwiki-source-99b0214e9f2113a0f6a2cf92
    resource: repo://src-tauri/src/main.rs
  - id: openwiki-source-0abfee918aaf0d7e3ea712fc
    resource: repo://src-tauri/tauri.conf.json
  - id: openwiki-source-c7ba30812d81c291cb627ef7
    resource: repo://src-tauri/tauri.macos.conf.json
  - id: openwiki-source-54631e6ebf1d3b815c4a5eed
    resource: repo://src/App.tsx
  - id: openwiki-source-7b4777db24e3917d09c987ae
    resource: repo://src/features/rules/model/descriptor-registry.ts
  - id: openwiki-source-e523845a56feec75d438c920
    resource: repo://src/features/rules/model/session.ts
  - id: openwiki-source-61edd4d2deb95e4b81381576
    resource: repo://src/shared/tauri/execution.ts
  - id: openwiki-source-99a93074334aa4f3f476fdcf
    resource: repo://src/shared/tauri/library.ts
  - id: openwiki-source-bfa866dbb93b4c8108ffdf7f
    resource: repo://src/shared/tauri/rules/wire.ts
  - id: openwiki-source-938e4156b9e76517514c2370
    resource: repo://src/shared/tauri/sources.ts
generated: { by: "pi", at: "2026-10-05T08:37:16.392Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-05T08:37:16.392Z
---


## 壳的组成

`main.rs` 只有两行实质内容: Windows release 用 `windows_subsystem = "windows"` 关掉额外控制台窗口, 然后调用 `lanjing_lib::run()`(`src-tauri/src/main.rs#L1-L6`)。

`run()` 里 Tauri builder 的插件顺序是(`src-tauri/src/lib.rs#L68-L90`):

| 条件 | 插件 | 用途 |
| --- | --- | --- |
| debug + desktop | `tauri_plugin_mcp_bridge` | 本地调试桥, 配置为 `localhost_only()` |
| desktop | `tauri_plugin_single_instance` | 第二实例转交 argv 后聚焦既有窗口 |
| 始终 | `deep_link` / `dialog` / `notification` / `opener` / `os` | 平台能力 |
| 始终 | `tauri_plugin_zustand` | `@tauri-store/zustand` 的后端, 替代官方 plugin-store |
| 始终 | `tauri_plugin_window_state` | 窗口几何恢复 |

`mcp_bridge` 只在 debug desktop 下注册, 因此它不会进入发布构建的能力面; `single_instance` 只在桌面目标编译(依赖声明带 `cfg(any(macos, windows, linux))`, `src-tauri/Cargo.toml#L114-L115`)。

`setup()` 做三件事(`src-tauri/src/lib.rs#L91-L111`):

1. Windows/Linux 上调用 `app.deep_link().register_all()` 注册 scheme(注释未写原因, 实测是这两个平台需要显式注册)。
2. 取 `app_data_dir()`, 建目录, 然后用 `tauri::async_runtime::block_on` 打开 `RuleSystem::open(RuleSystemConfig::desktop(data_dir/lanjing-event-store.db, data_dir/artifacts))`。**这是整个应用唯一的 RuleSystem 实例**, 也在启动阶段同步完成——数据库不可用会直接 panic 而不是让 UI 带着半可用状态起来。
3. `app.manage(AppState)`。

`AppState` 只有两个字段: `Arc<RuleSystem>` 与取消注册表 `Arc<Mutex<HashMap<ExecutionId, ExecutionCancellation>>>`(`src-tauri/src/commands/state.rs#L8-L23`)。注释明确「业务编排全部封装在 RuleSystem 内」, 所以 Tauri 层不持有 Plan、线程池或连接。

## 命令注册机制

命令清单由 `lanjing_commands!` 宏定义一次, 但被两个消费者展开(`src-tauri/src/lib.rs#L14-L60`):

- `generate_lanjing_handler!` 把它变成 `tauri::generate_handler![...]`, 交给 `invoke_handler`;
- 测试用的 `declare_registered_command_names!` 把它变成常量数组 `REGISTERED_COMMAND_NAMES`。

测试 `root_command_registry_contains_only_current_facade_commands` 断言三件事: 名单与期望完全相等、没有重复项、数量恰为 26, 且**没有任何 `source_document` 前缀的旧命令名**(`src-tauri/src/lib.rs#L124-L172`)。期望名单是测试里的 `EXPECTED` 常量, 因此增删命令必须同时改宏与测试(`#L126-L153`)。这把「命令表是唯一的公开 IPC 面」变成了可回归的约束, 顺带防止旧编辑链的命令名复活。

26 个命令分四组:

| 组 | 数量 | 代表命令 | 组文件 |
| --- | --- | --- | --- |
| import/install | 4 | `fetch_import_src`、`prepare_install`、`prepare_source_rollback`、`install` | `src-tauri/src/commands/import_install.rs#L47-L90` |
| execution | 3 | `execute`、`cancel_execution`、`catch_up_execution` | `src-tauri/src/commands/execution.rs#L43-L140` |
| query | 8 | `list_installed_sources`、`get_library_projection`、`update_library_entry`、`get_media_item(s)`、`list_media_units/assets` | `src-tauri/src/commands/query.rs#L12-L110` |
| document | 11 | `create/save/validate/list_native_rule_document`、`list_rule_node_descriptors`、`list_native_rule_revision_history`、`restore_native_rule_revision`、`get/rename/delete_native_rule_document`、`get_native_rule_provenance` | `src-tauri/src/commands/document.rs#L19-L120` |

每个命令都是薄包装: 取 `State<'_, AppState>`, 调 `state.system.<method>`, 返回 `Result<_, RuleError>`。

## execution 的事件推送

`execute` 是唯一有副作用的编排(`src-tauri/src/commands/execution.rs#L43-L74`):

```text
system.execute(request) → session
  → 取 session.id 与取消句柄, 登记到 AppState.cancellations
  → session.into_events()
  → spawn: forward_execution_events(events, id, registry, |payload| app.emit("rule-execution-event", payload))
  → 立即返回 ExecuteResponse { execution_id }
```

调用方拿到的是 `execution_id`, 事件走独立的 Tauri 事件通道 `rule-execution-event`(`src-tauri/src/commands/delivery.rs#L9`)。这个选择让 command 的返回值保持小而稳定, 而事件流可以持续推送。

`forward_execution_events` 的失败语义(`src-tauri/src/commands/delivery.rs#L20-L54`):

- 单个事件投递失败(前端窗口已关闭或事件系统不可用)只记 warn 并继续, **不取消 execution**;
- 观测到终态(`Completed` / `Failed` / `Cancelled`)后从取消注册表移除条目并返回;
- 如果事件流在终态前结束, 记 warn 并**保留**取消注册表条目——注释说明了原因: 此时 execution 在后台可能仍在推进, 提前清理会让 `cancel_execution` 失效。

`cancel_execution` 从注册表取句柄并调用 `cancel()`, 返回 `changed` 表示本次是否首次改变状态(幂等, `src-tauri/src/commands/execution.rs#L76-L96`)。

`catch_up_execution` 是断线重连路径(`#L98-L140`): 调 `system.catch_up_execution(execution_id, after_sequence)` 取回已持久化事件, 逐个 emit, 并在最后一条是终态时清理注册表; 返回 `replayed_count` 与 `delivered_through_sequence` 让前端知道自己追到了哪一位。

## 前端 wire 层

前端只在 `src/shared/tauri/` 里调 `invoke`, 四个文件共 23 个 wrapper:

| 文件 | 数量 | wrapper |
| --- | --- | --- |
| `sources.ts` | 6 | `list_installed_sources`、`list_source_revisions`、`fetch_import_src`、`prepare_install`、`install`、`prepare_source_rollback`(`src/shared/tauri/sources.ts#L86-L124`) |
| `library.ts` | 3 | `get_library_projection`、`get_media_items`、`update_library_entry`(`src/shared/tauri/library.ts#L39-L56`) |
| `rules/wire.ts` | 11 | document 组 11 个命令一一对应, 含 `list_rule_node_descriptors`(`src/shared/tauri/rules/wire.ts#L353-L416`, `#L547-L550`) |
| `execution.ts` | 3 + 1 订阅 | `executeRule`、`cancelExecution`、`catchUpExecution`, 加 `listenRuleExecutionEvents`(`src/shared/tauri/execution.ts#L181-L208`) |

`execution.ts` 除了三个 wrapper, 还把事件通道的合同镜像到前端: `RULE_EXECUTION_EVENT = 'rule-execution-event'` 与 Rust 侧 `commands::delivery::RULE_EXECUTION_EVENT` 是同一个字符串, `listenRuleExecutionEvents` 用 `isExecutionEvent` / `isExecutionEventKind` 做运行时形状校验后才把 payload 交给 handler, 并明确声明**一条订阅覆盖所有 execution**(调用方自己按 `execution_id` 过滤), 因此订阅应在 `execute` 之前建立以免错过启动阶段事件(`src-tauri/src/commands/delivery.rs#L9`, `src/shared/tauri/execution.ts#L16-L16`, `#L203-L208`)。这套 wrapper 已经有真实调用点: 规则编辑器的预览运行在 `session.ts` 里先订阅、再 `executeRule`、必要时 `cancelExecution`(`src/features/rules/model/session.ts#L353-L368`, `#L537`)。

`library.ts` 还包含纯前端的 `projectLibrary`: 它把投影过滤为「收藏/置顶/最近打开/有进度」并做排序, 不调 IPC(`src/shared/tauri/library.ts#L58-L73`)。

**缺口收窄到 3 个查询命令**: 26 个命令减去 23 个 wrapper, 剩下 `get_media_item`、`list_media_units`、`list_media_assets` 没有前端调用点(在 `src/` 内检索这三个名字无任何命中)。此外 `catchUpExecution` 虽然封装好了, 但 `src/` 里没有调用点 —— 也就是说「断线后补读 durable 事件」这条路径今天没有 UI 入口, 而发起与取消 execution 已经由规则编辑器接上。`get_media_items`(复数)是有消费方的: `library.ts` 的 `loadMediaItems` 用它批量取媒体条目。

wrapper 在非 Tauri 环境下也有明确降级: `isTauri()` 为假时读操作返回空值(`{global_seq: 0, entries: []}`、`[]`), 写操作返回 rejected Promise(`library_update_unavailable`)(`src/shared/tauri/library.ts#L39-L56`), 使浏览器里跑 `vite dev` 不至于崩在缺 IPC 上。

## capabilities 与 CSP

`capabilities/default.json` 只对一个窗口(`main`)授予权限, 清单是: `core:default`、`core:event:default`、`mcp-bridge:default`、`dialog:default`、`zustand:default`、`deep-link:default`、`opener:default`、`core:window:default` 加四个窗口动作(`allow-close`、`allow-minimize`、`allow-toggle-maximize`、`allow-start-dragging`)。没有 fs、shell、http 等插件权限——因为网络出口在 Rust 侧的 `lj-node-http`, 不经过 WebView。

CSP 是 self-only 加必要例外(`src-tauri/tauri.conf.json` 的 `app.security.csp`):

```text
default-src 'self'; img-src 'self' data: https:; script-src 'self';
style-src 'self' 'unsafe-inline'; connect-src 'self'; base-uri 'self'; object-src 'none'
```

注意 `img-src` 里的 `https:` 是当前唯一允许的外部资产来源; ADR 0007 规划的 `lanjing:` 协议**尚未**加入白名单(该网关未实现)。`withGlobalTauri: true` 让 `window.__TAURI__` 在页面可用。

窗口配置: 主窗口 1440x960、最小 768x720、`decorations: false`、`shadow: true`; macOS 用 `tauri.macos.conf.json` 覆盖为 `decorations: true` + `titleBarStyle: Overlay` + traffic light 位置, 因为 macOS 需要保留原生红绿灯按钮(`src-tauri/tauri.macos.conf.json#L1-L18`)。

## deep link 与单实例的接力

支持的 scheme 是 `legado` / `yuedu` / `lanjing`(`src-tauri/tauri.conf.json` 的 `plugins.deep-link.desktop.schemes`)。

Rust 侧的 `deeplink` 模块只做一件事: 第二实例被拒绝后聚焦主窗口。模块注释写明了分工——**URL 缓存与热启动投递由 `tauri-plugin-deep-link` 的 `getCurrent`/`onOpenUrl` 合同拥有, 本模块不建立第二个 queue 或 event**, 也不解析或记录 argv 以避免泄漏 URL query(`src-tauri/src/deeplink/mod.rs#L1-L18`)。`single_instance` 的 `deep-link` feature 会先把配置 scheme 的 argv 转交给 deep-link plugin, 所以聚焦函数拿到的是已经交接过的事件。

解析发生在前端: `src/shared/tauri/deep-link.ts` 在 Tauri 运行时订阅 `onOpenUrl` 并消费 `getCurrent`(冷启动), 交由 `deep-link-parse.ts` 转成 intent; `App.tsx` 的 `Startup` 组件再按 intent 导航(`item` → `/library/item/<id>`、`source` → `/sources?highlight=`、`install` → `/sources?import=`, `reject` → toast)。这条接力链路意味着「URL 语义」只在 TypeScript 里定义一次, Rust 不重复实现一遍。

## 当前边界

- 仍有 3 个查询命令没有 wrapper(`get_media_item`、`list_media_units`、`list_media_assets`), 且 `catchUpExecution` 有 wrapper 却无调用点, 所以 execution 的断线续播今天没有 UI 入口。
- `lanjing://` 资产协议未注册, CSP 也没有对应白名单; ADR 0007 定义的资产网关仍是设计。
- `mcp_bridge` 只存在于 debug desktop 构建; 发布构建里没有本地调试桥。
- 单实例与窗口聚焦只在桌面目标编译, 移动端依赖平台自身的单实例行为。
