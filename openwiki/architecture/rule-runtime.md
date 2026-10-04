---
type: "参考"
title: "Rule runtime"
openwiki_generated: true
sources:
  - id: openwiki-source-f70faf819fb2edbb8e236f45
    resource: repo://docs/adr/0004-rule-first-open-extension-architecture.md
  - id: openwiki-source-8f834099b0a246733c691fa3
    resource: repo://src-tauri/crates/lj-node-extract/src/processor.rs
  - id: openwiki-source-378ef8c9992cfb062d1d9087
    resource: repo://src-tauri/crates/lj-node-http/src/processor/adapter.rs
  - id: openwiki-source-0dcbf0271dec640cfb5161dc
    resource: repo://src-tauri/crates/lj-node-js/src/processor.rs
  - id: openwiki-source-8ece8d8ea6055cf2f800dcb4
    resource: repo://src-tauri/crates/lj-runtime/src/effect_registry.rs
  - id: openwiki-source-434a7d2349c644a4bec2150e
    resource: repo://src-tauri/crates/lj-runtime/src/effect_registry/builtin.rs
  - id: openwiki-source-1cb35e7f0702e9e046ce0dcd
    resource: repo://src-tauri/crates/lj-runtime/src/effect/cancellation.rs
  - id: openwiki-source-ba442c85857684a6872f9ae8
    resource: repo://src-tauri/crates/lj-runtime/src/effect/contracts.rs
  - id: openwiki-source-b5b31452901a813cae4d04db
    resource: repo://src-tauri/crates/lj-runtime/src/effect/witness.rs
  - id: openwiki-source-ab89dba4a21770ddfe72a714
    resource: repo://src-tauri/crates/lj-runtime/src/lib.rs
  - id: openwiki-source-4c3b6fba913fb883ef519a2f
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/api.rs
  - id: openwiki-source-60db92730bd8b30a2a14ff4f
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler.rs
  - id: openwiki-source-150ac7f1387e477217243644
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/control_archive.rs
  - id: openwiki-source-b98d6cf0b98f80e0aa1f7d56
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/effect_execution.rs
  - id: openwiki-source-34d489227f93cd0425da2fa7
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/live_capture.rs
  - id: openwiki-source-5c7572d3e7a28403c347b69c
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/loop_execution.rs
  - id: openwiki-source-ea6097f87e3300ae02b36e81
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/replay.rs
  - id: openwiki-source-25926d04f5e6688f77dced08
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/routing.rs
  - id: openwiki-source-5dd5df7cae61c75ccead7c81
    resource: repo://src-tauri/crates/lj-runtime/src/plan_runtime/validation.rs
