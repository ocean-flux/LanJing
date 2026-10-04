---
type: "参考"
title: "Rule document lifecycle"
openwiki_generated: true
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T10:14:16.110Z
sources:
  - id: openwiki-source-9faf227efd814ea6139f8535
    resource: repo://docs/adr/0002-versioned-source-and-rule-lifecycle.md
  - id: openwiki-source-d3b45bea7bb3b5cc6ed4ae2d
    resource: repo://messages/zh-CN/rules.json
  - id: openwiki-source-f4d8e498f8a27d9242bfdc4b
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/document.rs
  - id: openwiki-source-80e8762cff162b2ecd16e129
    resource: repo://src-tauri/crates/lj-rule-system/src/system/lifecycle/prepare_install.rs
  - id: openwiki-source-0e77489563e28087cde37771
    resource: repo://src-tauri/crates/lj-rule-system/src/types/candidate.rs
  - id: openwiki-source-66845dd8b3f72696149d5d09
    resource: repo://src-tauri/crates/lj-rule-system/src/types/document.rs
  - id: openwiki-source-104459762ca7b8bec4b219e2
    resource: repo://src-tauri/crates/lj-storage-migration/src/m20260826_000002_rule_document_effective_semantics.rs
  - id: openwiki-source-24f5e1cc6f37c0e91c4a4cc9
    resource: repo://src-tauri/crates/lj-storage-migration/src/m20260830_000003_rule_document_effective_history.rs
  - id: openwiki-source-aaaa1af7d45207ac8a646f67
    resource: repo://src-tauri/crates/lj-storage/src/repository/document.rs
  - id: openwiki-source-ad2c9abae600aae30fe52815
    resource: repo://src-tauri/crates/lj-storage/src/repository/execution/read.rs
  - id: openwiki-source-532cf8584a13c01f824e0ae7
    resource: repo://src-tauri/crates/lj-storage/src/storage/document.rs
  - id: openwiki-source-de4ee84fde9b59629e3a1d00
    resource: repo://src-tauri/crates/lj-storage/src/transaction/document.rs
  - id: openwiki-source-a0309a4f66f33e6814b5c22e
    resource: repo://src/features/rules/EditorToolbar.tsx
  - id: openwiki-source-ee130a462d446f70c12f0143
    resource: repo://src/features/rules/model/core.ts
  - id: openwiki-source-04750d843a810bbf356d4660
    resource: repo://src/features/rules/session-error.ts
  - id: openwiki-source-8780adbf81fc42a7d70c2315
    resource: repo://src/features/rules/use-session.tsx
generated: { by: "pi", at: "2026-10-04T10:14:16.110Z" }
---


## 版本模型

ADR 0002 给出的规则侧版本词汇(`docs/adr/0002-versioned-source-and-rule-lifecycle.md#L15-L19`):

- **Rule Draft Revision** 与 **Effective Rule Revision** 是两个不同角色; Explicit Rule Save 保存草稿, **通过校验的草稿自动晋升为生效版本**, 没有 prepare 或手动发布步骤(第 4 条);
- 无效草稿**不替换** Effective; 保存冲突时保留远端版本并创建 Recovery Draft, 语义冲突不自动合并(第 5 条);
- Draft Credential 只存在于其所属 Rule Revision 的受保护 secret artifact 中, owner 必须包含 `document_id`、semantic revision、节点与 JSON pointer; 它不进前端持久化状态(第 6 条);
- Rule Revision History **只展示 Effective**, 恢复历史版本要创建新草稿并重走校验与自动生效(第 7 条)。

在存储层, 一个文档有三个**互相独立**的 revision 计数器(`semantic_revision` / `layout_revision` / `link_revision`), 加一个 `state`; 摘要 DTO 三个都暴露, 因此冲突不会跨域传播(`src-tauri/crates/lj-rule-system/src/types/document.rs#L235-L257`)。

## 保存链路

前端一次保存可以只带一个域。DTO 是分域的, 且都用 `deny_unknown_fields` 收紧形状(`src-tauri/crates/lj-rule-system/src/types/document.rs#L92-L111`):

```text
SemanticSave { expected_revision, definition, credential_mutations[] }
LayoutSave   { expected_revision, layout_json }
```

