# 影视来源形态: Maccms10 与 TVBox

票据 #80 (issue #65 架构与分层重划) 的源码调研笔记。
每条断言带来源。路径别名如下。

- `M/` = maccms10 仓库根 (`C:/Users/wuhy/workspace/_research/maccms10`)
- `T/` = TVBoxOS 仓库根 (`C:/tmp/pi-github-repos/runtime-OarkXR/da217d3583c08ffe5c0a440732c3c3550ef5f18fc3d2c9501394c06cc822bba2`)
- `F/` = FongMi/TV 仓库根 (`C:/tmp/pi-github-repos/runtime-OarkXR/8525896d336405c2b60f5372ee035ed6d7b296bae26110380b70563037fc2a49`)

只依据本地源码。查不到的写"未验证"。

## 1. Maccms10 的 api.php/provide/vod/ JSON API

### 1.1 入口与格式开关

- `api.php` 是 ThinkPHP 入口脚本 (`M/api.php:1-12`)。
- 路径 `api.php/provide/vod` 对应控制器 `app\api\controller\Provide` 的 `vod()` 方法 (`M/application/api/controller/Provide.php:2`, `:21`)。
- 同一动作双格式输出。参数 `at=xml` 时返回 XML, 否则返回 JSON (`M/application/api/controller/Provide.php:132-136`)。
- JSON 外层信封是 `code` / `msg` / `page` / `pagecount` / `limit` / `total` / `list` (`M/application/common/model/Vod.php:91`, `:137`)。
- XML 形状是 `<rss version="5.1">` + `<list page= pagecount= pagesize= recordcount=>` (`M/application/api/controller/Provide.php:269-273`)。
- 响应可缓存, 缓存键含全部查询参数 (`M/application/api/controller/Provide.php:40-43`)。

### 1.2 查询参数

| 参数 | 语义 | 来源 |
| --- | --- | --- |
| `ac` | `videolist` 或 `detail` 时返回全字段含播放串, 其它取值只返回列表基础字段 | `M/application/api/controller/Provide.php:125-128`, `:246` |
| `ids` | 按 `vod_id` 过滤, 逗号分隔 | `M/application/api/controller/Provide.php:45-46` |
| `t` | 分类 `type_id`, 受 `typefilter` 白名单限制 | `M/application/api/controller/Provide.php:52-54` |
| `pg` | 页码, 默认 1 | `M/application/api/controller/Provide.php:115-116` |
| `pagesize` | 每页条数, 上限 100 | `M/application/api/controller/Provide.php:118-120` |
| `wd` | 按 `vod_name` 模糊搜索 | `M/application/api/controller/Provide.php:70-71` |
| `h` | 最近 N 小时内更新 | `M/application/api/controller/Provide.php:61-63` |
| `year` | 年份, 逗号分隔多值 | `M/application/api/controller/Provide.php:74-94` |
| `from` | 只回指定播放组 | `M/application/api/controller/Provide.php:96-113` |
| `at` | `xml` 切 XML 输出 | `M/application/api/controller/Provide.php:132-136` |

排序固定为 `vod_time` 倒序, `sort_direction=asc` 可翻转 (`M/application/api/controller/Provide.php:123-124`)。

### 1.3 播放串分隔约定

- 线路组之间用 `$$$` 分隔。`vod_play_from` / `vod_play_url` / `vod_play_server` / `vod_play_note` 四个字段同构平行 (`M/application/api/controller/Provide.php:199-202`)。
- JSON 输出里 `vod_play_from` 转成逗号分隔, 其余字段保持 `$$$` (`M/application/api/controller/Provide.php:219-222`, `:240`)。
- 单条线路内部用 `#` 分集 (`M/application/common.php:2226`)。
- 每集格式是 `名称$播放地址`, 可选第三个 `$` 字段覆盖 from (`M/application/common.php:2230-2242`)。
- 存库时换行符统一转成 `#` (`M/application/common/model/Vod.php:814-815`)。
- XML 侧每个线路是 `<dd flag="线路名">播放串</dd>` (`M/application/api/controller/Provide.php:159-172`)。
- 详情抓取方式是 `ac=detail&ids=<vod_id>`, 播放量会 +1 (`M/application/api/controller/Provide.php:144-149`)。

