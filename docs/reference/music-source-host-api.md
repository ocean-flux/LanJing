# 音乐源宿主 API 需求：lx-music 与 any-listen

> 票据：ocean-flux/LanJing#65「架构与分层重划」下的 #69。本文是 #71「宿主 API 面」的输入。
>
> 一手来源（均为浅克隆，行号对应当前 commit）：
>
> | 仓库 | 本地路径 | commit | 版本 |
> | --- | --- | --- | --- |
> | any-listen | `C:/Users/wuhy/workspace/_research/any-listen` | `3734f1a9ad91cc406524680dccfd08e0bbdd1c90` | origin/main |
> | lx-music-desktop | `C:/Users/wuhy/workspace/_research/lx-music-desktop` | `ad95d5091c9ed689fa72b5e5c849df65f5a679ce` | 2.12.6 |
> | lx-music-doc | `C:/Users/wuhy/workspace/_research/lx-music-doc` | `a033df5292dfb0b371043417ea06071df518c904` | origin/master |
>
> 下文路径分别相对上述三个仓库。未标来源的断言一律按未验证处理。

## 0. 前提修正

票据里三个前提与源码不符，先纠正。

### 0.1 两个生态都没有 `host.*`

| 生态 | 实际形状 | 来源 |
| --- | --- | --- |
| any-listen | 单模块 `require('any-listen')`，返回扁平对象，顶层成员即命名空间 | `any-listen/packages/shared/extension-preload/src/index.ts:13` |
| lx-music | 单个全局对象 `globalThis.lx` | `lx-music-desktop/src/main/modules/userApi/renderer/preload.js` |

`host` 这个词在 any-listen 里只出现在内部通道名上：`__ext_host_call__utils_*`（`any-listen/packages/shared/extension-preload/src/types/extension_vm.d.ts:69`）。「大约 20 个命名空间」这个量级对 any-listen 成立（见 §1.1）。

### 0.2 `musicSearch` / `hotSearch` / `songlist` 不是 lx-music 音乐源导出的函数

这三个名字在 lx-music 里确实存在，但属于应用内部的 music SDK，不属于源脚本 API。

- 源脚本 API 只认 3 个 action：`musicUrl` / `lyric` / `pic`（§2.3）。
- `musicSearch` / `hotSearch` / `songList` / `leaderboard` / `comment` / `album` / `tipSearch` 是 `lx-music-desktop/src/renderer/utils/musicSdk/kw/` 等目录下的模块名。
- 那套内置源已在 desktop 2.6.0 被移除（`CHANGELOG.md:400`「移除所有内置源」）。当前 `src/renderer/utils/musicSdk/api-source-info.ts` 的源列表是空数组（只剩注释掉的示例），`src/renderer/core/apiSource.ts:21` 只在 `^user_api` 与内置 api id 之间二选一。目录与模块还在，但已不可达，是死代码。

结论：lx-music 今天对外只剩 3 个 action。要 search / songlist 能力，只能从 any-listen 那一侧要。

### 0.3 两个生态都不提供第三方库

lx-music 源没有 `axios`，也没有 `crypto-js`。详见 §2.4。any-listen 扩展同理，只有 `require('any-listen')` 一个模块名可用（§1.3）。

## 1. any-listen（已考证）

### 1.1 宿主 API 顶层成员

接口定义 `packages/shared/extension-preload/src/types/api.d.ts:1187`（`interface API`），装配点 `.../src/apis/exposeAPI.ts:23`。

