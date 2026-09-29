# LanJing

本地优先的跨媒体发现与阅读工作台。本上下文描述用户可识别的本地内容、来源和规则概念。

## 来源

**Source**:
安装在本机、为资料发现提供能力的来源。来源由来源定义安装而来，而不是一段 JSON 或一个目录条目。
_Avoid_: Source JSON, catalog item

**Source Definition**:
用户提供以创建来源的输入。当前支持 Legado JSON 和 Maccms URL 两种格式。
_Avoid_: source, install candidate

**Adaptive Source Input**:
接收粘贴文本或本地文件、自动识别为来源定义的交互入口。它不是新的来源格式。
_Avoid_: format picker, source editor

**Install Candidate**:
后端为一个来源定义准备的、不透明且会过期的安装凭据，包含可展示的安全摘要和所需授权。
_Avoid_: install plan, source definition

**Stale Install Candidate**:
因目标 Source 在候选准备后发生更新而失效的 Install Candidate。它不能安装，必须重新准备和确认。
_Avoid_: retryable install, cached candidate

**Source Update**:
以已安装 Source 的同一稳定身份安装新的来源定义。更新会创建新的来源版本，并重新确认新候选提出的授权。
_Avoid_: duplicate source, overwrite

**Source Revision**:
一个 Source 的不可变安装或更新版本。回退通过从历史 Source Revision 创建新的 Source Update 实现。
_Avoid_: overwrite, mutable source version

**Source Rollback**:
从历史 Source Revision 准备新的 Source Update 的恢复操作。它不重写来源历史，且必须再次完成候选审阅和授权确认。
_Avoid_: undo update, restore in place

**Source Group**:
由 Source Definition 提供的只读分类元数据。它用于在存在多个分类时筛选 Source，不是用户维护的文件夹。
_Avoid_: source folder, source tag

**Source Inspector**:
用于查看和管理一个 Source 的当前资料、授权与 Source Revision 历史的上下文检查面。
_Avoid_: source detail page, source editor

## 资料库

**Media Item**:
由来源发现并以标准媒体模型表达的跨媒体资源。
_Avoid_: library entry

**Library Entry**:
一个标准媒体资源在本机资料库中的用户状态投影，包括收藏、置顶、最近打开时间和进度；它不是媒体资源本身。
_Avoid_: media item, bookmark

**Favorite Library Entry**:
用户明确希望长期保留和再次找到的资料库条目。收藏是置顶的前提。
_Avoid_: pinned entry, reading history

**Pinned Library Entry**:
在资料库中临时优先显示的收藏条目。置顶会自动收藏，取消收藏会同时取消置顶。
_Avoid_: favorite entry

**Favorite View**:
资料库中仅展示 Favorite Library Entry 的同一列表视图，不是独立的资料库或路由。
_Avoid_: favorites library, collection page

**Library Inspector**:
资料库条目的上下文检查面，展示条目的媒体元数据和本地状态；它不是媒体阅读器。
_Avoid_: reader, placeholder detail page

## 阅读与应用面

**App Surface**:
按媒体类型划分的沉浸式体验面，从工作台进入后脱离工作台 chrome，拥有自己的交互语言；应用面做全做深，工作台的克制约束不延伸进来。
_Avoid_: reader page, detail page

**TurnEngine**:
与媒体类型无关的通用翻页引擎，把指针、滚轮、键盘输入归一化为翻页意图，驱动导航、布局与转场；它不认识 MediaKind，也不回写进度。
_Avoid_: page turner, reader engine

**PageSource**:
App Surface 向 TurnEngine 提供页内容的契约；引擎通过它测量、获取、释放页并索取位图快照，不关心页里装的是文本还是图片。
_Avoid_: renderer, content adapter

**Turn Mode**:
layout × navigation × transition 三元的一种策展组合，共 7 种，所有 App Surface 全量通用；单页仿真与双页仿真是两个独立 Turn Mode。
_Avoid_: reading mode, viewer mode

**Reading Anchor**:
与排版参数无关的进度锚点，是阅读进度的真实来源；页码与总页数随排版变化，仅作展示。
_Avoid_: page number, scroll position

**Asset Gateway**:
以自定义协议代理网络资产请求的本地网关，注入来源规则提供的防盗链头并做磁盘缓存；它是图片能在 App Surface 显示的前提。
_Avoid_: image proxy, download cache

## 原生规则

**Native Rule Document**:
用户在本机编辑和保存的规则定义及其画布布局。
_Avoid_: install candidate, source definition

**Rule Draft Revision**:
已保存但校验未通过的 Native Rule Document 版本。它可继续编辑，但不会取代生效版本。
_Avoid_: active rule, failed rule

**Effective Rule Revision**:
最近一次通过校验并自动生效的 Native Rule Document 版本。
_Avoid_: published rule, install candidate

**Explicit Rule Save**:
由用户触发、将当前编辑状态持久化为 Rule Draft Revision 或 Effective Rule Revision 的操作。
_Avoid_: autosave, publish

**Recovery Draft**:
因保存冲突而从本地编辑状态创建的独立 Rule Draft Revision。它不覆盖原文档，也不会自动生效。
_Avoid_: conflict copy, active rule

**Draft Credential**:
只归属其所属 Rule Revision 的受保护凭证值。它的 owner 包含文档、语义 revision、节点和 JSON pointer；只有 Effective Rule Revision 的凭证能在安装时转换为 Source 的 runtime credential。
_Avoid_: client-side secret, active credential

**Rule Revision History**:
按时间保留的 Effective Rule Revision 列表。恢复历史版本会创建新的 Rule Draft Revision，而不会直接生效。
_Avoid_: direct rollback, publish history

**Rule Document Status**:
Rule Document 当前草稿与生效版本的关系摘要，用于说明用户正在编辑的版本是否已经生效。
_Avoid_: validation result, publish status
