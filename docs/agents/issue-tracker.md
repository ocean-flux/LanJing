# 事项跟踪：本地 Markdown

本仓库的事项和规格记录在 `.scratch/`，仅保留本机，不进入 Git。

## 约定

- 每个功能使用一个目录：`.scratch/<feature-slug>/`。
- 规格文件为 `.scratch/<feature-slug>/spec.md`。
- 实施事项位于 `.scratch/<feature-slug>/issues/<NN>-<slug>.md`，从 `01` 编号；不得把多个事项合并进单一文件。
- 每个事项文件顶部包含 `Status:`，值使用 `triage-labels.md` 的角色字符串。
- 对话与补充记录追加到文件末尾的 `## Comments`。

当技能要求“发布到 issue tracker”时，在对应 `.scratch/<feature-slug>/` 下创建文件；当要求获取事项时，读取用户给出的文件路径或编号。

## Wayfinder

- Map：`.scratch/<effort>/map.md`。
- 子事项：`.scratch/<effort>/issues/NN-<slug>.md`，顶部包含 `Type:` 与 `Status:`。
- 依赖：`Blocked by: NN, NN`；所有依赖为 `resolved` 后解除阻塞。
- Frontier：扫描未阻塞、未认领的事项，编号最小者优先。
- Claim：先写入 `Status: claimed` 再开始工作。
- Resolve：在 `## Answer` 写入结论，改为 `Status: resolved`，并更新 `map.md` 的决定指针。