| 成员 | 动作数 | 动作 | 来源 |
| --- | --- | --- | --- |
| `env` | 8 | clientType / version / platform / arch / locale / publicKey / extensionVersion / onLocaleChanged | api.d.ts `interface Env` |
| `app` | 6 | showMessage / showInputDialog / showOpenDialog / showSaveDialog / readOpenDialogFile / writeSaveDialogFile | api.d.ts `interface App` |
| `logcat` | 4 | debug / info / warn / error | api.d.ts `interface Logcat` |
| `storage` | 6 | writeFile / readFile / removeFile / fileExists / listFiles / statFile | api.d.ts `interface Storage` |
| `configuration` | 3 | getConfigs / setConfigs / onConfigChanged | api.d.ts `interface Configuration` |
| `musicUtils` | 2 | createProxyUrl / writeProxyCache | api.d.ts `interface MusicUtils` |
| `registerResourceAction` | 1 | 注册资源动作处理器 | api.d.ts:1199 |
| `registerListProviderAction` | 1 | 注册列表提供者动作处理器 | api.d.ts:1200 |
| `command` | 3 | registerCommand / executeCommand / getCommands | api.d.ts `interface Command` |
| `constants` | 2 | storageDir / extensionDir | api.d.ts:1224 |
| `utils.crypto` | 9 | aesEncrypt / aesDecrypt / rsaEncrypt / rsaDecrypt / randomBytes / md5 / sha1 / sha256 / sha512 | api.d.ts `interface Crypto` |
| `utils.iconv` | 2 | decode / encode | api.d.ts `interface Iconv` |
| `utils.zlib` | 4 | deflate / inflate / gzip / gunzip | api.d.ts `interface Zlib` |
| `utils.dataConverter` | 1 | 字符串与字节互转，两次重载 | api.d.ts `dataConverter` |
| `utils.createIsolateContext` | 4 | sendMessage / run / runFile / destroy，需 grant | api.d.ts `interface IsolateContext` |
| `t` | 1 | i18n 取词 | api.d.ts `interface API` |
| `request` | 1 | 需 grant `internet` | api.d.ts:1194 |
| `musicList` | 可选 | 需 grant `music_list`，上游未实现 | api.d.ts:1190 |
| `player` | 可选 | 需 grant `player`，上游未实现 | api.d.ts:1191 |

`request` 的选项形状（`RequestOptions`）：method / query / headers / timeout / maxRedirect / signal / json / form / binary / text / formdata / xml / needRaw。返回 `Response<Resp>`：statusCode / headers / raw(Uint8Array) / body / history。

### 1.2 grant 清单与门控

`packages/shared/types/types/extension.d.ts:4` 定义 4 个：`internet` / `player` / `music_list` / `isolate_context`。manifest 里未列出的 grant 会被静默过滤（`manifest.ts:409` 的 `formatManifest`，白名单 `manifest.ts:7` 的 `GRANTS`）。

实际生效的只有 `internet`（给 `request`）与 `isolate_context`（给 `utils.createIsolateContext`）。另两个在 `exposeAPI.ts` 里是注释掉的 TODO，且两个分支的变量名互换：`case 'player'` 下面写的是 `musicList`，`case 'music_list'` 下面写的是 `player`。

结论：上游的 grant 语义没有定型，不要照抄它的命名。

### 1.3 沙箱注入与移除

来源 `packages/shared/extension-preload/src/index.ts`。

- 注入 `env_setup`，只可调用一次，调用后自删除。
- 注入 `require`，只认 `'any-listen'`，其它模块名直接抛错。
- 覆盖 `setTimeout` / `clearTimeout` / `setInterval` / `clearInterval`，实现走宿主（`__ext_host_call__set_timeout` 等）。
- 覆盖 `AbortController` 为自定义实现（`src/event/AbortController.ts`）。
- 定义不可枚举、不可写、不可配置的 `__ext_preload__`。
- 删除 `globalThis.console`，扩展只能走 `logcat`。
- 最后 `freezeEnv(extensionAPI)` 冻结 API 对象。

### 1.4 resource action 清单

有两个不同的集合，容易混：

- 可在 manifest 里声明的 17 个：`extension.d.ts:5` 的 `ResourceAction` 联合，与 `manifest.ts:8` 的 `RESOURCE` 白名单一致。`parseResource`（`manifest.ts:201`）用它过滤，未列入的动作被静默丢弃。
- 可注册处理器的 18 个：`api.d.ts` 的 `interface ResourceAction`。

差集：manifest 独有 4 个 = `albumSearch` / `album` / `singerSearch` / `singer`。处理器独有 5 个 = `songlistSorts` / `songlistTags` / `songlistDetail` / `topSongsDate` / `topSongsDetail`。交集 13 个。

宿主调用扩展的签名在 `packages/shared/types/types/extension_ipc.d.ts` 的 `IPCExtension.ResourceAction`。参数都带 `extensionId` + `source` 两个公共字段。

