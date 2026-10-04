---
type: "参考"
title: "Rule system facade"
openwiki_generated: true
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T10:14:16.110Z
sources:
  - id: openwiki-source-e63ceb81b531375188f64063
    resource: repo://src-tauri/crates/lj-rule-system/src/error.rs
  - id: openwiki-source-f77c1b2f9f427db1e6d94a57
    resource: repo://src-tauri/crates/lj-rule-system/src/lib.rs
  - id: openwiki-source-aa1c93c7ddf499b5db0c1905
    resource: repo://src-tauri/crates/lj-rule-system/src/system.rs
  - id: openwiki-source-b530b0e45422d1b162bf33e3
    resource: repo://src-tauri/crates/lj-rule-system/src/system/capture.rs
  - id: openwiki-source-32d4001fffac613566845710
    resource: repo://src-tauri/crates/lj-rule-system/src/system/error_mapping.rs
  - id: openwiki-source-f4d8e498f8a27d9242bfdc4b
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/document.rs
  - id: openwiki-source-970f295896afa9156178188f
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/execution.rs
  - id: openwiki-source-80e8762cff162b2ecd16e129
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/prepare_install.rs
  - id: openwiki-source-00fc7c09c51d967590463257
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/tests.rs
  - id: openwiki-source-985022f215d1a8fb48d686c6
    resource: repo://src-tauri/crates/lj-rule-system/src/system/query_adapter.rs
  - id: openwiki-source-ab202b94798a4c907cf938e5
    resource: repo://src-tauri/crates/lj-rule-system/src/system/session_delivery.rs
  - id: openwiki-source-a7ae0a93a20e741aa0b31f5b
    resource: repo://src-tauri/crates/lj-rule-system/src/types/config.rs
  - id: openwiki-source-66845dd8b3f72696149d5d09
    resource: repo://src-tauri/crates/lj-rule-system/src/types/document.rs
generated: { by: "pi", at: "2026-10-04T10:14:16.110Z" }
---


## 门面边界

`lj-rule-system` 是规则导入、安装与执行的唯一 concrete façade。它对外只公开安全 DTO; `Definition`、immutable Plan、node effect adapter、`EventProjectionStorage` 与 execution registry 全部保持私有组合, Tauri 层不直接依赖 storage/importer/runtime(`src-tauri/crates/lj-rule-system/src/lib.rs#L1-L29`)。

`RuleSystem` 内部持有的跨命令共享状态是(`src-tauri/crates/lj-rule-system/src/system.rs#L50-L60`):

| 字段 | 作用 |
| --- | --- |
| `storage: EventProjectionStorage` | 唯一持久化真相来源 |
| `compiler: Compiler` | 唯一 compiler 实例, 其 version 同时写进 runtime config |
| `runtime: PlanRuntime` | 执行引擎 |
| `registry: Arc<FrozenRegistry>` | 装配后冻结的内置 capability |
| `candidate_ttl_ms` | Install Candidate 有效期 |
| `session_event_capacity` | delivery 有界通道容量 |
| `executions: Mutex<HashMap<Uuid, CancellationHandle>>` | 只存取消句柄, 终态后必定移除 |

注释明确了两个「不做」: 不保留 identity-keyed 的 Plan/current cache(candidate、installed snapshot、execution revision 只以 durable receipt 为真相), 外部不能注入 Graph、Plan JSON、processor registry、SQLite connection 或 storage handle(`src-tauri/crates/lj-rule-system/src/system.rs#L36-L49`)。

## open: 装配顺序

`RuleSystem::open(config)`(`src-tauri/crates/lj-rule-system/src/system.rs#L73`)的装配顺序是有依赖的:

