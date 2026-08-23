---
id: RFC-0003
title: SQLite Event Store 事务、幂等与读取合同
status: accepted
owners: [maintainers]
created: 2026-08-23
updated: 2026-08-23
links: [REQ-0004, SPEC-0003, EPIC-0002, REQ-0003, RFC-0002, ADR-0002, ADR-0003, ADR-0004]
---

# Summary

以 SQLite WAL/sqlx 实现单机 append-only 权威事件日志。每次 append 使用立即写事务，将完整已验证 envelope 与隔离索引一并插入；相同 event_id 的精确重试幂等成功，任何内容复用冲突；Stream sequence 在完整隔离键内严格连续。读取使用 keyset cursor，并重新通过固定 SchemaSet 的 `pareto-protocol` 验证。

# Motivation and requirements

REQ-0004 必须同时解决进程崩溃、多连接争用、隔离、旧 Schema 和后续 Replay 接口。事务起点、冲突优先级、durability、数据库与事件 migration 分界一旦被持久数据和消费者依赖就难以回退，因此需在实现前冻结。

# Proposed design

1. 新建最小 `pareto-kernel` crate，首片只有可运行 Event Store 纵切。Store、连接池、append 与 `AdmittedAppend` constructor 都是 crate-private；公共 API 不暴露写权威状态的方法。crate 依赖 `pareto-protocol`、sqlx/Tokio 和必要错误库；protocol 不反向依赖。REQ-0007 将在同一私有 admission 点增加完整 capability/delegation。
2. `open` 配置 `application_id`、`journal_mode=WAL`、`synchronous=FULL`、`foreign_keys=ON`、`trusted_schema=OFF` 和有限 `busy_timeout`。migration 独占受控连接：`BEGIN EXCLUSIVE` 后校验 application_id/user_version，按固定 SQL+SHA-256 checksum 逐版本执行，写 `schema_migrations(version, checksum, applied_at_explicit)`，最后设置 user_version 并 commit；任何一步失败 rollback。未知 newer version、identity/checksum 错误或 `quick_check` failure fail closed。
3. v1 `events` 使用显式 `append_ordinal INTEGER PRIMARY KEY AUTOINCREMENT`，event_id 另设 UNIQUE；ordinal 只增不复用、不得更新，事务 rollback 可留下 gap但不影响事件 sequence。到 `i64::MAX` 后 fail closed。表同时保存完整规范 envelope JSON、envelope fingerprint、append 时 exact SchemaSetRef/ProtocolLimitsRef 的规范 JSON和 fingerprint、完整 isolation columns、run/stream/sequence 与 causation/correlation；它们全部进入幂等 exact-match。协议 sequence 的 v1 范围限制为正 `i64`，超出返回稳定 `sequence_out_of_range`，未来 migration 不得重释旧值。
4. `user_present INTEGER NOT NULL CHECK (user_present IN (0,1))` 与 `user_id TEXT NOT NULL` 使用规范编码：缺省只能是 `(0,'')`，存在只能是 `(1,'user_...')`，CHECK 禁止其他组合。唯一键是完整 `(tenant,user_present,user_id,workspace,run,agent,stream,sequence_i64)`，因此不受 SQLite NULL-distinct 语义影响。`append_ordinal` 是唯一 PRIMARY KEY，`event_id TEXT NOT NULL UNIQUE`；append 先 `BEGIN IMMEDIATE`，先按 event_id 查精确 fingerprint：相同返回 AlreadyCommitted，不同返回 IdempotencyConflict；否则读取该 stream 最大 sequence，要求请求恰为 next，再插入并 commit。
5. 幂等检查优先于 sequence 检查，使已成功但响应丢失的旧 sequence 重试可确定成功；event_id 不同却复用 sequence 始终 SequenceConflict。
6. v1 使用 `BEFORE UPDATE/DELETE ON events` triggers 执行 `RAISE(ABORT, 'append_only')`；普通连接不暴露 raw pool/SQL且每次 open 校验 trigger SQL/checksum。migration 连接不得 drop/bypass trigger 或修改 events；未来确需物理迁移必须新 RFC。直接离线篡改文件不由 trigger 防止，但 open/read 的 identity/checksum、quick_check、row fingerprint、索引/envelope/validation identity 比对负责检测并 fail closed，不声称抗拥有文件写权限的攻击者。
7. 读取也只接受 crate-private `AdmittedRead`，外部自报 scope 无法调用。首次 scan 在读事务中获取当前最大 append ordinal 作为 immutable horizon；opaque Stream cursor 为 `(horizon,last_sequence,event_id)`，Run cursor 为 `(horizon,last_stream_id,last_sequence,event_id)`，并绑定 scope/query kind 与 persisted SchemaSetRef fingerprint。所有页限定 `append_ordinal <= horizon`；close/reopen、VACUUM 与任何受支持 migration 必须保留 ordinal，页间新追加不漏既有行也不混入新行。trusted Kernel registry 按 persisted exact SchemaSetRef/limits 恢复 reader；missing/wrong/alternate set 均拒绝，调用者不能替换。
8. causation 若存在，append 事务中要求原因 event 已在同一完整 scope/run/stream 可见；首片不允许跨 stream causation，避免未定义的跨流授权/排序。correlation 只是不可授权的查询外元数据。
9. `EventStoreError` 稳定分类 migration/database-corrupt/unknown-schema/protocol-invalid/isolation-conflict/idempotency-conflict/sequence-conflict/causation-conflict/busy/io；不回显 payload、SQL 或秘密。

