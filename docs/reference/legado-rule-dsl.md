# Legado 规则字符串 DSL（源码考证版）

> 来源：本地克隆 `C:/Users/wuhy/workspace/_research/legado`（LegadoTeam/legado 浅克隆）。下文路径均相对该根。
> JS 引擎：HtmlUnit Core JS 5.3.0-legado.4（Rhino 兼容），见 `app/src/main/assets/web/help/md/jsHelp.md:1` 与 `app/src/main/assets/licenses/htmlunit-core-js/5.3.0-legado.4/`。
> 每条断言带 `路径:行号`。查不到的写"未验证"。
> 配套文档：`docs/reference/legado-js-builtin-inventory.md`（`java.*` 能力面）。

## 0. 与票据表述的差异（先读）

票据写的四个 selector 前缀 `$.` / `##` / `//` / `@` 与源码不符：

- `##` 不是 CSS 前缀，是替换段分隔符（`AnalyzeRule.kt:824-834`）。CSS 前缀是 `@CSS:`（`AnalyzeByJSoup.kt:508-516`）。
- `@` 不是正则前缀，是规则链分隔符（`AnalyzeByJSoup.kt:208`）兼标志前缀字符。正则模式前缀是 `:`，只用于书籍列表和目录列表规则（`AnalyzeRule.kt:599-601`；`ruleHelp.md:15`）。
- 真正的提取引擎是四种：JSoup 默认（含 `@CSS:`）、XPath、JsonPath、Regex。另有 Js / WebJs 两种脚本段模式（`AnalyzeRule.kt:252-263` 的 Mode 分发）。

## 1. 规则字符串的总体结构

一条规则字符串由多段 SourceRule 串联。切段点是 `<js>...</js>`、`@js:...`、`@webjs:...`（`AnalyzeRule.kt:593-632`；`AppPattern.kt:7-10`）。

每段内部构成：`[标志前缀] 主体 [## 替换段]`（`AnalyzeRule.kt:664-695, 824-834`）。

标志前缀（`AnalyzeRule.kt:664-695`；`ruleHelp.md:10-16`）：

| 前缀 | 模式 | 说明 |
| --- | --- | --- |
| `@@` | JSoup 默认 | 显式声明，直接写主体时可省略（`AnalyzeRule.kt:670-672`） |
| `@CSS:` | JSoup CSS | 保留前缀传给 AnalyzeByJSoup 切掉（`AnalyzeRule.kt:665-668`；`AnalyzeByJSoup.kt:508-516`） |
| `@XPath:` | XPath | 可省略，主体以 `//` 开头即自动 XPath（`AnalyzeRule.kt:675-678, 690-692`） |
| `@Json:` | JsonPath | 可省略，主体以 `$.` 或 `$[` 开头即自动 Json（`AnalyzeRule.kt:680-688`） |
| （无前缀，内容是 JSON） | JsonPath | setContent 检出 isJSON 时整条走 Json（`AnalyzeRule.kt:110-113, 685-688`） |
| `:` | Regex | 不可省略，仅列表规则（getElement/getElements 传 allInOne=true）（`AnalyzeRule.kt:599-601`） |
| `<js>...</js>` / `@js:` | Js | `@js:` 匹配到字符串结尾，必须放最后（`AppPattern.kt:7-8`） |
| `@webjs:` | WebJs | 至少 5 个字符，交 WebView 执行（`AppPattern.kt:10`；`AnalyzeRule.kt:182-201`） |

隐式切到 Regex（模板）模式的条件（`AnalyzeRule.kt:704-715, 745-755`）：

1. 段内含 `{{...}}` 或 `@get:{...}`，且首个 `##` 之前没有替换段。
2. 段内含 `$N`（N 为 1 到 2 位数字）正则组引用（`regexPattern`，`AnalyzeRule.kt:1030`）。

模板模式的行为：getString 下返回拼装后的字符串本身（`AnalyzeRule.kt:358`）；getElement/getElements 下整段当正则链执行（`AnalyzeRule.kt:400-402, 464-466`）。

辅助语法：

- `@put:{"key":"规则"}`：解析前写变量，value 本身是规则（`AnalyzeRule.kt:516-541`；pattern `AnalyzeRule.kt:1027`）。
- `@get:{key}`：读变量，拼进主体（`AnalyzeRule.kt:717-720, 814-816`）。
- `##match##replacement`：替换段，见 §4。

