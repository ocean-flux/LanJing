# ADR 0005：通用翻页引擎 TurnEngine 与 PageSource 抽象

- 状态：已采纳
- 日期：2026-09-02
- 影响范围：`src/features/turn/**`（新增）、`src/features/apps/reading/**`、
  `src/features/apps/gallery/**`、后续 podcast / video / music 应用面

## 背景

reading（文本）与 gallery（图漫）都需要翻页：输入处理（指针 / 滚轮 / 键盘）、
离散与连续导航、RTL 镜像、单页 / 对开布局、多种转场动效、进度锚点换算。
若每个应用面各写一套，同一份逻辑要写两遍，后续 podcast / video / music
要翻页时还要再写；两份实现必然在边界行为（首页 / 末页、快速连翻、模式切换
时进度保持）上漂移。

## 决定

1. 抽出**与媒体类型无关的通用翻页引擎 TurnEngine**，落在 `src/features/turn/`——
   它服务多个应用面，不属于任何一个应用面，也不放进 `src/shared/`
   （它是实质 UI 逻辑，不是工具）。
2. 引擎内部分三层：输入归一化（→ TurnIntent）→ 状态机
   （idle / dragging / animating）→ 导航器 × 布局 × 转场器。
3. 应用面通过 **PageSource 契约**提供内容：测量、获取 / 释放页、位图快照、
   页索引与阅读锚点互转、声明支持的转场。转场器只向 `snapshotFor()` 要位图，
   不关心内容来源。
4. 翻页模式是 `layout × navigation × transition` 三元的**策展组合**（当前 7 种，
   见 GitHub issue #31），不是独立实现；
   加一种模式通常只是加一个转场器或一行组合。
5. 引擎边界：不持色值 / 间距 / 时长字面量（走 token）；不认识 MediaKind；
   不回写进度（只抛页索引 + 页内偏移，由应用面换算锚点落库）；
   尊重 `prefers-reduced-motion`。

## 依据

- 翻页的难点（输入归一化、状态机、RTL、边界行为）与「页里装什么」完全正交，
  抽象成本一次付清，应用面只付 PageSource 的实现成本。
- PageSource 的 `snapshotFor()` 是核心解耦点，并带来一个反直觉结论：
  **gallery 上仿真转场比 reading 更简单**——图片本来就是位图，零快照成本、
  不等字体就绪、无 CSS 兼容坑；仿真成本几乎全部落在文本 PageSource 一侧。
- `supportedTransitions` 能力协商让内容源可以声明限制（如长图不支持卷页），
  引擎据此禁用并降级，而不是在转场器里写媒体类型判断。

## 后果

- 新应用面要翻页时只需实现 PageSource，不得再写平行翻页逻辑。
- 引擎逻辑是纯逻辑，可用 FakePageSource 做确定性单测；这是本轮测试投入的
  最高点。
- 模式清单的增删改在策展层（issues/06）讨论，不动引擎结构。

## 备选方案

- **每个应用面各写一套翻页**： veto——重复实现与行为漂移，正是本 ADR 要消除的。
- **把引擎放进 `src/shared/`**：否决。`shared/` 是工具与契约，引擎是带状态机的
  实质 feature 逻辑，放错位置会让 shared/ 膨胀成杂物间。
- **只抽象转场器，导航与布局留给应用面**：RTL 镜像与进度换算恰恰在导航层，
  半抽象等于没抽象。
