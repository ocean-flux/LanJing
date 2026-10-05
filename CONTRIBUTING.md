# Contributing to LanJing

感谢你对 LanJing 的关注！我们欢迎各种形式的贡献。

## 开发环境

- Rust stable 工具链
- Node.js 20+
- pnpm 12（通过 `corepack enable` 启用）
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
pnpm test:web         # Vitest + React Testing Library（jsdom 环境，setup 在 src/test/setup.ts）
pnpm check            # 完整前端检查
```

Tauri 配置使用 `frontendDist: ../build`，开发端口固定为 `1420`。Rust workspace 命令直接使用 Cargo：

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --workspace
```

## 质量门禁

推送前跑一条命令，它逐条对应 `git push` 的 pre-push 门禁：

```bash
pnpm verify   # pnpm check + cargo clippy --all-features -D warnings + cargo test --workspace
```

钩子本身（`.git/hooks/pre-push`）做同样的事，但有两点要知道：

- **钩子按 PATH 解析 lefthook 二进制，优先于仓库 pin 的版本。** PATH 上有别的 lefthook 时门禁会静默用旧版（`git push` 输出里的 `lefthook vX.Y.Z` 就是实际用的版本）；要强制用仓库 pin 的那份就设 `LEFTHOOK_BIN`。
- **门禁失败时不保证打印失败步骤的输出。** 推送被拒后先跑 `pnpm verify` 看真实原因，不要靠钩子的输出猜。

### 已知陷阱

- **worktree 与共享 `CARGO_TARGET_DIR`**：本仓所有 worktree 共用主检出的 target 目录，而集成测试把 fixture 路径按**编译期**的 `CARGO_MANIFEST_DIR` 写死。删掉 worktree 之后，仍被当成“新鲜”的测试二进制会去读已不存在的路径，报出 `os error 3` 这类**假红**。所以删 worktree 必须排在最终验证之后；换 worktree 后先 `find src-tauri/crates src-tauri/src -name lib.rs | xargs touch` 再重编。
- **全新检出**：`src/shared/paraglide/` 是 gitignore 的生成物。先跑一次 `pnpm test:web`（或任何 vite/vitest）生成它，否则 `pnpm typecheck:web` 会以缺少 message key 报 TS2339。

## 项目结构

```text
src/
├── app/                 # AppShell、Titlebar、AppSidebar
├── components/          # Icon、PageToolbar 等通用件
│   └── ui/              # shadcn base-lyra registry 生成物（Base UI 原语）
├── features/            # realm、apps、sources、rules、library、settings、not-found
├── hooks/               # registry 附带的 use-mobile 等
├── shared/              # config、i18n、navigation、paraglide、tauri、theme、types
├── test/                # Vitest setup
├── index.css            # 主题 token 层与 appearance pack 色值
├── App.tsx              # BrowserRouter 和页面路线
└── main.tsx             # React 19 StrictMode 入口
src-tauri/               # Rust workspace、IPC 和 Tauri 配置
messages/                # en 与 zh-CN 翻译源，按命名空间分文件
project.inlang/          # Paraglide 项目配置
```

## 本地化

翻译源位于 `messages/{en,zh-CN}/*.json`，按命名空间分文件（`common`、`shell`、`realm`、`apps`、`sources`、`library`、`settings`、`rules`）。Vite 构建会生成 `src/shared/paraglide/`。React UI 文案优先通过 `src/shared/i18n` 边界读取，生成目录不手工编辑。两个 locale 的 key 集合必须保持一致。

## 代码规范

- TypeScript 使用 strict，组件优先保持小而聚焦。
- 使用 Oxfmt 格式化、Oxlint 检查，Tailwind utility classes 优先。
- UI 原语用 Base UI（`@base-ui/react`），组件从 shadcn `base-lyra` registry 取（`npx shadcn@latest add <name>`）。不引入 `radix-ui`。
- 图标统一走 Iconify，只用 `src/components/Icon.tsx` 的白名单（映射到 `icon-[ph--*]`）；类名必须是字面量，模板拼接不会被 Tailwind 扫描到。
- 颜色、间距、层级、圆角一律引用 `src/index.css` 的 token；不写字面量 `z-50`、`rounded-md` 或裸色值。
- 交互控件必须具备可访问名称和 `focus-visible` 状态。
- 跨组件状态用 zustand，需要持久化或跨窗口同步的偏好走 `@tauri-store/zustand`。
- 前端保持本地优先，不新增登录、云同步或云端后端假设。
- Rust 使用 `cargo fmt` 与 Clippy，不能通过无理由的 `#[allow]` 绕过警告。

`src/components/ui/**`、`src/shared/utils.ts` 与 `src/hooks/use-mobile.ts` 是 registry 生成物，但也纳入 Oxlint 与 Oxfmt，允许在保留 shadcn 可更新性的同时进行项目级魔改。每次 `shadcn add` 之后需把新组件里的 `@phosphor-icons/react` import 换成 `@/components/ui/icon-glyphs`，再运行格式化与 lint。

## Commit 规范

使用 Conventional Commits：

```text
<type>(<scope>): <subject>
```

类型包括 `feat`、`fix`、`refactor`、`docs`、`style`、`test`、`chore` 和 `perf`。提交前运行 `pnpm verify`。

## 文档规则

- 文档和注释使用中文，技术标识符保留英文。
- 不使用 Unicode 表情符号。
- 代码块使用带语言标记的三个反引号。