注意 **`SemanticSave` 里没有 activation 字段**: 客户端不能选择「这次保存是草稿还是生效」, 晋升由服务端依据校验结果决定。

facade 的 `save_native_rule_document`(`src-tauri/crates/lj-rule-system/src/system/lifecycle/document.rs#L270-L372`)只做**准备**, 不写盘:

1. 读现有文档详情(不存在即 `document_not_found`);
2. 从现有 semantic 快照解析 `CredentialManifest`;
3. `canonicalize` + `validate`, 收集全部诊断;
4. **只有** `current_semantic_revision == expected_revision` 时才准备凭证变更(过期的语义请求跳过凭证校验与写入);
5. 计算 canonical `definition_json` 与 `definition_hash`;
6. `activation = Draft` 当且仅当存在任一 Error 级诊断, 否则 `Effective`;
7. 把 semantic 与 layout 两个域放进**一个** `SaveDocumentRequest` 交给存储。

真正的事务在存储层(`src-tauri/crates/lj-storage/src/transaction/document.rs#L252-L317`):

```text
document_row 必须存在, occurred_at_ms >= 0
  ├─ save_semantic(可选)  →  校验 expected_revision, 处理凭证, 写 semantic, 视 activation 写 effective + history
  ├─ save_layout(可选)    →  校验 expected_revision, 写 layout
  └─ 任一域写入成功才 advance_document(semantic_revision, layout_revision)
```

**冲突判定依据是版本号相等**, 判定点是「客户端所知的已保存 revision 是否等于当前 revision」(`transaction/document.rs#L436-L448`)。不相等时:

- 返回 `DomainSaveOutcome { revision: 当前值, conflict: Some(RevisionConflict{ expected, current }), activation: None }`;
- **不写任何行**、不推进 revision、不返回 `StorageError`——冲突是**结果**而不是错误;
- 另一域不受影响: layout 的冲突判断独立走自己的计数器(`#L591-L603`);
- 任一底层失败则整个事务回滚, 不会出现「semantic 写了 layout 没写」。

## 自动晋升与危险草稿的保留

当 `activation == Effective` 时, 同一事务里连续写两张表(`transaction/document.rs#L570-L572`):

- `rule_document_effective_semantics`: 每个文档一行(UPSERT, 全文替换), 代表当前生效语义;
- `rule_document_effective_semantic_history`: 以 `(document_id, revision)` 为主键的不可变历史。

这两条正是两个 migration 补出来的语义:

| migration | 补了什么 | 回填 |
| --- | --- | --- |
| `m20260826_000002_rule_document_effective_semantics` | 先放宽元数据触发器允许 schema_version 1..2, 建 `rule_document_effective_semantics` 表, 再 `backfill_effective_semantics`, 最后 `refresh_to_version(2)`(`src-tauri/crates/lj-storage-migration/src/m20260826_000002_rule_document_effective_semantics.rs#L23-L49`) | **只回填能通过编译器校验的历史 semantic 快照**, 非法快照被跳过而不是让迁移失败(`#L60` 起) |
| `m20260830_000003_rule_document_effective_history` | 放宽触发器到 1..3, 建 `rule_document_effective_semantic_history` 表与 `(document_id, updated_at_ms DESC, revision DESC)` 索引, 建校验触发器, 再从 effective 表复制全部内容, `refresh_to_version(3)`(`src-tauri/crates/lj-storage-migration/src/m20260830_000003_rule_document_effective_history.rs#L12-L34`) | 把当时的 effective 当作历史的第一个版本 |

回填「跳过非法快照」是个有意的取舍: 迁移不能让整个数据库打不开, 代价是某些历史文档在迁移后**没有** effective 版本(它们仍是 draft 状态, 需要重新保存才会晋升)。这一点由 `v1_semantic_snapshot_migrates_to_effective_snapshot` 类测试固定。

无效草稿的保留是另一条不变量, 与晋升互补: 保存一个编译不过的 Definition 时 `activation = Draft`, 所以 semantic 快照与 revision 都推进了(用户的工作被持久化), 但 effective 表**不动**。facade 侧对应两个测试: `invalid_definition_preview_keeps_draft_only` 与 `invalid_save_preserves_effective_rule_and_records_draft`(`src-tauri/crates/lj-rule-system/src/system/lifecycle/document.rs#L1483`、`#L1532`)。这正是 ADR 0002 里被否决方案「有效规则保存失败时不持久化草稿」的反面。

