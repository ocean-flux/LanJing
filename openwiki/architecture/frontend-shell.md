---
type: "参考"
title: "Frontend shell"
openwiki_generated: true
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T10:14:16.110Z
sources:
  - id: openwiki-source-6ae244f79c5e27a2b1f08014
    resource: repo://components.json
  - id: openwiki-source-05b14d59724ec4fa57d1c0cc
    resource: repo://docs/adr/0005-turn-engine-and-pagesource.md
  - id: openwiki-source-f8d10828394c4129061d5b0e
    resource: repo://index.html
  - id: openwiki-source-3ba9330c4c4b2f4f12cf8dc0
    resource: repo://project.inlang/settings.json
  - id: openwiki-source-54631e6ebf1d3b815c4a5eed
    resource: repo://src/App.tsx
  - id: openwiki-source-c190ec06ba456f970051d4a6
    resource: repo://src/app/AppShell.tsx
  - id: openwiki-source-d38fbc1bf8ab98fd1f88cd19
    resource: repo://src/app/AppSidebar.tsx
  - id: openwiki-source-b3516cdc6691e9dc7dcdb85a
    resource: repo://src/app/Titlebar.tsx
  - id: openwiki-source-3b892c8111758ccede956992
    resource: repo://src/components/Icon.tsx
  - id: openwiki-source-b3eeee8972a1c1a0ed428993
    resource: repo://src/features/apps/AppsHome.tsx
  - id: openwiki-source-5480c4953d60af0a3c82e354
    resource: repo://src/features/library/workflow.ts
  - id: openwiki-source-6c8341f83ff1af3d767eeeee
    resource: repo://src/features/realm/RealmHome.tsx
  - id: openwiki-source-e09aeeb6133666577e5f07d8
    resource: repo://src/features/rules/RuleWorkspace.tsx
  - id: openwiki-source-5a4431c711fe83d06dbf05b6
    resource: repo://src/features/sources/SourcesHome.tsx
  - id: openwiki-source-7c8446b7383bfa65b6d9e1b7
    resource: repo://src/index.css
  - id: openwiki-source-95bfccfd0c712f6e72040e0d
    resource: repo://src/main.tsx
  - id: openwiki-source-654a4c5463c7ee47452788e0
    resource: repo://src/shared/i18n/locale.tsx
  - id: openwiki-source-3f29fd103f5b0c1546441d4a
    resource: repo://src/shared/i18n/messages.ts
  - id: openwiki-source-ce5132a614aee620cd6c0c79
    resource: repo://src/shared/navigation.ts
  - id: openwiki-source-5e435213321f075c4b503f7d
    resource: repo://src/shared/theme/theme.ts
  - id: openwiki-source-581dc5746c844c4ee0b781c7
    resource: repo://vite.config.js
generated: { by: "pi", at: "2026-10-04T10:14:16.110Z" }
---


## 职责与边界

工作台层外壳负责三件事: 在 WebView 里把路由渲染成页面、把「选中哪套外观包 / 哪个 locale」写到 `<html>` 上、以及维持工作台的设计语言(直角、`text-xs` 基线、32px 控件)。

它不承担媒体阅读体验。按 ADR 0004 的双层架构, `/apps` 下的沉浸式应用面使用另一套交互语言, 且不受 base-lyra 的密度与圆角约束。当前这些应用面只有占位列表, 没有任何子路由或沉浸式外壳(见本文末「完成度」)。

## 启动与路由

启动分两步, 顺序是被刻意固定的:

1. `index.html` 的内联脚本在 bundle 加载前同步读 `localStorage` 里的偏好, 把 `.dark` / `data-theme` / `data-appearance-pack` / `color-scheme` 写到 `<html>` 上, 避免暗色冷启动闪白; 脚本只做选择, 不含任何色值(`index.html#L9-L47`)。
2. `src/main.tsx` 先 `createRoot().render(<LocaleProvider><App/></LocaleProvider>)`, 之后才顶层 `await startPreferencesPersistence()`; 持久化接管失败时用 toast 提示, 因为「主题当次仍可用, 只是重启后还原」(`src/main.tsx#L15-L31`)。

