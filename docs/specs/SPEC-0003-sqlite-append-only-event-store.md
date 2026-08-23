---
id: SPEC-0003
title: SQLite append-only Event Store 规范
status: approved
owners: [maintainers]
created: 2026-08-23
updated: 2026-08-23
links: [REQ-0004, EPIC-0002, REQ-0003, RFC-0003, ADR-0002, ADR-0003, ADR-0004]
---

# Behavioral contract

Event Store 是可信内核的权威 append 边界，不是协议或授权替代物。调用方先通过 `pareto-protocol` 的公开边界获得不可伪造的 validated Event，再由 Kernel 完成当前需求范围内的目标 scope/stream admission，Event Store 才在单个 SQLite 事务中判定幂等、连续序号和提交。读取返回重新通过固定 SchemaSet 协议验证的 Event，不把数据库行本身视为可信类型。

# Inputs, outputs, states, and failure behavior

- 输入：Kernel 私有 admission（包含 `ValidatedEvent`、认证主体派生的 target、exact SchemaSetRef 与 ProtocolLimitsRef）、数据库路径/连接配置、显式 migration/open 请求，以及查询所需完整 scope/Stream 或 Run 身份；不接受裸 `EventEnvelope`、仅自报 target 的 ValidatedEvent 或任意 SQL。
- 输出：`Appended { event_id, sequence }`、`AlreadyCommitted`、按稳定游标排序的 validated Event，或不包含 payload 的结构化 `EventStoreError`。
- 状态：SQLite 文件拥有不可变 store identity、`user_version` migration 版本和 append-only `events` 表；WAL/sidecar 是 SQLite 事务机制，不是独立权威记录。
- 失败：未知数据库版本、损坏、未知 Schema、协议重验证失败、scope/index 漂移、序号 gap/reuse、event-id 内容冲突、busy timeout 与 I/O 错误均 fail closed。事务提交前失败必须 rollback；提交结果不确定时只能用同一 event_id 重试并查询幂等结果。

调用路径：

```text
untrusted EventEnvelope
  -> admitted SchemaSet + pareto-protocol exact validation
  -> ValidatedEvent (opaque)
  -> trusted-kernel private admission (principal/target/schema-set/limits bound)
  -> EventStore append transaction (kernel-private)
     -> idempotency exact-match check
     -> next sequence check under SQLite write lock
     -> insert immutable envelope + identity indexes
     -> commit
  -> stream/run reader
     -> index/envelope consistency check
     -> exact SchemaSet protocol validation
  -> future state machine / Projection / Replay
```

# Impact analysis

| Dimension | Finding | Evidence / response |
|---|---|---|
| Direct | 新增单个最小可运行 `pareto-kernel` crate 及其内部 `event_store` module、SQLite migration、映射/错误/API 与真实数据库测试；workspace/lockfile 增加 SQLite async 依赖 | `Cargo.toml` 当前只有 `pareto-protocol`；`ADR-0002`/`ARCH-0004` 已选 Rust + SQLite WAL + sqlx；不得创建空或独立 Event Store crate，不得修改 protocol 的 DB 独立性 |
| Indirect | REQ-0005 状态机消费 Stream 顺序，REQ-0006 Projection/Replay 消费 Run/Stream 游标，REQ-0008 Hook 与 REQ-0009 Effect 依赖不可变事件；备份/运维也依赖 WAL checkpoint 语义 | `docs/roadmap/requirement-backlog.md` 依赖图和 EPIC-0002 exit criteria；Kernel-private reader 合同保持业务无关且稳定 |
| Call/permission | 协议验证不等于授权；仅 `ValidatedEvent + AppendTarget` 仍可由任意 consumer 自报。本切片把 store 与最小 admission 放在同一 trusted-kernel crate，append 和 admission constructor 均为 crate-private；公共消费者不能写权威状态 | `RFC-0002` §6 与 `ADR-0003`；未来 REQ-0007 扩展完整 capability/delegation，不能把当前私有边界解释为已实现通用授权 |
| Data isolation | 索引至少绑定 tenant/user presence+value/workspace/run/agent/stream；SQLite UNIQUE 的 NULL 不冲突，因此 user 缺省必须用 `user_present=0,user_id=''` 的非 NULL 规范编码并由 CHECK 约束 | `IsolationScope`/REQ-0003 隔离负例；数据库约束、映射重验与查询 API 三层防护，覆盖 `user_id=None` 并发 |
| API/schema | 新公开存储 API、错误分类和游标会成为 REQ-0005/0006 依赖；不得复制协议类型或泄漏 sqlx 类型 | crate 依赖方向 Event Store -> protocol；外部 API 只使用 protocol ID/scope/validated 类型和 store 自有结果/错误 |
| Persistence/replay | 权威 JSON、append 时 exact SchemaSetRef 和 ProtocolLimitsRef 必须完整保留并进入幂等 fingerprint；索引仅用于约束/查询，读取时检查索引等于 envelope/validation identity | ADR-0003 保留旧 Schema/reader；read cursor 绑定 persisted SchemaSet，不允许调用者替换，旧事件不重写 |
| Concurrency | SQLite 单 writer；deferred transaction 会产生 TOCTOU。使用立即写事务获得写保留锁，再检查幂等/next sequence；unique constraints 是最后防线 | RFC-0003；并发多 pool/连接 barrier 测试同一 sequence，busy timeout 有界并分类 |
| Security | 路径、恶意数据库、payload shadowing、错误回显、SQL 注入与 trigger 绕过是主要风险 | 路径由 Kernel 配置而非 payload；读写均需不可公开构造的 Kernel admission；静态 SQL；固定 UPDATE/DELETE triggers 及其 SQL/checksum 在 open 时校验 |
| Transactions/recovery | migration 和 append 必须原子；Run 分页若只按业务 key，在页间插入较小 stream 会漏读 | WAL/FULL + finite busy timeout；首次页固定 `append_ordinal <= horizon`，cursor 绑定 scope/schema-set/horizon，后页验证绑定；未提交 drop/reopen 与并发插入较小 stream 测试 |
| Migration | 首版 schema v1；open 在 exclusive migration transaction 中按 `user_version` 逐步升级，未知 newer version 拒绝；store identity 防止换库 | migration 表记录版本/校验和；失败回滚。首版没有历史升级，不伪称已有迁移兼容证据 |
| Performance | 完整 JSON +隔离索引增加写放大；WAL/FULL 降低峰值吞吐；读取重验证增加 CPU | 首版质量/恢复优先；只建 Stream/Run 读取所需索引并记录基线，达到 ADR-0002 revisit trigger 再改后端 |
| Dependency/operations | sqlx/Tokio 增加供应链、编译时间和 SQLite native/bundled 风险；WAL 备份必须连同 SQLite 机制处理 | 锁定版本、offline gates、许可证检查；不手工复制活动 DB 文件；未来运维 API另立 Requirement |
| Documentation | README、index、EPIC、ARCH-0003/0004 与 backlog 状态需区分已实现和后续能力 | 完成时同步；不宣称状态机/Replay 已实现 |
| Rollback | 发布前可 revert；发布后代码回滚必须保留 v1 reader/migration，不能降 `user_version` 或删除事件 | 停止 writer、保留 reader；修复以向前 migration 或新 Event 进行，不原地改历史 |

