<div align="center">

<img src="static/brand/icon.png" width="120" alt="LanJing icon" />

# LanJing / 览境

一流前端体验驱动的跨媒体发现、展示与阅读工作台

[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri)](https://tauri.app)
[![Svelte](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte)](https://svelte.dev)
[![Tailwind CSS](https://img.shields.io/badge/Tailwind_CSS-4-06B6D4?logo=tailwindcss)](https://tailwindcss.com)
[![Rust](https://img.shields.io/badge/Rust-stable-000000?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/License-CC_BY--NC--SA_4.0-lightgrey.svg)](LICENSE)

</div>

## 产品愿景

LanJing（览境）不是普通规则执行器，也不是把抓取结果渲染成列表的工具壳。

它的最终形态是一个**规则驱动的本地媒体工作台**：不同媒体源通过规则输出固定数据模型，LanJing 用高质量前端模板统一呈现、组织和消费这些内容。

体验目标对标网易云音乐的发现与播放、Apple Books / iBooks 的书架与阅读、现代影视/漫画/播客客户端的详情页与沉浸式消费界面。规则解决“数据从哪里来”，LanJing 解决“内容如何被优雅地发现、理解、收藏、阅读和播放”。

## 核心思路

```text
媒体源规则 → 标准媒体模型 → 前端模板渲染 → 阅读 / 播放 / 收藏 / 管理
```

- **规则只产出数据**：搜索结果、详情、章节、曲目、播放地址、正文、图片等都归一到标准模型。
- **前端负责体验**：卡片、货架、榜单、详情页、阅读器、播放器都由 LanJing 模板系统渲染。
- **媒体类型平等**：小说、漫画、音乐、播客、影视、文章、图集、课程都不是特例。
- **本地优先**：单机桌面应用，无自有后端、无登录认证、无云端同步。
- **渐进加载**：列表可以轻，详情可补全，正文/播放资产按需解析。

## 支持的媒体形态

| 类型     | 示例                               | 核心体验                             |
| -------- | ---------------------------------- | ------------------------------------ |
| 文本     | 小说、文章、RSS、博客、长文        | 书架、目录、阅读进度、沉浸式阅读器   |
| 图像     | 漫画、图集、画集、摄影集           | 分页/长图阅读、画廊、封面墙          |
| 音频     | 音乐、播客、有声书、广播剧         | 专辑页、播放队列、进度记忆、后台播放 |
| 视频     | 电影、剧集、短视频、课程           | 海报墙、分集、线路/清晰度、播放器    |
| 混合内容 | 榜单、频道、专题、系列、歌单、书单 | 货架、集合页、相关推荐、跨媒体收藏   |

## 标准媒体模型

规则输出会被收敛到一组稳定模型，前端只依赖这些模型渲染：

- `SourceProfile`：来源、图标、能力、规则版本、风险提示。
- `MediaItem`：书、电影、专辑、播客、漫画、课程、文章等媒体主体。
- `MediaCollection`：榜单、书架、歌单、频道、专题、系列等集合。
- `MediaUnit`：章节、集、曲目、漫画页、文章段落、课程小节等消费单元。
- `MediaAsset`：正文、图片、音频流、视频流、字幕、封面、附件等载荷。
- `MediaRelation`：作者、系列、相似内容、上下集、同专辑、同频道等关系。
- `MediaAction`：继续加载、选择线路、切换清晰度、打开外部页面等动作。
- `PresentationHint`：封面比例、主色、卡片密度、首选模板等展示提示。

规则不能直接控制 UI，也不输出来源专属页面结构。它只能提供数据和有限展示提示；最终视觉由 LanJing 的模板系统决定。

## 前端体验目标

LanJing 的重点是一流数据展示能力，而不是功能堆叠。

- **发现页**：推荐流、横向货架、榜单、分类、最近更新、继续阅读/播放。
- **详情页**：沉浸式 hero、封面/海报、主信息、简介、创作者、标签、目录/分集/曲目、相关推荐。
- **阅读器**：文本排版、漫画翻页/长滚、图片浏览、进度记忆。
- **播放器**：音频/视频播放、播放队列、线路/清晰度、字幕和进度。
- **资料库**：收藏、历史、本地缓存、来源归属、跨媒体集合。
- **模板系统**：按 `media_kind + 数据完整度 + PresentationHint` 自动选择合适模板。

## 当前状态

当前阶段：**项目骨架**。

已完成：

- Tauri 2 桌面壳
- SvelteKit 2 + Svelte 5 SPA 基础架构
- Tailwind CSS 4 + shadcn-svelte
- 暗色/亮色/系统主题
- 自定义标题栏
- Toast 通知
- inlang/paraglide 多语言基础
- Rust IPC 示例命令

待落地：

- 标准媒体模型
- 规则导入与执行运行时
- 媒体模板系统
- 发现页、详情页、阅读器、播放器
- 本地资料库、历史、收藏与缓存

## 技术栈

| 层       | 技术                           |
| -------- | ------------------------------ |
| 桌面壳   | Tauri 2                        |
| 前端框架 | SvelteKit 2 + Svelte 5 runes   |
| 构建工具 | Vite 8                         |
| 样式     | Tailwind CSS 4 + shadcn-svelte |
| 图标     | Lucide                         |
| 本地化   | inlang/paraglide               |
| 后端     | Rust edition 2024              |

## 快速开始

### 环境要求

- Rust stable（通过 rustup 安装）
- Node.js（建议使用任意受支持版本管理器，如 mise、nvm、fnm 或系统包）
- pnpm 11.5.2
- 平台工具链：
  - Windows: WebView2, MSVC, CMake, Ninja, NASM
  - macOS: Xcode Command Line Tools
  - Linux: webkit2gtk, libssl-dev, pkg-config

### 安装与运行

```bash
pnpm install
pnpm tauri dev
```

### 构建

```bash
pnpm tauri build
```

## 常用命令

约定：**前端与 Tauri 壳相关命令使用 `pnpm`；Rust workspace 命令直接使用 `cargo --manifest-path src-tauri/Cargo.toml`，不再经 `pnpm` 包装。**

```bash
pnpm dev              # 前端开发服务器（端口 1420）
pnpm build            # 前端生产构建（输出 build/）
pnpm tauri dev        # 启动 Tauri 桌面应用
pnpm tauri build      # 构建桌面应用
pnpm check            # 完整前端检查（lint + typecheck + format）
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --workspace
```

## 文档

- **[贡献指南](CONTRIBUTING.md)** - 如何参与开发
- **[安全政策](SECURITY.md)** - 安全报告和边界
- **[免责声明](DISCLAIMER.md)** - 使用责任和边界
- **[许可证](LICENSE)** - CC BY-NC-SA 4.0

## 许可证

LanJing 以 [CC BY-NC-SA 4.0](LICENSE) 许可证发布。
