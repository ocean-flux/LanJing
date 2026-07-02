---
title: Real Source Integration Tests - Plan
type: feat
date: 2026-07-01
topic: real-source-integration-tests
artifact_contract: ce-unified-plan/v1
artifact_readiness: requirements-only
product_contract_source: ce-brainstorm
execution: code
---

# Real Source Integration Tests - Plan

## Goal Capsule

- **Objective:** 将 `.integration-tests/` 重设计为纯实时打站的源 QA 测试套件——每条测试真实请求目标源站，零 mock，结构断言，手动触发。
- **Product authority:** LanJing 规则引擎的源接入质量护栏。真实源是验证「导入→Graph→执行端点」全链路是否跑通的唯一保真手段；mock 无法暴露真实站点改版/反爬/字段漂移。
- **Resolved:** 真实源采集 URL 已选定并实测可达（Maccms 红牛 JSON 源、Legado 真实书源域名均 HTTP 200），具体地址见 `.tmp/hongnu.txt`（本地 gitignored，不入计划）。

---

## Product Contract

### Summary

重设计集成测试为纯实时打站：删除全部 wiremock mock 测试与响应 fixtures，改为对 Legado 真实书源、Maccms10 真实采集 URL 各跑一条全链路实时测试。每条测试 `#[ignore]` 手动触发，用 `HttpNodeProcessor::new()`（SSRF 开 = 生产路径）打真实源站，结构断言（产出非空 + variant 正确 + URL 绝对格式 + 章节非空），不硬编码业务值。

### Problem Frame

现有集成测试是 mock 回放轨与真网络轨的混合体。mock 轨（`legado_wiremock.rs`、`maccms_json.rs`、`maccms_xml.rs`、`real_source_maccms.rs`）用 `wiremock::MockServer` + 静态响应 fixtures 验证解析逻辑，但 mock 数据与真实源站脱节——源站改版、反爬、字段漂移在 mock 里永远绿，测试通过不代表源真能用。真网络轨仅 `real_source_legado.rs` 一条，且已坏：它引用 `legado_synthetic_source.json`，该 fixture 在 `.integration-tests/fixtures/` 下不存在。结果是两类测试都不 trustworthy：mock 绿不代表源可用，真网络轨跑不起来。

用户要求集成测试必须打真实网站、不允许 mock。这把测试定位从「CI 回归护栏」转为「源接入 QA 工具」——因为真实源站天然 flaky（改版/宕机/限流），不能进 CI 阻塞路径，只能手动触发做接入验证。代价是接受：`pnpm test:rs` 集成部分默认跑零测试（全 ignored），测试不定期手动跑会静默腐烂。换来的是测试通过即代表源此刻真能跑通。

### Key Decisions

- **KD1. 纯实时打站，零 mock。** 删除全部 wiremock 测试（4 文件）、`wiremock` Cargo 依赖、mock 响应 fixtures（`hongniu_*.json`、`hongniu_*.xml`）。保留真实书源规则 fixture `legado_star_free_novel.json`（它是源规则，非 mock 响应）。mock 的本质是用捏造数据冒充真实站点，与「不允许 mock」直接冲突。
- **KD2. 结构断言，不硬编码业务值。** 断言产出非空 + `NodeData` variant 正确 + bookUrl/chapterUrl 绝对 URL 格式 + 章节列表非空。不断言具体标题/ID/章节数——这些随源站内容变动，硬编码即持续维护负担。解析器回归（丢字段、空章节、类型错）仍被抓。
- **KD3. 全 `#[ignore]` 手动触发，不进 CI 阻塞。** 纯实时测试天然 flaky，进 CI 会因无关原因（源站宕机）阻塞提交。统一 `#[ignore]` + 文档化手动跑命令。`pnpm test:rs` 集成部分默认零用例执行。
- **KD4. `HttpNodeProcessor::new()`（SSRF 开，生产路径）。** 不再用 `new_test()`（关 SSRF，mock 用）。真实打站走与生产相同的 SSRF 防护路径，测试通过即代表生产能跑。
- **KD5. 复用 `common/mod.rs` 共享 helper。** `execute_and_collect`、`init_tracing`、`extract_books`/`extract_videos`、`is_absolute_url`、`print_diagnostics` 已存在，重设计后 `test_mode` 参数固定 `false`（真网络）。不重写已有 helper。
- **KD6. 留 `.integration-tests/`（gitignored 本地保留）。** 沿用现有约定：整个目录 gitignore，本地保留测试代码与 fixtures，fresh clone 需本地重建。符合 ADR-0018 离线定位与仓库既有 `.gitignore` 规则。

