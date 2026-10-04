---
type: "参考"
title: "App surfaces and asset gateway"
openwiki_generated: true
verified:
  - by: openwiki/0.7.0
    at: 2026-10-04T10:14:16.110Z
sources:
  - id: openwiki-source-415c14fc16b9bb2d14d082c5
    resource: repo://docs/adr/0004-app-surfaces-and-workbench-layers.md
  - id: openwiki-source-05b14d59724ec4fa57d1c0cc
    resource: repo://docs/adr/0005-turn-engine-and-pagesource.md
  - id: openwiki-source-1733f073948bff3c73282ba8
    resource: repo://docs/adr/0006-simulation-curl-prerender-cache.md
  - id: openwiki-source-fe996f1c611a94f015a96f71
    resource: repo://docs/adr/0007-asset-gateway-protocol.md
  - id: openwiki-source-b1cb1ede667e94b5278ffba1
    resource: repo://docs/reference/reader-architecture.md
  - id: openwiki-source-5b54a58d1b51cd490b0e7162
    resource: repo://package.json
  - id: openwiki-source-f37a0f7edcf7d757acfa95d3
    resource: repo://src-tauri/crates/lj-media/src/lib.rs
  - id: openwiki-source-e3712091d151264c5b636d0c
    resource: repo://src-tauri/crates/lj-node-http/src/ssrf.rs
  - id: openwiki-source-0abfee918aaf0d7e3ea712fc
    resource: repo://src-tauri/tauri.conf.json
  - id: openwiki-source-54631e6ebf1d3b815c4a5eed
    resource: repo://src/App.tsx
  - id: openwiki-source-3cb69885e6900d88473eb55d
    resource: repo://src/components/ui/item.tsx
  - id: openwiki-source-b3eeee8972a1c1a0ed428993
    resource: repo://src/features/apps/AppsHome.tsx
  - id: openwiki-source-938e4156b9e76517514c2370
    resource: repo://src/shared/tauri/sources.ts
  - id: openwiki-source-2640df53984077a61a4bb013
    resource: repo://src/shared/types/media.ts
generated: { by: "pi", at: "2026-10-04T10:14:16.110Z" }
---


## 两份来源的成熟度不同

仓库里有两份关于「阅读器」的设计文本, 它们的地位不同:

| 来源 | 状态 | 性质 |
| --- | --- | --- |
| `docs/adr/0004`-`0007` | 全部标记「已采纳」(0006 附带前置条件) | 决策记录, 有明确的后果与备选方案 |
| `docs/reference/reader-architecture.md` | 无状态标记, 自述为「外部方案」的一种 | 早期独立设计文档, 与 ADR 存在未收敛的分歧 |

两者没有互相引用。`reader-architecture.md` 提出的是**单层阅读器 + Mode Dispatcher**(工作台与阅读器同层), 而 ADR 0004 明确把应用面切成第二层; 前者列了 5 种模式, 后者改为 `layout × navigation × transition` 的策展组合(当前 7 种)。因此本页以 ADR 为主干, 把 `reader-architecture.md` 放在末尾单独对照。

**当前代码里没有任何一行实现这四份 ADR 中的机制**, 全部属于「已决定未实现」。

## 双层架构(ADR 0004)

决定内容(`docs/adr/0004-app-surfaces-and-workbench-layers.md#L20-L26`): 工作台层(境场、来源、规则、资料库、设置)保持 base-lyra 的克制语言; 应用面层(`/apps` 下)是另一套交互语言——进入后隐藏工作台 chrome, Esc/返回手势退出, 不受直角、`text-xs` 基线、32px 控件约束; 两层唯一共享的约束是**色值只能来自 `src/index.css` 的四个 appearance pack**。

依据是「克制是工作台层的目标而不是全产品约束」, 以及应用面按媒体类型分流后交互范式天然不同(`#L30-L35`)。后果里写死了新应用面的固定动作: 路由挂 `/apps/<surface>/:resourceId`、实现沉浸式外壳、声明视觉语言边界(`#L42-L43`)。

当前落地状态:

