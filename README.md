<div align="center">

# LanJing / 览境

跨平台媒体发现与阅读工作台

[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri)](https://tauri.app)
[![Svelte](https://img.shields.io/badge/Svelte-5-FF3E00?logo=svelte)](https://svelte.dev)
[![Tailwind CSS](https://img.shields.io/badge/Tailwind_CSS-4-06B6D4?logo=tailwindcss)](https://tailwindcss.com)
[![Rust](https://img.shields.io/badge/Rust-stable-000000?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/License-CC_BY--NC--SA_4.0-lightgrey.svg)](LICENSE)

</div>

## 项目状态

当前阶段：**项目骨架**。已完成基础架构搭建，待填充业务逻辑。

## 技术栈

| 层 | 技术 |
|---|------|
| 桌面壳 | Tauri 2 |
| 前端框架 | SvelteKit 2 + Svelte 5 (runes) |
| 构建工具 | Vite 8 |
| 样式 | Tailwind CSS 4 + shadcn-svelte |
| 图标 | Lucide |
| 本地化 | inlang/paraglide |
| 后端 | Rust (edition 2024) |

## 内置功能

- **暗色/亮色/系统主题** — 带 localStorage 持久化和系统偏好自动检测
- **自定义标题栏** — 无边框窗口，原生窗口控制按钮（最小化/最大化/关闭）
- **Toast 通知** — 基于 svelte-sonner
- **多语言** — 中/英文，inlang/paraglide 编译时类型安全
- **IPC 示例** — `greet` / `ping` Rust 命令

## 快速开始

### 环境要求

- Rust stable (通过 rustup 安装)
- Node.js 20+
- pnpm（通过 `corepack enable` 启用）
- 平台工具链：
  - Windows: WebView2, MSVC, CMake, Ninja, NASM
  - macOS: Xcode Command Line Tools
  - Linux: webkit2gtk, libssl-dev, pkg-config

### 安装与运行

```bash
corepack enable
pnpm install
pnpm tauri dev
```

### 构建

```bash
pnpm tauri build
```

## 常用命令

```bash
pnpm dev              # 前端开发服务器（端口 1420）
pnpm build            # 前端生产构建（输出 build/）
pnpm tauri dev        # 启动 Tauri 桌面应用
pnpm tauri build      # 构建桌面应用
pnpm check            # 完整前端检查（lint + typecheck + format）
pnpm lint:rs          # Rust clippy
pnpm test:rs          # Rust 测试
```

## 文档

- **[贡献指南](CONTRIBUTING.md)** - 如何参与开发
- **[安全政策](SECURITY.md)** - 安全报告和边界
- **[免责声明](DISCLAIMER.md)** - 使用责任和边界
- **[许可证](LICENSE)** - CC BY-NC-SA 4.0

## 许可证

LanJing 以 [CC BY-NC-SA 4.0](LICENSE) 许可证发布。