generated: { by: "pi", at: "2026-10-04T13:54:24.186Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T13:54:24.186Z
---


## 职责与入口

`lj-runtime` 只接收 compiler 产出的 `ExecutionPlan`, 经 typed HTTP/QuickJS/Extract effect seam 执行; 它不解析、迁移或执行历史规则 shape(`src-tauri/crates/lj-runtime/src/lib.rs#L1-L5`)。

唯一入口是 `PlanRuntime::execute(request, registry, archive)`(`src-tauri/crates/lj-runtime/src/plan_runtime/api.rs#L371-L428`)。它在同一处完成四件事: 校验 Plan、为该 intent 选路径、拒绝空 `source_id`/`trace_id` 与缺失 Tokio runtime、以及在 replay 模式下**清空静态凭据**(`request.credentials = HttpExecutionCredentials::default()`), 然后 `tokio::spawn` 调度并把有界事件流包装成 `ExecutionSession`(`#L377-L428`)。

启动后的失败不再变成同步错误: 它们进入 session 的 `ExecutionEventKind::Failed`。这条边界让调用方只有两种失败处理位置——`execute()` 的 `Result` 处理「根本不该开始」的问题, 事件流处理「开始之后」的问题。

`PlanExecutionRequest` 携带 `execution_id`、`source_id`、`trace_id`、`plan`、`intent`、`input`、`mode`、`capabilities`、`base_url` 与 `credentials`; `ExecutionMode` 只有 `Live` 与 `Replay { archived_execution_id }`(`src-tauri/crates/lj-runtime/src/plan_runtime/api.rs#L203-L243`)。

## 装配: EffectRegistry → FrozenEffectRegistry

handler 的注册与冻结只发生在 application composition 阶段, 冻结后不再变化, 因此一次 execution 绑定的 handler 集合不会在运行中被替换(`src-tauri/crates/lj-runtime/src/effect_registry.rs#L1-L5`)。

注册与查找的**唯一键是 Rule Contract 的 `EffectKind`**, 不再是 namespaced operation identity: `EffectRegistry::register_all(Vec<(EffectKind, EffectHandler)>)` 是原子批量注册, 重复的 effect kind(含同一批内重复)在写入任何 handler **之前**失败, 因此失败的注册不会留下半注册的 capability; 错误类型是 `EffectRegistryError::DuplicateEffectKind`(稳定码 `duplicate_effect_kind`)(`src-tauri/crates/lj-runtime/src/effect_registry.rs#L61-L83`)。`freeze()` 产出 `FrozenEffectRegistry`, 内部是 `BTreeMap<EffectKind, EffectHandler>`(`#L85-L100`)。

内置注册函数 `builtin::effects(http, quickjs, extract)` 直接返回三对 `(EffectKind, EffectHandler)`, 与 `EffectKind` 的三个变体一一对应; 因为不再需要解析 identity 字符串, 它不再返回 `Result`(`src-tauri/crates/lj-runtime/src/effect_registry/builtin.rs#L13-L27`)。

这一层曾经叫 `PluginHost` / `FrozenRegistry`, 带 `PluginManifest` 声明与 `HOST_CONTRACT_VERSION` 版本协商; 通用 plugin system 按 ADR 0004 第 7 节属一期明确不建设, 那套 contract 已随 `lj-plugin-contract` crate 删除(见「规则模型与合同」页)。

缺 handler 不会被容忍: live 分支按 `&declaration.kind` 在 frozen snapshot 里查不到 handler 时返回稳定的 `OperationUnavailable`, 不静默回退到别的 handler(`src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/effect_execution.rs#L99-L115`)。replay 分支**不需要** handler——历史 capture 不应因 registry 变化而失效(`#L116-L130`, 与 `#L101-L102` 的注释)。

## 启动校验与路径选择

`validate_plan` 是 Plan 进入执行前的最后一道门(`src-tauri/crates/lj-runtime/src/plan_runtime/validation.rs#L49-L106`):

- `compiler_version` 必须与 runtime config **完全一致**, 否则 `CompilerVersionMismatch`。
- 用 `execution_plan_hash` 重算 plan hash 并与 Plan 内 `plan_hash` 比对, 不一致即 `PlanHashMismatch`——这是对「Plan 是否被篡改或由不同 compiler 产出」的完整性检查。
- `definition_hash` 非空、`intent_entries` 非空、节点 ID 唯一且非 nil、边引用的节点存在且 semantic identity 不重复、每个 intent 的 `mapper_output` 必须是 Mapper、effect 声明与节点类型一致、Loop region 不嵌套, 最后对每个 intent 试算 `execution_path`。

`check_plan_support` 只回答一个问题: Plan 是否含 control flow(`has_control_flow()`), 从而给出 `PlanSupport::{Linear, ControlFlow}`(`src-tauri/crates/lj-runtime/src/plan_runtime/validation.rs#L40-L46`)。

`execution_path` 为请求的 intent 选出一段确定性的 control program(`#L300-L408`): 取「从 entry 前向可达」与「能反向到达 mapper」的交集作为选中节点集合, 校验 Loop 的 body 全部落在该集合内且不与其他 Loop 重叠, 再对 body 做拓扑排序得到 Loop 子程序, 最后把 outer 节点排序成顺序执行列表。这意味着**一次 execution 只执行与当前 intent 相关的子图**, 而不是整张 Flow。

## 调度与单终态

`EventEmitter` 保证一个 session 只发一个终态(`terminal_sent` 标志), sequence 从 1 递增; 事件发送失败(接收端已丢弃)只置 `receiver_gone` 并停止投递, **不隐式取消 execution**(`src-tauri/crates/lj-runtime/src/plan_runtime/scheduler.rs#L71-L120`)。

`run_execution` 的顺序是: 发 `Started` → 取 execution permit(等待期间也可被取消) → `execute_path` → 释放 permit → 发终态(`#L123-L163`)。`execute_path` 先给入口节点种下 `IntentInput`, 然后按 `path.node_ids` 顺序逐节点执行; 每个节点前检查取消; Loop 节点走 `execute_loop` 并在缺 control program 时报 `Internal`; 走完全程后若 Mapper 没有产出 Delta, 则以 `InputTypeMismatch` 失败(`#L165-L244`)。

## 控制流 routing

`execute_non_loop_node` 按节点 config 分派(`src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/routing.rs#L13-L118`):

| 节点 | 行为 |
| --- | --- |
| Http / Js / Extract | 从 `linear` 输入取上游值 → `execute_effect` → 把结果 route 到 `linear` 输出 |
| Mapper | `execute_mapper_node`: replay 下先校验 capture 完整, 再用 `MapperContext::map_plan_json` 产出 `DeltaProduced` |
| Condition | 取 `condition input` → 求分支(typed predicate 或 Js 表达式) → 把输入 route 到选中的 branch handle |
| Merge | 按显式 `order` 收集**已激活**的输入 → `merge_values` → 持久化 `ControlTrace::Merge{active_inputs}` → route 到 merge output |
| Loop(作为普通节点) | 直接以 `Internal` 失败: 嵌套 Loop runtime 未开放 |

Merge 的顺序语义落在 runtime: 物理存储顺序无关, 只有 `order` 决定收集顺序(`#L67-L100`)。Condition 的分支必须是 Plan 已声明的 branch handle, 否则失败。

## 有界循环

`execute_loop` 的约束是显式的(`src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/loop_execution.rs#L11-L77`):

- collection 必须是 JSON array, 否则 `InputTypeMismatch`。
- 迭代数必须同时不超过节点配置的 `max_iterations` 与全局常量 `MAX_LOOP_ITERATIONS`, 任一超出即 `InputTypeMismatch` 且**在开始 body 之前**失败。
- 迭代数写入 `ControlTrace::Loop { iteration_count }` 并走同一套 live/replay 持久化路径。
- 每个迭代构造 `LoopInvocationSegment`, body 节点按顺序执行; 抛出的结果 route 到 `LOOP_DONE_HANDLE`。

控制脚本不是直接 `eval`: `control_script(code)` 把作者代码包成受控 IIFE, 先尝试当作表达式求值, 语法错误时才退回当作语句块, 并在严格模式下以 `globalThis.input` 为唯一入参(`#L325-L330`)。

## effect 分派与 fingerprint

`execute_effect` 是 live/replay 的分叉点(`src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/effect_execution.rs#L12-L135`), 顺序为:

1. 取消检查;
2. `enforce_capabilities`——Plan 声明的能力必须落在安装 grant 内, 否则 `CapabilityDenied`;
3. 取 effect ID 与取消 token, **先等来源级 permit 再等全局 permit**(注释写明这是为了避免同一来源排队的 effect 占住全局 permit 阻塞其他来源);
4. 再次取消检查;
5. 计算 fingerprint;
6. Live → 按 `declaration.kind` 从 frozen registry 解析 handler; Replay → 走 archive。

`effect_fingerprint` = hash(`plan_hash` + `invocation_path` + `kind` + `config_hash` + `input_hash`)(`src-tauri/crates/lj-runtime/src/plan_runtime/validation.rs#L463-L483`)。它防的是**调用错位**: archive 里的一条记录只对「同一份 Plan、同一调用序号、同一节点配置、同一输入」成立, 换 Plan、加一次循环迭代、改动脚本或输入都会让 fingerprint 变化, 从而拒绝把旧记录复用到新调用上。fingerprint 只含 hash, 不含 payload。

## live: durable-before-advance

live 分支的次序是被刻意固定的(`src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/live_capture.rs#L10-L122`):

```text
invoke live handler
  → EffectCapture::from_live(...)        构造 witness 摘要, 失败即 CaptureWitnessInvalid
  → 校验 kind 与 JS 输出形状              失败即 CaptureWitnessInvalid
  → archive.persist_durable(capture)     失败即 CaptureFailed
  → receipt_matches(...)                 失败即 CaptureReceiptMismatch
  → 才 emit EffectCaptured
  → 下游才能读到 output
```

注释把理由写得很直接: 已经发生的外部 effect 必须完成 commit/rollback, 收据匹配前既不能断言成功, 也不能让下游读取输出。因此「先做网络请求、后写 archive」的窗口被压缩到不可观测。

## replay: 只读 archive, 绝不回退 live

replay 分支只从 archive 读, 并逐层比对(`src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/replay.rs#L12-L125`):

| 检查 | 失败码 |
| --- | --- |
| 记录存在 | `ReplayCaptureMissing` |
| `execution_id` / `invocation_path` / `node_id` / kind / 输出形状与声明一致 | `ReplayRecordMismatch` |
| `fingerprint` 与当前调用一致 | `ReplayFingerprintMismatch` |
| 记录的 `output_hash` 与输出内容重算结果一致 | `ReplayOutputHashMismatch` |
| `validate_replay_integrity()` 通过, 且 witness 重新绑定当前 Plan+input | `ReplayWitnessMismatch` |

任何一条不匹配都是硬失败, 代码注释明确「绝不调用 live adapter 补救」(`#L88-L100`)。控制轨迹走同样的双路径: live 时 `persist_control_trace` 并校验 receipt(invocation path + trace hash), replay 时 `load_control_trace` 后逐字段比对(`src-tauri/crates/lj-runtime/src/plan_runtime/scheduler/control_archive.rs#L16-L137`)。

## witness 能留下什么

`effect/witness.rs` 的模块注释定义了边界: witness 只保留可安全持久化的 URL、IP、hash、时序与 QuickJS host-call 元数据, **永远不含 query、body、cookie、token 或 authorization**(`src-tauri/crates/lj-runtime/src/effect/witness.rs#L1-L5`)。

`HttpRequestWitness` 里的 `safe_url` 明确规定只能包含 scheme、host、port 与 path, 禁止 query、fragment 与 userinfo; `headers` 只保留非敏感 header 的稳定摘要, cookie/token/authorization 不得出现(`#L29-L40`)。`HttpDnsTargetKind` 还记录了目标地址的来源(PinnedDns / IpLiteral / DirectHost), 使「这次请求经过 SSRF pin 还是走直连」在 replay 时可判定(`#L17-L27`)。

witness 的作用因此是双重的: 让 replay 能证明「当时确实以这种方式调用了」, 又不把凭据带进可持久化的记录。

## 取消与并发

取消是协作式的, 语义在 `effect/cancellation.rs` 顶部写死: 取消首先阻止新 effect 调度; **已经进入 archive 事务的 capture 必须自行完成 commit 或 rollback**, 不能在中间丢弃已发生的外部副作用; token 只传递状态, 不携带 HTTP client 或 QuickJS 非 `Send` 句柄, 因此可安全跨 blocking lane 使用(`src-tauri/crates/lj-runtime/src/effect/cancellation.rs#L1-L5`)。

`CancellationHandle::cancel()` 用 `compare_exchange` 保证幂等并返回「是否首次改变状态」(`#L33-L48`); handler 侧拿到的 `EffectCancellation::cancelled()` 是可 await 的通知, 供 HTTP future 或 watchdog 使用(`#L74-L95`)。

live effect 在发出 `EffectCaptured` **之后**才检查取消: 若此时已取消则返回 `Cancelled`, 但捕获记录已经落盘(`live_capture.rs#L97-L121`)。这保证了「已发生的副作用可复现」优先于「尽快停止」。

并发上限集中在 config: `max_concurrent_executions`、`max_concurrent_effects`、`max_concurrent_effects_per_source`, 加 `event_channel_capacity`(`src-tauri/crates/lj-runtime/src/plan_runtime/api.rs#L25-L36`)。来源级 permit 表按 `source_id` 持有 `Weak<Semaphore>` 并在分配时清理失效项, 因此来源数量不会让这张表无限增长(`#L431-L443`)。

## 适配层: 三个 crate 各管一类 effect

adapter 实现分别放在独立 crate, 都实现 runtime 定义的 typed trait(`src-tauri/crates/lj-runtime/src/effect/contracts.rs#L295-L341`):

| effect | 实现 | 关键约束 |
| --- | --- | --- |
| Http | `HttpEffectAdapter`(`src-tauri/crates/lj-node-http/src/processor/adapter.rs#L55-L75`) | 先查 `Network` capability; 测试构造器可关闭 SSRF 防护 |
| QuickJS | `QuickJsEffectAdapter`(`src-tauri/crates/lj-node-js/src/processor.rs#L51-L120`) | 内存上限 16 MB、超时 5000 ms; 所有 `rquickjs` 对象只在 `spawn_blocking` 内创建与销毁; watchdog 轮询取消 token 并触发 QuickJS interrupt |
| Extract | `ExtractEffectAdapter`(`src-tauri/crates/lj-node-extract/src/processor.rs#L20-L40`) | 直接借用上游 HTTP body 提取, 不为 Plan runtime 的 `Arc` 输出复制 body |

`EffectOutput` 是闭集 `Http` / `QuickJs` / `Extract` / `Failure`, 其中 `Failure` 表示「已发生并需要 durable capture 的安全失败, 不伪造协议响应」; `EffectInput` 是 `Intent` / `Output(Arc<..>)` / `Json(Arc<..>)`, 用 `Arc` 避免在分支边上复制 HTTP body(`src-tauri/crates/lj-runtime/src/effect/contracts.rs#L110-L155`)。

`EffectError::message` 的合同是「面向诊断, 不能包含 cookie、token、完整 URL query 或 body」(`#L64-L73`)。QuickJS 的**脚本执行失败**会作为 `QuickJsOutput::Error` 归档以便 replay 复现同一失败, 而取消与 capability 拒绝返回 `EffectError`, 因为它们没有可推进的 live 结果(`#L311-L336`)。

## 测试契约

runtime 的测试按关注点分文件, 断言的是可观察行为而不是内部结构:

| 文件 | 代表断言 |
| --- | --- |
| `replay_contract.rs` | compiler 产出的 Plan 通过 runtime hash 校验; 篡改 plan hash 或 compiler 身份被拒; live 与 replay 保持 typed 输出且不回退 live; 缺 capture、output hash 不符、fingerprint 不符、篡改 JS/Extract witness 都是硬失败 |
| `control_contract.rs` | typed condition/merge/loop 执行选中分支并能精确 replay; 缺/多/篡改 control invocation 都不调用 live handler; Merge 在物理输入倒序时仍按显式 order; Loop 的空集合/超限/非数组/body 失败都有类型化边界; 全局 hard max 是包含式且溢出时**不启动任何 body effect**; 第一个迭代内取消会停止后续所有 body effect |
| `scheduling_contract.rs` | 取消只发 `Cancelled` 且不再产生新 effect; 来源级与全局 semaphore 的阻塞行为; frozen registry 缺 effect kind 时以 `OperationUnavailable` 稳定失败; 有界事件通道在下游 effect 之前施加背压 |
| `effect_registry_test.rs` | 重复 effect kind(含同一批内重复)被拒且失败注册不留半 capability; frozen registry 只能解析已注册的 kind; `builtin::effects` 覆盖三个 `EffectKind` |
