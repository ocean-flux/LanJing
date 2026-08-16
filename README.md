<div align="center">

<img src="static/brand/icon.png" width="120" alt="LanJing icon" />

# LanJing / 览境

本地优先的跨媒体发现、展示与阅读工作台

[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri)](https://tauri.app)
[![React](https://img.shields.io/badge/React-19-149ECA?logo=react)](https://react.dev)
[![Vite](https://img.shields.io/badge/Vite-8-646CFF?logo=vite)](https://vite.dev)
[![Tailwind CSS](https://img.shields.io/badge/Tailwind_CSS-4-06B6D4?logo=tailwindcss)](https://tailwindcss.com)
[![Rust](https://img.shields.io/badge/Rust-stable-000000?logo=rust)](https://www.rust-lang.org)

</div>

## 产品定位

LanJing 是规则驱动的本地媒体工作台。来源规则只输出标准媒体模型和 `PresentationHint`，React 前端负责发现、收藏、阅读和播放体验。应用没有云端后端、登录或同步假设，用户的规则、历史和资料库留在本机。

```text
媒体源规则 → 标准媒体模型 → React 模板 → 阅读 / 播放 / 收藏 / 管理
```

支持文本、图像、音频、视频和混合内容。当前新架构提供境场、应用、来源管理、规则、资料库和设置工作区；规则编辑器目前是语义图的只读投影加布局保存，完整编辑能力仍在迁移中。

## 技术栈

| 层 | 技术 |
| --- | --- |
| 应用壳 | Tauri 2，跨 Windows/macOS/Linux/iOS/Android |
| 前端框架 | React 19 + React Router |
| 构建工具 | Vite 8，输出 `build/` |
| 样式 | Tailwind CSS 4 + Base UI + shadcn `base-lyra` registry |
| 图标 | Iconify（`@iconify/tailwind4` + Phosphor 图标集，构建期内联） |
| 状态 | zustand，偏好经 `@tauri-store/zustand` 持久化到应用数据目录 |
| 本地化 | inlang/paraglide，边界消息表位于 `src/shared/i18n` |
| 后端 | Rust edition 2024，IPC 契约保持不变 |

Tauri 配置继续使用 `frontendDist: ../build`、开发端口 `1420` 和 SPA fallback。

## 快速开始

环境要求：Rust stable、Node.js 20+、pnpm 11，以及目标平台的 Tauri 工具链。

```bash
pnpm install
pnpm dev              # Vite 开发服务器，端口 1420
pnpm tauri dev        # 启动 Tauri 应用
```

常用质量检查：

```bash
pnpm check            # Oxlint + TypeScript + Oxfmt + Vitest
pnpm build            # 生成 build/，供 Tauri frontendDist 使用
pnpm test:web         # React Testing Library + Vitest
pnpm tauri build      # 打包 Tauri 应用
```

Rust workspace 使用 Cargo：

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --workspace
```

## 目录与迁移边界

```text
src/
├── app/                 # AppShell、Titlebar、AppSidebar
├── components/          # Icon、PageToolbar 与 ui/ 下的 registry 原语
├── features/            # realm、apps、sources、rules、library、settings 工作区
├── shared/              # 配置、消息、导航、主题、Tauri 入口和媒体类型
├── index.css            # 主题 token 层与四套 appearance pack 色值
├── App.tsx              # BrowserRouter 路由与全局启动
└── main.tsx             # React 19 createRoot 入口
src-legacy/              # 一次性 Svelte 迁移归档
src-tauri/               # Rust 后端和 Tauri 配置
messages/                # Paraglide 翻译源文件
```

`src-legacy/` 保留迁移前文件内容和用户改动，仅供追溯与后续迁移参考。新代码不得从该目录导入；规则领域算法和 Rust IPC 契约也不得因前端切换而重写。

## 许可证

LanJing 以 [CC BY-NC-SA 4.0](LICENSE) 发布。