| 蓝图条目 | 代码证据 | 状态 |
| --- | --- | --- |
| 五个应用面声明 | `src/features/apps/AppsHome.tsx#L15-L21` 列出 reading/gallery/podcast/video/music | **占位**: 五个 `enabled: false` |
| 未实现不伪装可用 | 同上 `#L38-L50`: 整行 `aria-disabled` + 半透明, 并渲染 `apps_unavailable` 徽标 | 已实现的是**诚实占位**本身 |
| `/apps/<surface>/:resourceId` 路由 | `src/App.tsx#L97` 只有 `path="apps"` 一条, 挂在 `AppShell` 内 | **未实现** |
| 沉浸式外壳(隐藏 chrome、退出手势) | 无相关代码 | **未实现** |
| 色值单一来源 | `AGENTS.md` 与 `src/index.css` 的 appearance pack 已生效于工作台层 | 已实现(工作台层) |

前端 `MediaKind` 只有 5 个值(`src/shared/types/media.ts#L1`), 与 `AppsHome` 的五个面一一对应; 但 Rust 侧 `MediaKind` 有 12 个变体(Text/Image/Comic/Audio/Video/Article/Document/Course/LiveReplay/Mixed/LocalResource/RemoteResource, `src-tauri/crates/lj-media/src/lib.rs#L87-L100`)。也就是说 `comic` 与 `image` 的区分、以及 article/document/course/live_replay 等类型**尚无对应应用面**, 这个映射是 未决定 而不是未实现。

## TurnEngine 与 PageSource(ADR 0005)

决定内容(`docs/adr/0005-turn-engine-and-pagesource.md#L18-L31`):