校验接口本身也遵循同一取舍: `validate_native_rule_document`(`#L381-L453`)编译失败时返回 `valid: false` 且 `plan_hash` 为 `None`, 并把**全部**诊断一次性返回, 而不是报错。

## 凭证的归属与生命周期

文档凭证的所有权被编码进 `owner_id`, 格式固定为五段(`src-tauri/crates/lj-storage/src/repository/document.rs#L591-L600`):

```text
{document_id}::{scope}::{revision}::{node_id}::{json_pointer}
```

`scope` 取 `draft` / `effective` / `history` 三值(`src-tauri/crates/lj-storage/src/transaction/document.rs#L30-L32`)。这个格式同时是**批量释放的依据**: 删除或推进某个 revision 时, 用 `owner_id LIKE '{document_id}::{scope}::{revision}::%'` 找出该 revision 认领的全部 owner 并逐一释放(`repository/document.rs#L603-L622`)。

它满足 ADR 0002 第 6 条的四个要素(document_id、semantic revision、节点、JSON pointer), 而且多了一层 scope: 同一个槽位在 draft 与 effective 下是**两个独立的 owner**, 因此「草稿里有凭证但尚未生效」可以被引用计数如实表达。晋升时会对 draft / effective / history 三个 scope 各认领一次(`transaction/document.rs#L491-L519`), 替换则先认领旧 effective 的 history owner 再释放旧 revision 的 owner(`#L523-L555`)。

凭证明文是一次性的:

- facade 把 `CredentialMutationRequest.value` 送进 `SemanticSaveInput.credential_mutations`, 存储层在同一事务里写入/更新槽位;
- 前端在 `saveRequest` 步骤就把 `pendingCredentialMutations` 移进 `inFlightSave` 快照并清空 state, 注释写明「明文只存在于 inFlightSave 快照; 响应后不保留, 冲突时由 UI 重新输入」(`src/features/rules/model/core.ts#L1150-L1152`); 保存失败也不恢复明文(`#L1215`)。

**一处未接线**: 文档的有效凭证**不会**被转成运行时凭证。候选/安装路径的 `runtime_credentials` 只来自导入器(`take_credentials()`, 即 Legado/Maccms 源文本里的静态凭据)(`src-tauri/crates/lj-rule-system/src/system/lifecycle/prepare_install.rs#L114-L124`), 执行侧则从 `source_versions.runtime_credential_secret_id` 读取(`src-tauri/crates/lj-storage/src/repository/execution/read.rs#L223-L264`)。ADR 0002 第 6 条后半句「只有 Effective Rule Revision 的 credential 可在 prepare/install 时编码为 source runtime credential」属于**已决定未实现**。

## 历史与恢复

- `list_native_rule_revision_history` 走只读 lane 读 `rule_document_effective_semantic_history`, 按 `updated_at_ms DESC, revision DESC` 排序, 只返回**安全摘要**(document_id/revision/definition_hash/effective_at_ms)(`src-tauri/crates/lj-storage/src/repository/document.rs#L204-L224`);
- `restore_native_rule_revision` 从不可变历史**复制**出一个新草稿: 「历史行与 Effective 当前行均保持不变」(`src-tauri/crates/lj-storage/src/transaction/document.rs#L319-L320`), 因此恢复不是回滚, 而是产生一次新的编辑起点; 它同样返回 `RestoreDocumentRevisionOutcome`, Draft revision 冲突以 `conflict` 字段返回而非报错(`src-tauri/crates/lj-storage/src/storage/document.rs#L60-L72`);
- 完整 Definition 不通过读 API 暴露: `effective_history_row` 的注释写明「完整 Definition 不直接暴露给读 API」(`repository/document.rs#L226-L240`), 编辑路径通过 facade 拿到已脱敏的 Definition。

## 删除的守卫