1. 先拒绝无效配置: `candidate_ttl` 必须大于零(`candidate_ttl_invalid`), `session_event_capacity` 必须大于零(`bounded_capacity_invalid`)——后者是无界背压的根源(`#L75-L95`)。
2. 打开 `EventProjectionStorage`(keyring service 从 config 带入)(`#L97-L101`)。
3. `Compiler::default()`, 并把 `compiler.version()` 作为 `PlanRuntimeConfig.compiler_version`。这一步把「compiler 身份一致」的校验从约定变成装配约束——runtime 的 `compiler_version` 只能来自 compiler 自身(`#L102-L110`)。
4. 构造 HTTP adapter: `local_fixture_http` 为真时用 `HttpEffectAdapter::new_test()`(关闭环回地址的 SSRF 拒绝), 否则用生产构造器(`#L111-L115`)。
5. `PluginHost::new()` → `register_plugin(builtin::manifest(), builtin::effects(http, QuickJs, Extract))` → `freeze()`, 并**在接受任何规则请求之前**完成。注释写明 execution 始终绑定这个 snapshot(`#L116-L128`)。

`RuleSystemConfig::desktop` 给出默认容量: session event capacity 64、并发 execution 16、全局并发 effect 16、单一来源并发 effect 4、candidate TTL 24 小时、keyring service `lanjing.event-store.master-key`(`src-tauri/crates/lj-rule-system/src/types/config.rs#L20-L35`)。

## 唯一把 storage 当 EffectArchive 的地方

`RuleSystem::execute` 在启动 runtime 时把 storage 自身作为 archive 传入:

```rust
self.state.runtime.execute(
    PlanExecutionRequest { .. },
    self.state.registry.clone(),
    Arc::new(self.state.storage.clone()),
)
```

(`src-tauri/crates/lj-rule-system/src/system/lifecycle/execution.rs#L95-L110`)

这是整个仓库里唯一一处 `EffectArchive` 的真实实现绑定。它为什么必须在这里: runtime 的 durable-before-advance 要求「effect 的 capture 在推进之前落盘且拿到收据」, 而 session runner 又要求「Delta 与投影在同一事务提交后再投递」。两者如果各自写自己的存储, 就会出现两个真相与两次提交之间的窗口。让 runtime 直接写同一个 `EventProjectionStorage`, 同一 writer 串行化两类写入, 才谈得上「执行可精确回放」这个不变量。

## execute 的编排

`execute` 的顺序(`src-tauri/crates/lj-rule-system/src/system/lifecycle/execution.rs#L46-L131`):

1. `normalize_continue_action` 处理继续动作输入。
2. `start_execution_snapshot` 在 storage 里持久化 execution 起点并取得 `ExecutionSnapshot`(source identity、Plan、grant、base_url、凭据、mode、replay continue actions)与记录 revision(`#L53-L56`)。
3. 建 delivery 有界通道, 并 `flush_persisted` 把已持久事件先推给调用方; 随后校验 `persisted_sequence == record.revision`, 不连续即失败收尾(`#L57-L84`)。
4. 调 `runtime.execute(...)`; runtime 启动失败也要通过 `finish_started_execution_failure` 把已持久化的 execution 收尾, 不能留下悬挂记录(`#L85-L99`)。
5. 登记取消句柄、`tokio::spawn(run_session(...))`, 返回 `ExecutionSession`(`#L100-L131`)。

换言之: **先落库、再起执行**。任何一步失败都走同一条「把 durable 起点收尾」的路径, 这是「不存在只在内存里的 execution」的实现方式。

## session_delivery: 先提交后投递

`run_session` 是 `RuleSystem` 的唯一 session runner, 模块头列出五条不变量(`src-tauri/crates/lj-rule-system/src/system/session_delivery.rs#L1-L9`):

- 每个 execution 只有一个持久终态, runtime 流异常结束也会尝试写 `Failed`;
- `MediaGraphDelta` 必须先由 C2 在 Event + projection 同一 transaction 中提交, 再向 delivery 发送;
- 取消只阻止后续 effect, 已提交 Delta 仍保持先于 `Cancelled` 的可观察顺序;
- 丢弃 delivery stream 不能取消 execution, 有界 sender 只影响投递背压;
- catch-up 只接受严格连续的 C2 stream sequence, 绝不补造缺失事件。

