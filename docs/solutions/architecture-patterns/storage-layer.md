---
title: 存储层架构
date: '2026-07-01'
category: architecture-patterns
module: lj-storage
problem_type: architecture_pattern
component: development_workflow
severity: high
applies_when:
  - '理解 LanJing 的数据持久化策略'
  - '修改存储层或同步机制'
  - '添加新的存储表或查询'
tags:
  - storage
  - sqlite
  - webdav
  - cold-reload
---

# 存储层架构

LanJing 双存储架构：SQLite 领域数据 + Tauri Store 用户配置。

## Context

单机离线桌面应用需要持久化两类数据：结构化的领域数据（媒体库、规则、书签）和轻量的用户配置（UI 偏好）。同时需要可选的跨设备同步能力，但不能引入自有后端。

## Guidance

### 双存储架构

| 存储        | 用途                                       | 技术                        |
| ----------- | ------------------------------------------ | --------------------------- |
| SQLite      | 领域数据（媒体库、规则、书签、Cookie Jar） | rusqlite + `spawn_blocking` |
| Tauri Store | 用户配置（UI 偏好等）                      | KV JSON                     |

**边界清晰**：配置不入库，领域数据不进 KV，两层独立演进。

### SQLite 设计

- **三表 schema**：`media` + `media_metadata` + `media_relations`
- **Repository<T> trait**：泛型 repository 模式，`Repository<Graph>`、`Repository<Media>`
- **FTS5 全文搜索**：支持中文分词
- **WAL 模式**：提升并发读性能
- **连接池**：通过 `tokio::sync::Mutex` 序列化写操作

并发控制要点：

- SQLite 单文件跨进程同时写会损坏
- 批量操作使用事务保证原子性
- `rusqlite` 同步 API + `tokio::task::spawn_blocking` 包装

### WebDAV 同步与冷重载

LanJing 不内置 WebDAV 客户端——依赖操作系统文件系统层（Windows 映射网络驱动器 / macOS mount_webdav / Linux davfs2）。

**同步策略**：

- **启动时**：检测 SQLite 与 Tauri Store 文件的 mtime 是否与上次退出时记录不一致——不一致即外部改过文件，触发冷重载
- **运行期间**：不进行同步推拉，LanJing 独占 SQLite
- **退出时**：flush 并 fsync SQLite、记录最新 mtime。webdav 后台同步软件随后推上传

**冷重载 (Cold Reload)**：关闭可能过时的内存句柄，从磁盘重新打开文件以加载最新数据。全量丢弃内存状态 → 重新打开文件 → 重建索引/缓存。

mtime 是跨平台文件修改检测的最轻量方案——无需 inotify / fsevents / 文件锁。

### 设计原则

- **本地优先**：SQLite 为主存储，WebDAV 为可选外挂
- **OS 层挂载**：不内置 WebDAV 协议实现，复用操作系统能力
- **最小检测**：mtime 为唯一同步检测机制——不做文件锁、不做冲突合并
- **与离线原则一致**：用户自助同步，LanJing 不运营任何云端服务

## Why This Matters

双存储架构将"领域数据"和"用户配置"的变更频率、查询模式、一致性要求完全隔离。SQLite 负责需要关系查询和事务的领域数据，Tauri Store 负责高频读写的 KV 配置。

## When to Apply

- 新增领域数据实体：在 SQLite schema 中添加表
- 修改同步机制：调整 mtime 检测或冷重载逻辑
- 添加新的配置项：存入 Tauri Store，不进 SQLite

## Related

- `docs/solutions/best-practices/design-philosophy.md` — 离线应用原则与 Ponytail Ladder
- `docs/solutions/architecture-patterns/rule-engine.md` — 规则引擎如何使用存储层