| 动作 | 参数（除公共字段外） | 返回 |
| --- | --- | --- |
| `tipSearch` | keyword | string[] |
| `hotSearch` | — | string[] |
| `musicSearch` | page, limit?, name, artist?, albumName? | ListCommonResult\<MusicInfoOnline\> |
| `musicPic` | musicInfo | string |
| `musicUrl` | musicInfo, quality?, type? | MusicUrlInfo { url, quality } |
| `musicLyric` | musicInfo | LyricInfo |
| `musicPicSearch` | name, artist? | string[] |
| `lyricSearch` | name, artist?, interval? | LyricSearchResult[] |
| `lyricDetail` | id | LyricInfo |
| `songlistSearch` | page, limit?, keyword | ListCommonResult\<SongListItem\> |
| `songlistSorts` | — | TagItem[] |
| `songlistTags` | — | { tags: TagGroupItem[], hotTags: TagItem[] } |
| `songlist` | page, limit?, sort, tag | ListCommonResult\<SongListItem\> |
| `songlistDetail` | page, limit?, id | ListCommonResult\<MusicInfoOnline\> + { info: SongListDetailInfo } |
| `topSongs` | — | TopSongsItem[] |
| `topSongsDate` | id | TagItem[] |
| `topSongsDetail` | page, limit?, id, date | ListCommonResult\<MusicInfoOnline\> + { info: TopSongsDetailInfo } |
| `musicComment` | page, limit?, musicInfo, id?, type: 'hot' \| 'new' \| 'reply' | ListCommonResult\<MusicCommentItem\> |

`ListCommonResult<T>` = { list: T[], total, page, limit }。

分发不做声明校验：`worker/extensionService/index.ts:159` 的 `resourceAction` 只按 `extensionId` 找 VM 或内置扩展，不检查 `contributes.resource` 是否声明了该动作。`contributes.resource` 只用于发现与筛选。

### 1.5 `contributes.resource[].resource` 的形状

```ts
contributes: {
  resource: Array<{
    id: string                 // 资源 id，同时是动作参数里的 source，上限 64 字符
    name: string               // 展示名，上限 128 字符
    resource: ResourceAction[] // 该 id 提供哪些动作
  }>
}
```

来源：类型 `extension.d.ts:125`，长度上限 `manifest.ts:29`（`MANIFEST_STRING_LIMIT`），过滤逻辑 `manifest.ts:201`（`parseResource`）。

消费点：`packages/shared/app/modules/extension/onlineListProvider.ts:100` 用 `id` 非空且 `resource` 非空来判定该扩展是不是在线列表提供者。

第一方样例（内置 WebDAV 扩展，`internalExtension/extensions/webdav/index.ts`）：

```ts
resource: [{ id: 'webdav', name: 't(exts.webdav.name)', resource: ['musicUrl', 'musicPic', 'musicLyric'] }]
```

### 1.6 manifest 字段与版本协商

必填（`formatManifest`，`manifest.ts:409`，不满足就抛错）：

- `id`：非空，且只允许 `[\w-_.]`。
- `name`：非空。

`verifyManifest`（`manifest.ts:439`）额外要求 `main` 能解析：必须是相对路径、不得越出扩展目录、文件必须存在，否则抛 `Main enter not defined`。

类型上必填但校验未强制：`version`、`publicKey`（`extension.d.ts:121` 与 `:122`）。

可选：description / icon / target_engine / author / homepage / license / categories / tags / readme / grant / contributes。

字符串上限见 `MANIFEST_STRING_LIMIT`（`manifest.ts:29`）：id 64、name 128、description 1024、readme 10240、icon 260、main 260、version 32、target_engine 64、author 128、homepage 512、license 64、publicKey 8192。icon 只接受 `.png` / `.jpg` / `.jpeg` / `.webp` / `.svg`。

版本协商：

1. 引擎版本是常量 `EXTENSION_ENGINE = '1.4.0'`（`packages/shared/common/constants.ts:5`）。
2. 扩展用 `target_engine` 声明所需最低引擎版本。
3. 比较逻辑 `getCompareVersion`（`shared/app/modules/worker/extensionService/shared/index.ts:83`）与 `getCompareVersionMessage`（同文件 `:90`）：先比完整版本，若引擎低于 `target_engine` 则报 `incompatible_old_engine`。否则只比主版本号，引擎主版本大于目标主版本报 `incompatible_new_engine`，相等则兼容，主版本号解析失败也视为兼容。
4. 文案 `packages/shared/i18n/langs/zh-cn.json:118` 与 `:119`。

结论：`target_engine` 就是「最低引擎版本」，门控只按主版本号做，双向卡。没有 API 版本号、没有能力协商、没有 schema 版本。

### 1.7 any-listen 与 lx-music 的关系

any-listen 出自 lx-music 同一作者 lyswhut。一手证据：

- `.vscode/typescript.code-snippets:2` 的工作区名仍是 `lx-music-desktop-new`。
- `.github/workflows/release.yml:31` 提交身份用 `lyswhut@qq.com`。
- `CODE_OF_CONDUCT.md:50` 同一邮箱。
- `pnpm-lock.yaml:322` 依赖 `github:lyswhut/spinnies`。
- `packages/desktop/build-config/build-before-pack.cjs:32` 直接引用 `lx-music-desktop` 的 issue 编号。

