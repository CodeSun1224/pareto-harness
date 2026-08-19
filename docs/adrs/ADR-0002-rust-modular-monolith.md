---
id: ADR-0002
title: 以 Rust 模块化单体和 SQLite 建立首个纵向切片
status: accepted
owners: [maintainers]
created: 2026-08-20
updated: 2026-08-20
links: [REQ-0001, ARCH-0004]
---

# Context

首个实现需要先证明事件、版本、重放和证据语义，而不是处理分布式系统和多语言部署复杂度。

# Decision

可信内核使用 Rust stable/Tokio；首期持久化使用 SQLite WAL/sqlx；公共数据以 Serde 和版本化 JSON Schema 表达；部署为模块化单体。TypeScript SDK、Python Worker、WASM 插件和远程服务在核心语义稳定后增加。

# Alternatives

- 全 TypeScript/Cordis 扩展：启动快，但无法形成独立内核边界。
- 立即拆分 Rust/TypeScript/Python 多服务：并行表象大于产品证据。
- 首期 PostgreSQL/Kafka：增加运维和测试成本，尚无吞吐依据。

# Consequences

初期事务、调试和重放更简单，也可利用 Rust 的类型与资源安全；需要维护跨语言 Schema 生成，并在真实多节点需求出现时增加存储实现。

# Revisit triggers

可测量的 SQLite 竞争、独立故障域、组织所有权或远程执行需求成立时评审拆分。
