# Legado 内置 JS 函数清单（源码考证版）

> 来源：`.tmp/legado` 仓库（gedoor/legado），Rhino v1.8.0 引擎。
> 本文档逐条从 Rust 落地视角考证 legado 注入到 JS 运行时的变量、对象、函数，作为我们自研宿主 API（见 `docs/plans/proposal-own-js-host-api-namespace.md`）的输入与对齐基准。
> 仓库内同步参考 legado 自带帮助：`app/src/main/assets/web/help/md/jsHelp.md`（注意官方文档多处标注「部分函数」，本清单以源码为准，补全全部）。

## 0. 注入机制总览

legado 在 `AnalyzeRule.evalJS`（`app/src/main/java/io/legado/app/model/analyzeRule/AnalyzeRule.kt:774`）中通过 `buildScriptBindings` 向 JS 作用域注入以下顶层变量：

| 变量名           | 绑定来源                                                            | 可调方法集                                                                                                                                         |
| ---------------- | ------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `java`           | `this`（`AnalyzeRule` 实例，实现 `JsExtensions` + `JsEncodeUtils`） | JsExtensions + JsEncodeUtils + AnalyRule 公有方法                                                                                                  |
| `cookie`         | `CookieStore`（单例）                                               | getCookie / getKey / setCookie / replaceCookie / removeCookie                                                                                      |
| `cache`          | `CacheManager`（单例）                                              | get / put / delete / putFile / getFile / putMemory / getFromMemory / deleteMemory（及类型化 get）                                                  |
| `source`         | 当前 `BaseSource`                                                   | getKey / getVariable / setVariable / getLoginHeader(Map) / putLoginHeader / removeLoginHeader / getLoginInfo(Map) / removeLoginInfo / getHeaderMap |
| `book`           | 当前 `Book` 实体                                                    | 全部公有属性（见 §9）                                                                                                                              |
| `chapter`        | 当前 `BookChapter`                                                  | 全部公有属性（见 §10）                                                                                                                             |
| `rssArticle`     | 当前 `RssArticle`                                                   | 全部公有属性（订阅源正文规则）                                                                                                                     |
| `result`         | 上一步解析结果                                                      | 读写，JS 内可覆盖回传                                                                                                                              |
| `baseUrl`        | 当前页 URL（String）                                                | 只读                                                                                                                                               |
| `title`          | `chapter?.title`                                                    | 只读                                                                                                                                               |
| `src`            | 当前 `content`（请求返回源码）                                      | 只读                                                                                                                                               |
| `nextChapterUrl` | 下一章 URL                                                          | 只读                                                                                                                                               |

**关键警示**：legado 已把顶层 `java` 变量重定向为自家宿主对象（见 `jsHelp.md:12`「注意 `java` 变量指向已经被阅读修改，如果想要调用 `java.*` 下的包，请使用 `Packages.java.*`」）。我们自研运行时（rquickjs）**不存在这个历史包袱**，因此命名空间可彻底脱离 `java.` 前缀（见提案文档）。

Rhino 自带顶层（与 `java` 无关，由 Rhino 的 `ImporterTopLevel` 提供，受 `RhinoClassShutter` 沙箱过滤）：

| 构造函数 / 对象                      | 函数                            | 简述                               |
| ------------------------------------ | ------------------------------- | ---------------------------------- |
| `JavaImporter`                       | `importClass` / `importPackage` | 导入 Java 类到 JS                  |
| `Packages`、`java`（原生）、`javax`… | `getClass`                      | 默认 Java 包反射入口（被沙箱收敛） |
| `JavaAdapter`                        | —                               | 运行时继承 Java 类                 |

> 上述 Rhino 原生 Java 互操作在 rquickjs 下不可用，书源里的 `new JavaAdapter(...)` / `importClass(...)` 类用法属 **接入我们不支持的兼容面**，由导入器降级标注。

## 1. `java.*` 网络请求（JsExtensions #网络）

