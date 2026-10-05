---
type: "参考"
title: "Source install and update"
openwiki_generated: true
sources:
  - id: openwiki-source-9faf227efd814ea6139f8535
    resource: repo://docs/adr/0002-versioned-source-and-rule-lifecycle.md
  - id: openwiki-source-f526e068025e496e4b7dd5f1
    resource: repo://src-tauri/crates/lj-importer/src/imported_rule.rs
  - id: openwiki-source-ad6f13743e3311f7bddf3fbb
    resource: repo://src-tauri/crates/lj-importer/src/legado/mod.rs
  - id: openwiki-source-d50d2b8ac95f0636ecbbb93c
    resource: repo://src-tauri/crates/lj-importer/src/maccms/mod.rs
  - id: openwiki-source-fdb79c5a066f5001878b9aee
    resource: repo://src-tauri/crates/lj-importer/src/strict_json.rs
  - id: openwiki-source-980fd459aadb6afbaa667438
    resource: repo://src-tauri/crates/lj-integration-tests/tests/maccms_json_rule_system.rs
  - id: openwiki-source-aa1c93c7ddf499b5db0c1905
    resource: repo://src-tauri/crates/lj-rule-system/src/system.rs
  - id: openwiki-source-32d4001fffac613566845710
    resource: repo://src-tauri/crates/lj-rule-system/src/system/error_mapping.rs
  - id: openwiki-source-f96d083a6d8ce40c80cdd95a
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/package.rs
  - id: openwiki-source-80e8762cff162b2ecd16e129
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/prepare_install.rs
  - id: openwiki-source-0e77489563e28087cde37771
    resource: repo://src-tauri/crates/lj-rule-system/src/types/candidate.rs
  - id: openwiki-source-a7ae0a93a20e741aa0b31f5b
    resource: repo://src-tauri/crates/lj-rule-system/src/types/config.rs
  - id: openwiki-source-5a4e9d601d982611144a723a
    resource: repo://src-tauri/crates/lj-storage/src/repository/candidate_source/install.rs
  - id: openwiki-source-4815e4a3e566bd659f70abed
    resource: repo://src-tauri/crates/lj-storage/src/repository/candidate_source/read.rs
  - id: openwiki-source-805fcc595e4a29b21c7df917
    resource: repo://src-tauri/crates/lj-storage/src/repository/candidate_source/staging.rs
  - id: openwiki-source-132e0f0f697fcf351d1cdaf3
    resource: repo://src/features/sources/SourceInspector.tsx
  - id: openwiki-source-a44323d5e2cf8ef0e2280c78
    resource: repo://src/features/sources/SourceInstallDialog.tsx
  - id: openwiki-source-5a4431c711fe83d06dbf05b6
    resource: repo://src/features/sources/SourcesHome.tsx
  - id: openwiki-source-c79f60033c1e57537cf64e77
    resource: repo://src/features/sources/workflow.ts
  - id: openwiki-source-3975011ba287abb3ee532bf1
    resource: repo://src/shared/tauri/catalog.ts
  - id: openwiki-source-dd8aa1c4837bde66e5f32f6a
    resource: repo://src/shared/tauri/deep-link-parse.ts
  - id: openwiki-source-938e4156b9e76517514c2370
    resource: repo://src/shared/tauri/sources.ts