# Compatibility and migration

数据库 schema 版本与 Event SchemaRef 是两条独立轴。v1 行保存完整规范 JSON、exact SchemaSet/limits 和显式索引；重启后 trusted Kernel registry 只能按 persisted exact SchemaSetRef 恢复已 admission SchemaSet/decoder，缺失、wrong digest 或另一个兼容 set 均 fail closed，调用者不能提供替代 reader。增加索引/表使用保持 append ordinal 的向前 DB migration；改变事件语义使用新 Event Schema/reader，绝不重写旧行。

# Test traceability

| Acceptance | Scope/layer | Scenario | Planned evidence |
|---|---|---|---|
| AC-01 | Focused/integration/migration | 新库 v1、重复 open、未知 newer version、故障 migration rollback | `cargo test -p pareto-kernel event_store::migration` |
| AC-02 | Focused contract/security | 外部 compile-fail 证明 ValidatedEvent+自报 target 无 append API；kernel-private admission 可完成 round-trip；SchemaSet/limits 替换拒绝 | compile-fail/API surface test；`cargo test -p pareto-kernel event_store::protocol_contract` |
| AC-03 | Core security/integration | UPDATE/DELETE 被固定 trigger 拒绝；open 检测 trigger SQL/checksum drift；migration 不可 drop/bypass | `cargo test -p pareto-kernel event_store::append_only` |
| AC-04 | Focused/Core concurrency | 首序号、gap/reuse、两个连接竞争相同 next sequence（含 user=None），仅一项提交 | `cargo test -p pareto-kernel event_store::sequence`; `cargo test -p pareto-kernel event_store::concurrent` |
| AC-05 | Focused/integration | identical retry 返回 AlreadyCommitted；event_id 内容/scope/sequence 复用冲突 | `cargo test -p pareto-kernel event_store::idempotency` |
| AC-06 | Core crash/recovery | rollback/drop 未提交事务无行；成功后新连接可见；busy/I/O 分类 | `cargo test -p pareto-kernel event_store::atomicity` |
| AC-07 | Impacted contract/integration | 外部自报 scope 无 read API；页间插入较小 stream；显式 ordinal horizon 在 reopen/VACUUM/受支持 migration 后结果一致 | compile-fail/API surface test；`cargo test -p pareto-kernel event_store::reading` |
| AC-08 | Core isolation/security | tenant/user/workspace/run/agent/stream mismatch、payload shadow、causation 跨 scope、row drift 拒绝 | `cargo test -p pareto-kernel event_store::isolation` |
| AC-09 | Focused/Core E2E | 临时真实文件 append-read-close-reopen-continue | `cargo test -p pareto-kernel event_store::sqlite_integration` |
| AC-10 | Impacted compatibility/replay | reopen 时 registry missing/wrong digest/alternate compatible set 均拒绝；exact 旧 set 可读；DB migration 不改 Event JSON | `cargo test -p pareto-kernel event_store::compatibility` |
| AC-11 | Impacted/Core regression | REQ-0003 全套与 Schema byte identity；全仓门禁 | `cargo test -p pareto-protocol --all-targets --all-features --offline`; completion gates |

# Approval

独立架构评审经四次 focused re-review 关闭全部 Blocker/Major；RFC-0003 已接受，ADR-0004 记录 durable decision。本 Spec 于 2026-08-23 批准，实施必须按 Plan、Tasks、分层测试与独立 Code Review 推进。
