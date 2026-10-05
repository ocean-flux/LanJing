# ADR 0002：来源和规则采用版本化、审阅式生命周期

- 状态：已采纳
- 影响范围：来源安装与管理、Native Rule Document 持久化、凭证归属、资料库组织交互

## 背景

React 迁移是产品重构，不以复刻 Svelte 页面为目标。来源更新会改变本地发现能力和授权，规则保存会改变可执行语义；两者都不能用“直接覆盖当前状态”表达。现有来源候选已经具备不透明、过期和 revision 绑定的安全边界，而现有规则保存会持久化校验失败的语义，二者需要收敛为用户可理解的版本模型。

## 决定

1. 来源入口使用 Adaptive Source Input，同时接受 Legado JSON、Maccms URL 和本地 JSON 文件。解析格式不是用户先选的导航步骤。
2. Source 的安装、更新和回退都必须准备 Install Candidate 并经过审阅。系统能力授权每次与候选绑定确认；候选过期或 stale 后重新准备，不能自动重试。
3. Source Update 与 Source Rollback 都追加 Source Revision。回退从历史版本创建新的更新，不重写历史；Source Inspector 展示当前资料、授权和历史。
4. Native Rule Document 区分 Rule Draft Revision 与 Effective Rule Revision。Explicit Rule Save 会保存草稿；通过校验的草稿在同一生命周期中自动晋升为生效版本，不再存在 prepare 或手动发布步骤。
5. 无效草稿不会替换 Effective Rule Revision。保存冲突时，保留原文档的远端版本，并创建独立 Recovery Draft；语义冲突不自动合并。
6. Draft Credential 只存于其所属 Rule Revision 的受保护 secret artifact；secret owner 必须包含 `document_id`、semantic revision、节点和 JSON pointer。它不进入前端持久化状态，且只有 Effective Rule Revision 的 credential 可在 prepare/install 时编码为 source runtime credential。
7. Rule Revision History 只展示 Effective Rule Revision；恢复历史版本创建新的草稿并重新走校验与自动生效流程。
8. Library Entry 的 `pinned` 以 `favorite` 为前提。后端写入必须维护该不变量，前端只能作为即时反馈的镜像。

## 后果

- Rust 的规则持久化模型需要能同时读取草稿和生效快照，并以原子事务处理草稿、凭证和晋升；不能继续把一次 `save_native_rule_document` 的未通过定义当作唯一当前版本。
- 来源历史需要补充面向 UI 的查询与“由历史 revision 准备候选”契约；历史 package 和 plan 已由存储层保留，不能要求用户重新寻找旧输入。
- React 不新增来源或资料库详情路由。来源和资料库均以列表加 Sheet 检查器承载管理操作，阅读应用可用后才提供“打开”命令。
- 需要为候选 stale、授权变更、来源回退、草稿保存、自动晋升、凭证隔离、冲突恢复、收藏置顶不变量分别建立前后端测试。

## 已否决的方案

- 逐项复刻 Svelte 安装页和规则 prepare 按钮：保留旧流程形状，却不解决规则何时真正生效。
- 有效规则保存失败时不持久化草稿：用户修复复杂规则后可能因重启或冲突丢失工作。
- 允许无效草稿直接取代生效规则：错误配置会影响本地执行能力。
- 直接把历史来源或规则设为当前版本：会绕过最新授权、校验和并发控制。
