# Product

<!-- impeccable:product-schema 1 -->

## Platform

adaptive

## Users

主要用户是项目作者本人。他在自己的机器上导入来源定义、维护规则、管理资料库，并进入应用面沉浸阅读或播放。

项目开源，其他想用的人可以自行安装使用。他们不是设计目标：产品不为其提供内置来源商店、账号或托管服务。

## Product Purpose

LanJing 让用户在一台本机上发现、收藏、阅读与播放跨媒体内容。成功意味着作者日常的跨媒体阅读体验优于任何单一媒体阅读器，且全部资料留在本机。

## Positioning

跨媒体统一资料库加上按媒体类型做深的沉浸式应用面。邻居产品（Legado / 阅读 类）通常只覆盖一种媒体或只做列表式阅读。LanJing 的差异是同一资料库覆盖文本、图像、音频、视频，并为每种媒体给出自己的交互语言。

## Operating Context

- 用户在 Windows / macOS / Linux / iOS / Android 上运行同一个 Tauri 2 应用。
- 用户提供来源定义来创建来源。当前支持 Legado JSON 与 Maccms URL 两种格式。
- 来源通过 Source Revision 安装与更新。更新与回退都要重新审阅候选并确认授权。
- 工作台有六个工作区：境场、应用、来源、规则、资料库、设置。
- 阅读与播放发生在 `/apps` 下的应用面，进入后隐藏工作台 chrome。
- 开发与质量门禁走 `pnpm verify`：Oxlint、tsc、Oxfmt、Vitest、cargo clippy、cargo test。

## Capabilities and Constraints

- 规则只输出标准媒体模型与 `PresentationHint`，不定义 UI 布局或来源专属页结构。
- 双层架构：工作台层克制、效率优先，沿用 base-lyra 母版；应用面层有自己的交互语言，不受直角、`text-xs` 基线、32px 控件约束。
- 色值唯一来源是 `src/index.css` 的四个 appearance pack。应用面不得另存调色板。
- 无云端后端、无登录、无同步。规则、历史与资料库留在本机。
- 凭证明文不进 `tauri-store`。
- Rust crate 分层单向无环，应用层不绕过 `lj-rule-system` 门面。
- 当前状态：六个工作区已建。五个应用面 `reading` / `gallery` / `podcast` / `video` / `music` 仍 `enabled: false`。
- 领域术语以 `CONTEXT.md` 为准。

## Brand Commitments

- 名称：览境 / LanJing。
- 许可：CC BY-NC-SA 4.0。
- 语气：中文、克制、术语精确。英文只用于标识符、命令与技术专名。
- 领域术语表在 `CONTEXT.md`。后续工作不得另起同义术语。

## Evidence on Hand

- 仓库当前只有代码与文档。无截图、无演示、无用户证言、无基准数据。
- 后续工作不得编造用户数量、性能数字、客户或合作方。

## Product Principles

1. 应用面做深，工作台克制。任何「这个功能该多复杂」的问题先回答「它在哪一层」。
2. 规则与呈现解耦。新增来源能力不改前端模板。
3. 本地是唯一真相。需要服务器才能成立的功能不做。
4. 不为假想用户加功能。内置来源商店、账号体系、云同步都不在范围内。
