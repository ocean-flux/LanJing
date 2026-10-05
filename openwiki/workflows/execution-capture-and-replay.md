---
type: "参考"
title: "Execution capture and replay"
openwiki_generated: true
sources:
  - id: openwiki-source-378ef8c9992cfb062d1d9087
    resource: repo://src-tauri/crates/lj-node-http/src/processor/adapter.rs
  - id: openwiki-source-970f295896afa9156178188f
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/execution.rs
  - id: openwiki-source-ab202b94798a4c907cf938e5
    resource: repo://src-tauri/crates/lj-rule-system/src/system/session_delivery.rs
  - id: openwiki-source-60db92730bd8b30a2a14ff4f
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler.rs
  - id: openwiki-source-150ac7f1387e477217243644
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/control_archive.rs
  - id: openwiki-source-b98d6cf0b98f80e0aa1f7d56
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/effect_execution.rs
  - id: openwiki-source-34d489227f93cd0425da2fa7
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/live_capture.rs
  - id: openwiki-source-b47efa6314fbe5388bae632e
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/outcome.rs
  - id: openwiki-source-ea6097f87e3300ae02b36e81
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/replay.rs
  - id: openwiki-source-f6f9a424b4b7e9359bbdeae3
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/state.rs
  - id: openwiki-source-23e2d8269a1c49580e9fce54
    resource: repo://src-tauri/crates/lj-runtime/tests/plan_runtime_test/control_contract.rs
  - id: openwiki-source-fd9d63063632251bc8099b7c
    resource: repo://src-tauri/crates/lj-runtime/tests/plan_runtime_test/replay_contract.rs
  - id: openwiki-source-115e591341fdf411553c3c50
    resource: repo://src-tauri/crates/lj-runtime/tests/plan_runtime_test/scheduling_contract.rs
  - id: openwiki-source-04df110324feb1bf7e2d0443
    resource: repo://src-tauri/crates/lj-storage/src/repository/event/mod.rs
  - id: openwiki-source-54befb6bf8312a63f2f1fa44
    resource: repo://src-tauri/crates/lj-storage/src/repository/execution/commit.rs
  - id: openwiki-source-1bc1d9c324386214f3fd1224
    resource: repo://src-tauri/crates/lj-storage/src/repository/execution/start.rs
  - id: openwiki-source-c2eb95ea8633875bc0a7ab72
    resource: repo://src-tauri/crates/lj-storage/src/repository/maintenance/recovery.rs
  - id: openwiki-source-97a89101bffc0f1b3ed4ad08
    resource: repo://src-tauri/crates/lj-storage/src/repository/projection/write.rs
  - id: openwiki-source-2256ade68852111694ed64ef
    resource: repo://src-tauri/crates/lj-storage/src/storage.rs
  - id: openwiki-source-ed04a03424577bccc2bcdb0a
    resource: repo://src-tauri/crates/lj-storage/src/storage/execution.rs
  - id: openwiki-source-7d4ed639b034234af79f8973
    resource: repo://src-tauri/crates/lj-storage/src/types/execution.rs
  - id: openwiki-source-39dd156540df2869ff0cbfa0
    resource: repo://src-tauri/crates/lj-storage/tests/event_projection_storage_test/replay_contract.rs
  - id: openwiki-source-164707dfe3596be848b3ec22
    resource: repo://src-tauri/src/commands/delivery.rs
  - id: openwiki-source-68e2dde2c84dfbdac9ee9fd2
    resource: repo://src-tauri/src/commands/execution.rs
  - id: openwiki-source-723acd102ba9a8c53b06e21e
    resource: repo://src/features/rules/inspector/ExecutionPreview.tsx
  - id: openwiki-source-b4c5152a4e2170db5b29047e
    resource: repo://src/features/rules/model/execution.ts
  - id: openwiki-source-e523845a56feec75d438c920
    resource: repo://src/features/rules/model/session.ts
