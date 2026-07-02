---
title: 设计哲学与架构决策
date: '2026-07-01'
category: best-practices
module: lanjing
problem_type: best_practice
component: development_workflow
severity: high
applies_when:
  - '做架构决策时'
  - '评估技术方案复杂度'
  - '决定是否引入新依赖或新模式'
tags:
  - ponytail
  - offline-first
  - design-philosophy
  - tracer-bullet
---

# 设计哲学与架构决策

LanJing 开发中贯穿的设计哲学：Ponytail（最简可行方案）、Skeleton Principle（骨架原则）、离线优先、Tracer-bullet 策略。

## Context

项目初期需要建立一致的架构决策框架，防止过度设计和过早优化。

## Guidance

### Ponytail（首刀 MVP）

先交付最简版本，推迟 retry、caching、高级优化等功能——直到用户反馈形成合理的默认值：

- 默认不做重试——源站不可达时直接报错
- 默认不做缓存——每请求直连源站
- 默认不做并发控制——单线程顺序执行

这不是"不做"，而是"证据驱动的做"——每个优化都有实测数据支撑其必要性。

### Ponytail Ladder（设计阶梯）

选方案时从最低层级开始，满足需求即停：

1. **文件级方案**：能靠文件系统解决 → 不引入网络协议
2. **进程级方案**：能靠单进程解决 → 不引入 IPC / 多进程
3. **协议级方案**：只有文件级/进程级确实不够时才上升到协议级

WebDAV 同步是此原则的应用：OS 层挂载（文件级）而非内置 WebDAV 客户端（协议级）。

**反模式**：在缺乏反馈信号时添加 retry/cache——"先做了再说，默认值后面调"实际上后面永远不来。

### Skeleton Principle（骨架原则）

提前交付 schema 与接口骨架，使后续算法/特性增加不需要 schema 返工：

- `NodeKind` enum 从一开始就是闭集——后期新增 variant 只需加 match 分支
- `ExtractRule` IR 闭集——新增提取方式只需加 variant + 解析代码
- `NodeProcessor` trait 定义统一接口——新增 NodeKind 只需实现 trait

核心：先确定形态边界（闭集 enum、trait 签名），后填充实现——边界变更成本远高于实现变更。

### 离线应用原则

LanJing 是单机桌面离线应用：

- **无 LanJing 账号/用户 session/ID token**
- **无云端 API/跨设备同步服务/OAuth provider/远端配置中心**
- **无云端漫游服务运营**——但用户自助接同步通道不冲突（WebDAV mount 等）

边界澄清：

- JS sandbox "网络:true" 是访问目标源站的 HTTP 能力，不是 LanJing 后端
- Cookie jar 是源站会话状态，不是 LanJing 用户登录态
- UA/代理设置 是客户端本地配置，不是中转代理服务

这条决策**难逆转**：引入后端/登录/同步会把整个 trust boundary 与存储模型重写。

### Tracer-bullet 策略

首刀切 Legado 书源，端到端打通全链路后再反推通用抽象：

1. 不预先设计通用规则引擎——先用一条真实源切通整条链路
2. 从 Legado 切通的经验中提炼通用规则引擎抽象
3. 用 Maccms10 视频源验证抽象是否能容纳异构源

为什么选 Legado：规则生态最丰富，图文书源形态最复杂（搜索→详情→目录→内容四段），能容纳则能覆盖视频/音频。

### 配置与领域数据边界

| 类型     | 存储        | 技术    | 示例               |
| -------- | ----------- | ------- | ------------------ |
| 用户配置 | Tauri Store | KV JSON | UI 偏好、最近打开  |
| 领域数据 | SQLite      | 关系型  | 媒体库、规则、书签 |

原则：配置不入库，领域数据不进 KV，两层独立演进。

## Why This Matters

这些设计哲学不是学术练习——每个都有明确的工程价值：Ponytail 避免过早优化，Skeleton Principle 降低后续修改成本，离线原则简化 trust boundary。

## When to Apply

- 做架构决策时：先走 Ponytail Ladder
- 引入新依赖时：问"文件系统能解决吗？"
- 设计新类型时：先定闭集 enum 骨架
- 评估同步方案时：检查是否违反离线原则

## Related

- `docs/solutions/architecture-patterns/rule-engine.md` — Skeleton Principle 在规则引擎的应用
- `docs/solutions/architecture-patterns/storage-layer.md` — WebDAV 同步（Ponytail Ladder 示例）
- `docs/solutions/design-patterns/sandbox-security.md` — 安全架构