源：`app/src/main/java/io/legado/app/help/JsExtensions.kt`。返回 `StrResponse` 的方法暴露 `body() code() message() headers() raw() toString()`。

| 签名                                                                                   | 说明                          | 触发能力 |
| -------------------------------------------------------------------------------------- | ----------------------------- | -------- |
| `ajax(url: Any): String?`                                                              | GET 取正文，自动解析 url 规则 | Network  |
| `ajaxAll(urlList: Array<String>): Array<StrResponse>`                                  | 批量并发 GET                  | Network  |
| `connect(urlStr: String): StrResponse`                                                 | 简易连接，返回响应对象        | Network  |
| `connect(urlStr: String, header: String?): StrResponse`                                | 同上，带请求头 JSON           | Network  |
| `get(urlStr: String, headers: Map<String,String>): Connection.Response`                | jsoup GET                     | Network  |
| `head(urlStr: String, headers: Map<String,String>): Connection.Response`               | jsoup HEAD                    | Network  |
| `post(urlStr: String, body: String, headers: Map<String,String>): Connection.Response` | jsoup POST                    | Network  |

## 2. `java.*` WebView / 浏览器（JsExtensions #webview）

| 签名                                                                                                 | 说明                                       |
| ---------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| `webView(html: String?, url: String?, js: String?): String?`                                         | webView 载入 html/访问 url，用 js 取返回值 |
| `webViewGetSource(html: String?, url: String?, js: String?, sourceRegex: String): String?`           | webView 取资源 url                         |
| `webViewGetOverrideUrl(html: String?, url: String?, js: String?, overrideUrlRegex: String): String?` | webView 取跳转 url                         |
| `startBrowser(url: String, title: String)`                                                           | 内置浏览器打开链接                         |
| `startBrowserAwait(url: String, title: String, refetchAfterSuccess: Boolean): StrResponse`           | 内置浏览器打开并等待结果                   |
| `startBrowserAwait(url: String, title: String): StrResponse`                                         | 上一项的 `refetchAfterSuccess=true` 重载   |
| `getVerificationCode(imageUrl: String): String`                                                      | 阻塞获取用户手输验证码                     |

> WebView/浏览器类能力依赖系统 WebView 渲染 Gerente，本地优先、无系统组件可用的桌面单机环境下属 **可选能力**，提案将以独立 capability 门控。

## 3. `java.*` 编码 / 字节（JsExtensions #编码, JsEncodeUtils #摘要/签名）

| 签名                                                            | 说明                              |
| --------------------------------------------------------------- | --------------------------------- |
| `encodeURI(str: String): String`                                | URI 编码，默认 UTF-8              |
| `encodeURI(str: String, enc: String): String`                   | 指定字符集的 URI 编码             |
| `base64Decode(str: String?): String`                            | Base64 解码为字符串（默认 UTF-8） |
| `base64Decode(str: String?, charset: String): String`           | 指定字符集 Base64 解码            |
| `base64Decode(str: String, flags: Int): String`                 | Base64 解码带 flags               |
| `base64DecodeToByteArray(str: String?): ByteArray?`             | Base64 -> ByteArray               |
| `base64DecodeToByteArray(str: String?, flags: Int): ByteArray?` | 带 flags                          |
| `base64Encode(str: String): String?`                            | 字符串 -> Base64（默认 NO_WRAP）  |
| `base64Encode(str: String, flags: Int): String?`                | 带 flags                          |
| `hexDecodeToByteArray(hex: String): ByteArray?`                 | Hex -> ByteArray                  |
| `hexDecodeToString(hex: String): String?`                       | Hex -> UTF-8 字符串               |
| `hexEncodeToString(utf8: String): String?`                      | UTF-8 -> Hex 字符串               |
| `strToBytes(str: String): ByteArray`                            | String -> ByteArray（默认字符集） |
| `strToBytes(str: String, charset: String): ByteArray`           | 指定字符集                        |
| `bytesToStr(bytes: ByteArray): String`                          | ByteArray -> String（默认字符集） |
| `bytesToStr(bytes: ByteArray, charset: String): String`         | 指定字符集                        |