generated: { by: "pi", at: "2026-10-05T08:37:16.392Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-05T08:37:16.392Z
---


## 一次执行的链路

```text
execute(ExecuteRequest)
  ├─ normalize_continue_action         ContinueAction 且 source 属 Legado 时消耗一次性动作
  ├─ start_execution_snapshot          原子写 Started Event + source revision pin, 并载入该 revision 的快照
  ├─ flush_persisted 校验              persisted_sequence 必须等于 record.revision, 否则 execution_start_sequence_mismatch
  ├─ runtime.execute(...)              传 immutable Plan + grant + base_url + credentials
  ├─ registry 登记 CancellationHandle, spawn run_session(detached)
  └─ 返回 ExecutionSession(events stream, cancellation, storage)
```

入口在 `src-tauri/crates/lj-rule-system/src/system/lifecycle/execution.rs#L46-L109`。三个顺序不能换:

1. **先落账再执行**: `start_execution_snapshot` 在**一个 writer transaction 里**写 Started Event 与 insertion 投影, 并 pin 住当时的 source revision(`src-tauri/crates/lj-storage/src/repository/execution/start.rs#L9-L14`), 同时通过 `ArtifactLink::Existing` 把 package/plan 两个 artifact 挂到该事件上(`#L28-L37`), 因此「这次执行用的是哪个 Plan」可追溯到字节。
2. **再校验序列连续性**: 起始 flush 拿到的最后序号必须等于刚写入的 revision, 不等就视为持久层不一致并立即把该 execution 收为 Failed(`execution.rs#L59-L84`)。
3. **失败必须收尾**: 从这一步起任何错误都经 `finish_started_execution_failure` 写 `ExecutionStatus::Failed` 终态再返回(`execution.rs#L351-L377`)——不会留下一条永远 running 的记录。

## runtime 的推进

`PlanRuntime::execute` 先做 `validate_plan` 与 `execution_path`(见运行时页), 然后 `run_execution`(`src-tauri/crates/lj-runtime/src/plan_runtime/scheduler.rs#L123-L163`):

1. 发 `Started`(事件序号从 1 开始, `scheduler.rs#L86`);
2. 取 execution 级许可(队列/取消导致的失败也会**发一个终态**, `#L135-L149`);
3. `execute_path` 逐节点推进;
4. 无论成功失败, 最后 `emit_terminal(outcome)` 一次(`#L162`), 由 `terminal_sent` 保证整个 session 只发一个终态(`#L109-L119`)。

`execute_path` 的顺序语义(`scheduler.rs#L165-L244`): 把 `IntentInput` 注入 entry 节点的 `LINEAR_INPUT_HANDLE`, 然后**按 `path.node_ids` 顺序**走——每个节点前先查取消(`#L190-L193`), Loop 节点查 `path.loops` 里的 control program, 其余交给 `execute_non_loop_node`; 全部走完后如果 mapper 节点从未产出 delta, 判 `InputTypeMismatch`「当前控制路径未到达 Mapper」(`#L234-L242`)。也就是说「完成」的定义不是「没报错」, 而是**产出过一次媒体图增量**。

端口状态是执行局部的: `PortState` 只在内存里维护 `FlowPortRef → RuntimeValue`, `route` 时若同一 input 收到第二个活跃 producer 直接返回 `Err("同一 input 收到多个 active producer")`(`src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/state.rs#L35-L54`)——这是编译期端口校验的运行时兜底, 不是可恢复错误。

## live 分支: durable-before-advance

这是整条链路最核心的不变量, 五步全在 `src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/live_capture.rs#L10-L121`:

| 步 | 动作 | 失败码 |
| --- | --- | --- |
| 1 | 调 live handler(HTTP/QuickJS/Extract) | `Cancelled` → `RunOutcome::Cancelled`, 其余 `EffectFailed` |
| 2 | `EffectCapture::from_live` 构造 capture | `CaptureWitnessInvalid` |
| 3 | capture 的 kind 必须等于 Plan 声明, 且 JS 输出形状匹配声明 | `CaptureWitnessInvalid` |
| 4 | `archive.persist_durable(capture)` | `CaptureFailed` |
| 5 | 收据五字段匹配: `effect_id`、`invocation_path`、`fingerprint`、`output_hash`、`witness_hash` | `CaptureReceiptMismatch` |

只有在第 5 步通过之后才发 `EffectCaptured`, 下游才能读到输出——注释把它写成一句话: 「已经发生的外部 effect 必须完成 commit/rollback, 收据匹配前既不能发出 `EffectCaptured`, 也不能让下游读取输出」(`live_capture.rs#L66-L67`)。收据比对本身是纯函数 `receipt_matches`(`src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/outcome.rs#L44-L57`)。

**为什么必须是这个顺序**: 外部 effect(一次 HTTP 请求)无法回滚, 所以只能让「账」先于「用」。如果反过来(先发事件/先喂下游, 再异步落账), 崩溃窗口里就会出现「下游据此产生了新资源, 但那次请求没有任何记录」的不一致, replay 也会缺失该 capture。

**失败时的可见后果**:

- 进程活着时, 第 2-5 步任一失败都产生**一个带归属的终态**(`Failed` 且带 node_id/effect_id), 不是静默中断; 测试 `effect_error_becomes_one_failed_terminal_with_attribution`(`src-tauri/crates/lj-runtime/tests/plan_runtime_test/scheduling_contract.rs#L187`)与 `replay_missing_capture_fails_with_single_attributed_terminal`(`.../replay_contract.rs#L162`)锁的正是这一点。
- 进程在提交成功后、发事件前崩溃: 下次启动时那条 execution 会被收为 `incomplete`(见下节), 而已提交的 capture 仍在账上——它是**可被再次读到的**, 不会丢。
- 第 4 步已在事务中, 且注释明确「capture 已进入 archive 事务的必须完成自己的 commit 或 rollback」(见取消页), 因此不存在「写了一半」的中间态。
- 捕获成功后若已取消, 返回 `Cancelled` 而不是 `Completed`(`live_capture.rs#L109-L110`)——已提交的 delta 保持先于 `Cancelled` 的可观察顺序。

handler 返回的 typed failure 也要走同一条 durable 路径: HTTP 的 `EffectOutput::Failure` 会被归档, 这样 replay 复现的是同一次失败, 而不是重新打一次网络(`src-tauri/crates/lj-node-http/src/processor/adapter.rs#L118-L138`)。

## 控制轨迹的持久化

Loop 与 Merge 这类不产生外部 effect 的节点也要落账, 走 `persist_or_replay_control`(`src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/control_archive.rs#L9-L21`):

- Live: `ControlTraceCapture::new` 编码(失败 `CaptureFailed`「control trace 无法编码」)→ `persist_control_trace` → 收据的 `invocation_path` 与 `trace_hash` 必须匹配, 否则 `CaptureReceiptMismatch`(`#L23-L63`);
- Replay: `load_control_trace` 取回后逐字段比对。

这样 replay 才知道「上一次在这条路径上选了哪个分支、合并了哪些输入、循环了几轮」——否则同一条 Plan 在数据变化时会走出不同的控制轨迹, 结果无法复现。测试 `typed_condition_merge_loop_runs_selected_branch_and_replays_exact_invocations` 与 `replay_missing_extra_and_tampered_control_invocations_never_call_live_handlers`(`control_contract.rs#L39`、`#L175`)覆盖了缺失/多余/被篡改三种情形。

## 事件落账、投递与投影

runtime 发出的事件由唯一的 session runner 消费(`src-tauri/crates/lj-rule-system/src/system/session_delivery.rs#L47` 起), 模块头列了五条不变量(`#L3-L9`)。逐事件动作:

| 事件 | runner 动作 |
| --- | --- |
| `Started` / `EffectReplayed` | 无需动作 |
| `EffectCaptured` | 只用于触发 `flush_persisted` |
| `DeltaProduced` | 先 seal 一次性 continue action, 再 `commit_delta` |
| `Completed` / `Cancelled` | `finish_execution` + `terminal_observed = true` |
| `Failed` | 经 `runtime_failure_error` 脱敏后 `persist_runtime_failure` |

`commit_delta`(`session_delivery.rs#L262-L292`)调 `storage.commit_execution_delta(DeltaCommit{ execution_id, expected_version, event_id, occurred_at_ms, delta })`, 存储侧在**同一个事务**里做三件事(`src-tauri/crates/lj-storage/src/repository/execution/commit.rs#L1-L45`): 校验 delta 的每个资源都归属于该 execution 的 source(`validate_delta_source`, `src-tauri/crates/lj-storage/src/repository/projection/write.rs#L85` 起)、`append_event_transaction` 写 Event 与投影、更新 execution 的 revision 与 global_seq。`expected_version` 是乐观并发: 版本不符即 `VersionConflict`, 没有隐式合并。

**先持久再投递**在这里的可见后果:

- 前端拿到的每个 delta 都已经在账上, 因此「刷新后仍能看到刚才那条结果」是结构性保证, 而不是靠重试;
- 投递失败(窗口关闭/事件系统不可用)只 warn, 不取消执行(`src-tauri/src/commands/delivery.rs`), 中断后靠 `catch_up_execution` 从 C2 事件流补齐;
- delta 事件与投影在同一事务里提交, 所以不存在「事件写了但投影没更新」或反之: 事务边界在 `commit_execution_delta` 里由 `append_event_transaction` 与投影闭包共同持有(`src-tauri/crates/lj-storage/src/repository/execution/commit.rs#L1-L45`)。原先还有一条 `d12_thousand_resource_event_projection_transaction_gate` 规模门守着这条不变量, 该标定用例已随性能门禁一起删除, 现在只剩语义契约测试(`src-tauri/crates/lj-storage/tests/event_projection_storage_test/projection_retention_contract.rs`)。
- 事件流序号由单写入者的 `event_counters` 递增, 天然单调; catch-up 只接受**严格连续**的序号, 缺失即报错而不是补造(`session_delivery.rs#L3-L9`)。

## 终态收敛与中断恢复

终态在三个层面各自唯一:

1. **runtime 事件层**: `EventEmitter::terminal_sent`;
2. **投影层**: `ExecutionStatus::is_terminal()` 为 `!Running`, 且 `process_finish_execution` 对不同终态的重复写入返回 `InvalidInput`「execution 已处于终态」(`commit.rs#L47-L101`);
3. **runner 层**: `terminal_observed`, 一旦持久终态获胜就立即结束, 并从私有取消注册表删除该 execution, 避免 detached 条目泄漏(`session_delivery.rs#L43-L47`)。

如果 runtime 的事件流**异常结束**(没有终态), runner 仍会尝试写一个 Failed 终态(`session_delivery.rs#L187` 起)——「每个 execution 只有一个持久 terminal」是硬保证。

**进程崩溃**走另一条路: 启动恢复里的 `mark_interrupted_executions` 把所有仍是 `running` 的执行改成 `incomplete` 并补上 `finished_at_ms`(`src-tauri/crates/lj-storage/src/repository/maintenance/recovery.rs#L67-L76`), 由 `EventProjectionStorage::open` 在打开数据库时调用。语义差别值得记住:

- 它**只更新投影, 不追加事件**。因此一条被崩溃打断的执行在投影里是 `incomplete`(属于终态, 见 `src-tauri/crates/lj-storage/src/types/execution.rs#L87-L105`), 但它的 **event stream 里没有终态事件**。
- 后果是 `catch_up_execution` 对这类执行不会观测到 terminal(它的判断基于最后一个事件的 kind), 前端只能从投影/执行列表得到 `incomplete`。把「进程崩溃」当成一次 Failed 事件落账 是 已决定未实现 的行为——当前设计选择的是投影收敛而非事件补记。
- 另一面: 运行时不可恢复的错误会走 `persist_runtime_failure`, 它先写诊断事件、flush, 再 `finish_execution(Failed)`(`session_delivery.rs#L295-L331`), 所以「错误原因」与「终态」都在事件流里。

## replay 分支

**分叉条件只有一处**: `ExecutionMode`。`Live` 用当前 source 快照与凭据; `Replay { archived_execution_id }` 从历史 pin 建 archive, `start_replay_execution` 明确「不读取 current source」(`src-tauri/crates/lj-storage/src/storage/execution.rs#L31-L45`), 并由 `PlanRuntime::execute` 在 spawn 前把凭据重置为默认, 使静态凭据不进入 replay 任务。

`execute_replay_effect`(`src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/replay.rs#L12-L120`)是 **archive-only**, 永不调用 live handler。比对顺序与失败码:

| 顺序 | 检查 | 失败码 |
| --- | --- | --- |
| 1 | `load_replay(archived_execution_id, invocation_path, kind)` 必须有记录 | `ReplayCaptureMissing` |
| 2 | `execution_id`、`invocation_path`(含 node_id)、`kind`、输出 kind、JS 输出形状与声明一致 | `ReplayRecordMismatch` |
| 3 | `record.fingerprint == 本次计算的 fingerprint` | `ReplayFingerprintMismatch` |
| 4 | 重算输出 hash 必须等于记录 | `ReplayOutputHashMismatch` |
| 5 | `validate_replay_integrity()` 通过, 且 witness 能重新绑定当前 Plan、exact invocation 与输入 | `ReplayWitnessMismatch` |

第 5 步的注释说明了它的强度: 「不只校验 archive 自洽, 还将 witness 重新绑定当前 Plan、exact invocation 与输入; 任何不匹配均为硬失败, **绝不调用 live adapter 补救**」(`replay.rs#L88-L89`)。成功后发 `EffectReplayed`(带 fingerprint/output_hash/witness_hash), 然后与 live 同样的收尾: 已取消则 `Cancelled`, 归档输出若是 failure 则 `EffectFailed`(`#L101-L120`)。

**为什么 replay 不需要 handler**: `load_replay` 是纯读, 所以 effect registry 的变化(handler 被替换或升级)不会让历史 capture 失效——只有 fingerprint/输出/输入**语义**变化才会失败, 这正好是应该失败的时候。测试 `live_and_replay_preserve_typed_outputs_without_live_fallback` 与三个 tamper 用例(QuickJS 脚本/输入/输出 witness hash、Extract 输入 witness hash)锁定了这一点(`replay_contract.rs#L53`、`#L304`、`#L366`)。

存储侧的 replay pin 也是被验证的: `execution_replay_pin_uses_verified_artifact_and_rejects_tampering`、`replay_start_keeps_historical_source_snapshot_after_source_update`、`replay_pin_survives_restart_and_rejects_tampered_snapshot`(`src-tauri/crates/lj-storage/tests/event_projection_storage_test/replay_contract.rs#L10`、`#L69`、`#L321`)覆盖「源被更新后 replay 仍用历史快照」与「重启后 pin 仍有效」。

## 前端如何呈现 live 与 replay

预览运行在前端只有一条路径: `session.ts` 的 `runPreview(request, replayOf)` 把 live 与 replay 折进同一个状态机, 二者的差别只有请求里的 `mode`(`{mode: 'live'}` 对 `{mode: 'replay', execution_id}`); `startPreviewRun` 与 `startReplayRun` 是两个薄入口, `cancelPreviewRun` 只把取消当请求发出、终态仍由 runtime 的 `cancelled` 事件落定(`src/features/rules/model/session.ts#L330-L345`, `#L524-L538`)。两条前端不变量值得记住:

- **订阅先于启动**: 先建立 `listenRuleExecutionEvents`, `executeRule` 返回前到达的事件先进缓冲, 拿到 `execution_id` 后再补叠, 因此不会漏掉启动阶段的事件; 同一时刻只跑一次预览, 上一次订阅先收掉(`session.ts#L341-L352`)。
- **请求失败与运行失败同归诊断表面**: 订阅不上或启动被拒只落到运行诊断, 不向调用方抛错——诊断栏就是失败的去处(`session.ts#L330-L340`)。

运行结局在 `ExecutionPreview.tsx` 呈现: 状态 + 稳定 code, 再加一句「这次重放缺什么」的归类(`src/features/rules/inspector/ExecutionPreview.tsx#L77-L95`)。归类表把 Rust 侧写死的 replay 专属 code 收敛成三种对作者意味着不同下一步的语义(`src/features/rules/model/execution.ts#L166-L215`):

| 归类 | 含义 | 代表 code |
| --- | --- | --- |
| `capture_missing` | 这段重放缺一段没被捕获的 effect | `replay_capture_missing` |
| `history_mismatch` | 历史输入/输出/witness/收据校验不一致 | `replay_record_mismatch`、`replay_fingerprint_mismatch`、`replay_output_hash_mismatch`、`replay_witness_mismatch` |
| `history_unavailable` | 历史固定不出来(不存在、未成功完成、被 GC、pin 与请求或快照不符) | `replay_execution_missing`、`replay_execution_not_completed`、`replay_revision_unavailable`、`replay_pin_unavailable` |

未登记的 code 一律不归类(宁可退回普通运行失败, 也不把未知失败说成某种重放结局), 且 live 运行里出现同一个 code 时归类为 `null`——只有 `mode === 'replay'` 的运行才有可解释的历史(`execution.ts#L200-L215`)。live 运行则折叠出捕获清单: `CaptureList` 展示每次捕获的 effect_id、output_hash 与 artifact 数量, 空态也有文案(`ExecutionPreview.tsx#L101-L130`); 有可重放历史且不在运行时才允许重放(`#L184-L185`)。

## 取消只阻后续 effect

取消的语义链: 取消令牌只携带状态(不含 HTTP client、不含非 Send 的 QuickJS 句柄), 因此可以跨 blocking lane; `CancellationHandle::cancel` 用 `compare_exchange` 幂等并返回是否为首次状态变化; 每个节点执行前、每个 effect 取许可前后都检查一次。

可见后果:

- 已经提交的 delta 保持原有可观察顺序, `Cancelled` 排在它们之后;
- 正在进行的 HTTP 请求会被 `tokio::select!` 打断(返回 `EffectErrorCode::Cancelled`, 不产生可归档结果), QuickJS 侧由 watchdog 触发 interrupt;
- 已经在 archive 事务里的 capture 必须完成 commit 或 rollback, 不会因为取消留下半个事务;
- 取消不会撤销已写入的投影——`cancellation_stops_new_effects_and_emits_only_cancelled` 与 `cancellation_inside_first_iteration_stops_all_later_body_effects`(`scheduling_contract.rs#L6`、`control_contract.rs#L632`)是这两条断言的落点。

## 与存储侧的边界

runtime 只见 `Arc<dyn EffectArchive>` 这个 trait seam, **不依赖 `lj-storage`**; 把它接上具体实现的唯一位置是 facade 的 `runtime.execute(..., Arc::new(self.state.storage.clone()))`(`execution.rs#L63-L64`)。所以「runtime 的持久化契约」是 trait 上的 `persist_durable` / `load_replay` / `persist_control_trace` / `load_control_trace` 四个方法加两个 receipt 类型, 而不是某张表。

## 测试位置一览

| 关注点 | 测试 |
| --- | --- |
| 计划 hash/身份校验 | `replay_contract.rs#L6`、`#L18` |
| live 与 replay 的 typed 输出一致且无 fallback | `replay_contract.rs#L53` |
| 缺失/篡改 capture 的硬失败 | `replay_contract.rs#L162`、`#L198`、`#L251`、`#L304`、`#L366`、`#L423` |
| 控制轨迹 replay | `control_contract.rs#L39`、`#L175`、`#L365` |
| 循环边界与硬上限 | `control_contract.rs#L413`、`#L521` |
| 取消语义 | `scheduling_contract.rs#L6`、`control_contract.rs#L632` |
| 并发许可(per-source / global) | `scheduling_contract.rs#L50`、`#L115` |
| 终态唯一与归属 | `scheduling_contract.rs#L187`、`#L220` |
| 有界事件通道背压早于下游 effect | `scheduling_contract.rs#L255` |
| 端到端 live + replay(真实 SQLite/artifact) | `src-tauri/crates/lj-integration-tests/tests/legado_rule_system.rs`、`maccms_json_rule_system.rs` |

「有界通道背压先于下游 effect」这条值得单独说: 它意味着**投递阻塞会反过来限制执行速度**(而不是让事件在内存里堆积)。这是 `session_event_capacity`(默认 64)影响性能而非正确性的地方, 也是必须为正数的原因。

## 未验证 / 边界

- **崩溃点覆盖**: durable-before-advance 的窗口(提交成功后、发事件前)靠代码结构与 `DurableFileArchive` fixture 验证, 没有「在任意指令处 kill 进程再恢复」的故障注入测试。
- **incomplete 的客户端可见性**: 事件流没有终态事件这一点, 前端如何处理(catch-up 拿到空/无 terminal 的结果)未验证。
- **迁移到真实 SQLite 之外的后端**: 单写入者 + `BEGIN IMMEDIATE` 的事务语义是 SQLite 特有的, 其余后端未经检验。
- **投影的时间成本**: 原先那条千资源投影事务规模门已删除, 仓库里不再有任何性能用例; 并发多 execution 同时提交 delta 时的写队列延迟完全未测。