事件循环的分支(`#L60-L200`):

| runtime 事件 | 处理 |
| --- | --- |
| `Started` / `EffectReplayed` | 无需持久化动作 |
| `EffectCaptured` | `flush_persisted` 把已落盘的 capture 事件投递出去(test-support 下另登记 witness lookup) |
| `DeltaProduced` | 先 `seal_legado_continue_actions`, 再 `commit_delta` |
| `Completed` / `Cancelled` | `finish_execution` 写对应状态并置 `terminal_observed` |
| `Failed` | `persist_runtime_failure` 把 runtime failure code 经 `runtime_failure_error` 脱敏后持久化 |

`commit_delta` 的实现只有一句关键注释: 「在 C2 同一 transaction 中提交 Delta 与投影, 成功后才可 delivery」——它调 `storage.commit_execution_delta(DeltaCommit { expected_version, delta: ProjectionDelta { upserts, tombstones } })`, 成功后才 `flush_persisted`(`#L261-L292`)。`expected_version` 是乐观并发: 投影表版本与预期不符就整笔拒绝。

异常路径也收敛到同一处: 任一持久化返回错误 → 置 `terminal_observed` 并 `persist_runner_failure` 后 break(`#L169-L185`); 事件流在未发终态前结束 → 写 `runtime_stream_ended` 失败终态(`#L187-L200`)。因此「execution 永远有终态」不是靠 runtime 守约, 而是靠 runner 兜底。

## document 生命周期的编排

`save_native_rule_document` 只做「准备」, 真正的原子写在 storage(`src-tauri/crates/lj-rule-system/src/system/lifecycle/document.rs#L270-L372`):

1. 读现有文档 detail, 从既有 semantic snapshot 解析 `CredentialManifest`(缺失则空 manifest)。
2. `canonicalize` + `validate` 定义; **只有 `current_semantic_revision == expected_revision` 时才准备凭证变更**, 否则跳过——注释写明原因是「stale semantic 请求不进入凭证校验/写入」, 且 writer 会再次校验 revision(`#L296-L313`)。
3. 计算 canonical `definition_json` 与 `definition_hash`。
4. 决定激活状态: 只要诊断里存在 Error 级问题就是 `Draft`, 否则 `Effective`(`#L333-L340`)。
5. 把语义域与布局域打包成 `SaveDocumentRequest` 交给 storage 一次处理(`#L360-L371`)。

保存是**分域**的: 语义域与布局域各自携带独立 `expected_revision`, 冲突不影响另一域(`src-tauri/crates/lj-rule-system/src/types/document.rs#L118-L124`)。`SemanticActivation` 只有 `Draft` 与 `Effective` 两个值, `DomainOutcome` 携带新 revision、可选冲突与可选激活状态(`#L136-L158`)——「草稿还是生效」是保存结果的一部分, 而不是调用方的推测。

## prepare_install 的编排

`stage_prepared_candidate` 把导入产物变成候选之前依次过四道门(`src-tauri/crates/lj-rule-system/src/system/lifecycle/prepare_install.rs#L159-L190`):

1. `canonicalize`, 再把 `validate` 的结果与导入期诊断(importer 产出的 `diagnostics`)合并;
2. `compiler.compile(&definition)`, 失败映射为 stage `Candidate` 的 `RuleError` 并附诊断;
3. `runtime.validate_plan(&plan)`——编译通过不代表 runtime 接受, 例如 compiler 版本或 hash 完整性问题在这一步才拦;
4. 用 `now + candidate_ttl_ms` 计算到期时刻(溢出即 `candidate_expiry_overflow`), 再写入 candidate。

意图很明确: 候选是「已证明可执行的安装凭据」, 所以它必须同时通过编译与运行时两道校验。

