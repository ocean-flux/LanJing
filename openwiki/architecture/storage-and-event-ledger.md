---
type: "参考"
title: "Storage and event ledger"
openwiki_generated: true
sources:
  - id: openwiki-source-0942045685304146bceb6d57
    resource: repo://src-tauri/crates/lj-storage-entity/src/lib.rs
  - id: openwiki-source-8deae80dadb5d559b8894acb
    resource: repo://src-tauri/crates/lj-storage-migration/src/custom_schema.rs
  - id: openwiki-source-c33d51acf739e25ca33c8b1d
    resource: repo://src-tauri/crates/lj-storage-migration/src/lib.rs
  - id: openwiki-source-ab65860c28a9864fc20becd7
    resource: repo://src-tauri/crates/lj-storage-migration/src/schema_metadata.rs
  - id: openwiki-source-25f9975ec0abf8c6031e92de
    resource: repo://src-tauri/crates/lj-storage/src/artifact.rs
  - id: openwiki-source-b395647d8599e3994fb54f71
    resource: repo://src-tauri/crates/lj-storage/src/candidate_install.rs
  - id: openwiki-source-0edcca0fe903e3375ebb6b15
    resource: repo://src-tauri/crates/lj-storage/src/connection.rs
  - id: openwiki-source-bd620e0389d5800fc7208ba7
    resource: repo://src-tauri/crates/lj-storage/src/database.rs
  - id: openwiki-source-3a3fdfb5a00399ef7859b900
    resource: repo://src-tauri/crates/lj-storage/src/keyring_init.rs
  - id: openwiki-source-eb76945a7b81744f72bbb438
    resource: repo://src-tauri/crates/lj-storage/src/repository/candidate_source/contract.rs
  - id: openwiki-source-5a4e9d601d982611144a723a
    resource: repo://src-tauri/crates/lj-storage/src/repository/candidate_source/install.rs
  - id: openwiki-source-c43839edf96a2611e50704fd
    resource: repo://src-tauri/crates/lj-storage/src/repository/candidate_source/mod.rs
  - id: openwiki-source-4815e4a3e566bd659f70abed
    resource: repo://src-tauri/crates/lj-storage/src/repository/candidate_source/read.rs
  - id: openwiki-source-805fcc595e4a29b21c7df917
    resource: repo://src-tauri/crates/lj-storage/src/repository/candidate_source/staging.rs
  - id: openwiki-source-aaaa1af7d45207ac8a646f67
    resource: repo://src-tauri/crates/lj-storage/src/repository/document.rs
  - id: openwiki-source-04df110324feb1bf7e2d0443
    resource: repo://src-tauri/crates/lj-storage/src/repository/event/mod.rs
  - id: openwiki-source-7a101686bef533b44db33025
    resource: repo://src-tauri/crates/lj-storage/src/repository/maintenance.rs
  - id: openwiki-source-c2eb95ea8633875bc0a7ab72
    resource: repo://src-tauri/crates/lj-storage/src/repository/maintenance/recovery.rs
  - id: openwiki-source-34684cb3b78dd5c3e322ffc5
    resource: repo://src-tauri/crates/lj-storage/src/repository/mod.rs
  - id: openwiki-source-612028990ad235f913b145bb
    resource: repo://src-tauri/crates/lj-storage/src/repository/projection/read.rs
  - id: openwiki-source-b03c20565000a097dbf231cf
    resource: repo://src-tauri/crates/lj-storage/src/repository/secret.rs
  - id: openwiki-source-2256ade68852111694ed64ef
    resource: repo://src-tauri/crates/lj-storage/src/storage.rs
  - id: openwiki-source-9653aa1d8d274ae45eae1e3f
    resource: repo://src-tauri/crates/lj-storage/src/storage/query.rs
  - id: openwiki-source-7be932d6889e1346d9eafbd5
    resource: repo://src-tauri/crates/lj-storage/src/transaction/candidate.rs
  - id: openwiki-source-4f69dc8c710ab3da1c12053d
    resource: repo://src-tauri/crates/lj-storage/src/transaction/mod.rs
  - id: openwiki-source-f9f9422ec8101aa005a38079
    resource: repo://src-tauri/crates/lj-storage/src/types/config.rs
  - id: openwiki-source-7d4ed639b034234af79f8973
    resource: repo://src-tauri/crates/lj-storage/src/types/execution.rs
  - id: openwiki-source-0270102debfccc7de7fb79b9
    resource: repo://src-tauri/crates/lj-storage/src/writer.rs