含义：any-listen 的 18 个 resource action 不是 lx-music **源脚本** API 的扩张，而是 lx-music **应用内部 SDK** 的扩张。对应关系（lx-music 侧模块见 `src/renderer/utils/musicSdk/kw/`、`wy/`、`tx/`、`kg/`、`mg/`）：

| any-listen action | lx-music 应用内部模块 |
| --- | --- |
| `tipSearch` | `tipSearch.js` |
| `hotSearch` | `hotSearch.js` |
| `musicSearch` | `musicSearch.js` |
| `musicPic` | `pic.js` |
| `musicUrl` | `index.js` 的 `getMusicUrl` |
| `musicLyric` | `lyric.js` |
| `musicComment` | `comment.js` |
| `songlist` / `songlistSearch` / `songlistDetail` | `songList.js` |
| `topSongs` / `topSongsDetail` | `leaderboard.js` |
| `album` / `albumSearch` | `album.js` |
| `singer` / `singerSearch` | `singer.js` |

没有对应模块的 action 是 any-listen 新增的：`musicPicSearch` / `lyricSearch` / `lyricDetail` / `songlistSorts` / `songlistTags` / `topSongsDate`。

lx-music 的**源脚本** API 始终只有 3 个 action（`musicUrl` / `lyric` / `pic`，见 §2.3），没有跟着扩张。所以 any-listen 是把 lx-music 的内置源实现搬成了可安装扩展，并把原本只在应用内部的 action 面公开出去。

### 1.8 真实插件生态：`any-listen-extension-online-metadata`

官方在线元数据扩展（`any-listen/any-listen-extension-online-metadata`，Apache-2.0，v0.4.7 / 2026-10-06）。这是"插件生态"的本体：把 §1.7 说的 lx-music 内置源实现，按平台拆成可安装扩展。

**规模**：`src/onlineResource/` 共 7,690 行，5 个平台目录 —— `kg` 酷狗 / `kw` 酷我 / `mg` 咪咕 / `tx` QQ 音乐 / `wy` 网易云。

**真实插件消费的宿主能力面**（`src/shared/hostApi.ts` 全文，9 个）：

| 成员 | 用途 |
| --- | --- |
| `api.request` | HTTP |
| `api.utils.crypto` | 签名与摘要 |
| `api.utils.iconv` | 编码转换 |
| `api.utils.zlib` | 压缩解压 |
| `api.utils.dataConverter` | 数据结构转换 |
| `api.logcat` | 日志 |
| `api.env.version` | 引擎版本 |
| `api.t` | i18n |
| `api.registerResourceAction` | 注册动作处理器 |

这里**没有**文件系统、WebView、播放器控制、cookie 存储。§3.2 的 12 个口子是 API 面的并集，真实插件的用度是它的子集。

**注册的动作**（`src/onlineResource/index.ts`，14 个）：

`musicSearch` `musicPic` `musicLyric` `songlistSearch` `songlistDetail` `songlistTags` `songlistSorts` `songlist` `topSongs` `topSongsDate` `topSongsDetail` `tipSearch` `hotSearch` `musicComment`

另有 2 个被注释未实现：`musicPicSearch` `lyricSearch`。

**维护负担的形状**。`publish/version.json` 的 history 有 24 个版本（v0.2.2 / 2025-05-27 → v0.4.7 / 2026-10-06），几乎每条日志都是"修某个平台的某个动作"：

```
v0.2.6  fix kw music pic url
v0.2.7  fix wy music search
v0.2.11 fix kg search
v0.3.2  fix kg songlist turn page
v0.3.3  fix tx songlist load
v0.4.2  fix kg song list details names decode
v0.4.3  fix tx music search
v0.4.6  fix kg songlist load
```

单平台单动作的实现量也大：`kg/songlistDetail.ts` 19.1K、`tx/qrcDecode.ts` 13.9K（QQ 加密歌词）、`kg/topSongs.ts` 13.5K、`tx/songlist.ts` 12.4K、`wy/topSongs.ts` 12.4K。

**含义**：宿主 API 面小且稳定（9 个成员，一年没变），成本全在适配器。5 个平台各自持续漂移，扩展方要一直跟。LanJing 若支持这个生态，接的是"5 个持续漂移的平台适配器"，不是"一个稳定的插件规范"。
## 2. lx-music（已考证）

### 2.1 源 API 的调用形状

源不是导出一个对象，而是事件式。来源 `lx-music-doc/docs/desktop/custom-source.mdx`。

