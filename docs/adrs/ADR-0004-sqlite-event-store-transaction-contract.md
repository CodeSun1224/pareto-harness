---
id: ADR-0004
title: 采用 Kernel 私有 SQLite append-only Event Store 合同
status: accepted
owners: [maintainers]
created: 2026-08-23
updated: 2026-08-23
links: [REQ-0004, SPEC-0003, RFC-0003, ADR-0002, ADR-0003]
---

# Context

REQ-0004 首次建立权威持久化。事务模式、幂等冲突优先级、sequence scope、SchemaSet/limits 身份、读取 horizon 与 migration 一旦写入历史并被状态机/Replay 消费就难以回退。独立架构评审发现并关闭了公共读写绕过、reader 身份替换、NULL uniqueness 与非稳定 rowid horizon 风险。

# Decision

接受 RFC-0003。Event Store 位于最小 `pareto-kernel` crate 内部；读写只接受不可公开构造且绑定认证上下文、target/scope、exact SchemaSetRef 与 ProtocolLimitsRef 的 crate-private admission。`ValidatedEvent` 不是 capability，外部消费者不能凭它或自报 scope 读写权威日志。

SQLite 使用 WAL、`synchronous=FULL`、有限 busy timeout、固定 application identity 和原子 checksum migration。events 表以显式不可更新的 `append_ordinal INTEGER PRIMARY KEY AUTOINCREMENT` 提供 opaque scan horizon，`event_id` 为 UNIQUE；完整隔离键内 sequence 连续唯一，可选 user 用 presence + 非 NULL 规范值避免 NULL uniqueness 绕过。完整 envelope、SchemaSet/limits 和索引身份一起持久化、进入幂等比较并在读取时重验。

相同 event_id 的 exact retry 优先返回幂等成功；同 ID 异内容和同 sequence 异事件分别明确冲突。固定 triggers 拒绝 UPDATE/DELETE；DB migration 与 Event Schema migration 分离，旧 reader/SchemaSet 保留。Run/Stream cursor 绑定 scope、query kind、SchemaSet 与不可变 ordinal horizon，受支持 migration 必须保序。

# Alternatives

- 公开 `append(ValidatedEvent, target)` 或 read-by-scope：允许调用方自我声明权限，拒绝。
- 隐式 rowid/offset cursor：并发、VACUUM 或 migration 下不稳定，拒绝。
- NULL user 列直接参与 UNIQUE：SQLite 会允许重复 NULL，拒绝。
- `INSERT OR IGNORE/REPLACE`、全局 DB ordinal 充当 Stream sequence、只存 JSON blob、同步 rusqlite 或首发 PostgreSQL/Kafka：分别破坏冲突语义、协议顺序、约束/无损映射或既有技术基线，拒绝。

# Consequences

获得可测试的事务、隔离、恢复、幂等和 Replay reader 身份；代价是 WAL/FULL 写入成本、SQLite 单 writer、完整 JSON/索引写放大以及 sqlx/Tokio 依赖。v1 sequence 限制为正 i64，超界 fail closed；不宣称完整 capability、状态机、Projection 或 Replay executor 已实现。

# Revisit triggers

可测量的 SQLite 竞争或 durability 成本超过目标；需要多节点/远程存储；必须改变 sequence scope、幂等优先级、cursor horizon、SchemaSet binding 或 append-only migration。触发后需新 Requirement/RFC/ADR、向前 migration、兼容/Replay/隔离负测和独立架构评审。
