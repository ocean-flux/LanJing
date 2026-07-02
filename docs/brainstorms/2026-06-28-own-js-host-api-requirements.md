---
date: 2026-06-28
topic: own-js-host-api-namespace
---

## Summary

自研 JS 宿主 API 取代 legado 的 `java.xxx`，按能力分域多对象注入 rquickjs AsyncRuntime，host 函数 async-first（Promise + tokio 调度，不阻塞 JS thread）。首刀注入能力域 `net`/`enc`/`crypto`/`fs`/`web`/`util`/`rule`/`script` + 领域域 `cookie`/`source`/`book`/`chapter`，对齐 legado 全函数集（见 `docs/reference/legado-js-builtin-inventory.md`）。批量与规则解析都回 host async 调度、不下沉节点图。导入器静态映射 `java.xxx → 自研域对象` + 人工标注不兼容面。需立 ADR-0030 重定 host 能力边界，诚实记下从 ADR-0029「host 不越权、能力下沉」到「host async 集能」的姿态迁移。

## Problem Frame

legado 的 `java` 变量是一个对象横装网络/编码/加密/文件/规则解析/UI 跳转共 130+ 函数，无能力切分，capability 门控无处分类挂载；且 legado 已重定向顶层 `java` 变量为自家宿主对象，真 Java 包须 `Packages.java.*`，rquickjs 无此历史包袱，命名空间可彻底脱离 `java.` 前缀。

ADR-0029 首刀只授权 network+编码子集注入 + 本地零注入 fs/env/process。用户在对话中决定首刀扩到 fs/WebView/crypto 全集/importScript/openUrl/文本辅助——这破 ADR-0029 本地零注入底线，需立 ADR-0030 重定边界。原提案 X2（批量下沉节点图）/Z1（规则下沉 Extract）在对话中重议为回 host async：升级 rquickjs futures feature + AsyncRuntime + Promise 后，host 函数可返 Promise、JS `await` 拿结果、Rust 内 tokio 并发，不阻塞 JS thread，批量与规则解析回 host 的 async 张力消失。姿态从「host 不越权、能力下沉节点图」迁移到「host async 集能」，ADR-0030 须诚实记下这次迁移，不再以「下沉」为名义。

rquickjs 0.12 项目当前未开 `futures` feature，用的是同步 `JsRuntime::new` + `Context::with` 同步 eval；升级 AsyncRuntime 是本提案前置依赖。

## Key Decisions

**按能力分域多对象命名空间（脱离 `java.`）。** 能力域 `net`/`enc`/`crypto`/`fs`/`web`/`util`/`rule`/`script`，领域域 `cookie`/`source`/`book`/`chapter`。每个域对应一类 capability，门控独立；新增域内函数不触发新 ADR、跨域新增仍触发。

**runtime 升级 AsyncRuntime + Promise，host async-first。** 启 rquickjs `futures` feature + `AsyncRuntime`/`AsyncContext`，host 函数返 JS Promise（`Promise::wrap_future`），JS 侧 `await`，Rust 内 tokio 调度，不阻塞 JS thread。`net`/`web`/`script` 类 I/O 函数全 async；`enc`/`crypto`/`util` 纯计算同步。

**批量回 host async（回退 X2 下沉）。** `net.getAll(urls)` 返 `Promise<Array<StrResponse>>`，Rust 内 `tokio::join_all` 并发，JS `await` 拿数组。导入器把 legado `ajaxAll` 机械映射为 `await net.getAll(...)`。代价：与 ADR-0013 stream-async 范式有张力（批量绕过 stream 调度），scope 层确认可接受，机制不互踩留待 ce-plan 验证。

**规则解析回 host 统一（R1，回退 Z1 下沉）。** host 提供 `rule.getString(rule, content?, isUrl?)`/`getStringList`/`getElement`/`getElements`/`setContent(content, baseUrl?)`/`setBaseUrl`，背后调同一 Extract 节点处理器，无双实现。首批兼容面拓宽，legado `java.getString` 机械映射到 `rule.getString`。

**crypto 首刀全引 AES/RSA/digest/sign 对齐 legado。** 引 aes/cbc/cipher/rsa/sha2/hmac/md-5 等 crate，覆盖 legado `createSymmetricCrypto`/`createAsymmetricCrypto`/`createSign`/`digestHex`/`HMacHex`/`aesEncode*`/`aesDecode*`/`des*`/`tripleDES*` 全套。实现工作量大，用户已选。