## 2. TVBox 接口配置 JSON 的字段

解析入口是 `ApiConfig.parseJson` (`T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:743`)。

| 字段 | 语义 | 来源 |
| --- | --- | --- |
| `spider` | jar/dex 地址, 形如 `"./your.jar"`, 支持 `<url>;md5;<md5>` 校验 | `T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:746`, `T/README.md:14`, `T/app/src/main/java/com/github/catvod/crawler/JarLoader.java:157-159` |
| `sites` | 站点数组, 必填 `key` / `type` / `api` | `T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:753-759` |
| `parses` | 解析器数组, 字段 `name` / `url` / `ext` / `type` | `T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:798-808` |
| `lives` | 直播组数组, 参数含 `ua` / `epg` / `logo`, 另有 `playerType` 与独立 `spider` | `T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:829-830`, `:1083`, `:1397-1399`, `T/README.md:11` |
| `hosts` | 字符串数组, 形如 `"旧域名=新域名"`, 只按第一个 `=` 切 | `T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:857-866`, `T/README.md:17-20` |
| `doh` | DNS over HTTPS 服务器数组, 字段 `name` / `url` | `T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:935-936`, `T/README.md:23-37` |
| `rules` | 三类规则: `host`+`rule`/`filter` 嗅探规则, `hosts`+`regex` 广告过滤, `hosts`+`script` 嗅探脚本 | `T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:871-941` |
| `flags` | 需要进 vip 解析的线路旗标列表 | `T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:795` |
| `wallpaper` | 壁纸 URL, 下载后作 app 背景 | `T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:749-750`, `T/app/src/main/java/com/github/tvbox/osc/ui/fragment/ModelSettingFragment.java:218-219` |

附加字段: `jarCache` 与 `danmaku` (`T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:747-748`)。

`sites[]` 单站字段全集: `key` `name` `type` `api` `searchable` `quickSearch` `changeable` `filterable` `playUrl` `ext` `jar` `playerType` `categories` `timeout` `click` `style` (`T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:755-780`)。
站级 `jar` 可覆盖全局 `spider` (`T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:775`)。
`py_` 前缀的 key 强制 `filterable=1` (`T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:767-771`)。

## 3. sites[].type 的语义 (按源码定)

两个 fork 的源码一致, 与社区传闻不一致。

| type | TVBoxOS 源码行为 | FongMi/TV 源码行为 |
| --- | --- | --- |
| 0 | XML 采集协议。HTTP GET `api`, 传 `ac=videolist`, 响应按 XML 解析 | 同左, `Result.fromType` 走 `fromXml` |
| 1 | JSON 采集协议。HTTP GET `api`, 传 `ac=detail` + 筛选 `f`, 响应按 JSON 解析 | 同左, 走 `fromJson` |
| 3 | spider。经 `ApiConfig.getCSP` 取 jar/dex、js 或 python 爬虫实例 | 同左, `isSpider(site) = type == 3` |
| 4 | 远端 HTTP JSON 接口。GET `api` 加 `filter=true` 与 `extend=<ext>` | 同左, `ext` 为 base64(URL_SAFE) 传 `ext` 参数 |

来源:

- TVBoxOS 0/1 分派: `T/app/src/main/java/com/github/tvbox/osc/viewmodel/SourceViewModel.java:291`, `:307-311`, `:485` (`ac` 取值 0→`videolist`, 其它→`detail`)。
- TVBoxOS 3: `T/app/src/main/java/com/github/tvbox/osc/viewmodel/SourceViewModel.java:229-237` 调 `ApiConfig.get().getCSP(sourceBean)`。
- TVBoxOS 4: `T/app/src/main/java/com/github/tvbox/osc/viewmodel/SourceViewModel.java:341-351`。
- FongMi: `F/app/src/main/java/com/fongmi/android/tv/api/SiteApi.java:47-52` (`isSpider`, `ac()`), `:70-77`, `:100-102`, `:129-131` (`fromType: 0 ? fromXml : fromJson`, 定义在 `F/app/src/main/java/com/fongmi/android/tv/bean/Result.java:129-131`)。

结论三条。