路由集中在 `src/App.tsx`: `BrowserRouter` 包 `TooltipProvider`, 内含 `Startup`(深链监听)、`Routes` 与 `Toaster`; 所有页面挂在同一个父 route `<Route element={<AppShell/>}>` 下, 因此外壳对每个工作区都生效(`src/App.tsx#L91-L110`)。

| 路径 | 页面 |
| --- | --- |
| `/` | `RealmHome` |
| `/apps` | `AppsHome` |
| `/sources` | `SourcesHome` |
| `/sources/rules`, `/sources/rules/:documentId` | `RuleWorkspace`(经 `RuleWorkspaceRoute`) |
| `/library` | `LibraryHome` |
| `/library/item/:resourceId` | `LibraryItem` |
| `/settings` | `SettingsHome` |
| `*` | `NotFound` |

`RuleWorkspace` 走 `lazy()` 动态 import, 因为 xyflow / CodeMirror / elkjs 体积大; `RuleWorkspaceRoute` 用 `Suspense` + `Spinner` 兜底(`src/App.tsx#L19-L22`, `#L67-L82`)。深链监听在 `Startup` 里被翻译成导航: `item` 去 `/library/item/<id>`、`source` 去 `/sources?highlight=`、`install` 去 `/sources?import=`, `reject` 则 toast 报错(`src/App.tsx#L29-L58`)。

## 外壳组成

`AppShell` 的层级是: 跳转链接 `#main-content` → `SidebarProvider` → (`AppSidebar` + `SidebarInset`(→ `Titlebar` + `<main id="main-content">` → `Outlet`))(`src/app/AppShell.tsx#L26-L40`)。

页面标题不由页面自己声明, 而由 `pageTitle(pathname)` 从路由派生, 顺序上先匹配 `/sources/rules` 再匹配 `/sources`, 先 `/library/item` 再 `/library`, 否则回落到 `appConfig.name`(`src/app/AppShell.tsx#L10-L20`)。新增路由若忘记在这个链上加分支, 标题会静默退化成应用名。

侧栏用 `getNavigationItems()` 渲染四个主工作区, `/sources` 带子项 `/sources/rules`(渲染为 `SidebarMenuSub`); 设置与「本地-only」提示放在 `SidebarFooter`(`src/shared/navigation.ts#L12-L24`, `src/app/AppSidebar.tsx#L44-L96`)。激活判定对 `/` 做精确匹配, 其余做前缀匹配, 因此 `/sources/rules` 会同时点亮「来源」与其子项(`src/shared/navigation.ts#L26-L28`)。

标题栏是自绘的无边框拖拽区(`data-tauri-drag-region`)。窗口按钮只在桌面 Tauri 运行时(windows/macos/linux)渲染, 由 `isTauri()` + `platform()` 判定, 移动端不显示; 最小化/最大化/关闭直接调 `getCurrentWindow()`, 失败时 toast 而非静默(`src/app/Titlebar.tsx#L11-L31`)。

## 状态与持久化

全仓只有一处 `create()` 的 zustand store: `usePreferencesStore`, 存 `theme` / `lightThemeId` / `darkThemeId` 三个字段(`src/shared/theme/theme.ts#L116-L131`)。Rust 侧持久化用 `createTauriStore('ui-preferences', ...)`, 配置为 `autoStart: false`(由 `startPreferencesPersistence()` 显式启动)、`saveOnChange: true`、`filterKeys: ['setTheme','setAppearancePack']` + `filterKeysStrategy: 'omit'`(动作函数不进持久化)(`src/shared/theme/theme.ts#L132-L158`)。

存储分工需要分清:

- 真实来源是 `@tauri-store/zustand` 的 `ui-preferences`, 由 Rust 写到应用数据目录, 可跨窗口同步(`src/shared/theme/theme.ts#L10-L16`)。
- `localStorage` 是**派生副本**, 只服务 `index.html` 的首帧同步读; 因为 tauri-store 的 `start()` 是异步的, 只依赖它会闪白(`src/shared/theme/theme.ts#L10-L16`, `#L84-L115`)。
- 浏览器里跑 `vite dev` 时 `isTauri()` 为假, 直接不启动持久化, 偏好退化为仅 `localStorage`, 功能不受影响(`src/shared/theme/theme.ts#L146-L157`)。

## 主题: 从选择到 CSS token 的单向数据流

```text
SettingsHome / Titlebar 选 pack 或 theme
  → usePreferencesStore (zustand)            src/shared/theme/theme.ts#L116-L131
  → applyTheme() 写 <html>                    src/shared/theme/theme.ts#L175-L197
  → src/index.css 的 :root[data-appearance-pack='...'] 块给基色   src/index.css#L178-L279
  → :root 用 color-mix 派生 + 映射 shadcn 契约别名               src/index.css#L281-L330
  → @theme inline 暴露语义色名                                      src/index.css#L332-L410
  → 业务写 bg-surface-1 / text-ink-muted / border-hairline
```

关键不变量:

- `applyTheme()` 写四个东西: `classList.toggle('dark')`(供 Tailwind `dark:` 变体与 registry 组件)、`dataset.theme`(原始选择, 可为 `system`)、`dataset.appearancePack`(解析后的包 id)、`style.colorScheme`; `theme-color` meta 从 `getComputedStyle(root)` 的 `--canvas` 读回, 避免 JS 里维护第二份色值(`src/shared/theme/theme.ts#L175-L197`)。
- 四套外观包都是单面的, 由 `PACK_FACE` 声明亮暗轨: `porcelain-day` / `mist-studio` 亮, `obsidian-void` / `graphite-atelier` 暗; `setAppearancePack` 据此决定改 `lightThemeId` 还是 `darkThemeId`(`src/shared/theme/theme.ts#L64-L78`, `#L127-L130`)。
- store 的订阅在模块顶层注册, 同时覆盖本地操作与 tauri-store 从 Rust / 其他窗口推回的变更; `theme === 'system'` 时额外监听 `matchMedia('(prefers-color-scheme: dark)')` 的 change 实时重算(`src/shared/theme/theme.ts#L199-L219`)。
- 派生色一律 `color-mix` 自基色, 不手写第二份 rgba; shadcn 契约变量(`--background` / `--border` / `--sidebar-*` 等)保留为别名, 目的是让 `src/components/ui/**` 维持 registry 原样、可被后续 `shadcn add` 升级(`src/index.css#L281-L330`)。
- `@theme inline` 只做名字桥接: `--color-surface-1: var(--surface-1)` 等, 业务可见名是 `bg-surface-1` / `text-ink-muted` / `border-hairline`, 品牌色只叫 `lantern-*`(`src/index.css#L332-L348`)。

## 设计系统约束的落点

**母版与 primitive**: `components.json` 固定 `style: "base-lyra"`、`iconLibrary: "phosphor"`(`components.json#L3-L13`)。UI primitive 是 Base UI, 替代 Radix `asChild` 的是 `render` prop, 调用方传 JSX 元素, 例如 `render={<Link to={item.href} />}`(`src/app/AppSidebar.tsx#L47-L52`)。`src/components/ui/**` 是 CLI 生成物, 只保证可被后续 `shadcn add` 升级; 每次新增组件后要把 registry 里的 `@phosphor-icons/react` import 换成本地 `src/components/ui/icon-glyphs.tsx`。

**图标**: 全站唯一出口是 `src/components/Icon.tsx`。`IconName` 是联合类型, `ICON_CLASSES` 是 `Record<IconName, string>` 的字面量映射(`'book-open': 'icon-[ph--book-open]'`), 原因是 Tailwind 静态扫描源码, `icon-[ph--${name}]` 这类模板拼接不会被识别; 新增图标必须同时补两处(`src/components/Icon.tsx#L4-L10`, `#L11-L69`, `#L71-L136`)。`label` 有值渲染 `role="img"` + `aria-label`, 否则视为装饰性 `aria-hidden`(`src/components/Icon.tsx#L138-L152`)。