## 2. 四种提取引擎的确切语义与边界

### 2.1 JSoup 默认（`@@` / 无前缀）与 `@CSS:`

非 CSS 模式（`AnalyzeByJSoup.kt:196-218`）：

1. 主体按 `@` 切成链（`RuleAnalyzer.splitRule("@")`，`AnalyzeByJSoup.kt:208`）。
2. 链的每一步把上一步的元素集再选择一遍；只有一步时直接从根元素选（`AnalyzeByJSoup.kt:196-218`）。
3. 最后一步提取内容。关键字：`text`、`textNodes`（文本子节点拼接）、`ownText`、`html`（去 script/style 后 outerHtml）、`all`（全部 outerHtml）；其余字符串按属性名取 attr（`AnalyzeByJSoup.kt:224-270`）。

链步选择器写法（`AnalyzeByJSoup.kt:289-320`）：`children`、`class.x`、`tag.x`、`id.x`、`text.x`，其余整段交 CSS select。

索引语法（`AnalyzeByJSoup.kt:272-287`，实现 `289-505`）：

- 旧式：`tag.div.-1:10:2`、`tag.div!0:3`。`.` 选中，`!` 排除，`:` 是区间步进，索引可负。
- 新式：`tag.div[-1, 3:-2:-10, 2]`。`[!` 开头表示排除；区间为 `start:end:step`，可省略端点；`tag.div[-1:0]` 可反转列表。

`@CSS:` 模式（`AnalyzeByJSoup.kt:90-95, 508-516`）：主体直接 CSS select，提取用最后一个 `@` 之后的关键字或属性名，不走 `@` 链。

边界：内容以 `<?xml` 开头时用 jsoup xmlParser（`AnalyzeByJSoup.kt:28-33`）；引号内的 `&&`、`||`、`%%`、`@` 不参与切分（`RuleAnalyzer.kt:133-160` 的引号感知切分）。

### 2.2 XPath（`@XPath:` / `//` 开头）

- 解析器是 seimicrawler JXDocument / JXNode，逐段调用 `sel`（`AnalyzeByXPath.kt:24-52`）。
- getString 把多段结果用 `\n` 拼接（`AnalyzeByXPath.kt:135-152`）。
- getStringList / getElements 支持全部三种组合符（`AnalyzeByXPath.kt:58, 96`），getString 只切 `&&`、`||`（`AnalyzeByXPath.kt:135`）。

### 2.3 JsonPath（`@Json:` / `$.` / `$[` / isJSON）

- 解析器是 Jayway JsonPath（`AnalyzeByJSonPath.kt:21-28`）。
- 支持内嵌 `{$.rule}`：平衡花括号拉出子规则递归解析（`AnalyzeByJSonPath.kt:44, 82`；`RuleAnalyzer.innerRule` `RuleAnalyzer.kt:308-337`）。
- getString 只切 `&&`、`||`，结果用 `\n` 拼接（`AnalyzeByJSonPath.kt:38-70`）。getStringList / getList 支持 `%%`（`AnalyzeByJSonPath.kt:78, 136`）。
- getObject 供 getElement 取单个对象（`AnalyzeByJSonPath.kt:129`）。

### 2.4 Regex（`:` 前缀，或模板模式）

- 列表规则以 `:` 开头进入（`AnalyzeRule.kt:599-601`）。
- 正则链按 `&&` 切分，每级把上一级的全部匹配拼接成新输入交给下一级（`AnalyzeRule.kt:400-402, 433-435, 464-466`；`AnalyzeByRegex.kt:9-58`）。
- 最后一级：每个匹配产出 `group(0..groupCount)` 的列表；getElement 只取第一个匹配（`AnalyzeByRegex.kt:9-29`），getElements 取全部匹配（`AnalyzeByRegex.kt:32-58`）。
- 模板模式的 `$N` 引用上一步 List 的第 N 项，缺失时保留原文（`AnalyzeRule.kt:783-792`）。

## 3. 组合符 `&&` / `||` / `%%`