1. TVBoxOS `README.md` 的 `0:xml 1:json 3:jar 4:remote` (`T/README.md:9`) 与代码一致, 但 3 不止 jar, 也含 js 与 python (见第 4 节)。
2. 社区说法 `0:文艺 1:豆瓣 3:采集 4:spider` 与两个仓库的源码都相反。0/1 才是采集协议, 3 才是 spider。
3. 0 与 1 的差别只是响应格式 (XML rss vs JSON), 查询参数族相同, 都是苹果 CMS 采集协议。

## 4. 三种 spider 机制

派发规则在 `ApiConfig.getCSP`: `api` 以 `.js` 结尾走 JsLoader, 含 `.py` 走 pyLoader, 其它走 JarLoader (`T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:1456-1469`)。

### 4.1 jar / dex

- 配置形如 `"spider": "./your.jar"`, 下载到本地缓存, 支持 `;md5;` 校验 (`T/app/src/main/java/com/github/catvod/crawler/JarLoader.java:157-159`)。
- 用 `DexClassLoader` 加载, 站点类全名固定为 `com.github.catvod.spider.<api 去掉 csp_ 前缀>` (`T/app/src/main/java/com/github/catvod/crawler/JarLoader.java:217-221`, `:411-413`)。
- jar 可带三个约定入口类: `Init`、`Proxy`、`Danmaku` (`T/app/src/main/java/com/github/catvod/crawler/JarLoader.java:109-111`, `:126-128`, `:136-138`)。
- `ProtectedInitJar` 负责检查 jar 内嵌 dex、反射注入 Context 与 DexClassLoader (`T/app/src/main/java/com/github/catvod/crawler/ProtectedInitJar.java:33-56`)。
- 能力面 = 全量 Spider 接口, 含本地代理与弹幕 (`T/app/src/main/java/com/github/catvod/crawler/JarLoader.java:126-138`)。
- 可移植性 = 无。强依赖 Android dalvik 的 `DexClassLoader` 与固定 Java 类名 ABI。

### 4.2 js (QuickJS)

- `api` 以 `.js` 结尾进 JsLoader, 实例是 `JsSpider`, 引擎为 QuickJS (`T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:1456-1460`, `T/app/src/main/java/com/github/catvod/crawler/js/JsSpider.java:41`)。
- 脚本两种写法都收: 导出 `__jsEvalReturn()` (cat 系) 或导出 `default` (对象或工厂函数) (`T/app/src/main/java/com/github/catvod/crawler/js/JsSpider.java:298-306`)。
- Spider 接口逐个映射到 JS 函数: `home` `homeVod` `category` `detail` `search` `play` `live` `isVideo` `action` `proxy` (`T/app/src/main/java/com/github/catvod/crawler/js/JsSpider.java:178`, `:187`, `:197`, `:206`, `:215`, `:226`, `:235`, `:253`, `:273`, `:540`)。
- 也支持预编译 QuickJS 字节码模块, 前缀 `//DRPY` 与 `//bb`, 字节码版本 67 (`T/app/src/main/java/com/github/catvod/crawler/js/JsSpider.java:44-45`, `:368-387`)。
- 站点 `jar` 字段可附带 jsapi dex, 里面放 `com.github.catvod.js.Function` 或 `Method` 类扩展 JS 全局函数 (`T/app/src/main/java/com/github/catvod/crawler/JsLoader.java:56-88`, `:139-153`)。
- 可移植性 = 部分。QuickJS 可嵌入, 但 cat.js 等内置资产、字节码模块与 jsapi dex 绑定 TVBoxOS 的资产布局。

### 4.3 python

- 接口是 `IPyLoader`: `clear` / `setConfig` / `setRecentPyKey` / `getSpider` / `proxyInvoke` (`T/app/src/main/java/com/github/catvod/crawler/python/IPyLoader.java:7-15`)。
- TVBoxOS 按构建 flavor 分两套。normal flavor 是空桩, 日志写明 "Python is not supported" (`T/app/src/normal/java/com/github/catvod/crawler/pyLoader.java:9-41`)。
- python flavor 委托 `com.undcover.freedom.pyramid.PythonLoader` / `PythonSpider` (`T/app/src/python/java/com/github/catvod/crawler/pyLoader.java:10-71`)。
- 已知限制: Android 16+ 的 32 位进程禁用 python (`T/app/src/python/java/com/github/catvod/crawler/pyLoader.java:35-40`)。
- 直播配置的 `api` 也可以是 `.js` / `.py` spider (`T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:1349-1370`)。
- 可移植性 = 无。pyramid 绑定 Android Python 运行时。