generated: { by: "pi", at: "2026-10-05T08:37:16.392Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-05T08:37:16.392Z
---


## 入口: 同一条输入通道

ADR 0002 第 1 条要求「解析格式不是用户先选的导航步骤」(`docs/adr/0002-versioned-source-and-rule-lifecycle.md#L12`), 因此前端只有一个输入区: 粘贴文本、拖入/选择文件、或直接输入 URL(`src/features/sources/SourceInstallDialog.tsx#L274-L307`)。输入形态的判断在 workflow 里做——`prepareInput` 用 `isHttpUrl` 分流(`src/features/sources/workflow.ts#L89-L96`、`#L201-L270`):

- http(s) URL → 直接以 `{ kind: 'maccms_json', url }` prepare 一个候选, 跳过 pick 直通 confirm;
- 以 `{` / `[` 开头的文本 → 经 `parseBookSourceCatalog` 解析成 catalog(可能含多个书源)进 pick;
- 其他文本 → `input_unrecognized`。

注意 JSON 文本只按 **Legado 书源**解析: Rule Package 是后端 `RuleInput::Package` 支持的另一条输入通道(`src-tauri/crates/lj-rule-system/src/types/candidate.rs#L24-L28`), 前端没有对应入口(见末节)。

deep link 是第二条入口, 解析全在前端(`src/shared/tauri/deep-link-parse.ts#L1-L12`):

| scheme | 形式 | 结果 |
| --- | --- | --- |
| `legado` / `yuedu` | `legado://import/bookSource?src=<http url>`、`…/booksource/importonline?src=`、`legado:///import/bookSource?src=` | `{ kind: 'install', src }`, 要求 src 是**不含 userinfo 的 http(s) URL** |
| `legado` / `yuedu` | 其他路径 | `reject('unsupported-import')` |
| `lanjing` | `lanjing://source/<id>` / `lanjing://item/<id>` | `{ kind: 'source' }` / `{ kind: 'item' }` |

拒绝原因是稳定闭集(`invalid-url` / `unsupported-scheme` / `unsupported-import` / `missing-src`), 由 AppShell 的 Startup 转成 toast; 导入 URL 交给 `/sources?import=<src>`, 由 `SourcesHome` 消费查询参数(`src/features/sources/SourcesHome.tsx#L77-L78`、`#L100-L128`): 它用 `fetch_import_src` 取回文本后**直接**交给 `parseBookSourceCatalog`, 所以只走 catalog 分支(Maccms URL 不能经这条路径导入), 解析结果驱动 `SourcesHome` 自己内联的 Dialog 完成勾选、prepare 与提交(`#L320-L444`), **不经过** `SourceInstallDialog`。URL 的 scheme / 凭据校验、2 MiB 体量上限与超时在 Rust 侧的 import façade, 地址范围不设限(见信任边界页)。

## 解析与翻译: 一次性边界

`lj-importer` 的模块文件开头就把边界写死: 「第三方输入止于此边界, 不保留原文或可编辑格式状态。provenance 仅包含格式标记与内容 hash; credential bytes 只能移入加密的应用事务」(`src-tauri/crates/lj-importer/src/imported_rule.rs#L1-L4`)。

**strict JSON 前置校验**是自己写的解析器, 不用 `serde_json::Value` 做入口, 原因写在模块头:「parser 直接在原始 UTF-8 bytes 上计数并保留 token span; 不会先经 `serde_json::Value` 丢失 **duplicate key** 或把 UTF-8 byte offset 误当 UTF-16 code unit」(`src-tauri/crates/lj-importer/src/strict_json.rs#L1-L5`)。限制也是显式的(`#L25-L32`): 输入 ≤ 2 MiB、深度 ≤ 64、节点 ≤ 100000、属性 ≤ 32768、属性名 ≤ 1 KiB、单字符串 ≤ 256 KiB; 重复键会产生 `duplicate_key` 诊断(`#L599`)。

两个 importer 的差异:

- **Legado**(`src-tauri/crates/lj-importer/src/legado/`): 解析 JSON 后按固定映射翻译成 `RuleDefinition`, 并规定来源身份前缀 `source:legado:`(`legado/mod.rs#L23`), 因此 `LegadoImporter::owns_source` 只是前缀判断(`#L130-L132`)。它还拥有一个**有版本、来源归属、带完整性摘要、15 分钟过期的 ContinueAction 载荷**(`#L22`、`#L140-L162`), 由执行入口在真正跑规则前验签与消耗(`#L170` 起)——这就是「Legado 的 explore 续翻状态」不会变成无主输入的机制。
- **Maccms**(`src-tauri/crates/lj-importer/src/maccms/`): 输入是一个采集 API URL, 输出直接是 `RuleDefinition`; 模块头写明「Maccms 专属协议字段只停留在本模块的 `vocab` 与提取规则中, 调用方只会取得 `RuleDefinition`」(maccms/mod.rs#L1-L4)。

两者都只出 `RuleDefinition`, 都不会给出旧 Graph、节点处理器或执行器装配。

## Install Candidate: 内容与 TTL

`prepare_install` 的结果是一个**不透明的一次性候选**(`src-tauri/crates/lj-rule-system/src/types/candidate.rs#L127-L148`):

```text
InstallCandidate {
  id: CandidateId                       // opaque install token（serde transparent）
  expected_installed_revision: u64      // prepare 时固定的当前 source revision
  operation: SourceOperation            // install | update, 由上面的 revision 判定
  profile: SourceProfile                // 展示用资料（标题/图标/分组/意图/风险提示）
  required_grant: CapabilityGrant        // 最小授权
  diagnostics: Vec<Diagnostic>          // importer + validator + compiler 三源合并
  definition_hash, plan_hash: String    // 两个 BLAKE3
  expires_at_ms: i64                    // 到期时刻
}
```

候选的准备过程是一条完整的编译门: `stage_prepared_candidate` 依次做 `canonicalize` + `validate`(`validate` 只产出诊断, 不在这里失败)、`compiler.compile(&definition)`, 再 `runtime.validate_plan(&plan)`; compile 失败经 `compiler_error` 映射成 `Validation`(`definition_validation_failed`)或 `Compile`(`source_syntax_invalid` / `selector_unsupported` / `unsupported_version` 类), plan 失败才以 `RuleErrorStage::Candidate` 返回并附带已收集的全部诊断(`src-tauri/crates/lj-rule-system/src/system/lifecycle/prepare_install.rs#L182-L207`、`src-tauri/crates/lj-rule-system/src/system/error_mapping.rs#L126-L160`)。因此**候选存在就意味着「这份定义当时能编译、Plan 当时通过校验」**。

候选同时把 package 与 plan 的 artifact 及(若导入带来了静态凭据) runtime credential 一起冻结进存储(`#L232-L245` 的 `CandidateDraft`), 所以安装时不需要重新解析或重新取源。

TTL 默认 24 小时(`RuleSystemConfig::desktop` 的 `candidate_ttl: Duration::from_hours(24)`, `src-tauri/crates/lj-rule-system/src/types/config.rs#L28`; TTL 为 0 时打开系统即以 `candidate_ttl_invalid` 失败, `src-tauri/crates/lj-rule-system/src/system.rs#L75-L80`), 且**UI 会把剩余时间显示给用户**: `SourceInstallDialog` 把 `expires_at_ms` 渲染成「N 分钟后过期 / N 小时后过期 / 已过期」(`src/features/sources/SourceInstallDialog.tsx#L37-L43`)。

## 候选为什么会 stale

安装时存储层做四道判定, 顺序不能颠倒(`src-tauri/crates/lj-storage/src/repository/candidate_source/install.rs#L1-L33`):

| 判定 | 条件 | 失败结果 |
| --- | --- | --- |
| 到期 | `candidate.expires_at_ms <= now` | 先把 candidate 标为 `expired`, 再返回 `CandidateExpired` |
| 状态与 schema | `status` 必须是 `staged`, schema version 必须等于当前常量 | `CandidateExpired` / `CandidateUnavailable` / `CandidateSchemaMismatch` |
| staged 事件完整 | candidate stream 的 version 1 事件必须存在且 payload/artifact/secret 引用可解析 | `CandidateTampered` |
| revision 未漂移 | `actual_installed_revision == candidate.expected_installed_revision` | **`CandidateStale`** |

**stale 的定义就是「来源在当前 revision 上被别处改过」**: 候选记录的是 prepare 那一刻的 source revision, 若期间发生了别的安装/更新, revision 已经推进, 旧候选授权的对象就不存在了。这解释了为什么 candidate 要带 `expected_installed_revision` 而不只是 hash: 授权与并发控制的粒度是**来源的一个版本**, 不是一份定义。

另有两条与 stale 同类的失败: `GrantInsufficient`(传入 grant 未覆盖 `required_grant`, `#L31-L33`)与 `SourceRevisionMissing`(回退时历史 revision 不存在)。

## 为什么失败后不允许自动重试

候选是**时间受限 + revision 绑定 + 一次性**的 token, 这与「失败了再悄悄重来一次」在语义上不相容: 重试意味着重新 prepare, 而重新 prepare 会重新取源、重新计算 hash、重新过一通编译门。ADR 0002 第 2 条把这条写成硬规则: 「候选过期或 stale 后重新准备, **不能自动重试**」(`docs/adr/0002-versioned-source-and-rule-lifecycle.md#L13`)。

前端把它实现为**状态机回退而不是重试**: 安装失败时按 code 分两类(`src/features/sources/workflow.ts#L98-L109`、`#L345-L383`):

```text
可重试类(RETRYABLE_CANDIDATE_ERRORS)
  candidate_stale / candidate_expired / candidate_consumed /
  candidate_not_found / source_revision_conflict / grant_insufficient
  → 回到 'pick' 阶段并清空 prepared，用户必须重新 prepare 拿新候选

其他错误
  → 留在 'confirm' 阶段，只保留失败的那些条目，现场不变
```

也就是说「候选需要重做」时必须重新走一遍 prepare(界面不会拿着旧 token 重试), 而「传输/环境类错误」才保留现场让他重按一次安装。

安装的原子性也是这条规则的另一面: candidate 消费与 source revision 追加在同一个事务里, 并且写入的 source 事件用 `expected_version: expected_installed_revision` 做乐观并发(`install.rs#L36-L57`)。同一候选重复安装会命中 `idempotent_event` 直接返回已有 revision(`#L58-L65`)——「重复投递同一候选」是幂等的, 而「旧候选打在已推进的来源上」是 stale。

## 安装、更新与回退

三条路径共用同一份存储逻辑, 差别只在候选从哪来。操作类型在 prepare 阶段就已判定并写进候选, 前端不再自己推导: `prepare_install` 用定义里的 source identity 查已安装来源, 命中就用它的 revision、否则用 0, 再 `SourceOperation::from_installed_revision` 得出 `install` / `update`(`src-tauri/crates/lj-rule-system/src/system/lifecycle/prepare_install.rs#L158-L170`、`types/candidate.rs#L150-L168`); 前端只读 `candidate.operation` 决定「新建 / 更新」徽标与是否展示上一版对比(`src/features/sources/workflow.ts#L120-L123`、`src/features/sources/SourceInstallDialog.tsx#L372-L377`)。也就是说**基线 revision 本身就判定操作类型**, 在前端再比 hash 或比本地列表属重复推导。

| 路径 | 候选来源 | `operation` | `expected_installed_revision` | 事件 kind |
| --- | --- | --- | --- | --- |
| 首次安装 | `prepare_install`(Legado/Maccms/Package) | `install` | 0 | `installed` |
| 更新 | 同上(同一 source_identity 已存在) | `update` | 当前 revision | `updated` |
| 回退 | `prepare_source_rollback`(历史 revision) | `update` | 当前 revision | `updated` |

事件 payload 记录 `candidate_id`、`definition_version`、`definition_hash`、`plan_hash` 与 `expected_installed_revision`, 并把 candidate id 写成 `causation_id`(`install.rs#L46-L56`)——所以「这个来源版本是由哪个候选产生的」在账本里可追。

**回退不重写历史**: `process_stage_source_rollback` 从历史 revision **只读地**取出 package/plan artifact 与 profile/grant, 重新校验 package 与 plan 一致(`validate_candidate_package_and_plan`), 再构造一个**新的候选**(其 `expected_installed_revision` 是当前 revision)(`src-tauri/crates/lj-storage/src/repository/candidate_source/staging.rs#L130-L175`)。因此回退与更新在账本上是同一件事: 追加一个新 revision, 而不是把指针挪回去。这也满足 ADR 0002 第 3 条「回退从历史版本创建新的更新, 不重写历史」(`docs/adr/0002-versioned-source-and-rule-lifecycle.md#L14`)。

前端提交回退走独立命令 `prepare_source_rollback`(`src/shared/tauri/sources.ts#L118-L126`): `SourceInspector` 的历史 revision 列表把当前 revision 标成 current、其余渲染成可点的回退项, 点击后 `prepareRollback` 出候选并进入 confirm 区, 再走同一个 `install`(`src/features/sources/SourceInspector.tsx#L183-L195`、`#L337-L380`)。得到的仍是普通候选, 因此**同样需要再确认一次并重新 prepare 审阅**(过期或 stale 的候选一律被存储层拒绝)。

## 能力授予的现状

来源安装**没有联网确认开关**, 也没有网络授权这一步: 网络不是 capability, 规则可以请求任意 http(s) 目标(见信任边界页)。安装期唯一会拦住用户的是系统 API:

| 环节 | 位置 | 行为 |
| --- | --- | --- |
| 计算所需能力 | importer + compiler | `definition.capability_manifest().required` 进候选 `required_grant`(`prepare_install.rs#L223`) |
| 明确不支持的能力 | 前端 | 任一候选的 `required_grant` 里 `env`/`fs`/`process` 为真时以 `system_grant_unsupported` 拦下, 不下发安装(`workflow.ts#L149-L152`、`#L347-L350`) |
| 覆盖检查 | 存储层 | `grant_covers(&request.grant, &required_grant)` 否则 `GrantInsufficient`(`install.rs#L31-L33`) |
| 实际生效 | 运行时 | 每个 effect 前 `enforce_capabilities`, 有效能力是 host ∩ source grant ∩ invocation |

因此 `CapabilityGrant::none()` 是**唯一的生产 grant**(`src-tauri/crates/lj-rule-system/src/types/candidate.rs#L112-L119`), `install` 命令也只会送候选 id、不带 grant(`src/shared/tauri/sources.ts#L112-L116`): 前端那一层是提前给出明确反馈, 真正的拒绝语义在 storage 的覆盖校验里。声明了系统能力的来源目前装不进去, 这是当前的产品决定而不是实现缺口。

## 前端状态机

`createSourceWorkflow` 是一个闭包 store(与资料库同构), 阶段是 `idle → pick → preparing → confirm → installing → done | error`(`src/features/sources/workflow.ts#L29-L58`、`#L145-L158`)。几条值得一提的行为:

- 目录输入可能包含**多个书源**: 解析后进入 `pick` 让用户多选, 上限 `CATALOG_INSTALL_CAP = 50`(`src/shared/tauri/catalog.ts#L1-L2`);
- `prepareSelected` 对每个选中项独立 prepare, **部分失败不阻断全部**: 全部失败进 `error`, 部分失败仍进 `confirm` 并把 code 设为 `source_prepare_partial`、把失败项名字列进 `failedItems`(`workflow.ts#L283-L338`);
- 安装阶段并发提交所有候选, 任一失败按「可重试性」回退阶段(`#L345-L383`); 候选在 confirm 区停留时会把剩余 TTL 显示给用户(`SourceInstallDialog.tsx#L392`);
- 阶段文案与错误码的映射集中在对话框的 `errorLabel`(`SourceInstallDialog.tsx#L45-L60` 起), 模型层不出面向用户的字符串。

已安装来源的浏览与检查在 `SourcesHome` + `SourceInspector`(列表 + Sheet), 深链高亮用 `?highlight=<sourceId>`(`SourcesHome.tsx#L78-L79`、`#L294`)。

## 测试覆盖现状

| 路径 | 单测 | 集成测试 | fixture |
| --- | --- | --- | --- |
| Legado | `lj-importer/tests/legado_test.rs`(11 个用例: 六意图稳定映射、敏感 header 剥离、凭据不回显、重复键/上限/已知不支持行为、诊断与 provenance 不泄漏、ContinueAction 版本化与过期、翻页与续翻四种形态) | `lj-integration-tests/tests/legado_rule_system.rs`(6 个) | `lj-importer/fixtures/legado_synthetic_source.json`、`lj-integration-tests/fixtures/legado_star_free_novel.json` |
| Maccms | `lj-importer/src/maccms/mod.rs` 的内联 `mod tests`(6 个: 四意图导出不带 graph、稳定身份与节点 id、discover/detail URL 保留协议参数、XML 定义走 XPath 字段规则、端点文档把运行时凭据与定义分离、非法 URL 在定义创建前被拒)(`#L195`、`#L210-L353`) | `lj-integration-tests/tests/maccms_json_rule_system.rs`(8 个, 含 `maccms_json_four_intents_live_and_replay_use_only_rule_system`) | **无 JSON fixture**——测试用 `maccms_json_input(base_url)` 加 wiremock 路由现场构造响应(`#L133` 起) |
| Rule Package | `lj-rule-model/tests/descriptor_test.rs` 等只覆盖声明/合同层 | `core_out_rule_package.rs`(3 个, 磁盘上的 package 走真 SQLite/artifact 与 wiremock 端到端) | `lj-integration-tests/fixtures/` 的 package fixture |

也就是说两层都有覆盖, 差别只在 Maccms 没有独立测试文件与 fixture: 它的翻译细粒度回归靠内联单测, 端到端靠集成用例。候选的安全边界由三个集成用例覆盖: `candidate_boundary_is_opaque_and_rejects_tampering_expiry_and_insufficient_grant`(候选 DTO 序列化后不含 definition/package/plan/graph、篡改 token、grant 不足、已消费、过期五种)、`candidate_install_revalidates_event_metadata_after_restart`(重启后逐个篡改 profile / 必填 grant / 过期时刻 / definition_hash / 诊断 / artifact 算法六种 durable metadata, 全部落到 `candidate_tampered`)、`source_revision_history_and_rollback_require_a_new_reviewed_candidate`(回退必须走新候选)(`maccms_json_rule_system.rs#L487`、`#L574`、`#L608`)。

存储侧另有候选自身的契约测试(`candidate_hashes_are_verified_before_staging_and_installation`、`candidate_summary_round_trips_safe_preview_and_rejects_insufficient_grant`、`policy_gc_expires_candidates_and_honors_pins`), 以及启动恢复 `recover_candidates_sync`: 淘汰 expired / schema-invalid / 非 staged candidate 并释放全部 ownership(`src-tauri/crates/lj-storage/src/repository/candidate_source/read.rs#L186-L187`; 该恢复函数没有直接单测, 覆盖状态未验证)。

## 未实现与边界

- 候选本身**不携带二维码/来源市场**之类分发能力; 输入只有粘贴、文件、URL 与 deep link 四种。
- **Rule Package 没有前端入口**: 后端 `RuleInput::Package` 已能端到端安装(集成用例覆盖), 但前端 JSON 文本一律按 Legado 书源解析; facade 上的 `inspect_rule_package`(报告未安装能力节点)没有 IPC 命令, 因此 preflight 结果也到不了界面。
- `failedItems` 只记录失败项名称, 没有「逐项重试」的 UI。
- 回退入口已接到 `SourceInspector` 的历史列表; 「更新」仍是重新打开完整安装对话框(`SourcesHome.tsx#L445-L452`), 没有从当前来源一键更新的独立路径。
- Maccms 翻译没有独立测试文件与 JSON fixture(单测内联在 `maccms/mod.rs`), 属已知的覆盖形态差异。
- 前端 `isHttpUrl` 只判断协议; 更严格的校验(scheme/凭据、体量、超时)在 Rust 侧, 因此前端可以先收下一个最终会被拒的 URL。地址范围**不**做限制。
