---
title: 导入系统架构
date: '2026-07-01'
category: architecture-patterns
module: lj-importer
problem_type: architecture_pattern
component: development_workflow
severity: high
applies_when:
  - '实现新的导入器（Legado/Maccms10/LXMusic 之外的源）'
  - '理解异构规则如何翻译为节点图'
  - '修改导入流程或预览确认机制'
tags:
  - importer
  - legado
  - maccms10
  - schema-independence
---

# 导入系统架构

导入器将异构第三方规则格式翻译为 LanJing 一等公民节点图。导入器 = 翻译器，不是适配器——产出是 Graph，不是包装后的原始格式。

## Context

LanJing 支持三类异构媒体源，规则文件格式完全不同：

- **图文书源 (Legado)**：JSON/YAML 规则文件，含 `@text`/`@href`/`<js>` 语法
- **视频源 (Maccms10)**：无第三方规则文件，从采集 URL 推断协议模板
- **音频源 (LXMusic)**：Legado-like 规则格式（后续接入）

## Guidance

### 核心设计原则

- **导入器即翻译器**：全量翻译，不走兼容模式——LanJing 只理解 Graph，不保留原始第三方格式
- **自有 Schema 独立**：不被 Legado 格式演进绑架，三类源共用一套内部 schema
- **双 ID 模式**：`NodeId`（运行时标识） + `ImportHash`（导入内容哈希，用于去重合并）
- **模板约束**：导入器只能按端点子图模板组装节点，不能随意连

### Importer trait

```rust
trait Importer<Opts> {
    fn import(&self, input: &str, opts: Opts) -> Result<Graph>;
}
```

### 三种翻译路径

| 源类型   | 输入               | 翻译路径                                                             | 节点数 |
| -------- | ------------------ | -------------------------------------------------------------------- | ------ |
| Legado   | JSON/YAML 规则文件 | lj-compiler 解析规则字符串 → ExtractRule IR → lj-importer 组装节点图 | 5 端点 |
| Maccms10 | 采集入口 URL       | 内置协议模板推断端点结构 → 生成节点图                                | 3 端点 |
| LXMusic  | 脚本文件           | 类似 Legado 翻译路径（后续接入）                                     | —      |

### Legado 导入器工作流

1. 解析 Legado 规则 JSON/YAML
2. lj-compiler 把规则字符串（`@text`/`@href`/`||`/`##regex##` + `<js>` 块）解析成 `ExtractRule` IR
3. 按端点子图模板组装节点图：search/discover/detail/toc/content 五端点各生成 `Http → Extract` 子图
4. 端点子图模板验证
5. 产出完整 `Graph`（含 nodes/edges/subroutines）

**字段映射与丢弃规则**：维护一套白名单，不在名单中的字段导入时静默丢弃，保证 LanJing schema 不受 Legado 版本变化的污染。

### Maccms10 导入器差异

- 无第三方规则文件——内置 Maccms10 采集协议模板，从 URL 推断生成
- 3 端点（vs 书源 5 端点），Detail 一次性产 VideoMedia（含所有线路+分集），不拆 Detail + Toc
- 协议适配，非翻译

### 字段级集合提取

Extract 节点支持字段级集合提取：

```yaml
# 扁平字段级集合
field_rules:
  - { field: "authors", selector: ".author", extract: "text", collection: true }

# 嵌套结构字段级集合
field_rules:
  - field: "episodes", selector: ".episode", collection: true, nested:
    - { field: "title", selector: ".title", extract: "text" }
    - { field: "url", selector: "a", extract: "attr:href" }
```

### 导入时验证

所有边的类型检查和模板符合性验证在导入阶段完成，而非运行时：

- **零运行时开销**：图一旦被导入，执行器可假定图已通过验证
- **节点 I/O 类型保证**：每个 NodeKind 声明 input_type/output_type，导入器按类型签名验证边连接

### 导入入口

| 入口          | 触发方式                   | 特点                 |
| ------------- | -------------------------- | -------------------- |
| 深度链接      | `lanjing://import?src=...` | 适合分享、书签       |
| 拖拽本地文件  | 文件拖入                   | 适合本地规则文件     |
| 菜单手动导入  | UI 菜单                    | 适合浏览选择         |
| Gist 推荐分享 | 深度链接变体               | 稳定源，适合社区分享 |

所有入口统一经过**预览中介步**：展示来源 URL、规则名、节点图预览、JS 块数量、沙箱能力声明。用户显式确认后才落库激活。

### 节点双 ID 模式

```rust
struct Node {
    id: NodeId,           // 运行时分配，节点图内唯一
    import_hash: ImportHash,  // 导入内容哈希，用于去重合并
}
```

- **NodeId**：运行时标识，用于执行、日志、IPC 事件
- **ImportHash**：导入内容哈希，用于重复导入去重、规则合并、导入历史追踪
- **Stub 合并算法**：相同 ImportHash 的规则自动合并，保留最新版本

## Why This Matters

"导入器即翻译器"模式使 LanJing 保持自有 schema 独立性——不被任何第三方格式绑定。Legado 格式演进不会影响 LanJing 内部表示，LanJing 可以独立扩展自己的 schema。

## When to Apply

- 实现新的导入器：实现 `Importer<Opts>` trait，产出 `Graph`
- 修改 Legado 规则解析：调整字段映射白名单
- 新增端点类型：修改端点子图模板

## Related

- `docs/solutions/architecture-patterns/rule-engine.md` — 规则引擎整体架构
- `docs/solutions/design-patterns/rust-patterns.md` — 闭集 enum 与 trait dispatch
- `docs/solutions/best-practices/design-philosophy.md` — Skeleton Principle
