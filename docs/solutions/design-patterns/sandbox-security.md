---
title: 沙箱与安全模式
date: '2026-07-01'
category: design-patterns
module: lj-sandbox
problem_type: security_issue
component: development_workflow
severity: critical
applies_when:
  - '修改 JS 沙箱能力配置'
  - '调整 SSRF 防护规则'
  - '新增 host API 注入'
tags:
  - sandbox
  - security
  - ssrf
  - quickjs
---

# 沙箱与安全模式

JS 运行时安全隔离与能力控制。

## Context

LanJing 执行用户定义的 JS 规则代码，需要在安全隔离的环境中运行，防止恶意代码访问本地资源或内网。

## Guidance

### 能力分层

- **默认最小权限**：按需注入
- **零注入强默认**：fs/env/process 零注入
- **SSRF 硬边界**：`validate_url_and_pin` 阻止内网 IP + DNS 重绑定
- **纵深防御**：沙箱（兜底）+ 预览中介步（信任边界）

### Host API 注入

| API                                   | 能力       | 默认 |
| ------------------------------------- | ---------- | ---- |
| `java.ajaxGet` / `java.ajaxPost`      | 网络访问   | ✅   |
| `encodeURI` / `decodeURI` / `log`     | 编码与日志 | ✅   |
| `baseUrl`、`book`、`source`、`cookie` | 只读属性   | ✅   |
| fs / env / process                    | 本地资源   | ❌   |

### JS 运行时

- **rquickjs**：ES2020 支持
- 资源限制：内存上限 + watchdog 超时
- 无 `eval` / `Function` 动态代码

### 网络边界演进

- **早期愿景**：`network:true` 仅访问目标源站
- **当前实际**：允许任意公网 URL，仅强制 SSRF 硬边界
- **理由**：Legado 实际生态有跨站 fetch 需求（图床、搜索聚合 API），强制同源会破坏大量现有规则

SSRF 防护实现：

- `validate_url_and_pin`：SSRF 检查 + DNS 重绑定防护
- `hickory-resolver`：异步 DNS 解析 + IP pinning
- HTTPS 重定向 TOCTOU 修复：每跳重建 client 并 pin 地址

### Cookie Jar

源站点 Cookie 管理，按源站 origin 隔离：

- **Origin-scoped**：Cookie 按 (scheme, host, port) 隔离
- **持久化**：存储在 SQLite，跨会话保留
- **沙箱隔离**：JS 沙箱无法直接读写 Cookie，只能通过 host API 间接使用

### 安全架构

```
预览中介步（信任边界）
    ↓ 用户确认
沙箱执行（兜底）
    ↓ SSRF 检查
目标源站
```

预览步是信任边界——展示来源 URL、规则名、节点图预览、JS 块数量、沙箱能力声明。用户显式确认后才落库激活。沙箱仅限制危险能力。

## Why This Matters

LanJing 执行不受信任的 JS 规则代码，安全隔离是核心要求。纵深防御（预览确认 + 沙箱 + SSRF 检查）确保即使单层防护失效，整体仍然安全。

## When to Apply

- 修改沙箱能力配置：调整 host API 注入列表
- 新增 SSRF 规则：修改 `validate_url_and_pin`
- 添加新的 host API：评估安全影响

## Related

- `docs/solutions/best-practices/design-philosophy.md` — 离线应用原则
- `docs/solutions/architecture-patterns/rule-engine.md` — 规则执行中的沙箱