## query_adapter: 只读安全投影

`query_adapter` 的职责边界写在模块头: query 只读 C2 的规范化投影或更新 library aggregate, **不会返回 Definition、Plan、artifact ref、secret 或 storage transaction**; 媒体投影读取只按稳定 ID 与有界分页恢复, 不替代 live execution, 也不下发整图 `MediaGraphDelta`(`src-tauri/crates/lj-rule-system/src/system/query_adapter.rs#L1-L8`)。

分页与批量边界是硬编码常量: 默认页大小 50、硬上限 100、批量点查 ID 上限 64(`#L28-L33`)。公开查询包括 `list_installed_sources`、`list_source_revisions`、`get_library_projection`、`update_library_entry`、`catch_up_execution`、`get_media_item` / `get_media_items`、`list_media_units`、`list_media_assets`(`#L41-L286`)。

注意 `update_library_entry` 是写操作, 但写的是 library aggregate 投影, 不是规则或执行数据; 它与 query 放在同一模块是因为它对外的形状同样是「小、有界、无内部引用」。

## error_mapping: 脱敏收敛

`error_mapping` 把 compiler、runtime、plugin、storage 的内部错误收敛成稳定、脱敏的 `RuleError`。不变量写在模块头: **任何 message、diagnostic 或 trace 都不得包含 body、cookie、token、完整 URL query、Plan JSON 或 opaque payload**(`src-tauri/crates/lj-rule-system/src/system/error_mapping.rs#L1-L11`)。

四类映射函数各管一段(`#L15-L244`): `runtime_failure_error` 把每个 `RuntimeFailureCode` 映射成 (stage, code, message) 三元组, 例如 `CapabilityDenied → Capability/runtime_capability_denied`、`EffectFailed → Effect/effect_failed`、`ReplayFingerprintMismatch → Replay/replay_fingerprint_mismatch`; `plugin_error`、`compiler_error`、`runtime_error`、`storage_error` 分别收敛其余来源, storage 一侧还按原因进一步区分契约类错误(`candidate_storage_contract` / `contract_storage_contract` / `durability_storage_contract`)。

对外的 `RuleError` 结构固定为 `stage` + `code` + `message` + `trace_id` + `retryable` + `diagnostics`(`src-tauri/crates/lj-rule-system/src/error.rs#L41-L56`)。`retryable` 是显式字段而不是让调用方猜; `trace_id` 让用户可报告的问题能对上本地记录, 同时 message 本身不携带任何载荷。

`RuleErrorStage` 用 12 个阶段表达「在哪一步坏了」(`#L11-L39`): `Import`、`Validation`、`Compile`、`Candidate`、`Install`、`Capability`、`Execution`、`Effect`、`Persistence`、`Replay`、`Cancelled`、`Internal`。这套阶段划分与生命周期一一对应, 使前端可以按 stage 决定呈现方式(例如 `Candidate` 阶段的失败通常是用户可修的输入问题, `Replay` 阶段的失败是本地记录完整性问题)。

## 测试与遗留

- `system/lifecycle/tests.rs` 有端到端用例 `current_control_candidate_passes_gate_and_installs_executes_and_replays`: 同一份当前规则走完 candidate → 安装 → 执行 → 回放(`src-tauri/crates/lj-rule-system/src/system/lifecycle/tests.rs#L142`)。
- `capture.rs` 只在 `feature = "test-support"` 下编译, 通过真实 `EffectArchive::load_replay` seam 读取已 durable 的 capture, 且 lookup 只在 `EffectCaptured` 被 runner catch-up 之后登记——它是测试观察口, 不是生产路径(`src-tauri/crates/lj-rule-system/src/system/capture.rs#L1-L47`)。
- `test-support` 还导出 witness DTO(仅测试用), 生产构建不包含(`src-tauri/crates/lj-rule-system/src/lib.rs#L31-L42`)。
