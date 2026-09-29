# 事项跟踪：GitHub Issues

本仓库的事项与规格记录在 GitHub Issues（`ocean-flux/LanJing`），统一用 `gh` CLI 操作。`.scratch/` 已退役，不再使用。

## 约定

- 建事项：`gh issue create --title "..." --body-file <文件>`；多行正文写入文件，不把 JSON 拼进命令行。
- 读事项：`gh issue view <number> --comments`。
- 列事项：`gh issue list --state open --json number,title,labels`，按 label 与 state 过滤。
- 评论：`gh issue comment <number> --body "..."`。
- 改标签：`gh issue edit <number> --add-label "..."` / `--remove-label "..."`。
- 关闭：`gh issue close <number> --comment "..."`。

仓库由 `git remote -v` 推断，在 clone 内运行时 `gh` 会自动识别。

## 状态标签

五个 triage role 的取值见 `triage-labels.md`。事项的状态用同名 label 表达，不写进正文。

## 结构

- Effort 父 issue：一个 effort 一个父 issue，正文承载该 effort 的 spec 与实施地图的决定指针。
- 事项：每个事项是父 issue 的 GitHub sub-issue（`gh issue create --parent <父 issue>`）。粒度不变：一个事项一个 issue，不把多个事项合并进同一 issue。
- 依赖：用 GitHub 原生 blocked-by 关系表达（`gh issue edit <n> --add-blocked-by <blocker>`），不写 `Blocked by:` 文本行。
- Frontier：扫描父 issue 的未关闭 sub-issue，去掉仍有未关闭 blocker（`issue_dependencies_summary.blocked_by > 0`）或已有 assignee 的，编号最小者优先。
- Claim：`gh issue edit <n> --add-assignee @me` 作为本会话的第一次写入。
- Resolve：`gh issue comment <n>` 写入结论与验证证据，再 `gh issue close <n>`，并更新父 issue 的决定指针。

## PR 作为请求入口

**不开**：本仓库不把外部 PR 当作事项入口。

## 技能要求 "publish to the issue tracker" 时

建一个 GitHub issue。

## 技能要求 "fetch the relevant ticket" 时

`gh issue view <number> --comments`。