**fs/WebView/importScript/openUrl/文本辅助 首刀注入（破 ADR-0029 本地零注入）。** `fs` 域：downloadFile/readFile/readTxtFile/getFile/deleteFile/unzipFile/unrarFile/un7zFile/getTxtInFolder/cache.putFile/getFile（相对缓存目录沙箱）。`web` 域：webView/webViewGetSource/webViewGetOverrideUrl/startBrowser/startBrowserAwait/getVerificationCode（依赖 Tauri WebView 组件）。`script` 域：importScript（fetch+eval，需 eval 安全门）。`util` 域：t2s/s2t/htmlFormat/toNumChapter/openUrl/randomUUID/timeFormat/timeFormatUTC。需 ADR-0030 重定 fs/env 边界 + capability 门控。

**androidId 走真实机器标识。** 读 Windows MachineGuid / macOS IOPlatformUUID / Linux machine-id，归一为 hex 字符串。不随应用重装变、真实硬件级稳定。代价：隐私敏感、格式不一需归一、需引机器标识库或写平台分支代码。legado 内部用 androidId 前 16 字节做 AES key 加密登录头/登录信息，我们单机应用不跑 legado Android 端数据，跨端解密场景不存在，JS 侧只需"拿一个稳定字符串"。

**导入器 N1 静态映射 + 人工标注。** import 阶段对 `java.xxx` 做字面映射到自研域对象；遇首刀不注入的函数（字体反查、远端压缩包）标注「需人工重写」。批量回 host 后 `ajaxAll` 可机械映射为 `await net.getAll`，规则回 host 后 `getString` 可机械映射为 `rule.getString`，需人工重写的面收窄到不注入子集。

**姿态迁移诚实记入 ADR-0030。** 从 ADR-0029「host 不越权、能力下沉节点图」落到「host async 集能」，ADR-0030 须记下：批量/规则回 host 的理由（async 非阻塞 + 兼容面）、fs/WebView 破本地零注入的理由 + capability 边界、与 ADR-0013 stream 并存的张力。

**字体反查/远端压缩包 首刀不注入。** `queryTTF`/`queryBase64TTF`/`replaceFont`/`cache.put(getQueryTTF)`、`getZip/Rar/7zStringContent`/`getZip/Rar/7zByteArrayContent` 不注入，按真实书源密度另立 ADR 逐步引。

## Requirements

### 命名空间与域对象清单

| 域对象    | 域               | 能力门 (Capability)  | async                  | 首刀注入函数子集                                                                                                                                                                                                              |
| --------- | ---------------- | -------------------- | ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `net`     | 网络             | Network              | 是                     | `get`/`post`/`head`/`getAll`(批量 tokio join_all)                                                                                                                                                                             |
| `enc`     | 编码/字节        | 无                   | 否                     | `urlEncode`/`b64Decode`/`b64Encode`/`hexDecode`/`hexEncode`/`strToBytes`/`bytesToStr`                                                                                                                                         |
| `crypto`  | 加解密/摘要/签名 | 无                   | 否                     | `md5`/`md5_16`/`sha1`/`sha256`/`hmacHex`/`hmacBase64`/`digestHex`/`digestBase64`/`createSymmetricCrypto`/`createAsymmetricCrypto`/`createSign` + Cipher 方法 + aes/des/tripleDES 便捷函数                                     |
| `fs`      | 磁盘文件/压缩    | Fs                   | 是                     | `download`/`readFile`/`readTxt`/`getFile`/`delete`/`unzip`/`unrar`/`un7z`/`unArchive`/`getTxtInFolder`/`cache.putFile`/`cache.getFile`（相对缓存目录沙箱）                                                                    |
| `web`     | WebView/浏览器   | WebView              | 是                     | `webView`/`webViewGetSource`/`webViewGetOverrideUrl`/`startBrowser`/`startBrowserAwait`/`getVerificationCode`                                                                                                                 |
| `util`    | 杂项工具         | 无                   | 否(纯计算)/是(openUrl) | `log.info`/`log.type`/`cache.get`/`cache.put`/`cache.del`/`cache.getMem`/`cache.putMem`/`cache.delMem`/`t2s`/`s2t`/`htmlFormat`/`toNumChapter`/`randomUUID`/`timeFormat`/`timeFormatUTC`/`openUrl`/`androidId`/`getWebViewUA` |
| `rule`    | 规则解析         | 无（规则节点内注入） | 否                     | `getString`/`getStringList`/`getElement`/`getElements`/`setContent`/`setBaseUrl`（调同一 Extract 处理器）                                                                                                                     |
| `script`  | 动态脚本         | Script               | 是                     | `import`(fetch+eval，eval 安全门)                                                                                                                                                                                             |
| `cookie`  | Cookie           | 无                   | 否                     | `get`/`getKey`/`set`/`replace`/`remove`                                                                                                                                                                                       |
| `source`  | 当前书源         | 无                   | 否                     | `key`/`getVariable`/`setVariable`/`getHeaderMap`/`getLoginHeader`/`getLoginHeaderMap`/`putLoginHeader`/`removeLoginHeader`/`getLoginInfo`/`getLoginInfoMap`/`removeLoginInfo`                                                 |
| `book`    | 当前书籍         | 无                   | 否                     | Book 只读属性（见 inventory §14）                                                                                                                                                                                             |
| `chapter` | 当前章节         | 无                   | 否                     | Chapter 只读属性（见 inventory §14）                                                                                                                                                                                          |