## 5. JS spider 的 API 面 (cat.js 与宿主注入)

### 5.1 cat.js 本体

- 位置是 `T/app/src/main/assets/js/lib/cat.js`, 单行 475KB 压缩产物。
- 模块解析按名字别名映射: 含 `cat.js` 就落到资产 `cat.js` (`T/app/src/main/java/com/github/tvbox/osc/util/FileUtils.java:286-289`)。
- 同目录资产还有 `cheerio.min.js` `crypto-js.js` `gbk.js` `net.js` `similarity.js` `utils.js` `模板.js` (目录清单)。
- grep 证据 (存在性): `cheerio` `req` `local` `aes` `rsa` `jinja` `load` 命中。即 cheerio 解析 + aes/rsa + jinja 模板的打包库。
- grep 证据 (不存在): `pdfa` `js2Proxy` `md5` `base64` `gzip` `pako` `JSON5` `gbk` `xpath`。
- 模块形态是 ESM named export (`export{...}` 命中), 无 `export default` 与 `module.exports`。
- 精确导出名单未验证。文件是单行压缩, 无 shell 条件下 grep 只能证明标识符存在。

### 5.2 宿主注入面

- 全局函数由 `Global` 绑定: `getProxy` `js2Proxy` `joinUrl` `pd` `pdfh` `pdfa` `pdfla` `s2t` `t2s` `aesX` `rsaX` `rsaEncrypt` `rsaDecrypt` `_http` `setTimeout` (`T/app/src/main/java/com/github/catvod/crawler/js/Global.java:41-327`, 绑定于 `T/app/src/main/java/com/github/catvod/crawler/js/JsSpider.java:415`)。
- 全局对象 `local` 提供本地 KV: `get` / `set` / `delete` (`T/app/src/main/java/com/github/catvod/crawler/js/local.java:8-23`, 绑定于 `T/app/src/main/java/com/github/catvod/crawler/js/JsSpider.java:418-420`)。
- 资产 `net.js` 在启动时求值, 定义 `req` / `http` 包装注入的 `_http`, 支持同步与 Promise (`T/app/src/main/assets/js/lib/net.js:1-17`, 求值点 `T/app/src/main/java/com/github/catvod/crawler/js/JsSpider.java:421-422`)。
- 模板资产 `模板.js` 被预载为 `globalThis.muban` 与 `getMubans` (`T/app/src/main/java/com/github/catvod/crawler/js/JsSpider.java:469-472`), 内容是 drpy 风格规则模板 (`T/app/src/main/assets/js/lib/模板.js:188`)。
- 网络模块地址支持 http(s) 拉取 (缓存 7 天)、`assets://` 与 `js/lib` 内置三类 (`T/app/src/main/java/com/github/tvbox/osc/util/FileUtils.java:296-309`)。

### 5.3 FongMi 侧同型

- 同一套 `__jsEvalReturn` / `default` 双协议 (`F/quickjs/src/main/assets/js/lib/spider.js:1-10`)。
- 注入 `http.js` 与 `local`, 模块解析 `lib/` → 资产 `js/` (`F/quickjs/src/main/java/com/fongmi/quickjs/crawler/Spider.java:162-171`, `F/quickjs/src/main/java/com/fongmi/quickjs/utils/Module.java:26-27`)。
- `Global` 同样提供 `js2Proxy` 等 (`F/quickjs/src/main/java/com/fongmi/quickjs/method/Global.java:112`)。

## 6. parses 解析层与 playerType

