---
type: "参考"
title: "Library and media query"
openwiki_generated: true
sources:
  - id: openwiki-source-980fd459aadb6afbaa667438
    resource: repo://src-tauri/crates/lj-integration-tests/tests/maccms_json_rule_system.rs
  - id: openwiki-source-f37a0f7edcf7d757acfa95d3
    resource: repo://src-tauri/crates/lj-media/src/lib.rs
  - id: openwiki-source-985022f215d1a8fb48d686c6
    resource: repo://src-tauri/crates/lj-rule-system/src/system/query_adapter.rs
  - id: openwiki-source-612028990ad235f913b145bb
    resource: repo://src-tauri/crates/lj-storage/src/repository/projection/read.rs
  - id: openwiki-source-97a89101bffc0f1b3ed4ad08
    resource: repo://src-tauri/crates/lj-storage/src/repository/projection/write.rs
  - id: openwiki-source-9653aa1d8d274ae45eae1e3f
    resource: repo://src-tauri/crates/lj-storage/src/storage/query.rs
  - id: openwiki-source-1fd096c367528b2e56c4c977
    resource: repo://src-tauri/crates/lj-storage/src/types/library.rs
  - id: openwiki-source-5ea3a9b2991078e724d279d8
    resource: repo://src-tauri/crates/lj-storage/tests/event_projection_storage_test/media_query_contract.rs
  - id: openwiki-source-80fc8f51454bf5d5e7fa3f1d
    resource: repo://src-tauri/src/commands/query.rs
  - id: openwiki-source-54631e6ebf1d3b815c4a5eed
    resource: repo://src/App.tsx
  - id: openwiki-source-616e35f2ce008b6f07885f4e
    resource: repo://src/features/library/LibraryHome.tsx
  - id: openwiki-source-b140626abec4f20955ebafdc
    resource: repo://src/features/library/LibraryInspector.tsx
  - id: openwiki-source-b5c3856af90304d1d07dbd84
    resource: repo://src/features/library/LibraryItem.tsx
  - id: openwiki-source-5480c4953d60af0a3c82e354
    resource: repo://src/features/library/workflow.ts
  - id: openwiki-source-99a93074334aa4f3f476fdcf
    resource: repo://src/shared/tauri/library.ts
  - id: openwiki-source-2640df53984077a61a4bb013
    resource: repo://src/shared/types/media.ts
generated: { by: "pi", at: "2026-10-05T08:37:16.392Z" }
verified:
  - by: openwiki/0.7.0
    at: 2026-10-05T08:37:16.392Z
---


## 两层状态, 两套所有权

资料库页面显示的东西来自两个互不拥有的数据源:

| 层 | 拥有者 | 内容 | 存在哪 |
| --- | --- | --- | --- |
| 媒体空间 | 来源规则 | 主体、单元、资产、关系、展示提示 | `projection_*` 表, 由执行时的 `MediaGraphDelta` 写入 |
| 资料库状态 | 用户 | 收藏、置顶、最近打开、进度 | `library_projection` 表, 由 IPC 更新写入 |

这条边界写在类型文档里: 资料库状态「与媒体空间共享稳定 resource ID, 但只保存用户收藏、固定和进度; **不复制 source Graph、规则 Plan 或任何 secret**」(`src-tauri/crates/lj-storage/src/types/library.rs#L1-L5`)。因此删除规则或清空来源投影都不会动用户的收藏与进度, 反之亦然; 两者只靠 `resource_id` 关联。

关联的键是可逆的自描述 ID(`src-tauri/crates/lj-media/src/lib.rs#L12-L53`): `item:<source>:<key>` 与 `unit:<source>:<item>:<unit>`, 各段做 hex 编码, 空段归一化为 `unknown`; `parse_item_resource_id` / `parse_unit_resource_id` 反向解出原 key。好处是前端拿到的 ID 就足够定位来源与原始键, 不需要额外映射表。

## 增量表达