generated: { by: "pi", at: "2026-10-05T08:37:16.392Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-05T08:37:16.392Z
---


## 职责与配置

`EventProjectionStorage` 是 single-writer Event Store、规范化投影与 durable archive 的具体 façade, 内部只有三样东西: 一个 writer 命令通道、一个只读连接池、一个 `ArtifactStore`(`src-tauri/crates/lj-storage/src/storage.rs#L25-L31`)。

`StorageConfig` 描述真实文件路径、keyring service、artifact 配额、archive TTL 与读连接上限(`src-tauri/crates/lj-storage/src/types/config.rs#L15-L30`)。桌面默认值是 quota 2 GiB、archive TTL 30 天、`read_concurrency` 4; 移动端只把 quota 降到 512 MiB(`#L32-L53`)。`validate_config` 明确禁止 `:memory:`, 并要求 `read_concurrency >= 1`, 同时预建数据库父目录与 artifact 根目录。

writer 队列容量是常量 `WRITER_CAPACITY = 256`, 由 `EventProjectionStorage::writer_capacity()` 对外暴露(`src-tauri/crates/lj-storage/src/types/config.rs#L8-L9`, `src-tauri/crates/lj-storage/src/storage.rs#L72-L76`)。

## 打开流程

`EventProjectionStorage::open` 的顺序是(`src-tauri/crates/lj-storage/src/storage.rs#L39-L70`):

```text
validate_config
  → open_connection(path, migrate = true)     迁移 + schema 自检 + PRAGMA
  → ArtifactStore::new(root, keyring_service)
  → 启动期恢复(顺序固定):
       normalize_artifact_relative_paths   路径归一化
       candidate::recover                  candidate 恢复
       validate_secret_ref_counts          secret 引用一致性
       purge_zero_ref_artifacts            零引用 artifact 清理
       purge_zero_ref_secrets              零引用 secret 清理
       recover_orphans_sync                orphan 清理
       mark_interrupted_executions        未完成 execution 收敛
  → connect_pool(read_concurrency)            只读池
  → spawn writer_loop(容量 256)               唯一 writer
```

`open_connection` 在迁移路径上先跑 `bootstrap_current_schema`, 然后设置 `journal_mode = WAL`、`synchronous = NORMAL`、`wal_autocheckpoint = 1000`; 所有连接(含只读)都会设置 `busy_timeout = 2000` 与 `foreign_keys = ON`(`src-tauri/crates/lj-storage/src/connection.rs#L8-L42`)。schema 自检失败会被折叠成 `StorageError::CurrentSchemaRequired`, 即**启动时失败而不是带着未知结构继续跑**。

## schema 与迁移自检

`lj-storage-migration` 注册三个迁移(`src-tauri/crates/lj-storage-migration/src/lib.rs#L15-L26`):

| 版本 | 迁移 | 作用 |
| --- | --- | --- |
| v1 | `m20260729_000001_current_schema` | baseline: 要求空库后建全部表, 再应用自定义索引/CHECK 触发器并写入计数器种子 |
| v2 | `m20260826_000002_rule_document_effective_semantics` | 建 `rule_document_effective_semantics`, 从旧语义表**只回填通过 compiler 校验**的快照 |
| v3 | `m20260830_000003_rule_document_effective_history` | 建 `rule_document_effective_semantic_history` 及索引/触发器, 复制 effective 内容 |

`bootstrap_current_schema` 先分类再行动(`src-tauri/crates/lj-storage-migration/src/lib.rs#L43-L58`):

- `Empty`(库中无任何对象) → 直接 `Migrator::up`;
- `Current`(存在 `storage_schema_metadata` 标记表) → 先 `validate_upgrade_path`, 再 `Migrator::up`;
- `Unknown`(有对象但没有标记表) → `MigrationError::CurrentSchemaRequired`, 拒绝继续。

最后一定跑 `validate`: marker 必须是 `lanjing_current` 且版本等于 3, 三条迁移记录齐备, 且 **marker 里的 fingerprint 必须等于当前 `sqlite_master` 的重算结果**(`src-tauri/crates/lj-storage-migration/src/schema_metadata.rs#L167-L190`, `#L225`)。fingerprint 是对全部 schema 对象的 BLAKE3, 因此**未知 fingerprint 的后果是拒绝启动**(`SchemaCorrupt`), 而不是尝试修复或忽略——`unknown_or_tampered_schema_is_rejected` 测试正是通过删掉一个索引来触发这条路径(`src-tauri/crates/lj-storage-migration/src/lib.rs#L280-L304`)。

自定义 schema 部分(`custom_schema.rs`)带 24 个索引与逐表 CHECK 触发器: 每个表生成 `ck_<table>_insert` / `ck_<table>_update`, 违反即 `RAISE(ABORT, 'current schema check failed')`, 并插入 `event_counters(id=1, next_global_seq=0)` 种子(`src-tauri/crates/lj-storage-migration/src/custom_schema.rs#L6-L31`, `#L33-L113`, `#L108-L129`)。这套 SQL 级约束是第二道防线: 即使代码有 bug, 非法行也进不了库(`current_checks_reject_invalid_rows` 断言了这一点)。

## 表与实体

`lj-storage-entity` 的 `schema_builder()` 注册 31 个 Entity, 是 baseline 建表的唯一清单(`src-tauri/crates/lj-storage-entity/src/lib.rs#L57-L93`), 按六组划分:

| 组 | 内容 |
| --- | --- |
| core | `storage_schema_metadata`、`event_counter`、`event_stream`、`event` |
| artifact | `artifact_metadata`、`event_artifact_ref`、`vault_key_metadata`、`secret_artifact`、`secret_artifact_owner` |
| lifecycle | candidate、source、source version、execution |
| archive | invocation ledger、effect capture、control trace、source checkpoint、library checkpoint |
| projection | source/item/collection/unit/asset/relation/action/hint/library |
| document | rule document 与其 semantic/layout/provenance 三张从属表 |

再加 v2、v3 各引入的一张 effective 语义表, 迁移后的业务表总数为 33(fresh baseline 测试断言 33 且重开后不变, `src-tauri/crates/lj-storage-migration/src/lib.rs#L164-L176`)。

## 分层: 谁拥有哪个 aggregate

| 层 | 职责 |
| --- | --- |
| `lj-storage-entity` | current SeaORM relational model, 不含行为 |
| `repository/*` | 按 aggregate 的数据库读写 owner: archive / candidate_source / document / event / execution / maintenance / projection / secret(`src-tauri/crates/lj-storage/src/repository/mod.rs#L1-L10`) |
| `transaction/*` | 跨表不变量与原子写; 所有操作统一走 `connection.immediate_transaction`(`src-tauri/crates/lj-storage/src/transaction/mod.rs#L1-L19`) |
| `storage/*` | 分域 façade: archive / candidate / document / execution / maintenance / query(`src-tauri/crates/lj-storage/src/storage.rs#L1-L8`) |
| `writer.rs` | 只在 crate 内传递的 `WriterCommand` 枚举, 不进入对外 API(`src-tauri/crates/lj-storage/src/writer.rs#L5-L65`) |

`WriterCommand` 把写操作枚举化(`Append` / `StageCandidate` / `InstallCandidate` / `StartExecution` / `StartReplayExecution` / `CommitDelta` / `FinishExecution` / `SetExecutionPin` / `UpdateLibrary` / `PersistEffect` / `PersistControlTrace` 等), 使「所有写都经过一个队列」在类型上可见。

## Candidate 与 source revision: 一次 composite publish, 一次原子消费

`repository/candidate_source` 拥有 candidate/source 的 SQL 与 row mapping, 内部按职责拆成四个模块: `contract.rs`(候选合同校验与 stream/owner id 生成)、`staging.rs`(composite publish 与回退候选)、`install.rs`(原子消费 candidate 并追加权威 source revision)、`read.rs`(candidate summary、installed source、revision 列表、启动恢复与 TTL 过期)(`src-tauri/crates/lj-storage/src/repository/candidate_source/mod.rs#L1-L6`)。crate-private 的读入口由 `src-tauri/crates/lj-storage/src/candidate_install.rs#L1-L8` 再导出, 写路径统一经 `transaction::candidate`, 四个入口 `stage` / `stage_source_rollback` / `install` / `recover` 各自包一个 `transaction::run`(`src-tauri/crates/lj-storage/src/transaction/candidate.rs#L10-L77`)。

staging 的顺序是**先落文件, 再落行**: 先 `write_secret` 写随机 locator 的 runtime credential 文件, 再写 package 与 Plan 两个内容寻址 artifact, 算出覆盖 candidate_id / source_identity / runtime secret id / `expected_installed_revision` / 两个 artifact hash / `definition_hash` / `plan_hash` / profile / grant / diagnostics / expiry 的 `candidate_contract_hash`, 最后在**同一个 Event transaction** 里追加 `candidate` 事件、`retain_pending_secret` 并插入 `candidate_projection` 行(`src-tauri/crates/lj-storage/src/repository/candidate_source/staging.rs#L2-L95`)。contract hash 把「候选这一整份承诺」折叠成一个值, 因此后续任何一项被替换都会在 install 时暴露。回退候选 `process_stage_source_rollback` 走同一发布形状, 只从不可变 source revision 复制受控 artifact 与 secret 内容。

install 在同一个 Event transaction 里「先验证完再消费」(`src-tauri/crates/lj-storage/src/repository/candidate_source/install.rs#L2-L32`):

1. 候选行缺失 → `CandidateMissing`; `expires_at_ms <= occurred_at_ms` → 先 `expire_candidate` 再返回 `CandidateExpired`(`#L5-L12`)。
2. status/schema 校验 + `validate_staged_candidate_event` 重读第 1 版 candidate 事件(`#L13-L14`)。
3. 当前 source revision 与候选记录的 `expected_installed_revision` 不等 → `CandidateStale`, 所以「候选期间来源被改过」不会静默覆盖(`#L17-L26`)。
4. `ensure_candidate_secrets` 逐个确认 secret 文件与 owner 仍在, 再从 artifact 重读并校验 package 与 Plan(`#L28-L30`)。
5. `grant_covers(&request.grant, &required_grant)` 不成立 → `GrantInsufficient`; 授权是 install 调用方给出的输入, 不信任候选自述(`#L31-L32`)。
6. 以 `expected_version = expected_installed_revision` 打开 source stream, 追加 `installed` / `updated` 事件并在同一事务里投影 source revision(`#L35-L111`)。

第 1-5 步全部在 `append_event_transaction` 之前返回, 所以失败不会留下半条 source revision; 同一 `event_id` 的重试由 `idempotent_event` 直接返回既有 revision 而不是写第二条(`install.rs#L58-L62`)。

篡改检测落在 `validate_staged_candidate_event` 上: 它重新读回 candidate stream 的第 1 版事件, 重算 `candidate_contract_hash`, 并逐项比对 event_id / source_identity / event_type / schema_version / payload 里的 `kind`/`contract_hash`/`candidate_schema_version`/`expires_at_ms`、artifact 引用集合、codec 必须是 `zstd`、secret 引用必须为空; 任一项不符即 `CandidateTampered`(`src-tauri/crates/lj-storage/src/repository/candidate_source/install.rs#L323-L400`)。也就是说候选的完备性不是「同一次 writer 调用内」的自证, 而是下次读回来仍然成立。

包与 Plan 的自洽性由 `validate_candidate_package_and_plan` 守住: `definition_hash(package.definition())` 必须等于 Plan 的 `definition_hash`, Plan 的 `plan_hash` 必须等于 `canonical_plan_hash(plan)`, 且 package 的 source identity 与内嵌 definition 一致(`src-tauri/crates/lj-storage/src/repository/candidate_source/contract.rs#L52-L67`)。`grant_covers` 是同一文件里的授权覆盖判定。

过期与启动清理只有一条路径: `recover_candidates_sync` 按 `created_at_ms` 扫全部 candidate, 把 schema 不符、状态非 `staged` 或已过期的候选交给 `expire_candidate`; `expire_candidate` 释放 secret owner 引用、移除事件引用并删除 `candidate_projection` 行(`src-tauri/crates/lj-storage/src/repository/candidate_source/read.rs#L187-L227`)。这与 `open` 启动顺序里的 `candidate::recover` 是同一份实现。

## 事件账本是唯一真相来源

`events` 表是 append-only 账本, 其唯一索引 `idx_events_stream_version` 保证同一 stream 的版本号不重复; `global_seq` 由 `event_counters.next_global_seq` 单调分配(`src-tauri/crates/lj-storage-migration/src/custom_schema.rs#L7`, `src-tauri/crates/lj-storage/src/repository/event/mod.rs#L421-L435`)。

`append_event_transaction` 是一个事务内的固定次序(`src-tauri/crates/lj-storage/src/repository/event/mod.rs#L100-L164`):

```text
1. 读当前 stream_version, 与 draft.expected_version 比对 → VersionConflict
2. stream_version + 1, 分配下一个 global_seq
3. INSERT events(global_seq, stream_id, stream_version, event_type, payload_json,
                  artifact_refs_json, secret_refs_json, ...)
4. UPSERT event_streams(stream_id, version)
5. attach_artifacts(...)   认领 artifact/secret 引用
6. projection(conn, global_seq, stream_version)   调用方传入的投影闭包
7. 返回 CommitReceipt
```

关键点是**它自己不 commit**: 事务边界在调用方(`transaction::run` 的 `BEGIN IMMEDIATE`), 投影闭包在同一事务里执行。这就是「投影是事件的物化视图」能够成立的原因——事件与投影要么同时生效, 要么都不生效, 不存在「事件写了但投影没写」的中间态。乐观并发用 `expected_version` 表达, 冲突即 `VersionConflict`, 不做隐式合并。

读路径完全绕开事件重放: 查询走只读连接读 `projection_*` 表里已物化的 `payload_json`(`src-tauri/crates/lj-storage/src/storage/query.rs#L23-L39`)。只有 replay 与 catch-up 才读事件账本本身。

## 单 writer 为什么能同时保证单调与不阻塞

写入全部经 `mpsc`(容量 256)派发到唯一 `writer_loop`, 由它持有一个可写连接串行执行; 读取走 `read_concurrency`(默认 4)个独立只读连接(`src-tauri/crates/lj-storage/src/storage.rs#L51-L70`, `#L100-L123`)。

这带来两个性质:

- **账本单调**: `global_seq` 的分配是 `UPDATE ... + 1` 再读回, 而这只会发生在唯一 writer 的事务里, 所以不存在两个写者竞争同一序号导致乱序或跳号。
- **查询不阻塞**: 只读池用 `mode=ro` 连接(`src-tauri/crates/lj-storage/src/database.rs#L146-L152`), WAL 模式允许读事务与写事务并发推进, 读的是已提交快照而不是等待写锁。

`BEGIN IMMEDIATE` 的用途也在这里: 它在事务开始时就取得写锁, 避免「先读后写」的事务在提交阶段才升级锁从而与其他写者死锁。代价是写事务不能长时间持有, 所以所有跨表工作都组织成短事务。

`library_projection_sync` 示范了读侧的形状: 在同一个只读事务里同时取 `global_seq` 与全部 entries, 并用 SQL 排序 `pinned DESC, favorite DESC, last_opened_at IS NULL ASC, last_opened_at DESC, resource_id ASC`——即「置顶优先、收藏次之、未打开排后、最近打开靠前」的稳定顺序由数据库保证, 而不是前端二次排序(`src-tauri/crates/lj-storage/src/repository/projection/read.rs#L1-L24`)。

## 凭证: keyring + AES-256-GCM + owner 引用计数

存储侧的凭证机制有四层:

1. **keyring store 安装**: 进程内若已有 keyring-core 默认 store 则复用, 否则按平台安装(Windows native / Apple keychain / Android / zbus secret service); 锁定返回 `KeyringLocked`, 不可用返回 `KeyringUnavailable`(`src-tauri/crates/lj-storage/src/keyring_init.rs#L34-L45`)。
2. **vault key**: 随机 AES-256 密钥, 以 hex 存平台 secure store; `key_verifier` 用带域分离前缀的 BLAKE3 计算独立 verifier, 注释明确该值不能用于推导任何明文(`src-tauri/crates/lj-storage/src/artifact.rs#L127-L137`)。
3. **secret 文件**: `write_secret` 生成随机 `SecretArtifactId` 与随机 blob locator(不是内容 hash), envelope 为 `[version=2][12B nonce][ciphertext]`, `ciphertext_hash = BLAKE3(envelope)`; 读取时先校验 hash 再解密(`src-tauri/crates/lj-storage/src/artifact.rs#L140-L171`, `#L172-L196`)。注释点出了设计动机: ciphertext hash 覆盖带随机 nonce 的 envelope, 因此不对明文形成可预计算 oracle。
4. **SQLite 只存引用**: 数据库里只有随机 `SecretArtifactId`、随机 locator、`key_id` 与 ciphertext hash(`src-tauri/crates/lj-storage/src/repository/secret.rs#L1-L5`)。

`owner row 是 ref-count 的唯一证明`(同文件注释)。`retain_pending_secret` / `retain_existing_secret` / `release_secret_owner` 同时维护 owner 行与 `secret_artifact_projection.ref_count`, owner 冲突返回 `SecretOwnershipMismatch`(`src-tauri/crates/lj-storage/src/repository/secret.rs#L108-L227`)。最关键的是 `read_owned_secret`: 只有调用方给出的 owner(`owner_kind` + `owner_id`)恰好认领了该 secret_id 才解密, 否则返回 `SecretOwnershipMismatch`——这正是「防止跨记录 ID 替换形成 secret-ref oracle」的实现(`#L247-L260`)。

文档侧复用同一机制, owner kind 为 `native_rule_document`(provenance 原文)与 `native_rule_document_credential`(凭证槽位)(`src-tauri/crates/lj-storage/src/repository/document.rs#L19-L24`)。

内存侧同样有约束: `ExecutionSourceCredentials` 保存 `cookie_namespace` 与可选 `secret_bytes`, 但**不实现 `Debug`、`Clone` 或 serde**, 防止 secret 进入日志、DTO 或默认导出; 提供 `into_secret_bytes()` 把所有权交给执行适配器以避免复制(`src-tauri/crates/lj-storage/src/types/execution.rs#L15-L45`)。

## 保留策略与启动恢复

- archive 保留: `DEFAULT_ARCHIVE_TTL_MS = 30 天`, candidate 默认 24 小时(`src-tauri/crates/lj-storage/src/types/config.rs#L10-L13`)。
- GC 是有状态可恢复的单向流程: `active → marked → external_refs_removed → finalized`, checkpoint 与引用删除都有持久状态支撑幂等恢复(`src-tauri/crates/lj-storage/src/repository/maintenance.rs#L1-L3`)。
- 启动恢复覆盖六类路径: artifact 相对路径归一化、候选恢复、secret 引用计数校验、零引用 artifact 与 secret 清理、orphan 清理、以及把「重启前未完成的 execution」收敛为终止状态(`src-tauri/crates/lj-storage/src/storage.rs#L43-L49`, `src-tauri/crates/lj-storage/src/repository/maintenance/recovery.rs#L1-L9`)。

这套恢复的存在解释了为什么 `open` 在建立 writer 之前就把恢复做完: 恢复必须在没有任何并发写者的情况下进行, 否则「零引用清理」会与正在进行的引用认领竞争。

## 测试契约

| 测试 | 断言 |
| --- | --- |
| `fresh_baseline_builds_33_tables_and_reopens` | 新库建 33 张业务表, 再次 bootstrap 幂等 |
| `baseline_down_up_rebuilds_current_schema` | baseline 可 down/up 重建 |
| `v1_semantic_snapshot_migrates_to_effective_snapshot` | v1→v2 回填只保留通过 compiler 校验的快照, 非法快照被跳过 |
| `unknown_or_tampered_schema_is_rejected` | 未知 schema 报 `CurrentSchemaRequired`, 被篡改 schema 报 `SchemaCorrupt` |
| `current_checks_reject_invalid_rows` | CHECK 触发器拒绝非法行并抛 `current schema check failed` |
| `invocation_uniqueness_is_scoped_to_execution` | invocation 路径/序号/payload 的唯一性只在单个 execution 内要求, 跨 execution 允许相同 |

最后一组断言了一个容易搞错的边界: 重放标识的唯一性是 **per-execution** 的, 因此同一份 Plan 在不同 execution 里可以出现完全相同的 invocation 路径。
