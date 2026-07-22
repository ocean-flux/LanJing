# LanJing 设计系统（Ethereal 工作台）

> 版本：2026-07-21 · 范围：**工作台壳与管理主路径**。媒体沉浸（阅读器/播放器/mini-player）后置，不在本版交付。

## Overview

览境是本地优先的跨媒体工作台。本版视觉北极星是 **Ethereal Glass 工作台**：OLED/冷灰画布、侧/底 **浮动玻璃岛**导航、限定磨砂、double-bezel 面板、精密 cubic-bezier 动效。

主题模型对齐 VS Code：**暗色主题与亮色主题分轨独立选择**，不是「一 pack 亮暗镜像」。

### 原则覆盖（相对历史 quiet-precision）

| 旧 | 现 |
| --- | --- |
| 装饰玻璃默认禁止 | 壳层/浮层允许 Ethereal 玻璃，可降级 |
| inkstone / cinnabar 默认 | 四主题见下；禁黄橙紫靛主色 |
| 全局搜索等空壳 | 可不实现 |

### design-taste 边界

产品工作台，非营销落地页。`VARIANCE≈5 / MOTION≈4 / DENSITY≈6`。不采用落地页 hero/bento 教条；保留 Lucide + shadcn。

## 色相禁令（硬）

**禁止**作为品牌强调、CTA、导航激活、mesh、主题色石：

- 黄、橙、琥珀、朱砂、暖棕黄
- 紫 / violet / lila / 靛紫蓝（如 `#3b5bdb`、`#a78bfa`）

**允许** lantern：冷青、青钢、海洋青、真钢蓝（低红）+ zinc/slate 中性。

语义 `warning`/`danger` 仅表义、低饱和，不作主题主色。

## 主题目录

| id | face | 默认 | lantern |
| --- | --- | --- | --- |
| `obsidian-void` | dark | 默认暗 | `#6ec8d4` |
| `graphite-atelier` | dark | | `#5b9fd4` |
| `porcelain-day` | light | 默认亮 | `#0f6e7a` |
| `mist-studio` | light | | `#0e7490` |

持久化：`mode` + `lightThemeId` + `darkThemeId`。  
设置页分轨列表；Island Expand 仅链到外观，不铺主题网格。

迁移：`inkstone-precision` / `paper-lantern-*` → porcelain / obsidian；`cold-cinnabar` → mist / graphite（**丢弃橙**）。

## 壳：Glass Island Adaptive

- 桌面：侧浮玻璃脊岛，四境一键。
- 移动：底浮玻璃岛。
- Island Expand：玻璃展开 + stagger；次要入口（设置等），**非搜索产品**。
- titlebar：`data-tauri-drag-region` + 窗控；岛 no-drag。
- 无 mini-player 消费条（本版）。

## 材质

- double-bezel；blur 仅 fixed/sticky 壳与浮层。
- `prefers-reduced-transparency` / 移动：降 blur 或实色。
- 动效仅 `transform`/`opacity`；`prefers-reduced-motion` 瞬时终态。

## 字体

Outfit（UI）· JetBrains Mono（代码）· Source Serif 4（阅读预留，本版不进 chrome）。

## 与 sources 功能树

`07-20-sources-prod` 负责装源业务合同。  
视觉/材质由 `07-21-ui-*`（尤其 `ui-views-settings-sources-ops`）按本文重做；IA（装源区 + denselist + chip）可保留。

## 后置

文本/漫画阅读器、音视频播放、mini-player、封面飞越、真实全局搜索。