## 4. `java.*` 加解密 / 摘要 / 签名（JsEncodeUtils）

源：`app/src/main/java/io/legado/app/help/JsEncodeUtils.kt`，由 hutool-crypto 5.8.22 实现。

### 4.1 对称加密（Cipher 工厂 + 方法）

| 签名                                                                                                     | 说明                                                   |
| -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| `createSymmetricCrypto(transformation, key: ByteArray?, iv: ByteArray?)`                                 | 创建对称 Cipher（key/iv 为 ByteArray）                 |
| `createSymmetricCrypto(transformation, key: ByteArray)`                                                  | 重载                                                   |
| `createSymmetricCrypto(transformation, key: String)`                                                     | key 字符串重载                                         |
| `createSymmetricCrypto(transformation, key: String, iv: String?)`                                        | 全字符串重载                                           |
| Cipher 方法：`decrypt(data)` `decryptStr(data)` `encrypt(data)` `encryptBase64(data)` `encryptHex(data)` | data 支持 ByteArray/Base64String/HexString/InputStream |

### 4.2 非对称加密

| 签名                                                                                                             | 说明                                      |
| ---------------------------------------------------------------------------------------------------------------- | ----------------------------------------- |
| `createAsymmetricCrypto(transformation)`                                                                         | 创建非对称 Cipher                         |
| `.setPublicKey(key)` `.setPrivateKey(key)`                                                                       | 设置密钥（key 支持 ByteArray/Utf8String） |
| 方法：`decrypt(data, usePublicKey=true)` `decryptStr(...)` `encrypt(...)` `encryptBase64(...)` `encryptHex(...)` |                                           |

### 4.3 签名

| 签名                                       | 说明                                   |
| ------------------------------------------ | -------------------------------------- |
| `createSign(algorithm)`                    | 创建 Sign                              |
| `.setPublicKey(key)` `.setPrivateKey(key)` |                                        |
| `sign.sign(data)` `sign.signHex(data)`     | data 支持 ByteArray/InputStream/String |

### 4.4 摘要 / MD5 / HMAC

| 签名                                                               | 说明             |
| ------------------------------------------------------------------ | ---------------- |
| `digestHex(data: String, algorithm: String): String?`              | 摘要 Hex 输出    |
| `digestBase64Str(data: String, algorithm: String): String?`        | 摘要 Base64 输出 |
| `md5Encode(str): String`                                           | MD5 32 位        |
| `md5Encode16(str): String`                                         | MD5 16 位        |
| `HMacHex(data: String, algorithm: String, key: String): String`    | HMAC Hex         |
| `HMacBase64(data: String, algorithm: String, key: String): String` | HMAC Base64      |

### 4.5 AES / DES / 3DES 便捷函数（逐参数模式＋padding＋iv）

| 签名                                                                                                                                                                                                       | 说明   |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ |
| `aesDecodeToByteArray(str, key, transformation, iv)` `aesDecodeToString(...)` `aesBase64DecodeToByteArray(...)` `aesBase64DecodeToString(...)`                                                             | AES 解 |
| `aesEncodeToByteArray(...)` `aesEncodeToString(...)` `aesEncodeToBase64ByteArray(...)` `aesEncodeToBase64String(...)` `aesEncodeArgsBase64Str(data, key, mode, padding, iv)` `aesDecodeArgsBase64Str(...)` | AES 加 |
| `desDecodeToString(...)` `desBase64DecodeToString(...)` `desEncodeToString(...)` `desEncodeToBase64String(...)`                                                                                            | DES    |
| `tripleDESDecodeStr(data, key, mode, padding, iv)` `tripleDESDecodeArgsBase64Str(...)` `tripleDESEncodeBase64Str(...)` `tripleDESEncodeArgsBase64Str(...)`                                                 | 3DES   |

