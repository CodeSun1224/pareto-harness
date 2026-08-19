---
id: ARCH-0004
title: 技术选型基线
status: accepted
owners: [maintainers]
created: 2026-08-20
updated: 2026-08-20
links: [ADR-0002]
---

# 技术选型基线

## 决策

| 领域 | 首选 | 理由 | 延后或拒绝 |
|---|---|---|---|
| 可信内核 | Rust stable + Tokio | 类型安全、并发控制、资源效率、可部署单二进制 | 不用 TypeScript 承担可信内核 |
| 序列化 | Serde + versioned JSON Schema | 人可读、便于事件导出和跨语言生成 | 首期不先引入 Protobuf/gRPC |
| 本地存储 | SQLite WAL + sqlx | 事务、迁移、单机可运维、快速纵切 | PostgreSQL 在多节点需求成立后增加 |
| Event API | Rust traits + JSONL export | 内部强类型，对外可检查 | 不暴露 Rust 动态库 ABI |
| 插件隔离 | Wasmtime/WASI | capability 和资源限制明确 | 强隔离实现延后到核心语义稳定后 |
| TypeScript | SDK、插件工具、Web 控制台 | 生态和 UI 效率 | 不在首期创建空 workspace |
| Python | 研究、模型和评测 Worker | 研究生态丰富 | 不进入权威状态机 |
| 部署 | 模块化单体 | 保持事务和调试简单 | 无真实部署边界前不拆微服务 |

## 依赖原则

- 公共协议包不依赖数据库、网络或具体模型 SDK。
- Provider SDK 只能存在于 adapter 边界。
- 数据库迁移与事件 Schema 迁移分别管理。
- 新依赖需记录维护状态、许可证、供应链风险、二进制体积和替代方案。
- 首个纵切使用 Fake Model/Tool，避免把 Provider 波动误认为内核问题。

## 可移植性

开发和文档工具支持 Windows。核心 Runtime 目标为 Windows、Linux 和 macOS；强 Sandbox 的安全声明优先以 Linux 为基准，其他平台必须明确能力差异。

## 触发重新评审的条件

- SQLite 写入竞争成为可测量瓶颈。
- 出现独立扩缩容、故障域或团队所有权需求。
- JSON Schema 无法满足兼容性或高吞吐协议需求。
- WASI 无法表达必需能力或隔离强度。