### runtime 升级

启 rquickjs `futures` feature，`JsRuntime` → `AsyncRuntime`，`Context` → `AsyncContext`，host 函数用 `Promise::wrap_future` 返 JS Promise，支持顶层 `await`（`eval_promise`）。`processor.rs` 现有同步 `JsRuntime::new` 调用点改 AsyncRuntime。

### 导入器静态映射表

legado `java.xxx` → 自研域对象，覆盖 inventory 全函数。`ajax`/`connect`/`get`/`head`/`post` → `net.*`（加 `await`）；`ajaxAll` → `await net.getAll`；`encodeURI`/`base64*`/`hex*`/`strToBytes`/`bytesToStr` → `enc.*`；`md5*`/`digestHex`/`HMac*`/`createSymmetricCrypto`/`aes*`/`des*`/`tripleDES*` → `crypto.*`；`downloadFile`/`readFile`/`unzip*` 等 → `fs.*`；`webView*`/`startBrowser*` → `web.*`（加 `await`）；`t2s`/`s2t`/`htmlFormat`/`timeFormat*`/`randomUUID`/`androidId`/`openUrl` → `util.*`；`log`/`logType` → `util.log.*`；`cache.*` → `util.cache.*`；`cookie.*` → `cookie.*`；`source.*` → `source.*`；`getString*`/`getElement*`/`setContent`/`setBaseUrl` → `rule.*`；`importScript` → `await script.import`；`queryTTF`/`replaceFont`/`getZip/Rar/7z*Content` → 标注「需人工重写」（首刀不注入）。

### 安全与沙箱

`net.*` 走 `check_capability(Network)` + `validate_url_and_pin`（SSRF + DNS-rebinding IP pin），URL 由 JS 动态拼，校验在调用瞬间做（沿用 ADR-0029）。`fs.*` 走 `check_capability(Fs)` + 路径沙箱（限定缓存目录，禁止逃逸）。`web.*` 走 `check_capability(WebView)`。`script.import` 走 eval 安全门（白名单 URL 或源级批准）。`crypto`/`enc`/`util` 纯计算无 capability 强制。

## Key Flows

**JS 调 host async 流**：JS `@js` 块内 `var r = await net.get(url, {headers})` → rquickjs Promise → Rust host 函数返 `Promise::wrap_future(async {...})` → tokio 调度 HTTP + SSRF 校验 → resolve StrResponse → JS 拿结果。全程不阻塞 JS thread，其他 JS 逻辑可并发。

**批量流**：JS `var list = await net.getAll([u1,u2,u3])` → Rust `tokio::join_all` 三个 HTTP 并发 → resolve 数组 → JS 拿。与 legado `ajaxAll` 语义一致（并发），导入器加 `await` 即可。

**规则解析流**：JS `var t = rule.getString(ruleStr, content)` → Rust host 调同一 Extract 节点处理器 → 返字符串。JS 内可多次调，状态变化走 `rule.setContent`。

**导入器映射流**：legado 书源 JS 字符串 → 导入器解析 `java.xxx` 调用 → 按静态映射表改写为自研命名（加 `await`/改域对象）→ 遇不注入函数标注「需人工重写」→ 输出可跑的规则。

## Acceptance Examples

粘贴一个真实 legado 书源（含 `java.ajax` GET 搜索 + `java.getString` 规则解析 + `java.base64Decode` 解密 + `java.md5Encode` 签名 + `java.ajaxAll` 批量详情），经导入器静态映射后，在我们的 rquickjs host 跑通：搜索返 N 本 → 批量详情并发 → 规则解析出字段 → 解密签名通过 → 全程 async 不阻塞 UI。

粘贴一个含 `java.queryTTF` 字体反查的书源，导入器标注「需人工重写」，首刀不跑通（显式非目标）。

## Success Criteria

- legado 主流书源 JS 函数覆盖率 ≥ 90%（按 inventory 全函数计，不注入子集明确标注）。
- host async 调用不阻塞 JS thread：`net.get` 期间其他 JS 逻辑可并发执行。
- `net.*` SSRF + IP pin 双门生效，私网/内网/DNS-rebinding URL 被挡。
- `fs.*` 路径沙箱生效，缓存目录外读写被挡。
- 导入器静态映射覆盖 inventory 全函数，不注入子集人工标注率 100%。
- rquickjs futures feature 启用，AsyncRuntime 跑通 Promise + 顶层 await。

## Scope Boundaries

### Deferred for later