## 5. `java.*` 时间 / 文本 / 标识（JsExtensions #杂项）

| 签名                                                          | 说明                                                      |
| ------------------------------------------------------------- | --------------------------------------------------------- |
| `timeFormatUTC(time: Long, format: String, sh: Int): String?` | 按时区偏移 sh、format 格式化时间                          |
| `timeFormat(time: Long): String`                              | 默认格式化时间                                            |
| `htmlFormat(str: String): String`                             | HTML 净化（去标签/转义处理）                              |
| `t2s(text: String): String`                                   | 繁体转简体（ChineseUtils）                                |
| `s2t(text: String): String`                                   | 简体转繁体                                                |
| `getWebViewUA(): String`                                      | 返回系统 WebView User-Agent                               |
| `randomUUID(): String`                                        | UUID                                                      |
| `androidId(): String`                                         | 设备 android_id（首刀桌面无对应，提案以稳定机器标识替代） |
| `toURL(urlStr: String): JsURL`                                | 字符串包装成 JsURL                                        |
| `toURL(url: String, baseUrl: String?): JsURL`                 | 相对 url + baseUrl 解析                                   |
| `toNumChapter(s: String?): String?`                           | 文本章节号归一（阿拉伯数字）                              |

## 6. `java.*` 文件 / 缓存 / 压缩（JsExtensions #文件，相对 cache 目录）

> 所有路径相对阅读缓存目录「/android/data/{pkg}/cache/…」，仅缓存域内可读写删。

| 签名                                                                                                             | 说明                                                  |
| ---------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| `downloadFile(url: String): String`                                                                              | 下载到缓存，返回路径                                  |
| `downloadFile(content: String, url: String): String`                                                             | 用 content 生成缓存文件                               |
| `unArchiveFile(zipPath: String): String`                                                                         | 自动识别格式解压，返回目录                            |
| `unzipFile(zipPath: String): String` / `unrarFile(...)` / `un7zFile(...)`                                        | 指定格式解压                                          |
| `getTxtInFolder(path: String): String`                                                                           | 递归读文件夹内全部 txt                                |
| `readTxtFile(path: String): String` / `readTxtFile(path, charsetName): String`                                   | 读文本文件                                            |
| `readFile(path: String): ByteArray?`                                                                             | 读二进制文件                                          |
| `getFile(path: String): File`                                                                                    | 取文件句柄                                            |
| `deleteFile(path: String): Boolean`                                                                              | 删除文件                                              |
| `getZipStringContent(url, path[, charset]): String` `getRarStringContent(...)` `get7zStringContent(...)`         | 远端压缩包内指定路径取文本                            |
| `getZipByteArrayContent(url, path): ByteArray?` `getRarByteArrayContent(...)` `get7zByteArrayContent(...)`       | 远端压缩包内取字节                                    |
| `importScript(pathOrUrl: String): String`                                                                        | 取本地/远端 JS 文件内容（需手动 `eval(String(...))`） |
| `cacheFile(urlStr: String): String` / `cacheFile(urlStr, saveTime): String`                                      | 缓存网络文件                                          |
| `queryBase64TTF(data: String?): QueryTTF?` / `queryTTF(data, useCache): QueryTTF?` / `queryTTF(data): QueryTTF?` | 字体反查（TTF 字形映射）                              |
| `replaceFont(text, errorTTF, correctTTF, filter): String` / `replaceFont(text, errorTTF, correctTTF): String`    | 用 QueryTTF 反替换正文乱码字体                        |

## 7. `java.*` 调试 / 提示 / 跳转（JsExtensions #交互）