1. 脚本用 `lx.on(EVENT_NAMES.request, handler)` 注册请求处理器。handler 收 `{ source, action, info }`，必须返回 Promise。
2. 脚本用 `lx.send(EVENT_NAMES.inited, { sources, openDevTools })` 声明自己支持哪些源与动作。`inited` 只能发一次。
3. `inited` 之前的首个未捕获错误（含 unhandledrejection）即初始化失败，宿主会弹窗报错。
4. 宿主通过 `lx.send(EVENT_NAMES.updateAlert, { log, updateUrl })` 让脚本弹更新提示，每个脚本只能调用一次。

源的 key 固定为 `kw` / `kg` / `tx` / `wy` / `mg` / `local`（`lx-music-desktop/src/main/modules/userApi/renderer/preload.js` 的 `allSources`）。脚本不能自定义源 key。

宿主到脚本的调用走 IPC，参数形状 `{ requestKey, data }`（`lx-music-desktop/src/common/types/user_api.d.ts` 的 `UserApiRequestParams`）。

### 2.2 `globalThis.lx` 成员

| 成员 | 形状 | 来源 |
| --- | --- | --- |
| `version` | 字符串常量，桌面端为 `'2.0.0'` | preload.js `version: '2.0.0'` |
| `env` | `'desktop'` 或 `'mobile'` | preload.js `env: 'desktop'` |
| `currentScriptInfo` | { name, description, version, author, homepage, rawScript } | preload.js |
| `EVENT_NAMES` | { request: 'request', inited: 'inited', updateAlert: 'updateAlert' } | preload.js:19 |
| `on(event, handler)` | 返回 Promise，只接受 `request` | preload.js |
| `send(event, data)` | 返回 Promise，只接受 `inited` / `updateAlert` | preload.js |
| `request(url, options, cb)` | 返回取消函数；options 只支持 method / timeout / headers / body / form / formData | preload.js |
| `utils.crypto` | aesEncrypt / rsaEncrypt / randomBytes / md5 | preload.js |
| `utils.buffer` | from / bufToString | preload.js |
| `utils.zlib` | inflate / deflate（仅桌面端） | preload.js |

`lx.request` 的细节（preload.js）：method 默认 `get`，timeout 被钳到最大 60000 毫秒且默认 60000，底层用 `needle`，代理走 `tunnel`。回调收到 `(err, resp, body)`，`resp` 形状是 `{ statusCode, statusMessage, headers, bytes, raw, body }`，`body` 会尝试 `JSON.parse`。返回的取消函数调用 `request.abort()`。

`lx.utils.crypto` 的细节（preload.js）：

- `aesEncrypt(buffer, mode, key, iv)`：直接用 Node `crypto.createCipheriv(mode, key, iv)`，所以 `mode` 是 Node 的 `aes-128-cbc` 形式，不是自定义枚举。返回 Buffer。
- `rsaEncrypt(buffer, key)`：固定 `RSA_NO_PADDING`，且先 `Buffer.alloc(128 - buffer.length)` 左补零到 128 字节。
- `randomBytes(size)`：返回 Buffer（移动端返回字符串，见 §2.7）。
- `md5(str)`：返回 hex 字符串。

### 2.3 导出函数（action）清单

只有 3 个。

类型定义 `lx-music-desktop/src/common/types/user_api.d.ts`：

```ts
type UserApiSourceInfoActions = 'musicUrl' | 'lyric' | 'pic'
```

每个源可声明的 action 由宿主白名单决定（preload.js 的 `supportActions`）：

| 源 key | 可声明 action | 可声明音质 |
| --- | --- | --- |
| kw / kg / tx / wy / mg | `['musicUrl']` | `['128k', '320k', 'flac', 'flac24bit']` |
| local | `['musicUrl', 'lyric', 'pic']` | `[]` |

`handleInit` 对脚本声明的能力做求交：`actions.filter(a => userSource.actions.includes(a))`、`qualitys.filter(q => userSource.qualitys.includes(q))`。

注意 preload.js 的 `supportActions` 里多一个 `xm: ['musicUrl']`，但 `allSources` 不含 `xm`，所以 `xm` 不可达，是死项。

`info` 的形状（`docs/desktop/custom-source.mdx`）：

| action | info | 返回 |
| --- | --- | --- |
| `musicUrl` | `{ type, musicInfo }`，`type` 是音质，`local` 源为 `null` | `http(s)` URL 字符串 |
| `lyric` | `{ musicInfo }` | `{ lyric, tlyric, rlyric, lxlyric }` |
| `pic` | `{ musicInfo }` | `http(s)` URL 字符串 |

返回值校验在宿主侧（preload.js 的 `handleRequest`）：

