# LanJing / 览境 — Agent Instructions

Local-first cross-media discovery & reading workbench.
Stack: **Tauri 2 + React 19 / React Router 7 + Vite 8 + Tailwind 4 + Base UI + Paraglide JS + Rust workspace**。
当前 React 源码位于 `src/`。

UI primitive 是 **Base UI**（`@base-ui/react`），不使用 `radix-ui` / `@radix-ui/*`。Base UI 用 `render` prop 承担 Radix `asChild` 的职责。
组件母版是 shadcn **`base-lyra`** registry（`components.json` 的 `style: "base-lyra"`，直角 / `text-xs` 基线 / `ring-1` / 控件 32px）。
新增组件先走 `npx shadcn@latest add <name>`（自动落到 base-lyra），不手写平行实现。
`Sidebar` / `Sheet` 是导航与移动端抽屉的基线，不手写平行的 sidebar 或 bottom navigation。
选型依据见 `docs/adr/0001-base-ui-and-base-lyra.md`。

**图标**：统一走 Iconify（`@iconify/tailwind4` + `@iconify-json/ph`），只允许 `src/components/Icon.tsx` 里的字面量白名单映射到 `icon-[ph--*]`。
Tailwind 静态扫描源码，`icon-[ph--${name}]` 这类模板拼接不会被识别，必须写字面量。不引入 `lucide-react` / `@phosphor-icons/react` 运行时依赖。
`components.json` 的 `iconLibrary: "phosphor"` 只用于让 CLI 生成语义正确的图标名；**每次 `shadcn add` 之后必须把 registry 组件里的 `@phosphor-icons/react` import 换成 `@/components/ui/icon-glyphs`**。

**registry 产物**：`src/components/ui/**` 与 `src/hooks/use-mobile.ts` 是 CLI 生成物，`.oxlintrc.json` 已对其豁免纯风格规则，改动应保持可被后续 `shadcn add` 升级。

Human docs: `README.md`, `CONTRIBUTING.md`.

## Platforms

目标平台：`windows` / `macos` / `linux` / `ios` / `android`（一码多端）。

前后端实现、依赖选型与 UI 都须面向上述平台；前端与共享逻辑避免桌面-only 假设。后端可按平台选用原生 API / 条件依赖，但能力应对齐、接口对上层保持一致。

## How to work

1. **开发基线**：动手前阅读 `README.md`、`CONTRIBUTING.md` 与受影响模块的实现和测试；完成后运行对应质量检查。
2. **产品硬边界**：本地-only（无云端后端 / 登录 / 同步假设）；规则只出标准媒体模型 + `PresentationHint`，不定义 UI 布局或来源专属页结构。
3. **文案与注释**：文档 / 需求说明 / 注释用中文；英文仅限标识符、命令、技术专名。生成物不手改（如 `src/shared/paraglide/**`）。
4. **主题与本地化**：色值的唯一来源是 `src/index.css` 里四个 `:root[data-appearance-pack='...']` 块；`src/shared/theme/` 只负责「选了哪套」并把选择写到 `<html>` 的 `class` / `data-appearance-pack` / `color-scheme` 上，不得在 JS 里再存一份调色板。业务代码用 `@theme inline` 暴露的语义色（`bg-surface-1` / `text-ink-muted` / `border-hairline`），不写字面量 `z-50` / `rounded-md`。前端文案必须通过 `src/shared/i18n` / Paraglide message functions，生成物 `src/shared/paraglide/**` 不手改。
5. **状态**：跨组件状态用 zustand；需要跨窗口同步或持久化到应用数据目录的偏好，用 `@tauri-store/zustand`（Rust 侧 `tauri-plugin-zustand`）。凭证明文永远不进 `tauri-store`。
6. **crate 分层**：Rust 依赖单向无环，应用层不绕过 `lj-rule-system` 门面。允许的依赖边写在 `src-tauri/tests/workspace_layering.rs` 的表里，改 `Cargo.toml` 前先读 `docs/adr/0008-rust-crate-layering.md`。

命令与质量门禁：`README.md`、`CONTRIBUTING.md`；推送前跑 `pnpm verify`（等于 pre-push 门禁）。

## Testing boundary

测试优先覆盖领域逻辑、workflow/model、storage、RuleSystem facade 和 IPC contract，并断言状态转换、持久化结果、错误边界与安全不变量。测试应使用确定性输入，优先放在离 UI 更深且稳定的公共契约上。

UI 单测只在用户可观察合同无法由逻辑或公共集成边界证明时使用，并限制为少量关键旅程。不要为 DOM 层级、CSS class、文案排列、快照或组件内部实现新增测试；UI 改动以 `lint`、`typecheck`、生产构建和必要的 Tauri 冒烟验证。


## Agent skills

### Issue tracker

事项与规格记录在 GitHub Issues（`ocean-flux/LanJing`），用 `gh` 操作。见 `docs/agents/issue-tracker.md`。

### Triage labels

使用默认五个 triage role 字符串。见 `docs/agents/triage-labels.md`。

### Domain docs

使用单上下文领域文档：根 `CONTEXT.md` 与 `docs/adr/`。见 `docs/agents/domain.md`。

<!-- OPENWIKI:START -->

## OpenWiki

This repository has a generated `openwiki/` evidence index. It is optional just-in-time context, not required startup reading.

- Do not enumerate, preload, or search wikis at task start. Use retrieval when the user asks for it, when unfamiliar architecture or dependency behavior materially affects the task, or when source inspection leaves an important uncertainty. Stop once the question is grounded.
- When those conditions apply and OpenWiki retrieval tools are available, use `openwiki_search` for just-in-time context and `openwiki_read` for the relevant complete sections. If search returns `workspace_required`, ask which listed workspace to use and retry with its ID.
- Use `openwiki_list_workspaces` or `openwiki_list_wikis` when workspace membership itself needs to be discovered.
- If the retrieval tools are unavailable, read `openwiki/quickstart.md` and follow its links to the relevant pages.
- Treat source code and tests as authoritative. A brief's unknowns and review items are verification gaps, not automatic requirements.
- Prefer the narrowest quiet validation that proves the changed behavior. Preserve complete failure output.

The scheduled OpenWiki GitHub Actions workflow refreshes the repository wiki. Do not hand-edit generated OpenWiki pages unless explicitly asked; prefer updating source code/docs and letting OpenWiki regenerate.

<!-- OPENWIKI:END -->