### Key Flows

- F1. 统一测试路径（每条测试同构，仅源/端点不同）
  - **Trigger:** 手动 `cargo test --manifest-path .integration-tests/Cargo.toml -- --ignored`。
  - **Actors:** 测试 runner。
  - **Steps:** 读取真实源（Legado 书源 JSON 规则 fixture / Maccms 采集 URL 字符串）→ `Importer.import()` 产出 `Graph`（Legado 走第三方规则翻译，Maccms 走协议适配）→ `execute_and_collect` 用 `HttpNodeProcessor::new()`（SSRF 开）执行指定 `SegmentSpec` 端点 → 真实 HTTP 请求打源站 → 收集 `Vec<NodeData>`。
  - **Outcome:** 对产出做结构断言（非空 + variant + 绝对 URL + 章节非空）。
  - **Covered by:** R1, R2, R3, R4, R5.

### Requirements

**测试轨与门控**

- R1. 集成测试全部 `#[ignore]`，手动 `--ignored` 触发，不进 `pnpm test:rs` 默认路径。
- R2. 每条测试用 `HttpNodeProcessor::new()`（SSRF 防护开启，生产路径），禁用 `new_test()`。

**源覆盖**

- R3. Legado 真实书源一条全链路：复用 `legado_star_free_novel.json`（真实源 `https://cn.zhys.tw`），跑 search/discover/detail/toc/content 五端点串联，后续端点依赖前一端点产出（bookUrl/chapterUrl）。
- R4. 真实 Maccms10 采集 URL（红牛源 JSON 格式）全链路：discover（`ac=list`）→ detail（`ac=detail&ids=<vod_id>`），验证 `VideoMedia` 与 `play_lines` 解析。双端点已实测 HTTP 200 返回有效数据，具体地址见 `.tmp/hongnu.txt`。

**断言策略**

- R5. 结构断言：产出 `Vec<NodeData>` 非空 + 预期 variant（Legado→`Media::Book`，Maccms→`Media::Video`）+ bookUrl/chapterUrl 为绝对 `http(s)://` URL + 章节列表/分集列表非空。不断言具体标题、ID、章节数等业务值。

**清理**

- R6. 删除全部 mock 测试文件（`legado_wiremock.rs`、`maccms_json.rs`、`maccms_xml.rs`、`real_source_maccms.rs`）与 mock 响应 fixtures（`hongniu_list.json`、`hongniu_list.xml`、`hongniu_detail_sample.json`、`hongniu_detail_sample.xml`）。
- R7. 删除 `wiremock` 依赖（`.integration-tests/Cargo.toml`）。
- R8. 删除孤儿文件 `.integration-tests/recordings/discover_output.json`（无测试引用）。
- R9. 修复 `real_source_legado.rs` 对不存在 fixture `legado_synthetic_source.json` 的引用，改为复用 `legado_star_free_novel.json` 真实源。

### Acceptance Examples

- AE1. **Covers R3, R5.** Given Legado 测试手动触发。When 源站 `https://cn.zhys.tw` 在线且规则有效。Then search 产出非空 `BookMedia` 列表，首条 `book_url` 为绝对 URL；detail/toc/content 串联产出非空 `BookMedia`，toc 章节 `chapter_url` 非空，content 正文 `title` 字段非空。
- AE2. **Covers R4, R5.** Given Maccms 测试手动触发。When 真实采集 URL 在线。Then discover 产出非空 `VideoMedia` 列表且 `play_lines` 为空（list 端点无播放线路）；detail 产出 `VideoMedia` 且 `play_lines` 非空（含线路与分集）。
- AE3. **Covers R1, R3, R4.** Given 源站不可达或改版。When 测试手动触发。Then 测试失败（panic/超时），不跳过——失败信号即「源此刻不可用」，是预期行为而非 flaky 噪音。
- AE4. **Covers R6, R7, R8, R9.** Given 重设计完成。When `cargo test --manifest-path .integration-tests/Cargo.toml -- --ignored` 运行。Then 仅剩纯实时测试，无 wiremock 依赖，无 mock fixture，无坏引用。