- `musicUrl` 与 `pic`：必须是字符串、长度 ≤ 2048、匹配 `/^https?:/`，否则视为失败。
- `lyric`：`lyric` 必填且 ≤ 51200。`tlyric` ≤ 5120、`rlyric` ≤ 5120、`lxlyric` ≤ 8192，超限的字段被替换为 `null`。
- `updateAlert`：`log` 必填，超 1024 截断并追加 `...`。`updateUrl` 必须是 http(s) URL 且 ≤ 1024，否则被丢弃。

### 2.4 源能用的库

没有第三方库。`axios` 和 `crypto-js` 都不提供。

证据：

1. 官方文档只列 `lx.utils` 的 7 个方法，并明写「目前仅提供以上工具方法，如果需要其他方法可以在 GitHub 提交 Issue 进行讨论」（`docs/desktop/custom-source.mdx`）。
2. 桌面端脚本跑在一个隐藏的 Electron renderer 里，`webPreferences` 是 `nodeIntegration: false`、`nodeIntegrationInWorker: false`、`contextIsolation: true`、`sandbox: false`、`images: false`、`webgl: false`（`lx-music-desktop/src/main/modules/userApi/main.ts`）。页面 CSP 是 `default-src 'none'`（`src/main/modules/userApi/renderer/user-api.html`）。
3. 因此脚本没有 `require`、没有 Node 模块。要用 `axios` / `crypto-js`，作者必须把库内联进脚本源码。

桌面端是浏览器环境，所以 `window` / `fetch` / `XMLHttpRequest` / `console` 可用（文档只说 `lx.request` 不受跨域限制）。**未验证**：官方文档没有正面列出桌面端脚本可用的宿主 API 清单，本节只按「Electron renderer 无 nodeIntegration」推断。

### 2.5 沙箱

（`lx-music-desktop/src/main/modules/userApi/main.ts`）

- 屏蔽窗口事件：`will-navigate` / `will-redirect` / `will-attach-webview` / `will-prevent-unload` / `media-started-playing`。
- 所有权限请求一律 `resolve(false)`。
- `setWindowOpenHandler` 一律返回 `{ action: 'deny' }`。
- 窗口不可见、不可调整、不可最小化/最大化。
- 同时最多 20 个源（i18n `user_api__max_tip`，`lang/zh-cn.json:736`）。

官方在界面上明写这是「尽可能地隔离」而非强隔离（`lang/zh-cn.json:738`）。

### 2.6 源格式的版本演进

`lx.version` 的编号与桌面应用版本无关，是独立的源 API 版本。全部来自 `lx-music-desktop/CHANGELOG.md`。

| 源 API 版本 | desktop 版本 | 日期 | 变更 |
| --- | --- | --- | --- |
| v1（未编号） | 1.8.0 | 2021-03-07 | 新增自定义源功能（`CHANGELOG.md:1326`） |
| v1.x | 1.15.0 | 2021-10-29 | 新增 `version` 字段与 `utils.buffer.bufToString`（`CHANGELOG.md:1121`） |
| v1.x | 1.18.0 | 2022-02-26 | 新增 `updateAlert` 更新弹窗（`CHANGELOG.md:977`） |
| v1.3.0 | 2.3.0 | 2023-06-29 | 新增 `utils.zlib.inflate` 与 `zlib.deflate`，文档明写「API版本更新到 v1.3.0」（`CHANGELOG.md:562`） |
| **v2.0.0** | 2.6.0 | 2024-02-01 | 不兼容变更，见下（`CHANGELOG.md:404`） |
| v2.0.0 | 2.12.6 | 2026-09-19 | 当前版本，preload.js 仍写 `version: '2.0.0'` |

v2.0.0 的不兼容变更（`CHANGELOG.md:406` 到 `:414`）：

1. 不再推荐 `window.lx`，改用 `globalThis.lx`，为了与移动端统一。
2. `inited` 不再传 `status`，改成「inited 之前的首个未捕获错误即初始化失败」。
3. 新增 `globalThis.lx.env`。
4. 新增 `globalThis.lx.currentScriptInfo`。
5. `globalThis.lx.version` 更新到 `2.0.0`。
6. 不再用 `<script>` 标签执行脚本，原始代码改从 `currentScriptInfo.rawScript` 取。
7. `local` 源新增支持 `musicUrl` / `pic` / `lyric`。

同一版本还移除了所有内置源（`CHANGELOG.md:400`，原因写的是腾讯投诉）。这解释了 §0.2 的死代码。