- `parses[]` 字段是 `name` / `url` / `ext` / `type` (`T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:798-808`)。
- type 语义: 0 是嗅探, 用自带播放器在 webview 里抓真实地址。1 是 json 解析端点, 返回直链 (`T/README.md:10`)。
- 进解析的决策在播放结果处理: `parse` 或 `jx` 为真才进解析, 且当 `playUrl` 为空且线路旗标命中 `flags` 列表时强制走解析器列表 (`T/app/src/main/java/com/github/tvbox/osc/ui/fragment/PlayFragment.java:1258-1263`)。
- `playUrl` 前缀分流: `json:` 建 type 1 解析器, `parse:` 按名字重定向到 `parses` 里某项, 其它建 type 0 嗅探 (`T/app/src/main/java/com/github/tvbox/osc/ui/fragment/PlayFragment.java:2071-2095`)。
- json 解析端点的响应形如 `{url, parse, header, user-agent, referer}`, `url` 前缀 `video://` 表示还要再嗅探 (`T/app/src/main/java/com/github/tvbox/osc/ui/fragment/PlayFragment.java:2097-2136`)。
- `playerType` 是播放内核选择: 0 系统播放器, 1 IJK, 2 EXO (`T/README.md:8`)。站点可覆盖, 默认 -1 跟随全局 (`T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:776`)。直播另有独立 `playerType` (`T/app/src/main/java/com/github/tvbox/osc/api/ApiConfig.java:1397-1399`)。
- 判断: 解析层不属于来源层。它是播放解析与播放内核选择, 挂在播放流程上, 与"来源能给出什么媒体"正交。

## 7. FongMi/TV 与 TVBoxOS 的差异

| 维度 | TVBoxOS | FongMi/TV |
| --- | --- | --- |
| 工程结构 | 单 `app` module, `com.github.catvod` 包内联 | 多 module: `app` + `catvod` + `quickjs` + `chaquo` |
| Spider 基类 | 具体类, 空实现, 无异常声明 (`T/app/src/main/java/com/github/catvod/crawler/Spider.java:16-135`) | 抽象类, 方法 `throws Exception` (`F/catvod/src/main/java/com/github/catvod/crawler/Spider.java:16-77`) |
| loader 位置 | `com.github.catvod.crawler.{JarLoader,JsLoader,pyLoader}` | `com.fongmi.android.tv.api.loader.{JarLoader,JsLoader,PyLoader}` |
| 闭源 spider | 无 | `app/libs/*.aar`: forcetech / hook / jianpian / thunder / tvbus (二进制, 内容未验证) |
| python | pyramid (PythonLoader), 分 normal / python flavor | chaquo module (Chaquopy) |
| 结果模型 | 内部 Xml/Json 解析到 `Movie` 模型 | 显式 bean: Result / Vod / Flag / Episode / Class, Gson `@SerializedName` + SimpleXML 双注解 (`F/app/src/main/java/com/fongmi/android/tv/bean/Vod.java:65-67`) |
| 文档 | README 里的 JSON 示例 (`T/README.md:1-40`) | 自带文档站源码 `F/website/app/spider-fields.ts` (方法表 `:3-25`, 结果字段表 `:27-140`) |
| 协议兼容 | spider 结果 JSON 用 `vod_play_from` / `vod_play_url` | 同左, 两 fork 的消费模型一致 |

## 8. 影视源的消费单元

- 线路 = `vod_play_from` 里的一个组, 对应 maccms 的播放组 / 播放器, 用 `$$$` 分组 (`M/application/api/controller/Provide.php:199-202`)。FongMi 把它落成 `Flag` 对象, 字段 `flag` + `episodes` (`F/app/src/main/java/com/fongmi/android/tv/bean/Flag.java:23-34`)。
- 集 = 线路内一条 `名称$播放地址`, 用 `#` 分隔 (`M/application/common.php:2226-2230`)。FongMi 落成 `Episode`, 字段 `name` / `desc` / `url`, 并从名字解析集号 (`F/app/src/main/java/com/fongmi/android/tv/bean/Episode.java:17-31`)。
- 分隔符约定在 FongMi 文档站有总结: `$$$` 分组, `#` 分集, `$` 分名称与 id (`F/website/app/spider/page.tsx:431-448`)。
- 播放地址 = 选中一集后 `playerContent` / `play` 的输出, 核心是 `url` 加 `header` (`F/website/app/spider-fields.ts:52-60`)。可带 `playUrl` 前缀交给解析层 (`T/app/src/main/java/com/github/tvbox/osc/ui/fragment/PlayFragment.java:1258-1263`)。
- 季 = 没有一等数据结构。maccms 的 vod 表与 API 无 season 字段 (grep `M/application` 无匹配)。TVBoxOS 只在搜索词处理里识别"季"后缀并猜下一季标题 (`T/app/src/main/java/com/github/tvbox/osc/util/SearchHelper.java:85-112`)。季实际是剧集名字符串约定, 或者干脆一条线路一部季。
- 文件夹形态: `vod_tag=folder` 或 `cate` 字段表示资料夹, 分类 `type_flag=1` 表示资料夹分类 (`F/website/app/spider-fields.ts:176-178`, `:186-192`)。

