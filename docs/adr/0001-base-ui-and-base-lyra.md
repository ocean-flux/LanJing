# ADR 0001：UI primitive 选 Base UI，组件母版选 shadcn base-lyra

- 状态：已采纳
- 日期：2026-08-16
- 影响范围：`src/components/**`、`components.json`、`package.json`、`src/index.css`

## 背景

`src/` 从 SvelteKit 迁到 React 19 后处于骨架状态，`src/components/ui/` 正被并行改写：
button / separator / sheet / sidebar / tabs / tooltip 已经指向 `@base-ui/react`，
input / skeleton / toast / badge / card 仍是 Radix 版本，`radix-ui` 与 `@radix-ui/*` 同时留在
dependencies 里。结果是 `pnpm typecheck:web` 红的 —— Base UI 没有 Radix 的 `asChild`，
`<Button asChild>` 与 4 处 `Slot.Root` 直接编译失败。

两套 primitive 并存不是可以放着的技术债：它们的插槽机制、data 属性命名、Portal 与
Positioner 的分层方式都不同，同一个 `cn()` 类名串在两边行为不一致，任何一次
`shadcn add` 都会再引入一批不确定属于哪一套的文件。必须二选一。

## 决定

1. **UI primitive 选 Base UI**（`@base-ui/react`），彻底移除 `radix-ui` 与 `@radix-ui/*`。
2. **组件母版选 shadcn `base-lyra` registry**，`components.json` 记为 `style: "base-lyra"`。
3. **图标走 Iconify**（`@iconify/tailwind4` + `@iconify-json/ph`），移除 `lucide-react`，
   不引入 `@phosphor-icons/react` 运行时依赖。

## 依据

**为什么是 Base UI 而不是回退到 Radix**

- 迁移方向已经既成：多数导航与浮层组件（sidebar / sheet / tooltip / tabs）已经改完，
  回退的改动量大于推进。
- Base UI 的 `render` prop 比 `asChild` 更显式：合并目标以 JSX 元素传入
  （`render={<Link to="/settings" />}`），类型能查到目标元素的 props，而 `asChild`
  依赖运行时把 props 透传给唯一 child，错用只在运行时暴露。
- Base UI 把 Portal / Backdrop / Positioner / Popup 拆成独立部件，浮层的定位与
  层级可以直接绑到 `--layer-*` token，不需要覆盖库内联样式。
- 它是 Radix 原班人员的后继项目，无障碍行为与键盘交互模型同源，迁移不牺牲 a11y。

**为什么是 base-lyra**

实抓 `styles/base-lyra/button.json` 确认它产出 `import { Button as ButtonPrimitive } from "@base-ui/react/button"`，
37 个目标组件在该 preset 下全部存在。母版特征与本项目的工具化密集方向一致：

- 容器 `rounded-none`：直角语言，与「桌面工作台而非营销页」的定位吻合，
  也省掉一整套圆角协商 —— 只在浮层保留 `--radius-overlay`。
- `text-xs` 基线、控件 `h-6 / h-7 / h-8 / h-9`：32px 默认行高，密集列表不用逐个压。
- `focus-visible:ring-1` 而非 `ring-[3px]`：密集布局下焦点环不会互相挤压。

代价是 `text-xs` 对中文偏小，已在 `src/index.css` 用 `--text-ui: 0.8125rem` 把正文抬到 13px，
控件保持 12px。

**为什么图标不用 CLI 默认的运行时图标库**

`components.json` 的 `iconLibrary` 只能选 `lucide | radix | tabler | hugeicons | phosphor | remixicon`，
选中的库会作为运行时依赖被 import。Iconify 的 Tailwind 4 插件在构建期把 SVG 内联进 CSS，
不进运行时 bundle —— 对 Tauri 冷启动更友好。保留 `iconLibrary: "phosphor"` 是为了让 CLI
生成语义正确的图标名，生成后再把 import 换掉。

## 后果

**必须遵守的约定**

- 每次 `npx shadcn@latest add <name>` 之后，把新组件里的 `@phosphor-icons/react` import
  换成 `@/components/ui/icon-glyphs`。涉及的字形集中在 check / caret / x / spinner / panel-left。
- 图标类名必须是字面量。Tailwind 静态扫描源码，`icon-[ph--${name}]` 不会被识别，
  所以 `src/components/Icon.tsx` 用字面量 `Record<IconName, string>` 白名单。
- `src/components/ui/**` 与 `src/hooks/use-mobile.ts` 是生成物，`.oxlintrc.json` 已豁免
  其纯风格规则；改动应保持可被后续 `shadcn add` 升级。

**接受的代价**

- Base UI 的组件覆盖面小于 Radix，`base-lyra` 之外的社区组件不能直接拿来用。
- registry 组件依赖 shadcn 语义变量（`--background` / `--primary` / `--border` 等），
  所以 `src/index.css` 保留这批变量作为 appearance pack token 的**别名**，而不是让
  registry 组件改写成项目自己的语义色名 —— 前者可升级，后者每次 `add` 都要重做。

## 备选方案

- **回退到 Radix，把已改的组件改回去**：改动量更大，且要放弃已经落地的 `render` prop 用法；
  Radix 本身进入维护节奏，长期还得再迁一次。
- **两套并存，按组件挑**：`asChild` 与 `render` 的插槽语义不同，同一棵树里混用会让
  「这个组件属于哪一套」变成每次改动都要先查的问题。已否决。
- **不用 registry，全部手写**：37 个组件的键盘交互与无障碍属性自己维护，
  与「本地-only 小团队」的维护预算不匹配。