执行产出的不是「整棵媒体树」而是增量, 由 `MediaGraphDelta` 表达(`src-tauri/crates/lj-media/src/lib.rs#L249-L258`): `sources` / `items` / `collections` / `units` / `assets` / `relations` / `actions` / `hints` 八组, 合并语义是**同 ID 后到覆盖**(`merge_by_id`, `#L260-L288`), 只有 `relations` 例外——重复关系不追加(`#L269-L273`), 因为关系是无序集合而不是值快照。这带来两个直接结果:

- 一次 Search 只需要交付它真正发现的那几个主体与单元, 不必回读历史图;
- 同一 ID 在两批结果里出现时, 语义是「更新」而不是「重复」, 因此投影是 upsert 而不是 append。

投影里的展示信息与内容信息分开: `MediaItem` 携带 `cover_asset_id`(封面是**资产引用**, 不是 URL), 而视觉线索放在 `PresentationHint`(只含 `resource_id`、`card_density`、`cover_ratio`、`dominant_color`、`preferred_template`)。这是「规则只出标准媒体模型 + 展示提示, 不定义 UI 布局」这条产品边界在类型层的落点。

## 读路径

存储侧的媒体查询 façade 全部走**只读连接池**的 `self.read(...)`, 与写入的单 writer 分离(`src-tauri/crates/lj-storage/src/storage/query.rs#L31-L118`):

| 方法 | 查询 | 边界 |
| --- | --- | --- |
| `get_library_projection` | 单个只读事务里取 `global_seq` + 全部条目 | 无分页(资料库是用户规模) |
| `get_library_entry` | 单条目 | |
| `source_projection` | 按来源聚合的规范化投影 | |
| `get_item` / `get_items` | `projection_items` 按 ID(payload JSON) | 存储层不做上界; 「最多 64 个 ID」与分页 `limit 1..=100`(默认 50)由门面 `query_adapter` 强制(`src-tauri/crates/lj-rule-system/src/system/query_adapter.rs#L29-L33`, `#L209-L216`) |
| `get_unit` | `projection_units` 按 ID | |
| `list_items_by_source` | 按 `source_identity` 列 | |
| `list_units_for_item` | 按 item 索引的有界分页 | `limit` 默认 50 / 硬上限 100 |
| `list_assets_for_unit` | 按 unit 索引的有界分页 | 同上 |

**排序由数据库保证**, 不由前端重排(`src-tauri/crates/lj-storage/src/repository/projection/read.rs#L13-L24`):

```sql
ORDER BY pinned DESC, favorite DESC, last_opened_at IS NULL ASC,
         last_opened_at DESC, resource_id ASC
```

即「置顶在前, 其次收藏, 未打开的排最后, 其余按最近打开倒序, 同分按资源 ID」。同时它用 `LEFT JOIN event_streams ON stream_id = 'library/' || resource_id` 把每条 entry 的 stream revision 一并取出(`COALESCE(..., -1)` 表示尚无 stream), 这个 revision 不是展示字段, 而是下一次写回的乐观并发令牌。

前端的 `projectLibrary` 会**再排一次序**(`src/shared/tauri/library.ts#L58-L72`)并且**过滤掉没有用户状态的条目**——只保留 `favorite || pinned || last_opened_at !== null || progress !== null`。两条 SQL 排序因此看起来重复, 但前端那次是必需的: 它承担过滤与本地更新后的重排, 且其比较逻辑必须与 SQL 一致, 否则本地 upsert 后行会跳位。这是一处隐式的双实现约束, 仓库里没有测试断言两者一致。

「有界」是可验证的行为而不是注释: `bounded_media_queries_order_page_and_skip_missing`(`src-tauri/crates/lj-storage/tests/event_projection_storage_test/media_query_contract.rs#L97`)同时断言分页顺序与「缺失 ID 被跳过而不是报错」。

## 收藏与置顶不变量

不变量有两条: **置顶蕴含收藏**, 以及**取消收藏同时取消置顶**。它的权威实现在存储层, 而且只写一次:

```rust
// 固定条目意味着用户保留它；事件和投影必须写入同一规范状态。
entry.favorite |= entry.pinned;
```