| 签名                                                              | 说明                                                          |
| ----------------------------------------------------------------- | ------------------------------------------------------------- |
| `log(msg: Any?): Any?`                                            | 写调试日志，原样返回 msg                                      |
| `logType(any: Any?)`                                              | 打印变量类型                                                  |
| `toast(msg: Any?)` / `longToast(msg: Any?)`                       | 短/长吐司                                                     |
| `openUrl(url: String)` / `openUrl(url, mimeType: String?)`        | 跳转外部链接/应用                                             |
| `getCookie(tag: String): String` / `getCookie(tag, key?): String` | 取 cookie（JsExtensions 上的便捷别名，与 `cookie.` 对象重复） |

## 8. `java.*` AnalyzeRule 规则解析（仅书源规则上下文，`java.` 直调）

源：`AnalyzeRule.kt`。规则解析是 legado 最核心的 `java.*` 能力。

| 签名                                                                             | 说明                                      |
| -------------------------------------------------------------------------------- | ----------------------------------------- |
| `getString(ruleStr: String?, mContent: Any? = null, isUrl = false): String`      | 用规则取单一文本                          |
| `getString(ruleStr: String?, unescape: Boolean): String`                         | 取文本并反转义                            |
| `getStringList(ruleStr: String?, mContent = null, isUrl = false): List<String>?` | 用规则取文本列表                          |
| `getElement(ruleStr: String): Any?`                                              | 取单个 Element                            |
| `getElements(ruleStr: String): List<Any>`                                        | 取 Element 列表（循环正文入口）           |
| `setContent(content: Any?, baseUrl: String? = null): AnalyzeRule`                | 设置待解析源码                            |
| `setBaseUrl(baseUrl: String?): AnalyzeRule`                                      | 设置 baseUrl                              |
| `put(key: String, value: String): String`                                        | 写变量（书源变量，与 `source.put` 路由）  |
| `get(key: String): String`                                                       | 读变量                                    |
| `reGetBook()`                                                                    | 重新搜索书籍/重取目录 url（仅刷新目录前） |
| `refreshTocUrl()`                                                                | 重新获取目录 url                          |
| `splitSourceRule(ruleStr: String?, allInOne = false): List<SourceRule>`          | 规则切分（内部辅助）                      |

## 9. AnalyzeUrl 函数（仅「登录检查 JS」规则内有效）

源：`AnalyzeUrl.kt`。该作用域内 `java` 指向 `AnalyzeUrl` 实例。

| 签名                                                                                                                                                                                                       | 说明                                                                 |
| ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `initUrl()`                                                                                                                                                                                                | 重新解析 url（登录后重访）                                           |
| `getHeaderMap(): Map<*, *>?`                                                                                                                                                                               | 取/设置请求头（配合 `putAll(source.getHeaderMap(true))` 重设登录头） |
| `getStrResponse(jsStr: String? = null, sourceRegex: String? = null)`                                                                                                                                       | 返回访问结果文本                                                     |
| `getResponse(): Response`                                                                                                                                                                                  | 返回访问结果对象（网络朗读引擎用）                                   |
| `getByteArray(): ByteArray` / `getInputStream(): InputStream`                                                                                                                                              | 取字节/流                                                            |
| `getUserAgent(): String` / `isPost(): Boolean`                                                                                                                                                             | UA / 方法查询                                                        |
| `put(key, value)` / `get(key)`                                                                                                                                                                             | 变量读写                                                             |
| setter/getter：`setMethod/getMethod` `setCharset/getCharset` `setOrigin/getOrigin` `setRetry/getRetry` `setType/getType` `setHeaders/getHeaderMap` `setBody/getBody` `setWebJs/getWebJs` `useWebView(...)` | 运行时改写 url 配置                                                  |

## 10. RssJsExtensions（仅订阅源 `shouldOverrideUrlLoading` 规则）

源：`RssJsExtensions.kt`，实现 `JsExtensions`，额外两个方法。

| 签名                       | 说明                       |
| -------------------------- | -------------------------- |
| `searchBook(key: String)`  | 触发阅读内搜索（变量 url） |
| `addBook(bookUrl: String)` | 加入书架                   |