## 9. 结论: LanJing 能覆盖哪一层, 哪一层必须降级

按第 1 至 8 节的分层逐条判断。

| 层 | 判断 | 依据 |
| --- | --- | --- |
| 来源数据协议层 (maccms provide/vod JSON 与 XML, TVBox spider Result JSON) | 覆盖 | 分类 / 卡片 / 详情 / 线路 / 集都能映射进标准媒体模型。maccms 侧字段见 1.2 与 1.3, TVBox 侧字段见 `F/website/app/spider-fields.ts:153-178`。该层只有数据形状, 不含代码执行 |
| 来源描述层 (`sites[]` 的 key/type/api/ext/jar/header) | 部分覆盖 | 静态描述可映射为来源定义。`type` 语义按第 3 节的源码定义写死, 不采用社区传闻。jar/dex 与 python 两类 `api` 无法执行, 降级为"仅登记, 不运行" |
| spider 执行层 (jar/dex) | 降级 | `DexClassLoader` 与 `com.github.catvod.spider.*` 类名 ABI 是 dalvik 专属 (`T/app/src/main/java/com/github/catvod/crawler/JarLoader.java:217-221`)。Rust 侧不打包 ART/JVM |
| spider 执行层 (js) | 有条件覆盖 | QuickJS 可嵌入, Global / local / req 注入面可映射为规则沙箱能力 (`T/app/src/main/java/com/github/catvod/crawler/js/Global.java:41-327`)。但 cat.js 等资产、`//bb` 字节码模块与 jsapi dex 绑定 TVBoxOS 布局, 只能承诺子集, 标"尽力执行" |
| spider 执行层 (python) | 降级 | pyramid 与 chaquo 都是 Android 绑定 (`T/app/src/python/java/com/github/catvod/crawler/pyLoader.java:10-71`) |
| 解析层 (`parses` / `playerType` / `flags`) | 不覆盖 | 不属于来源层, 见第 6 节判断。只在 PresentationHint 里保留"此线路需解析 / 可直链"的提示位 |
| 消费单元 (线路 / 集 / 播放地址) | 覆盖 | 三者都能无损映射进标准媒体模型, 分隔符解析照抄 1.3 与第 8 节 |
| 季 | 降级 | 源码无一等结构 (第 8 节)。保持剧集名字符串, 不建 Season 实体 |
| app 配置 (`wallpaper` / `doh` / `hosts` / `rules` / `lives`) | 不覆盖 | 播放器端基础设施, 与来源能力无关 (第 2 节)。导入来源配置时忽略 |

## 10. 未验证项

1. cat.js 的精确导出名单。单行压缩, 只有标识符存在性证据。
2. `sites[].type` 是否存在 0/1/3/4 以外的取值。两仓派发代码只出现这四个。
3. 社区说法 `0:文艺 1:豆瓣 3:采集 4:spider` 的出处。本地源码无此语义。
4. `app/libs/*.aar` 内容。二进制未读。
5. `rules[].hosts+script` 的脚本语义。只读到注册进 `VideoParseRuler`, 未读执行端。
6. maccms `ac` 的其它取值。源码只区分 `videolist` / `detail` 与其它。

## 11. 假设清单

1. 假设 `M/` 检出对应 magicblack/maccms10 主线, 与线上 `api.php/provide/vod` 行为一致。
2. 假设 `T/` 检出是 TVBoxOS 主线, `F/` 检出是 FongMi/TV 主线。
3. 假设 LanJing 的规则系统输入是"来源描述 + 规则", 不含任意代码执行。
4. 假设"消费单元"按 issue #65 的标准媒体模型讨论, PresentationHint 只带表现提示。
