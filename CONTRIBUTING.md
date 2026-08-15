# Contributing to LanJing

感谢你对 LanJing 的关注！我们欢迎各种形式的贡献。

## 开发环境

- Rust stable 工具链
- Node.js 20+
- pnpm 11（通过 `corepack enable` 启用）
- Tauri 平台依赖：Windows WebView2/MSVC、macOS Xcode Command Line Tools，或 Linux webkit2gtk/libssl-dev/pkg-config

```bash
corepack enable
pnpm install
pnpm tauri dev
```

## 开发命令

```bash
pnpm dev              # Vite 开发服务器，端口 1420
pnpm build            # React 生产构建，输出 build/
pnpm lint:web         # Oxlint
pnpm typecheck:web    # TypeScript strict 检查
pnpm format:web       # Oxfmt 格式化新前端源码
pnpm format:check:web # Oxfmt 格式检查
pnpm test:web         # Vitest + React Testing Library
pnpm check            # 完整前端检查
```

Tauri 配置使用 `frontendDist: ../build`，开发端口固定为 `1420`。Rust workspace 命令直接使用 Cargo：

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --workspace
```

## 项目结构

```text
src/
├── app/                 # AppShell、标题栏、BottomNav 和启动逻辑
├── components/ui/       # shadcn/ui 风格 React 原语
├── features/            # realm、apps、sources、library、settings
├── shared/              # config、i18n、tauri、types、theme
├── App.tsx              # BrowserRouter 和页面路线
└── main.tsx             # React 19 StrictMode 入口
src-legacy/              # 一次性 Svelte 迁移归档，不得被新代码导入
src-tauri/               # Rust workspace、IPC 和 Tauri 配置
messages/                # en 与 zh-CN 翻译源
project.inlang/          # Paraglide 项目配置
```

`src-legacy/` 必须保持为迁移归档。新 React 代码不得导入其中的 Svelte 组件或旧 store；纯 TypeScript 领域算法如需迁移，应复制到新的共享边界并保持契约，不要改动 Rust IPC。

## 本地化

翻译源位于 `messages/{en,zh-CN}.json`，Vite 构建会生成 `src/shared/paraglide/`。React UI 文案优先通过 `src/shared/i18n` 边界读取，生成目录不手工编辑。

## 代码规范

- TypeScript 使用 strict，组件优先保持小而聚焦。
- 使用 Oxfmt 格式化、Oxlint 检查，Tailwind utility classes 优先。
- 交互控件必须具备可访问名称和 `focus-visible` 状态，图标按钮使用 lucide-react。
- 前端保持本地优先，不新增登录、云同步或云端后端假设。
- Rust 使用 `cargo fmt` 与 Clippy，不能通过无理由的 `#[allow]` 绕过警告。

## Commit 规范

使用 Conventional Commits：

```text
<type>(<scope>): <subject>
```

类型包括 `feat`、`fix`、`refactor`、`docs`、`style`、`test`、`chore` 和 `perf`。提交前运行 `pnpm check`，并确保没有将 `src-legacy/` 中的归档内容作为新代码依赖。

## 文档规则

- 文档和注释使用中文，技术标识符保留英文。
- 不使用 Unicode 表情符号。
- 代码块使用带语言标记的三个反引号。
