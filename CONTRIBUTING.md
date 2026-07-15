# Contributing to LanJing

感谢你对 LanJing 的关注！我们欢迎各种形式的贡献。

## 开发环境

### 前置要求

- **Rust**: stable 工具链（通过 rustup 安装）
- **Node.js**: 20+
- **pnpm**: 通过 `corepack enable` 启用
- **系统依赖**:
  - Windows: WebView2, MSVC, CMake, Ninja, NASM
  - macOS: Xcode Command Line Tools
  - Linux: webkit2gtk, libssl-dev, pkg-config

### 安装

```bash
corepack enable
pnpm install
pnpm tauri dev
```

## 开发命令

约定：**前端与 Tauri 壳相关命令使用 `pnpm`；Rust workspace 命令直接使用 `cargo --manifest-path src-tauri/Cargo.toml`，不再经 `pnpm` 包装。**

### 前端

```bash
pnpm dev              # Vite 开发服务器（端口 1420）
pnpm build            # SvelteKit 生产构建（输出 build/）
pnpm lint:web         # ESLint 检查
pnpm typecheck:web    # svelte-check 类型检查
pnpm format:web       # Prettier 格式化
pnpm format:check:web # Prettier 格式检查
```

### Rust 后端

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --all-features -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --workspace
```

### Tauri 桌面应用

```bash
pnpm tauri dev        # 开发模式（热重载）
pnpm tauri build      # 生产构建
```

### 完整质量检查

```bash
pnpm check            # lint + typecheck + format 一站式检查
```

### 本地化

```bash
# 添加翻译键后，运行构建自动编译
pnpm build
```

翻译文件位于 `messages/{en,zh-CN}.json`，编译产物生成在 `src/lib/paraglide/`。

## 项目结构

```
LanJing/
├── src/                          # 前端源码
│   ├── app.html                  # HTML 模板
│   ├── routes/                   # SvelteKit 路由
│   │   ├── +layout.svelte        # 全局布局（AppShell）
│   │   ├── +layout.ts            # SSR 禁用配置
│   │   └── +page.svelte          # 首页
│   ├── lib/
│   │   ├── app/
│   │   │   └── AppShell.svelte   # 应用外壳（标题栏/导航/内容区/状态栏）
│   │   ├── components/ui/        # shadcn-svelte UI 组件
│   │   │   ├── button/
│   │   │   ├── input/
│   │   │   ├── label/
│   │   │   ├── separator/
│   │   │   ├── textarea/
│   │   │   └── sonner/           # Toast 通知
│   │   ├── stores/
│   │   │   └── theme.svelte.ts   # 主题 store（runes）
│   │   ├── config/
│   │   │   └── app.ts            # 应用配置
│   │   └── i18n.ts               # 本地化统一入口
│   └── index.css                 # Tailwind 全局样式 + CSS 变量
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs               # 桌面入口
│   │   └── lib.rs                # 插件注册 + 命令处理器
│   ├── Cargo.toml
│   └── tauri.conf.json
├── messages/                     # 翻译源文件
│   ├── en.json
│   └── zh-CN.json
└── project.inlang/               # inlang 配置
```

## 代码规范

### Rust

- 使用 `cargo fmt` 格式化
- Clippy 级别 `all` + `pedantic` = deny
- 不允许 `#[allow]` 绕过 warning
- 公开 API 必须有文档注释

### TypeScript / Svelte

- 使用 Prettier 格式化
- 通过 ESLint 检查
- Svelte 5 runes（`$state`、`$derived`、`$effect` 等）
- 组件 props 使用 `$props()` + TypeScript 类型
- Tailwind utility classes 优先于手写 CSS
- 状态管理优先用 `.svelte.ts` 中的 `$state`

### Commit 规范

使用 Conventional Commits：

```
<type>(<scope>): <subject>
```

**类型**：`feat` / `fix` / `refactor` / `docs` / `style` / `test` / `chore` / `perf`

**主题要求**：中文、祈使句、现在时、不加句号、50 字符以内。

**示例**：

```
feat(ui): 添加暗色主题切换按钮
fix(rust): 修复 single-instance 窗口聚焦
docs: 更新 README 开发命令说明
```

## 提交 Pull Request

1. Fork 仓库
2. 创建 feature 分支：`git checkout -b feat/my-feature`
3. 提交改动并确保 `pnpm check` 通过
4. 推送分支并创建 PR

### PR 检查清单

- [ ] `pnpm check` 通过
- [ ] `cargo clippy -- -D warnings` 通过
- [ ] `cargo test` 通过
- [ ] Commit 消息符合规范
- [ ] 文档已更新（如需要）

## 文档规则

- 所有文档使用中文编写
- 不使用 Unicode 表情符号（用文字或 Markdown 表达）
- 标题使用 `#` / `##` / `###`
- 代码块使用三个反引号并指定语言

## 许可证

所有贡献以 **CC BY-NC-SA 4.0** 许可证发布。