- 字体反查（`queryTTF`/`queryBase64TTF`/`replaceFont`/`cache.put(getQueryTTF)`）：首刀不注入，按真实书源密度另立 ADR。
- 远端压缩包取内容（`getZip/Rar/7zStringContent`/`getZip/Rar/7zByteArrayContent`）：首刀不注入，`fs.unzip` 等本地解压已覆盖大部分场景。
- `cache` 文件子集超大文件上限策略：首刀不定，按真实用例引。
- WebView 复杂交互（JS 注入时序、cookie 同步、UA 伪装）：首刀注入基础 `webView`/`startBrowser`，复杂交互按真实书源密度引。

### Outside this product's identity

- legado Rhino 原生 Java 互操作（`JavaAdapter`/`importClass`/`importPackage`/`Packages.java.*`）：rquickjs 无此能力，书源里这类用法属不兼容面，导入器标注不兼容，不做兼容层。
- legado `rssArticle` 对象：订阅源正文规则首刀不在本提案范围（ADR-0001 首刀图文书源），订阅源 host API 另立。
- legado `RssJsExtensions`（`searchBook`/`addBook`）：订阅源专用，不在本提案。
- legado `AnalyzeUrl` 登录检查 JS 作用域（`initUrl`/`getStrResponse`/`getResponse` 等）：登录检查链路另立，不在本提案 host API。

## Dependencies and Assumptions

- rquickjs 0.12 `futures` feature 启用（当前未开），AsyncRuntime/AsyncContext/Promise 可用。
- 加密 crate 选型（aes/cbc/cipher/rsa/sha2/hmac/md-5 等），ce-plan 定具体 crate 与版本。
- Tauri WebView 组件可用（`web` 域依赖），ce-plan 定是 Tauri 内置 WebView 还是系统组件。
- 机器标识库或平台分支代码（Windows MachineGuid 注册表 / macOS ioreg / Linux machine-id），ce-plan 定具体方案。
- ADR-0030 立案接受本提案 host 能力边界（演进/部分否决 ADR-0029）。
- 假设：legado 主流书源 JS 不强依赖 Rhino 原生 Java 互操作（`JavaAdapter`/`importClass`），若真实书源密度高则不兼容面扩大，需 grill 复议。

## Outstanding Questions

### Resolve before planning

- `fs` 域 capability 边界：全磁盘访问 vs 限定缓存目录沙箱？ADR-0029 本地零注入底线已破，沙箱边界需 ADR-0030 定。
- `script.import` eval 安全门机制：白名单 URL / 源级批准 / 签名验证？dynamic eval 是高危面，门控机制需定。
- `web` 域用 Tauri 内置 WebView 还是系统组件？影响 capability 门控与跨平台一致性。
- crypto crate 选型：aes/rsa/cipher 各 crate 版本与 API 形状，影响 host 函数签名。
- `rule` 域注入时机：所有 JS node 内注入还是仅规则节点内注入？影响 host API 暴露面。

### Deferred to planning

- 机器标识归一具体实现（注册表/ioreg/machine-id 调用与 hex 归一）。
- 导入器静态映射表的字面匹配规则（正则 vs AST 解析 legado JS）。
- `cache` 文件子集大小上限与淘汰策略。
- ADR-0013 stream 与 host async 批量并存的机制验证（是否互踩）。

## Sources and Research

- `docs/reference/legado-js-builtin-inventory.md` — legado 全函数清单（源码考证版，本提案输入基准）。
- `.tmp/legado/app/src/main/assets/web/help/md/jsHelp.md` — legado 官方 JS 帮助（标注「部分函数」，inventory 以源码补全）。
- `.tmp/legado/app/src/main/java/io/legado/app/help/JsExtensions.kt` — legado JS 扩展主接口。
- `.tmp/legado/app/src/main/java/io/legado/app/help/JsEncodeUtils.kt` — legado JS 加解密接口。
- `.tmp/legado/app/src/main/java/io/legado/app/model/analyzeRule/AnalyzeRule.kt` — legado 规则解析 + evalJS 注入点。
- `.tmp/legado/app/src/main/java/io/legado/app/constant/AppConst.kt` — legado androidId 实现。
- `docs/adr/0029-js-host-api-injection-and-network-boundary.md` — 首刀 host API 注入与网络边界（本提案演进）。
- `docs/adr/0008-js-sandbox-tiered-configurable.md` — JS 沙箱分层配置。
- `docs/adr/0013-streaming-async-execution-paradigm.md` — 流式异步执行范式（与 host async 批量并存张力）。
- `src-tauri/crates/lj-node-js/src/host_api.rs` — 现有 host_api 骨架（首刀空操作，待落地）。
- rquickjs 0.12 文档：`AsyncRuntime`/`AsyncContext`/`Promise::wrap_future`/`eval_promise`。