删除要求显式确认, 但只在 `state == "linked"` 时生效: 「linked 且未确认时守卫拒绝」(`transaction/document.rs#L825-L836`)。同时它会释放 provenance 原文与全部凭证槽位的 secret owner, 并显式删除子行与主行(`#L826-L827`)。

**但 `linked` 这个状态目前永远不会出现**: 文档插入时固定写 `state = 'draft'` 与 `link_revision = 0`(`src-tauri/crates/lj-storage/src/repository/document.rs#L306`), 而仓库里没有任何一条 SQL 更新 `rule_documents.state` 或 `link_revision`——`UPDATE rule_documents` 只出现两处, 分别改 semantic/layout revision 与 title(`#L495`、`#L532`)。所以:

- 摘要里的 `state` 恒为 `draft`, `link_revision` 恒为 0;
- 删除守卫实际是防御性代码, 当前不会触发;
- ADR 0002 里「文档与来源的链接(link revision 随安装推进)」属**已决定未实现**。

## 前端会话与用户可见状态

编辑器 session 是 vanilla zustand, React 只做订阅绑定(`src/features/rules/use-session.tsx#L1-L4`)。保存的状态机在 `core.ts` 的 `saveRequest` / `saveResponse` / `saveFailure` 三个 reducer 之间步进, 并用 `epoch` 丢弃过期响应(同一响应重复投递也会被拒)(`src/features/rules/model/core.ts#L1137-L1212`)。

保存失败与冲突是**两种不同的可见结果**:

| 情形 | 模型行为 | 用户看到 |
| --- | --- | --- |
| 传输/后端硬失败(抛异常) | `saveFailure` 清空 in-flight, **保留 dirty**, 不恢复一次性凭证明文(`core.ts#L1215`) | 工具栏 `toast.error(sessionErrorText(...))`(`src/features/rules/EditorToolbar.tsx#L33-L42`), 文案由 `SessionError.code` 映射(`src/features/rules/session-error.ts#L10-L31`) |
| 版本冲突(正常 resolve) | 该域 revision 不推进, 写入 `conflict.{semantic,layout}` 与 `recoveryDraft.{semantic,layout}`, dirty 保持 |
| 成功 | 该域推进 `savedSemanticRevision` / `savedLayoutRevision`, 清 conflict 与 recoveryDraft; 若响应期间语义又漂移则保持 dirty(`core.ts#L1137-L1212`) |

**冲突的用户可见性目前是断的**: 冲突是一次正常返回而不是异常, 所以工具栏的 `run(...)` 会照常弹 `toast.success(m.rules_saved())`(`EditorToolbar.tsx#L104-L111`), 同时「有未保存的更改」指示器仍然亮着(因为 dirty 没清)。仓库里存在 `rules_conflict_title`、`rules_conflict_hint`、`rules_layout_conflict_detail` 等文案键(`messages/zh-CN/rules.json#L37-L38`、`#L154`), 但**没有任何组件读取 `conflict` 或 `recoveryDraft`**——本轮检索 `src/features/rules/**/*.tsx` 里没有这两个字段的使用点。因此当前真实体验是: 冲突时提示「已保存」, 而实际写入被拒, 恢复草稿留在内存里且没有出口。

Recovery Draft 本身也只是**内存态**: 结构里只保留脱敏 Definition、布局与凭证槽引用(`core.ts#L1150-L1165`), 存储层没有对应表(检索 `recovery` 只命中 artifact 的孤儿恢复)。关闭窗口即丢失, 与 ADR 0002 第 5 条「创建独立 Recovery Draft」的耐久含义存在差距——按「已决定未实现」计。

## 未实现与边界清单

- 文档导出的可安装路径不存在: `RuleInput` 只有 `MaccmsJson` 与 `Legado` 两个变体(`src-tauri/crates/lj-rule-system/src/types/candidate.rs#L13-L24`), 因此**手工创作的 native rule document 无法变成已安装来源**, 也就无法被执行。`prepare_install` 不谈 document。
- 文档 effective 凭证 → runtime credential 的编码未接(见上文)。
- `state` / `link_revision` 恒为初始值, 文档↔来源链接未实现。
- 冲突与 Recovery Draft 无 UI 出口, 且冲突被当作成功提示。
- 三域 revision 中只有 semantic 与 layout 有写入路径, link 域无写入者。
