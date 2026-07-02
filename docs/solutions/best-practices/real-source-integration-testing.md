---
module: '.integration-tests'
date: '2026-07-02'
problem_type: best_practice
component: testing_framework
severity: medium
tags:
  - integration-testing
  - real-source
  - no-mock
  - legado
  - maccms
  - ssrf
---

# Real-Source Integration Testing Pattern

## Context

LanJing 的集成测试需要验证「导入->Graph->执行端点」全链路是否跑通。传统 wiremock mock 测试用捏造数据冒充真实站点，源站改版/反爬/字段漂移在 mock 里永远绿——测试通过不代表源真能用。对于规则驱动的媒体源应用，mock 测试的保真度不足以充当质量护栏。

## Guidance

**纯实时打站，零 mock。** 每条集成测试真实请求目标源站，结构断言（非空 + variant + URL 格式），不硬编码业务值。

### 关键设计决策

| 决策                                  | 理由                                                                                                |
| ------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `HttpNodeProcessor::new()`（SSRF 开） | 测试走与生产相同的 SSRF 防护路径，测试通过即代表生产能跑                                            |
| 结构断言，不硬编码业务值              | 断言产出非空 + `NodeData` variant 正确 + 绝对 URL + 章节非空。不断言具体标题/ID——这些随源站内容变动 |
| 目录 gitignore，本地保留              | `.integration-tests/` 整体不入版本库，fresh clone 需本地重建。符合离线桌面应用定位（ADR-0018）      |
| 复用 `common/mod.rs` 共享 helper      | `execute_and_collect`、`init_tracing`、`extract_*` 等已存在，不重写                                 |

### 测试文件结构

```text
.integration-tests/
  Cargo.toml              # 独立 workspace crate，无 wiremock 依赖
  fixtures/
    legado_star_free_novel.json  # 真实书源规则（非 mock 响应）
  tests/
    common/mod.rs          # 共享 helper（execute_and_collect, init_tracing 等）
    real_source_legado.rs  # Legado 全链路：search->discover->detail->toc->content
    real_maccms.rs         # Maccms 全链路：discover->detail
```

### 测试模式

```rust
// common/mod.rs — test_mode=false 走生产路径
pub fn build_processors(test_mode: bool) -> HashMap<NodeKind, Box<dyn NodeProcessor>> {
    let http = if test_mode {
        HttpNodeProcessor::new_test()  // SSRF 关（已无调用方）
    } else {
        HttpNodeProcessor::new()       // SSRF 开 = 生产路径
    };
    // ...
}
```

### 结构断言模板

```rust
// 非空 + variant + 绝对 URL
let books = extract_books(&results);
assert!(!books.is_empty(), "应产出 BookMedia");
assert!(is_absolute_url(&books[0].book_url.clone().unwrap()));
```

### 跑法

```bash
cd .integration-tests/
cargo test                                    # 全部
cargo test --test real_source_legado          # 只跑 Legado
cargo test --test real_maccms                 # 只跑 Maccms
```

## Why This Matters

真实源是验证全链路的唯一保真手段。mock 无法暴露源站改版、反爬、字段漂移。测试通过即代表源此刻真能跑通——这是源接入 QA 的核心价值。

代价：测试天然 flaky（源站宕机/改版），不能进 CI 阻塞路径，只能手动触发。`pnpm test:rs` 集成部分默认跑零测试。

## When to Apply

- 新增媒体源类型时，写一条真实源全链路测试验证接入
- 源站规则变更后，手动跑对应测试验证兼容性
- 调试源接入问题时，先跑集成测试确认是源站问题还是解析器问题

## Examples

### Legado 全链路（5 端点串联）

```rust
#[tokio::test]
async fn test_real_source_full_pipeline() {
    let (graph, base_url) = import_source();  // legado_star_free_novel.json
    // search -> detail -> toc -> content，后续端点依赖前一端点产出
    let search_results = execute_and_collect(&graph, SegmentSpec { ... }, &base_url).await;
    let book_url = extract_books(&search_results)[0].book_url.clone();
    // ... 串联 detail/toc/content
}
```

### Maccms 全链路（2 端点）

```rust
#[tokio::test]
async fn test_maccms_real_full_pipeline() {
    let (graph, base_url) = import_source();  // 红牛 JSON 真实 URL
    let discover_videos = extract_videos(&discover_results);
    let vod_id = discover_videos[0].vod_id.clone();
    // detail 依赖 discover 产出的 vod_id
    let detail_videos = extract_videos(&detail_results);
    assert!(!detail_videos[0].play_lines.is_empty());
}
```