**版本协商：没有。** 脚本不能声明所需 API 版本，宿主也不检查。脚本只能运行时读 `lx.version` 自己判断。这与 any-listen 的 `target_engine` 是相反的取舍：any-listen 有门控无能力协商，lx-music 两者都没有。

### 2.7 桌面端与移动端的差异

来源 `lx-music-doc/docs/mobile/custom-source.mdx`。移动端仓库未克隆。

- `lx.env` 在移动端固定为 `mobile`。
- `inited` 事件没有 `openDevTools` 选项。
- 移动端「浏览器、Node.js 等常见宿主环境 API 不可用」，可用宿主 API 只有 `setTimeout` / `clearTimeout`。
- `utils.buffer.from` / `bufToString` 只支持 `base64` / `hex` / `utf8`。
- `utils.crypto.aesEncrypt` 只支持 `aes-128-cbc` 与 `aes-128-ecb`。
- `utils.zlib.inflate` / `deflate` 未实现。
- `utils.crypto.randomBytes` 在移动端返回字符串，桌面端返回 Buffer。同一 API 两种返回类型。
- 除 `Function.prototype.toString` / `toLocaleString` / `Object.prototype.toString` 外，内置属性全部被冻结，`Array.prototype.push = ...` 无效，但可扩展（`Array.prototype.myPush = ...` 有效）。
- `updateAlert` 标注为「源版本 v1.2.0 新增」，与桌面端 v2.0.0 的编号体系不一致。

**未验证**：移动端 `lx.version` 的实际值。文档只说要读它，没有写值，移动端仓库也没有克隆。

## 3. 能力映射：哪些落在 host.http / host.crypto，哪些要新开口子

### 3.1 直接映射

| 生态 | 原语 | 目标口子 | 备注 |
| --- | --- | --- | --- |
| any-listen | `request(url, options)` | `host.http` | Promise + `signal` 取消 |
| any-listen | `utils.crypto.*`（9 个） | `host.crypto` | 含 RSA 与 AES 两种模式枚举 |
| any-listen | `utils.dataConverter` | 无 | 纯函数，可留在脚本侧 |
| lx-music | `lx.request(url, options, cb)` | `host.http` | 回调式 + 返回取消函数 |
| lx-music | `lx.utils.crypto.md5` / `randomBytes` | `host.crypto` | |
| lx-music | `lx.utils.crypto.aesEncrypt` / `rsaEncrypt` | `host.crypto` | 参数形式与 any-listen 不同，见 §3.3 |
| lx-music | `lx.utils.buffer.from` / `bufToString` | 无 | 纯转换，可留在脚本侧 |

### 3.2 必须新开的口子

lx-music 侧没有新增，它的需求是 any-listen 的子集。

| 能力 | any-listen 来源 | lx-music 侧对应 |
| --- | --- | --- |
| `host.compress` | `utils.zlib` deflate / inflate / gzip / gunzip | `lx.utils.zlib.inflate` / `deflate`（仅桌面端） |
| `host.encoding` | `utils.iconv` decode / encode | 无 |
| `host.asset` | `musicUtils.createProxyUrl` / `writeProxyCache` | 无 |
| `host.fs` | `storage` 6 个动作，加 `app.readOpenDialogFile` / `writeSaveDialogFile` | 无 |
| `host.kv` | `configuration` getConfigs / setConfigs / onConfigChanged | 无 |
| `host.dialog` | `app.showMessage` / `showInputDialog` / `showOpenDialog` / `showSaveDialog` | 无（`updateAlert` 是反向的宿主到脚本通知） |
| `host.isolate` | `utils.createIsolateContext` + `isolate_context` grant | 无 |
| `host.player` | `player` grant | 无 |
| `host.musicList` | `music_list` grant | 无 |
| `host.timer` | 注入的 setTimeout 族加自定义 `AbortController` | `setTimeout` / `clearTimeout`（移动端仅有这两个） |
| `host.log` | `logcat` 4 个动作 | 无（桌面端有 `console`，移动端没有） |
| `host.i18n` | `t` 与 `env.i18nMessages` / `onLocaleChanged` | 无 |
| `host.env` | `env` 与 `constants` | `lx.env` / `lx.version` / `lx.currentScriptInfo` |

合计：2 个直接映射（另 2 个纯转换不需要口子），12 个新能力口子，加 1 组元数据。口子数量由 any-listen 决定，lx-music 全覆盖。

### 3.3 别名层必须抹平的三个真实差异

