# Context Map

## Contexts

- [产品体验](./docs/contexts/product-experience/CONTEXT.md) — 定义跨媒体发现、媒体空间、资料库与消费活动
- [规则系统](./docs/contexts/rule-system/CONTEXT.md) — 定义来源规则、标准意图、媒体资源模型与规则编辑语义

## Relationships

- **规则系统 → 产品体验**：规则系统通过标准意图、媒体资源图和媒体资源图增量提供内容能力；产品体验只消费标准媒体模型。
- **产品体验 → 规则系统**：内容界面可以发起来源诊断或规则定位请求，但不直接依赖规则内部流程节点。
- **规则系统 ↔ 产品体验**：`PresentationHint` 只能辅助模板选择和排序，来源规则不能定义 UI 结构。