1. 抽出与媒体类型无关的翻页引擎, 落在 `src/features/turn/`, **明确不放进 `src/shared/`**(理由是它是带状态机的实质 UI 逻辑, 不是工具);
2. 引擎内含三层: 输入归一化(→ `TurnIntent`)→ 状态机(idle/dragging/animating)→ 导航器 × 布局 × 转场器;
3. 应用面通过 **PageSource 契约**提供内容: 测量、获取/释放页、位图快照(`snapshotFor()`)、页索引与阅读锚点互转、声明支持的转场;
4. 翻页模式是 `layout × navigation × transition` 的策展组合(当前 7 种, 见 issue #31), 不是独立实现;
5. 引擎边界: 不持色值/间距/时长字面量(走 token)、不认识 `MediaKind`、**不回写进度**(只抛页索引 + 页内偏移, 由应用面换算锚点落库)、尊重 `prefers-reduced-motion`。

其中两条是有价值的反直觉结论: `snapshotFor()` 是核心解耦点, 导致「gallery 上做仿真转场比 reading 更简单」(图片本来就是位图, 零快照成本); `supportedTransitions` 让内容源声明限制(如长图不支持卷页)并据此降级, 而不是在转场器里写媒体类型判断(`#L36-L41`)。

当前落地状态: **全部未实现**。仓库里没有 `src/features/turn/`(`src/features/` 下只有 apps/library/not-found/realm/rules/settings/sources), 没有 `TurnIntent`、`PageSource` 或任何转场代码; `/apps` 只渲染一个禁用列表。ADR 提到「引擎逻辑是纯逻辑, 可用 FakePageSource 做确定性单测; 这是本轮测试投入的最高点」(`#L46-L47`), 这部分测试同样不存在。

## 仿真卷页的空闲帧缓存(ADR 0006)

决定内容(`docs/adr/0006-simulation-curl-prerender-cache.md#L19-L30`): 把 DOM 快照从手势关键路径移到**空闲帧**(`requestIdleCallback` + 等 `document.fonts.ready`), 页面稳定后异步渲染相邻 2-3 页为位图纹理; 按下时命中缓存、0ms 进入 WebGL 卷页, 抬起后切回 DOM 以恢复文字选中与朗读。

预算与失效条件也是决定的一部分:

- 单页纹理约 12MB/屏(CSS 像素 × DPR 上限 2), 单页仿真缓存 3 屏约 36MB, 双页 4 屏约 48MB, 超出淘汰最远屏(`#L23-L24`);
- 缓存失效条件: 改排版参数、转屏、换外观包、字体就绪、分页重排(`#L25`);
- 降级链: 预渲染连续失败 / WebGL 上下文丢失 / 内存紧张 → CSS 2.5D; `prefers-reduced-motion` → 无动效(强制)(`#L26-L27`)。

**状态是「已采纳(立项以真机最小验证原型通过为前提)」**(`#L3`): 实现前必须在低端真机跑文本单页/文本双页/图漫单页/图漫双页四组合并拿到帧时间与内存实测, 未通过则只交付 CSS 2.5D(`#L28-L30`)。这条前置条件意味着 ADR 0006 的**成立与否尚未被验证**, 不是单纯「还没排期」。

它对被否决方案的理由也值得记下: 外部方案的「按下时快照」不仅时序错, 而且骨架代码不可运行——`createImageBitmap` 的合法入参不含 `HTMLElement`, 而 html2canvas / SVG foreignObject 类 DOM 快照在满屏中文页面上是百毫秒级(`#L12-L15`)。ADR 还明确禁止引用外部方案的性能数字(120fps、65MB 等)因为它们无实测支撑(`#L46-L47`)。

## 资产网关 `lanjing://`(ADR 0007)

这是四份 ADR 里唯一直接指向**当前卡点**的一份。决定内容(`docs/adr/0007-asset-gateway-protocol.md#L18-L25`):

1. 前端用 `lanjing://asset/<asset-id>?w=<px>` 引用资产, Rust 侧自定义协议处理器代理真实网络请求;
2. 网关按资产所属来源规则注入防盗链头(Referer / UA / Cookie);
3. 磁盘缓存 + ETag 条件请求, 重复打开同一资产不走网络;
4. **网关必须复用 `lj-node-http/src/ssrf.rs` 的 SSRF 防护, 不得另起裸 HTTP 客户端绕过**;
5. CSP 白名单补 `lanjing:`;
6. 按需降采样(`?w=`)后置实现, 但协议形态现在预留。

背景陈述是: 资产 locator 是 `Url(String)`, 图站/资源站普遍校验 `Referer`/`User-Agent`/Cookie 防盗链, 而 WebView 的 `<img>` 无法携带规则定义的请求头, 直连必然 403, **当前全库没有渲染过一张图片, 根因在此**(`#L8-L14`)。

当前落地状态:

| 蓝图条目 | 代码证据 | 状态 |
| --- | --- | --- |
| 自定义协议处理器 | `src-tauri/src/` 与各 crate 中检索 `register_uri_scheme` / `register_asynchronous_uri_scheme_protocol` 无命中 | **未实现** |
| 无 `lanjing://` 出现 | 全仓库检索 `lanjing://` 无命中 | 未实现 |
| CSP 白名单 | `src-tauri/tauri.conf.json` 的 CSP 里 `img-src 'self' data: https:` 是唯一放行外部图像的位置, 没有 `lanjing:` | **未实现**(且当前 `https:` 是唯一出口) |
| 前端零直连 | 业务代码里没有任何 `<img>`; `SourceProfile.icon_url` 只有类型与 IPC 镜像, 无渲染点(`src/shared/tauri/sources.ts#L18` 声明, 无使用处) | 与「发现直连视为缺陷」一致, 但也意味着图片路径整体不存在 |
| SSRF 复用 | `lj-node-http/src/ssrf.rs` 已有可复用实现: `PinnedTarget`(#L53-L55)、`is_blocked_ip`(#L68-L70)与带 DNS 解析 + 防 rebinding 的校验入口(#L165), 单元测试覆盖回环阻断与公网放行(#L389-L395) | **已实现**(网关可直接复用, 但尚未被网关调用) |

ADR 里有一条常被忽略的结论: 外部方案宣称的「零拷贝直通 GPU 纹理」不成立——custom protocol 返回字节流, 仍要经 WebView 网络栈解码; 网关的收益是**防盗链 + 缓存**, 不是零拷贝(`#L41-L42`)。被否决的替代方案也各有技术理由: 前端 fetch 带自定义头再转 blob URL(Tauri WebView 的 fetch 同样受限, 且缓存/SSRF/凭据散在前端)、Rust fetch 后经 IPC 返回字节或 Base64(33% 膨胀 + JSON 序列化开销, 大图与视频不可接受)(`#L44-L50`)。

## reader-architecture.md 的方案与偏差

这份文档(387 行)描述的是**另一种**架构, 与 ADR 只在「需要翻页」「需要归一化」两点上一致:

| 议题 | `reader-architecture.md` | ADR 0004/0005 | 差异性质 |
| --- | --- | --- | --- |
| 分层 | 单一阅读内核, 壳是 Tauri WebView(`docs/reference/reader-architecture.md#L32-L58`) | 工作台层 + 应用面层两层 | 架构冲突 |
| 模式清单 | 5 种固定模式 + `Mode Dispatcher`(`#L183-L232`) | `layout × navigation × transition` 策展组合, 当前 7 种 | 已由 ADR 取代 |
| 数据模型 | 自建 Book AST(Book/Chapter/Block/TextRun/AssetIndex)(`#L63-L81`) | 未定义; 当前只有 `lj-media` 的标准媒体模型 | 未收敛 |
| 分页 | Web Worker + `@chenglou/pretext`(`#L129-L176`) | 未涉及具体排版库 | 未收敛 |
| 文本快照 | 「备用」Snapdom(`#L383`) | ADR 0006 已论证 DOM 快照走空闲帧预渲染 | 已由 ADR 修正 |

技术栈清单里的多数依赖**当前未安装**: `@chenglou/pretext`、Snapdom、`three.js`、`cn-font-split`、Jotai 在 `package.json` 里都不存在(前端实际装的是 `@base-ui/react`、`@xyflow/react`、`elkjs`、`codemirror`、`zustand`、`sonner` 等, 见 `package.json#L23-L54`)。清单里还写着「前端框架 React 19 / Vue 3 / Solid 任选, 建议 React」(`#L380`), 这是文档写作时尚未定型的状态。

它也有两处仍站得住的内容:

- MVP 路径的排序逻辑(`#L349-L359`): P0 数据管线(Book AST + txt/cbz importer + 分页 + 滚动), P1 滑动/覆盖, P2 仿真卷曲, 并明确「滚动+滑动+覆盖用 DOM 就能做, 不必等 WebGL」。这与 ADR 0006 把真机验证作为立项前提是同一思路。
- 章级懒分页与内存兜底(`#L363-L373`): 只常驻当前章 ±1、大书虚拟滚动只渲染可见 ±2 屏。

## 落地状态总表

| ADR | 主题 | 状态 | 关键判据 |
| --- | --- | --- | --- |
| 0004 | 双层架构与应用面交互语言 | 已决定未实现 | `/apps` 只有一条路由, 五面均 `enabled: false` |
| 0005 | TurnEngine 与 PageSource | 已决定未实现 | 无 `src/features/turn/`, 无相关测试 |
| 0006 | 空闲帧预渲染纹理缓存 | 已决定未实现, 且**成立与否待真机验证** | 无 WebGL/转场代码; 前置条件未执行 |
| 0007 | `lanjing://` 资产网关 | 已决定未实现 | 无自定义协议, CSP 无 `lanjing:`; SSRF 基础已具备 |
| (reference) | reader-architecture 单层方案 | 与 ADR 冲突, 未采纳也未删除 | 技术栈依赖未安装 |

## 未验证 / 未决定

- 应用面与 `MediaKind` 的具体映射(尤其 `comic` 是否独立成面、article/document/course/live_replay/local_resource/remote_resource 归属)没有文档化决定。
- ADR 0006 的纹理内存预算(12MB/屏等)是估算值, 无实测数据支撑——ADR 自己也把实测列为立项前提。
- `assets` 目录与 `MediaAsset` 目前只服务资料库投影, 与资产网关的 `<asset-id>` 命名空间是否一致 未验证。
- 七个翻页模式的具体清单只在 issue #31 里(仓库内不可见), 本页无法核实。