切分器（`RuleAnalyzer.splitRule`，`RuleAnalyzer.kt:165-236`）：跳过 `[ ]`、`( )` 平衡组与引号内容，转义符 `\` 生效（`RuleAnalyzer.kt:368`，`chompRuleBalanced` / `chompCodeBalanced` 两种平衡策略）。

语义（`AnalyzeByJSoup.kt:84-116`）：

| 符号 | 语义 |
| --- | --- |
| `&&` | 顺序合并全部非空结果 |
| `||` | 取第一个非空结果即停（`AnalyzeByJSoup.kt:102`） |
| `%%` | 按下标交错：第 1 轮取各段第 1 项，第 2 轮取各段第 2 项，依此类推（`AnalyzeByJSoup.kt:106-116`） |

优先级：没有优先级语法。首个出现的组合符决定整条规则的组合方式，其余种类的组合符留在段内不再被切分（`RuleAnalyzer.kt:186-196`）。后果：

- JSoup 段内残留的组合符按普通字符处理，进 CSS select 或选择器（`AnalyzeByJSoup.kt:97`）。
- XPath / JsonPath 会对段递归再切一次，混用行为与 JSoup 不同（`AnalyzeByXPath.kt:106, 144`；`AnalyzeByJSonPath.kt:62, 101, 148`）。

结论：一条规则只用一种组合符。混用的行为是实现细节，不是契约。

## 4. `{{...}}` 模板与替换段

`{{...}}` 在规则字符串里（`AnalyzeRule.kt:704-733, 776-816`）：

- 内容以 `@`、`$.`、`$[`、`//` 开头时当子规则解析（`isRule`，`AnalyzeRule.kt:837-841`；`ruleHelp.md:10`）。
- 否则当 JS 求值。返回 null 跳过；整数值的 Double 格式化为无小数字符串；其余 toString（`AnalyzeRule.kt:794-810`）。

`{{...}}` 在 URL 里（`AnalyzeUrl.kt:216-228`）：

- 一律当 JS 求值，值转字符串（同样有整数 Double 格式化）。
- 常见写法：`{{key}}`（搜索关键字）、`{{page}}`、`{{page-1}}`（页码算术）。旧书源 `searchKey` / `searchPage-1` 的等价转换见 `ImportOldData.kt:324-350`。

`{{key, option}}` 形式：**本地源码未见**。`{{...}}` 内容是完整 JS 表达式，逗号是 JS 逗号运算符。未发现带 option 参数的模板语法。未验证：是否存在社区文档描述的变体。

真正的 "option" 面是 URL 后缀 `url,{"json"}`（`paramPattern = \s*,\s*(?=\{)`，`AnalyzeUrl.kt:817`）。UrlOption 全部键（`AnalyzeUrl.kt:829-886`，应用处 `266-299`）：

| 键 | 作用 |
| --- | --- |
| `method` | GET / POST / HEAD（`AnalyzeUrl.kt:267-273`） |
| `charset` | query / body 编码字符集，`escape` 表示不编码 |
| `headers` | 请求头，Map 或 JSON 字符串（字段定义 `AnalyzeUrl.kt:829-886`） |
| `body` | POST body，字符串或 JSON（字段定义 `AnalyzeUrl.kt:829-886`） |
| `origin` | 源 URL 记录 |
| `retry` | 重试次数 |
| `type` | 资源类型（如文件扩展名、音频 mime） |
| `webView` | 布尔，走 WebView 渲染（应用点 `AnalyzeUrl.kt:283`） |
| `webJs` | WebView 内执行的 JS |
| `timeout` | 读取超时毫秒 |
| `followRedirects` | 是否跟随重定向 |
| `dnsIp`（别名 `resolveIp`） | 强制指定目标域名 IP，逗号分隔多个（`AnalyzeUrl.kt:865-866`） |
| `js` | URL 解析完执行，结果回写 url（`AnalyzeUrl.kt:292-295`） |
| `bodyJs` | 响应后执行，结果作为 body（`AnalyzeUrl.kt:548-549`） |
| `serverID` | 音频服务器 ID |
| `webViewDelayTime` | WebView 加载后延迟毫秒 |

替换段 `##`（`AnalyzeRule.kt:544-567, 824-834`）：

- `规则##match##replacement`：把结果中 match 的全部匹配替换为 replacement。
- 第 4 段存在时（`规则##match##replace###`）只替换第一个匹配；无匹配返回空串（`AnalyzeRule.kt:548-559`）。
- 只有 2 段时（`规则##match`）replacement 为空，效果是删除 match。
- 替换段在 `{{...}}` 求值之后切分（`AnalyzeRule.kt:776-834`）。

## 5. 分页与"下一页"的声明方式

没有名为 `nextPage` 的书源规则字段。分页共五处：

1. **搜索 / 发现 URL 页码**：`{{page}}` 由调用方传入（`AnalyzeUrl.kt:401`）；`<a,b,c>` 页码表按页取第 page 项，超出取末项（`AnalyzeUrl.kt:232-241`；`pagePattern` `AnalyzeUrl.kt:818`）。
2. **目录下一页**：`TocRule.nextTocUrl`（`data/entities/rule/TocRule.kt:19`）。用 `getStringList(rule, isUrl = true)` 取 URL 列表，与当前 redirectUrl 相同的跳过，由 WebBook 循环抓取（`BookChapterList.kt:211-221`）。
3. **正文下一页**：`ContentRule.nextContentUrl`（`data/entities/rule/ContentRule.kt:16`）。同 `getStringList(..., isUrl = true)`（`BookContent.kt:261-268`）。
4. **订阅源列表下一页**：`RssSource.ruleNextPage`（`data/entities/RssSource.kt:58`）。值为字面量 `PAGE` 时直接复用当前分类 URL，否则按规则取（`RssParserByRule.kt:56-62`）。**正文下一页**是 `RssSource.nextContentUrl`（`RssSource.kt:73`；`Rss.kt:232-238`）。
5. **段评翻页**：`ReviewRule.reviewDetailNextPageUrl`（`data/entities/rule/ReviewRule.kt:29`）。规则返回非空即表示还有下一页，URL 本身不被使用（`ReviewRuleParser.kt:142-163`）。

## 6. JS 块：`@js:` / `<js>` / `{{}}`

### 6.1 语法与切段

- `<js>...</js>` 或 `@js:`（后者匹配到字符串结尾）（`AppPattern.kt:7-8`）。
- `@webjs:` 至少 5 个字符，返回值按 JSON 数组或原文处理（`AppPattern.kt:10`；`AnalyzeRule.kt:252-258`）。
- URL 里的 `<js>` / `@js:` 先于参数替换执行，链式传递：上一段结果以变量 `result` 传入下一段，非 JS 片段里的 `@` + `result` 被上一段结果文本替换（`AnalyzeUrl.kt:188-212`）。

### 6.2 上下文变量

规则上下文（`AnalyzeRule.evalJS`，`AnalyzeRule.kt:896-914`）：`java`（指向 AnalyzeRule）、`cookie`、`cache`、`source`、`book`（`ruleData as? BaseBook`，可空）、`result`、`baseUrl`、`chapter`、`chapters`（批量正文）、`title`、`src`、`nextChapterUrl`、`rssArticle`、`fromBookInfo`，以及局部绑定 `paraIndex`、`paraData`、`page`（`AnalyzeRule.kt:912-914`）。

URL 上下文（`AnalyzeUrl.evalJS`，`AnalyzeUrl.kt:395-412`）：`java`（指向 AnalyzeUrl）、`baseUrl`、`cookie`、`cache`、`page`、`key`（搜索关键字）、`speakText`、`speakSpeed`（朗读引擎）、`book`、`source`、`result`、`infoMap`（发现按钮值），以及 extraParams 全部键。

`java` 变量指向宿主对象而非 Java 包；调 Java 包要写 `Packages.java.*`（`jsHelp.md:12`）。

### 6.3 返回值约定

- JS 返回值经 toString 进入字符串管道；整数值的 Double 格式化为无小数字符串（`AnalyzeRule.kt:799-804`；`AnalyzeUrl.kt:222-227`）。
- getString 对最终结果做 HTML 反转义（含 `&` 时）（`AnalyzeRule.kt:368-372`）。
- getStringList 的 isUrl=true 会对每项做绝对化并去重（`AnalyzeRule.kt:282-292`）；getString 的 isUrl=true 空结果回退 baseUrl（`AnalyzeRule.kt:374-378`）。
- getElement / getElements 的 WebJs 段返回值按 JSON 对象 / JSON 数组解析（`AnalyzeRule.kt:182-201`，分发 `387-475`）。
- getElements 的返回值过滤 null 与 `Scriptable.NOT_FOUND`（`AnalyzeRule.kt:476-500`）。

## 7. 任意 Java 类调用面（QuickJS 复刻的覆盖率上限）

### 7.1 互操作面

引擎原生暴露 `JavaImporter`（`importClass` / `importPackage`）、`Packages` / `java` / `javax`、`getClass`、`JavaAdapter`（`jsHelp.md:6-10`）。

### 7.2 沙箱

`RhinoClassShutter`（`modules/rhino/src/main/java/com/script/rhino/RhinoClassShutter.kt:47-190`）按黑名单过滤：

- 黑名单类名：`java.lang.Class`、`ClassLoader`、`Runtime`、`ProcessBuilder`、`java.io.File` 系列、`ObjectInputStream` / `ObjectOutputStream`、`android.content.Intent`、hutool 反射 / 序列化系列、`io.legado.app.data.*` 等（`RhinoClassShutter.kt:49-125`）。
- 黑名单包前缀：`java.lang.reflect`、`java.nio.file`、`dalvik.system`、`sun`、`libcore`、`org.mozilla`、`com.script`、`io.legado.app.data.dao` 等（同上）。
- 受保护类型：`ClassLoader`、`Class`、`Member`、Rhino `Context`、`ObjectInputStream` / `ObjectOutputStream`、okio 文件系统、`android.content.Context`（`RhinoClassShutter.kt:127-175`）。
- `System.load` / `loadLibrary` / `exit` 屏蔽（`RhinoClassShutter.kt:122-124`）。

其余类默认可见。即：书源可以 `Packages.*` 调用任意未列入黑名单的 Java 类。

### 7.3 真实调用证据

随应用发布的默认规则与官方文档里真实使用任意 Java 类：

- `app/src/main/assets/defaultData/httpTTS.json:13`（阿里云语音登录）：`new JavaImporter(Packages.javax.crypto.Mac, Packages.javax.crypto.spec.SecretKeySpec, Packages.javax.xml.bind.DatatypeConverter, Packages.java.net.URLEncoder, Packages.java.lang.String, Packages.android.util.Base64)`。
- `app/src/main/assets/defaultData/dictRules.json:5`（百度汉语字典规则）：`new JavaImporter(Packages.com.jayway.jsonpath)`。
- 官方图片解密示例：`ruleHelp.md:373-375, 394-395`（`Packages.java.io.ByteArrayInputStream` / `ByteArrayOutputStream`）。

据此的最小 Java 类白名单（已验证被真实规则使用）：`javax.crypto.Mac`、`javax.crypto.spec.SecretKeySpec`、`javax.xml.bind.DatatypeConverter`、`java.net.URLEncoder`、`java.lang.String`、`android.util.Base64`、`java.io.ByteArrayInputStream`、`java.io.ByteArrayOutputStream`、`com.jayway.jsonpath`。

### 7.4 未验证项

- **第三方真实书源的任意 Java 类调用覆盖率**：未验证。本地没有真实书源语料。lj-importer 的 fixtures 是合成源（`src-tauri/crates/lj-importer/fixtures/legado_synthetic_source.json:1` 自述"合成测试书源,非真实规则"）。
- `JavaAdapter`（运行时继承 Java 类）在真实书源中的使用率：未验证。

### 7.5 复刻结论

QuickJS 侧需要两条线：

1. `java.*` 宿主 API 对齐（见 `legado-js-builtin-inventory.md`）。
2. `Packages.*` 任意 Java 类调用无法直接复刻。只能按 §7.3 白名单实现常用类，或由导入器识别并降级标注。上限取决于真实语料统计，当前语料只够给出上述最小集。

## 8. 对照 lj-importer 翻译器

- 位置：`src-tauri/crates/lj-importer/src/legado/{mod,parser,translator,types}.rs`。
- 现有测试覆盖 `{{key}}` / `{{page}}` 模板与 `nextTocUrl` / `nextContentUrl` 分页（`src-tauri/crates/lj-importer/tests/legado_test.rs:87, 277, 388-393`）。
- 未覆盖：`%%` 组合符、`:` 正则列表规则、`@put:` / `@get:`、`##` 替换段、UrlOption。导入器补齐前需按本文档核对。