1. HTTP 形状不同。any-listen 是 `Promise<Response>` 加 `signal` 取消。lx-music 是 `request(url, options, callback)`，返回一个取消函数，且 `timeout` 被宿主钳到 60000 毫秒。别名层要么给 lx-music 包一层 Promise 加取消句柄，要么让 `host.http` 同时暴露两种形状。
2. 注册形状不同。any-listen 直接调 `registerResourceAction({...})` 传对象。lx-music 是 `lx.on('request', handler)` 加 `lx.send('inited', ...)` 声明能力，且 handler 必须返回 Promise。别名层要能把「声明能力」和「处理请求」拆开。
3. 加密参数不同。lx-music 的 `aesEncrypt` 直接吃 Node 的 mode 字符串（如 `aes-128-cbc`）与 Buffer。any-listen 用自定义枚举（`CBC_128_PKCS7Padding` / `ECB_128_NoPadding`）并带独立 iv 参数。`host.crypto` 需要同时接受两种 mode 表示，或在别名层做映射。RSA 也不一致：lx-music 固定 `RSA_NO_PADDING` 且左补零到 128 字节，any-listen 有 `RSA_PKCS1_OAEP_PADDING` / `RSA_NO_PADDING` 两档且不做补零。

### 3.4 影响口子设计的三个观察

1. 取消是一等契约。any-listen 按 `requestKey` 取消（`extension-preload/src/apis/request.ts:7`），lx-music 按返回的函数取消（preload.js）。宿主出口不能只给一次性 Promise。
2. 返回值要强校验。any-listen 逐字段检查并抛带 `type=` / `value=` 的错误（`extension-preload/src/apis/resource.ts` 的 `withErrorReason`）。lx-music 校验 URL 长度与 `http(s)` 前缀、校验歌词各字段长度上限。宿主出口应对每个动作声明返回值上限。
3. 日志通道不对称。any-listen 删掉 `console`，只能走 `logcat`。lx-music 桌面端保留 `console`，移动端没有。宿主出口要有结构化日志通道，并允许脚本侧降级。

## 4. 对 #71 的结论

1. 别名投影可行。`host.http` 一个口子能同时吃掉 any-listen 的 `request` 与 lx-music 的 `lx.request`，前提是它同时暴露 Promise 与回调加取消句柄两种形状（§3.3）。
2. 只开 http 加 crypto 不够。按 any-listen 需要 12 个新能力口子（§3.2）。lx-music 侧是它的子集，不额外增加口子。
3. 动作面按 any-listen 的 18 个设计。lx-music 音乐源只剩 3 个 action（`musicUrl` / `lyric` / `pic`），是子集，没有 search / songlist / comment。
4. 版本协商要自己定义。lx-music 完全没有协商机制，脚本只能运行时读 `lx.version`。any-listen 只有主版本号双向门控（`target_engine`）。两者都不够，LanJing 若同时投影两个生态的别名，得自己定义能力协商与版本协商。
5. 不要照抄上游的 grant 命名与语义，any-listen 的还没定型（§1.2）。
6. `contributes.resource[].resource` 是「声明」不是「授权」。any-listen 分发时不做校验（§1.4）。LanJing 若把它当权限面用，要自己加校验。

## 5. 本次考证的环境阻碍

- `pi.bash` 在本会话全部失败：`Theme not initialized. Call initTheme() first.`（`pi-fabric-setup.js:23`）。任何命令都跑不了，因此本代理无法 `git clone`，也无法 `git commit` / `git push`。提交由主代理完成。
- 本会话没有注册联网工具：`tools.list()` 里没有 `extensions.web_search` / `extensions.fetch_content`，`extensions` 是空对象。全局 AGENTS.md 指定的联网检索通道不可用。
- 试过的替代通道，全部不可用：`pi.powershell`（未注册）、`tools.call({ref: 'pi.bash'})`（同一 theme 错误）、`mcp.idea.execute_tool`（IDEA 只注册 8 个工具，无终端类）、`components.apply` 启用 `fabric.provider.extensions`（被拒：`Live component changes cannot modify reserved built-in components`）。
- 前置条件改用等价读文件验证：`.git`（worktree 指针）指向 `LanJing/.git/worktrees/lanjing-wt-music-host-api`，故 toplevel 是 `C:/Users/wuhy/workspace/workspace-rust/lanjing-wt-music-host-api`。`LanJing/.git/config` 里 `remote "origin"` 的 `url = https://github.com/ocean-flux/LanJing.git`。worktree HEAD 是 `ref: refs/heads/research/music-source-host-api`。三项与票据要求一致。

## 6. 待办

- [ ] 移动端 `lx.version` 的实际值（需克隆 `lyswhut/lx-music-mobile`）。
- [ ] 桌面端脚本可用的宿主 API 正面清单（§2.4 只做了推断，未见官方明文）。