### Scope Boundaries

**Deferred for later**

- 多源同类覆盖（如多个 Legado 源、多个 Maccms 源）——首刀每类一源跑通即可，扩源待首条稳定后按需加。
- 测试结果的录制/快照留存（用于人工对比源站历史变化）——非本轮范围。

**Outside this product's identity**

- VCR/录制回放机制——用户明确「不允许 mock」，VCR 的录制数据本质是历史响应回放，与定位冲突。
- 集成测试进 CI 自动化——纯实时测试 flaky，进 CI 阻塞路径与离线桌面应用定位冲突。

### Dependencies / Assumptions

- **网络依赖：** 测试运行机器需能直连目标源站（`https://cn.zhys.tw`、待选 Maccms URL）。SSRF 防护在 `HttpNodeProcessor::new()` 开启，源站域名需通过 SSRF 校验（非环回/非内网）。
- **源站存活假设：** 测试通过前提是源站此刻在线、规则未被源站改版失效。这是已知 flaky 来源，接受。
- **Maccms 真实源：** 选定的红牛 JSON 源，list/detail 双端点实测 HTTP 200。具体 URL 及 xml/m3u8/yun 变体域名见 `.tmp/hongnu.txt`（本地，不入计划）。
- **fixtures 路径：** `.integration-tests/fixtures/legado_star_free_novel.json` 保留（真实书源规则），其余 mock 响应 fixtures 删除。

### Outstanding Questions

**Resolve Before Planning**

- Q1. Maccms10 真实采集 URL 具体选哪个？现有红牛源 API（`hongniu_*.json` 对应源）是否仍在线？需在规划阶段验证可达性，不可达则另选。

**Deferred to Planning**

- Q2. 测试文件如何拆分？按源（`real_legado.rs` / `real_maccms.rs`）还是合并单文件？属实现层组织，留 ce-plan。
- Q3. `common/mod.rs` 中 `expected_first_video`/`expected_first_video_xml`（动态期望值解析）随 mock fixtures 删除后是否一并清理？结构断言不再需要值比对，大概率删，留 ce-plan 确认。

### Sources / Research

- 现有 mock 测试：`.integration-tests/tests/legado_wiremock.rs`、`maccms_json.rs`、`maccms_xml.rs`、`real_source_maccms.rs`（均用 `wiremock::MockServer` + `HttpNodeProcessor::new_test()`）。
- 现有真网络测试：`.integration-tests/tests/real_source_legado.rs`（`#[ignore]`，引用不存在的 `legado_synthetic_source.json` → 已坏）。
- 共享 helper：`.integration-tests/tests/common/mod.rs`（`execute_and_collect`、`build_processors(test_mode)`、`init_tracing`、`extract_books`/`extract_videos`、`is_absolute_url`）。
- 处理器路径：`src-tauri/crates/lj-node-http/src/processor.rs:121`（`new()` SSRF 开）、`:127`（`new_test()` SSRF 关）。
- Legado 真实源：`.integration-tests/fixtures/legado_star_free_novel.json`（`bookSourceUrl=https://cn.zhys.tw`）。
- Maccms 导入：`src-tauri/crates/lj-importer/src/maccms/mod.rs:29`（`Importer<MaccmsSourceUrl>`，协议适配路径）。
- 真实源地址清单：`.tmp/hongnu.txt`（红牛源 JSON/XML/m3u8/yun 变体域名，本地 gitignored）；`.tmp/maccms-protocol-audit.md`（Maccms 协议核验，含可达性标注）。
- 依赖清单：`.integration-tests/Cargo.toml`（含 `wiremock = "0.6"`，待删）。
- 离线定位：`docs/adr/0018-offline-app-no-backend-no-auth-no-sync.md`。
- 首刀范围：`docs/adr/0001-tracer-bullet-legado-book-source.md`（Legado 首刀 + 第二类源验证抽象）。