## 11. `cookie` 对象（CookieStore）

| 签名                                         | 说明                 |
| -------------------------------------------- | -------------------- |
| `getCookie(url: String): String`             | 取该 url 全部 cookie |
| `getKey(url: String, key: String): String`   | 取单键 cookie        |
| `setCookie(url: String, cookie: String?)`    | 设置 cookie          |
| `replaceCookie(url: String, cookie: String)` | 替换 cookie          |
| `removeCookie(url: String)`                  | 删除 cookie          |
| `clear()`                                    | 清空（内部用）       |

## 12. `cache` 对象（CacheManager）

| 签名                                                                   | 说明                        |
| ---------------------------------------------------------------------- | --------------------------- |
| `put(key: String, value: Any, saveTime: Int = 0)`                      | 保存（saveTime 秒，0 永久） |
| `get(key: String): String?`                                            | 读取（字符串）              |
| `getInt/getLong/getDouble/getFloat/getByteArray(key)`                  | 类型化读取                  |
| `delete(key: String)`                                                  | 删除                        |
| `putFile(key: String, value: String, saveTime: Int = 0)`               | 存为缓存文件（50M 上限）    |
| `getFile(key: String): String?`                                        | 读缓存文件内容              |
| `putMemory(key: String, value: Any)`                                   | 存内存                      |
| `getFromMemory(key: String): Any?`                                     | 读内存                      |
| `deleteMemory(key: String)`                                            | 删内存                      |
| `put(key: String, queryTTF: QueryTTF)` / `getQueryTTF(key): QueryTTF?` | 字体反查缓存                |

## 13. `source` 对象（BaseSource）

| 签名                                                                                                                        | 说明               |
| --------------------------------------------------------------------------------------------------------------------------- | ------------------ |
| `getKey(): String`                                                                                                          | 书源 url（唯一键） |
| `getTag(): String`                                                                                                          | 书源标签           |
| `getVariable(): String` / `setVariable(variable: String?)`                                                                  | 书源变量读写       |
| `getHeaderMap(hasLoginHeader = false)`                                                                                      | 取请求头           |
| `getLoginHeader(): String?` / `getLoginHeaderMap(): Map<String,String>?` / `putLoginHeader(header)` / `removeLoginHeader()` | 登录头操作         |
| `getLoginInfo(): String?` / `getLoginInfoMap(): Map<String,String>?` / `removeLoginInfo()`                                  | 登录信息操作       |
| `getLoginJs(): String?`                                                                                                     | 取登录 JS          |

## 14. `book` / `chapter` / `rssArticle` 对象（只读属性）

`book`（Book.kt）：`bookUrl tocUrl origin originName name author kind customTag coverUrl customCoverUrl intro customIntro charset type group latestChapterTitle latestChapterTime lastCheckTime lastCheckCount totalChapterNum durChapterTitle durChapterIndex durChapterPos durChapterTime canUpdate order originOrder variable`

`chapter`（BookChapter.kt）：`url title baseUrl bookUrl index resourceUrl tag start end variable`

`rssArticle`：订阅源正文上下文只读属性，随 RssJsExtensions 注入。

## 附：原生 Rhino 全局与 QuickJS 差异要点

- legado 靠 Rhino 的 `ImporterTopLevel` 暴露 `Packages`/`importClass`/`JavaAdapter`，书源偶有 `importClass(java.util.HashMap)`/`new java.util.HashMap()` 写法；rquickjs 无此能力，导入器需识别并降级到我们自研对应对象或标注不兼容。
- legado 重定向了顶层 `java`，源里 `java.xxx` 等价其宿主 API；而真正调 Java 包用的是 `Packages.java.xxx`。我们无此歧义。
- `const` 块级作用域在 Rhino 实现有 bug（`jsHelp.md:20` 提示循环内用 `var`），rquickjs/QuickJS 语义正确，源里见到的 `var` 习惯仅历史遗留。