(`src-tauri/crates/lj-storage/src/repository/projection/write.rs#L13-L14`)——注意归一化发生在**写事件与写投影之前**, 所以事件 payload 与投影行里的 `favorite` 是同一个值, 不会出现「事件记录的是原始输入、投影记录的是归一化结果」这种 replay 分叉(`#L17-L29`)。

其余三处都只是**镜像**, 不是权威:

- facade `update_library_entry` 原样透传 `favorite` / `pinned`(`src-tauri/crates/lj-rule-system/src/system/query_adapter.rs#L111-L145`);
- 前端 `updateOwnership` 在构造请求时算 `favorite: favorite || pinned`(`src/features/library/workflow.ts#L141-L148`);
- `LibraryHome` 的两个切换回调在点击时把另一维一起算进去(`src/features/library/LibraryHome.tsx#L91-L111`)。

前端镜像的意义是**乐观 UI 与后端结果一致**(它直接用请求里的值更新本地行, `workflow.ts#L154-L166`), 而不是强制不变量。真正被测试守住的是后端那一条: `library_entry_normalizes_pinned_and_favorite_ownership`(`src-tauri/crates/lj-integration-tests/tests/maccms_json_rule_system.rs#L954`)分别验证「置顶自动收藏」与「取消收藏必须同时取消置顶」两个方向。

顺带一个写入细节: `library_projection` 的 upsert 用 `ON CONFLICT(resource_id) DO UPDATE SET ...` 却不更新 `revision`——revision 来自 `event_streams.version` 的 LEFT JOIN(`read.rs#L17`), 即**事件流的版本就是条目的并发令牌**, 投影行不自带版本号。

## 进度锚点

`LibraryProgress { unit_id: Option<MediaResourceId>, position: u64, total: Option<u64> }`(`src-tauri/crates/lj-storage/src/types/library.rs#L11-L20`)。三点值得注意:

- 锚点是**单元 ID + 单元内位置**, 不是页码。单元 ID 本身可逆(可解出来源与 chapter key), 位置是规则定义的消费单位(第几屏/第几章), 由应用面在翻页时换算——引擎不回写进度(见 ADR 0005)。
- `total` 可空, 表示「来源没给总长度」。前端为此有两个分支: 有 total 时画 `position/total` 与进度条, 无 total 时只显示 `已记录 position`(`src/features/library/LibraryHome.tsx#L35-L44`); `total <= 0` 也不画进度条。
- `last_opened_at` 是 RFC3339 **文本**(`types/library.rs#L31-L32`), 排序按文本比较——只有在所有写入都严格使用同一时区格式时才是正确的, 这一点没有类型或校验强制。

## 写回与并发

写回的请求带 `expected_version`(取自读到的 `entry.revision`), 存储侧在事务里做乐观并发: 版本不符即 `VersionConflict`, 不做隐式合并(`session_delivery.rs` 之外的写路径同理)。成功后返回 `LibraryUpdateReceipt { global_seq, revision }`(`src/shared/tauri/library.ts#L13-L16`)。

前端据此把本地行推到新 revision 并把 `globalSeq` 取 `max(current, receipt.global_seq)`(`workflow.ts#L167-L170`), 同时在写入期间把该 resource 放进 `pendingResourceIds` 禁用按钮、失败则把错误文本挂到该行(`workflow.ts#L131-L196`)。因此并发冲突的可见形态是「这一行显示错误、其余行不变」, 而不是整页失败。

## IPC 与前端消费

三个命令支撑这一页(`src-tauri/src/commands/query.rs#L33-L110`): `get_library_projection`(无参)、`update_library_entry`、`get_media_items`(批量 ID)。`list_media_units` / `list_media_assets` / `get_media_item` 存在但**前端没有调用点**(见 IPC 页的缺口清单)。

前端的数据流是一条单向链:

```text
LibraryHome(useSyncExternalStore)
  └─ workflow.refresh()
       ├─ loadLibraryProjection()        → 全量 library 条目 + global_seq
       ├─ projectLibrary(response)       → 过滤 + 排序
       ├─ loadMediaItems(ids)            → 只查这些 id 的媒体主体
       └─ rows: [{ entry, media | null }] → 表格 + Inspector
```

