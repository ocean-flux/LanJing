---
title: 媒体模型设计
date: '2026-07-01'
category: architecture-patterns
module: lj-core
problem_type: architecture_pattern
component: development_workflow
severity: medium
applies_when:
  - '理解 Media 类型层次结构'
  - '新增媒体类型或修改现有类型'
  - '前端消费 Media 数据时'
tags:
  - media-model
  - bookmedia
  - videomedia
  - audiomedia
---

# 媒体模型设计

分层媒体模型，`Media` 抽象根统摄各类媒体实体。

## Context

三类异构源（书源/视频源/音源）产出的媒体数据结构差异巨大。需要一个统一的类型系统，既能让前端 discriminate-by-kind，又不会因 flat 设计丢失类型信息。

## Guidance

### Media 枚举

```rust
enum Media {
    Book(BookMedia),
    Video(VideoMedia),
    Audio(AudioMedia),
}
```

- **Enum trait dispatch**：`MediaKind` 闭集枚举
- **分层而非 flat**：三类源专属字段差异巨大，flat 会让前端到处 `if media.kind === 'book'`，把类型分支逻辑漏到前端
- **存储**：SQLite `media` 表，fields_json 按 kind 存储不同字段集

### VideoMedia 嵌套结构

```rust
struct PlayLine {
    name: String,
    episodes: Vec<VideoEpisode>,
}
```

**否决扁平设计**：`chapters + play_from` 双并列字段丢失线路↔分集绑定。嵌套正确保留线路与分集位置关系。

### Variants 详情

| Variant    | 用途        | 核心字段                                  |
| ---------- | ----------- | ----------------------------------------- |
| BookMedia  | 小说/文章类 | 章节列表 + 正文文本                       |
| VideoMedia | 视频/影视类 | 多播放线路 + 分集列表 + 流地址 + 弹幕字段 |
| AudioMedia | 音频类      | 音源切换 + 歌词                           |

### 节点数据中的 Media

`Media` 是 `NodeData` enum 的 variant 之一——规则执行时节点间传递的数据载荷可含 `Media`，流式产出最终也是 `Media` stream。

## Why This Matters

分层强类型使前端消费端 discriminate-by-kind 由编译器/类型系统强制，而非运行时字符串比较。VideoMedia 的嵌套结构保留了线路与分集的位置关系，扁平设计会丢失这个信息。

## When to Apply

- 新增媒体类型：添加 `Media` enum variant + 对应 struct
- 修改视频源结构：调整 `PlayLine` / `VideoEpisode` 嵌套
- 前端渲染 Media：按 `Media` kind 分发渲染

## Related

- `docs/solutions/architecture-patterns/rule-engine.md` — 规则执行产出 Media
- `docs/solutions/architecture-patterns/import-system.md` — 导入器产出 Media