# Interfaces, data flow, and invariants

`append(AdmittedAppend)` 与 `read(AdmittedRead)` 均为 crate-private；两种 admission 只能在同 crate 的 Kernel admission 中由认证上下文、exact target/scope、SchemaSetRef/limits 共同构造。外部仅持有 ValidatedEvent 或已知 scope 无法读写权威日志。Kernel registry 按 cursor/event 固定 identity 选择 SchemaSet，不接受替换；返回 validated events 与绑定 explicit append ordinal horizon 的 opaque cursor。

不变量：验证先于状态变化；幂等结果由权威行精确比较；sequence 无 gap；失败不留部分行；事件不可更新/删除；索引不能覆盖 envelope；未知 Schema 不降级；Replay 只按已固定 SchemaSet 解释历史。

# Failure modes and security

- writer 竞争：有限等待；获锁者提交，落后者重查幂等或返回 sequence conflict/busy，不自动改 sequence。
- commit 响应丢失：调用者用同一 event_id 重试；禁止生成新 ID 猜测结果。
- 进程崩溃：SQLite rollback/WAL recovery 保证未提交不可见；reopen 做版本/identity 检查。介质损坏返回 corrupt，不声称自动修复。
- migration 中断：事务回滚；migration 不混入业务 append。未知新版本拒绝旧二进制写入。
- payload shadowing/row tamper：索引来自 envelope 非 payload；读取双向比对并协议重验。
- 权限：Validated 不代表 capability；当前公共 API 完全不开放 append，crate-private admission 是未来 capability 的唯一插入点。REQ-0007 前不得把此机制宣称为完整授权。路径不能来自 event/payload。
- cancellation：commit 前取消 rollback；进入 commit 后以 event_id 重试判定结果，不把不确定性当失败重放新事件。
- cursor 重放/混用：cursor 安全摘要绑定 scope、query kind、SchemaSet 与 horizon；跨 scope/Run/Stream 或被篡改 cursor fail closed。

# Alternatives considered

1. 单独 writer actor/channel：可减少 busy，但把跨进程/多连接语义藏在进程内且增加取消恢复状态；首片拒绝，SQLite 事务仍是事实来源。
2. `INSERT OR IGNORE/REPLACE`：代码短但会吞掉 idempotency/sequence 冲突或覆盖历史；拒绝。
3. 把数据库自增值当 Event sequence：不能表达每 Stream 声明的协议 sequence；拒绝。仅使用显式 append ordinal 作为 opaque scan horizon，它不是事件身份且不暴露给 Replay 业务语义。
4. 只存 JSON blob：无法由数据库约束隔离/顺序且篡改难发现；拒绝。只存拆列则可能丢失未知兼容字段；采用 blob + 最小权威索引。
5. `synchronous=NORMAL`：吞吐更高但掉电 durability 较弱；首版质量优先选 FULL，基线后再以独立 Requirement 评估。
6. rusqlite 同步实现：依赖较轻，但偏离 ADR-0002 的 sqlx/Tokio 基线；拒绝。
7. PostgreSQL/Kafka：当前没有多节点证据，增加运维；按 ADR-0002 延后。

# Compatibility, migration, and rollback

DB v1 与 Event Schema 1.x 独立。DB migration 只改变物理表示/索引并保留原 envelope；Event major/minor 由 SchemaSet reader 管理。旧 SchemaSet 只要仍被历史引用就保留。发布后不可向下迁移或删事件；回滚 writer 时保留兼容 reader。破坏公开 cursor、幂等优先级、sequence scope、durability 或 envelope 存储语义需新 Requirement/RFC/ADR 与迁移/Replay 证据。

# Evaluation and acceptance

- 质量：真实 SQLite 文件覆盖 migration、append-only、无损 round-trip、并发、crash、幂等、隔离、旧 Schema 与重启。
- Token/费用：无模型或 Provider，记录不适用。
- 延迟：记录单写、顺序读、竞争 busy 基线；busy timeout 有限，不设无证据优化阈值。
- 批准：独立架构评审检查权限链、事务/恢复、Replay、隔离和后续消费者；Blocker/Major 清零后接受并创建 ADR-0004。