`workflow.ts` 是一个**没有外部状态库的闭包 store**(`createLibraryWorkflow`), 用 `useSyncExternalStore` 订阅(`LibraryHome.tsx#L46-L49`)。它把 adapter 作为参数注入(`workflow.ts#L18-L53`), 这正是 `workflow.test.ts` 能测的部分——测试用假 adapter 断言状态转换, 不渲染组件。

媒体主体缺失不是错误: `media` 为 `null` 时表格与 Inspector 都渲染 `library_media_missing()`(`LibraryHome.tsx#L235-L242`、`LibraryInspector.tsx#L82-L84`)。这对应「用户收藏过但来源投影已被清理」的合法状态。

## 与 Rust 契约的一处真实缺口

前端的 `MediaItem` 类型与后端返回的形状**不一致**(`src/shared/types/media.ts#L1-L8`):

| 前端声明 | 后端实际序列化(`src-tauri/crates/lj-media/src/lib.rs#L128-L141`) |
| --- | --- |
| `title: string` | `title` ✓ |
| `creator: string` | `creators: string[]` ✗ |
| `kind: MediaKind`(5 值) | `media_kind: MediaKind`(12 值) ✗ |
| `source?: string` | `source_id` ✗ |
| `progress?: number` | 不存在(进度只在 library entry 里) ✗ |
| — | `subtitle`、`description`、`cover_asset_id`、`metadata`、`completeness`、`updated_at` 未建模 |

`MediaItem` 上没有 `#[serde(rename_all)]`(`lib.rs#L128-L129`), 所以这些字段会以 Rust 名出现在 JSON 里。后果: Inspector 的「创作: {creator}」「类型: {kind}」「来源: {source}」在真实后端下都会渲染空值(`LibraryInspector.tsx#L72-L80`), 表格副标题同样为空(`LibraryHome.tsx#L237-L238`)。`library.test.ts` 用 `vi.mock` 替换了 `invoke`, 因此这个漂移不会被测试发现。

**这是一个已知缺口而非假设**: 类型层不一致是可从两侧源码直接比对的, 但「真实运行时确实为空」这一点属于推断(未实跑验证)。

## LibraryItem 是占位

`/library/item/<resourceId>` 路由存在(`src/App.tsx#L102`), 由 `LibraryItemRoute` 解析参数后交给 `LibraryItem`; 而 `LibraryItem` 的实现只有九行——把 `initialResourceId` 传给 `LibraryHome` 并让 Inspector 关闭时回退到 `/library`(`src/features/library/LibraryItem.tsx#L1-L9`)。

也就是说「打开一个媒体」的**全部语义就是打开资料库页并选中该行的 Inspector**:

- 没有阅读器、播放器或任何应用面跳转, 页面上也没有「打开/继续阅读」按钮——Inspector 的 `SheetFooter` 是空的(`LibraryInspector.tsx#L163`), 唯一的动作是收藏与置顶;
- 因此深链 `item` intent 落地后用户看到的是资料库详情, 而不是内容;
- 真正的打开路径(跳 `/apps/<surface>/:resourceId`)属 ADR 0004 的未实现部分。

所以这一页当前的能力是: **读投影、管收藏与置顶**。进度只被显示, 没有任何 UI 能写入它(`update_library_entry` 会把已有 `progress` 原样回传以避免清空, `workflow.ts#L141-L148`)。

## 未验证 / 边界

- 前端 `MediaItem` 形状漂移的实际渲染后果未经实跑确认(见上文)。
- `last_opened_at` 的文本排序在混用非 UTC 偏移的 RFC3339 时会不会错序, 没有测试; 写入方是否始终用同一格式也未验证。
- `projectLibrary` 的排序与 SQL 排序一致这一点没有守门测试, 靠人工对读。
- 资料库投影的规模上限: `get_library_projection` 全量返回, 没有分页或截断; 收藏量级增长后的 IPC 体积未评估。
- `MediaKind` 的 5 vs 12 值差异对资料库筛选(全部/收藏两档)暂时无影响, 但媒体类型展示会失真。