**字面量禁令的三种落点**: 圆角档位全部塌陷为 `0px`(`--radius-media: 2px` 是媒体缩略图例外), 所以业务即使误写 `rounded-md` 也不破坏直角语言(`src/index.css#L62-L75`); 层级只通过 `--layer-*` token 使用, 业务代码写 `z-(--layer-chrome)`, 字面量 `z-50` 只出现在 registry 生成物里(`src/index.css#L110-L119`); CSS 侧用 `@import 'tailwindcss' source(none)` + 显式 `@source './'` / `@source '../index.html'` 关掉自动扫描, 以免把 Markdown 里的 `icon-[ph--*]` 示例吃进构建产物(`src/index.css#L11-L20`)。

## 本地化

边界是 `src/shared/i18n`: `messages.ts` 重新导出 Paraglide 的 `m` 与 `LocaleProvider`, 并提供 `useMessages()`(`src/shared/i18n/messages.ts#L1-L10`)。`useMessages()` 内部先调 `useLocale()`, 这样语言切换会触发订阅方重渲染——直接 import `m` 而不能订阅 locale 的模块只能拿到切换前的取值。

`LocaleProvider` 用 `getLocale()` 初始化, 切换时 `persistLocale(next, { reload: false })` 后 `setState`, effect 把 locale 写到 `document.documentElement.lang` / `dir`(`src/shared/i18n/locale.tsx#L30-L46`)。翻译源在 `messages/{en,zh-CN}/*.json`, 按 `common/shell/realm/sources/apps/library/settings/rules` 八个命名空间分文件, `baseLocale` 是 `zh-CN`, 两个 locale 的 key 集合必须一致; Vite 构建经 `paraglideVitePlugin` 生成 `src/shared/paraglide/`, 该目录是生成物、不手改(`project.inlang/settings.json#L3-L4`, `vite.config.js#L12-L16`)。

## 完成度与遗留

| 工作区 | 状态 | 依据 |
| --- | --- | --- |
| 境场 `/` | 已实现, 并发拉 library projection / installed sources / rule documents | `src/features/realm/RealmHome.tsx#L37-L83` |
| 来源 `/sources` | 已实现, 含安装、revision、回退与深链导入 | `src/features/sources/SourcesHome.tsx#L47-L100`, `src/features/sources/SourceInspector.tsx#L33-L37` |
| 规则 `/sources/rules` | 已实现, 前端最重的模块 | `src/features/rules/RuleWorkspace.tsx#L24-L27` |
| 资料库 `/library` | 已实现, 表格 + Sheet 检查器 + favorite/pinned 乐观更新 | `src/features/library/workflow.ts#L74-L135` |
| 设置 `/settings` | 已实现, 外观包 / 语言 / 隐私说明 / 版本 | `src/features/settings/SettingsHome.tsx#L89-L96` |
| 应用面 `/apps` | **占位**: 五个 surface 全部 `enabled: false`, 无 `/apps/<surface>` 子路由 | `src/features/apps/AppsHome.tsx#L15-L21`, `#L46-L50` |

占位不只体现在 `enabled: false`: `messages/zh-CN/rules.json` 等消息表里存在大量在 `src/` 中没有任何引用的 key(如 `rules_leave_guard_*`、`sources_diagnostic_*`、`library_detail_*`), 说明规则编辑器、来源诊断与资料库详情仍有未接线的 UI 分支。该判断基于字面量检索, 标为可信但非绝对(不排除将来动态拼接)。

另外, 按 ADR 0004-0007 已采纳的应用面 / TurnEngine / 资产网关均无前端代码: `src/features/turn/**` 不存在, 仓库中也检索不到 `PageSource` / `TurnEngine` / `TurnIntent`。这部分的设计蓝图与落地状态见「应用面、翻页引擎与资产网关蓝图」。
