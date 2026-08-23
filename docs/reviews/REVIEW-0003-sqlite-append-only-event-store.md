---
id: REVIEW-0003
title: REQ-0004 SQLite append-only Event Store 独立代码评审
status: changes-requested
owners: [maintainers]
created: 2026-08-23
updated: 2026-08-23
links: [REQ-0004, SPEC-0003, RFC-0003, ADR-0004]
independence: independent
reviewed_revision: 9aab8614a7083958659f5969f2caec38b7c597cb
open_blockers: 0
open_majors: 5
---

# Verdict

不批准。精确提交 `9aab8614a7083958659f5969f2caec38b7c597cb` 有 0 个 open Blocker、5 个 open Major。实现建立了真实 SQLite 纵向切片，但尚未满足 approved SPEC-0003 的 authority admission、cursor integrity、索引/envelope 一致性、store identity 和分层验收证据合同。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `crates/pareto-kernel/src/event_store.rs:82-104` | `AdmittedAppend::admit` 只用 envelope 自带的 scope、actor、stream 再验证一次，未输入或比较“认证主体派生的 target”；`AdmittedRead` 也只是可直接填字段的私有结构。违反 SPEC-0003 输入合同和 AC-02/AC-07：当前 crate-private 可见性阻止外部 crate 调用，但没有建立 approved design 要求的最小 Kernel authority admission，未来同 crate consumer 仍能以自报 target 构造权威请求。 | 增加独立于 envelope/查询请求的可信 principal/target 输入及不可绕过的私有 constructor，负测证明 scope/actor/stream 任一自报差异被拒绝，并以外部 compile-fail/API-surface 测试证明裸 `ValidatedEvent` 或 scope 无读写入口。 | open |
| F-002 | Major | `crates/pareto-kernel/src/event_store.rs:107-115,271-333,378-380` | cursor 的 `binding` 只摘要 query/scope/schema/limits，不包含 `horizon`、`last_stream`、`last_sequence`、`last_event`；后续页无条件信任这些可变字段。违反 RFC-0003 cursor 安全摘要绑定 horizon 与 opaque keyset position、AC-07/AC-08 的 tamper fail-closed 合同。改小/改大 horizon 或跳改位置可截断、跳过或扩大分页结果；现有测试仅篡改 `binding`，没有篡改被信任字段。 | 将全部 cursor state 纳入可验证的 opaque encoding/MAC 或等价不可伪造表示；增加 horizon 与三个 position 字段逐项篡改、跨 scope/kind/schema-set、重启/VACUUM/并发 append 的负测。 | open |
| F-003 | Major | `crates/pareto-kernel/src/event_store.rs:202-218,336-376` | open/read 一致性校验遗漏持久化的 `causation_id`、`correlation_id`；open 也未重算 envelope/schema-set/limits fingerprints。离线删除 trigger 后可只漂移 causation/correlation 列并恢复 trigger，open 和 read 都会接受该行。违反 AC-02/AC-08 及 RFC-0003 “索引/envelope/validation identity 双向比对并 fail closed”；这会使未来按这些显式身份消费的 projection/replay 看到与权威 envelope 不一致的数据。 | open 与逐行 read 对所有持久化身份列及三个 JSON/fingerprint 对进行完整双向校验；增加 causation、correlation、fingerprint、SchemaSet/limits JSON 与 digest 单独漂移的真实文件负测。 | open |
| F-004 | Major | `crates/pareto-kernel/src/event_store.rs:147-226` | store identity 只验证任意一行 32 字符 `store_id` 存在，从未由配置/调用方固定并在 reopen 比对；此外 `user_version=1, application_id=0` 会被接受。因而另一个结构合法的 Event Store 文件或清掉 application id 的 v1 文件可在同一路径被静默打开，不能实现 SPEC/RFC 所述 immutable identity 防换库及未知数据库 fail-closed。 | open 返回并由可信配置持久绑定 expected store identity（或提供等价不可替换机制），v1 非零 application id 严格匹配；真实文件负测覆盖换入另一合法 store、清零/篡改 application id、identity metadata 漂移和 reopen。 | open |
| F-005 | Major | `crates/pareto-kernel/src/event_store/tests.rs`; `.agents/work/active/REQ-0004-sqlite-event-store/VALIDATION.md` | 7 个 crate 内测试不足以支撑逐 AC 验证：没有 external compile-fail read/write；没有失败 migration rollback fixture；没有未提交事务/drop、成功后独立新连接可见、busy/I/O 分类；没有 tenant/user value/workspace/run/agent/payload-shadow 隔离矩阵；没有 registry missing/wrong digest/alternate compatible SchemaSet/旧 fixture；竞争测试共用单 pool 且只断言一个成功，不断言另一个稳定分类或最终行。VALIDATION 的覆盖声明超出原始测试证据，AC-01/02/06/08/10 未被 planned test matrix 证明。 | 按 SPEC-0003 traceability 补齐真实 SQLite、API-surface、隔离、安全、恢复、兼容与稳定错误分类测试；更新 VALIDATION 为逐命令原始结果，不把未执行场景记为已覆盖，并重跑 Focused/Impacted/Core/full gates。 | open |

# Acceptance trace

| Acceptance | Review result | Evidence |
|---|---|---|
| AC-01 | 不满足 | 新库、重复 open、newer version 与 trigger drift 有测试；无失败 migration rollback；application id 清零的 v1 库被接受，store identity 未绑定。见 F-004/F-005。 |
| AC-02 | 不满足 | envelope/SchemaSetRef/limits 被存储且 append 类型不公开，但 admission 使用自报 target；缺 external API test，持久身份校验不完整。见 F-001/F-003/F-005。 |
| AC-03 | 部分满足 | 固定 UPDATE/DELETE trigger 真实执行且 drift 被 open 拒绝；尚无 migration bypass/drop 的完整负测。 |
| AC-04 | 部分满足 | `BEGIN IMMEDIATE`、完整 non-NULL user key、sequence UNIQUE 与一次竞争测试支持核心实现；缺多个 pool/连接、user-present value 和稳定 loser 结果证据。 |
| AC-05 | 部分满足 | exact envelope/schema/limits fingerprints 支持 retry/conflict；测试只改变 correlation，未覆盖 scope/run/stream/sequence 与同 sequence 异 event 的完整矩阵。 |
| AC-06 | 不满足证据门禁 | append 错误路径显式 rollback，但未测试未提交 drop/cancel、成功后新连接可见、busy/I/O 或 commit 不确定性。见 F-005。 |
| AC-07 | 不满足 | 查询排序和 append ordinal horizon 已实现，但 cursor state 可篡改且 admission 未绑定 authority。见 F-001/F-002。 |
| AC-08 | 不满足 | query predicate 使用完整 scope，然而 causation/correlation 索引漂移不被发现，隔离负测矩阵缺失。见 F-003/F-005。 |
| AC-09 | 部分满足 | tempfile 真实 SQLite 覆盖 open/append/read/reopen、sequence、幂等、竞争和 VACUUM；失败原子性与独立新连接证据不足。 |
| AC-10 | 不满足证据门禁 | 没有旧 Schema fixture 或 registry missing/wrong/alternate set 测试；当前 read 仅由调用方直接提供 SchemaSet，而非由 persisted identity 驱动 registry 恢复。见 F-005。 |
| AC-11 | 满足现有原始记录 | VALIDATION 记录 protocol 9 unit + 17 contract、workspace tests 均通过；commit 依赖方向仍是 kernel -> protocol，protocol 未新增 SQLite/runtime/network 依赖。 |

# Compatibility, permission, and isolation review

- 依赖方向正确，`pareto-protocol` 未反向依赖 Kernel/SQLite；DB schema 与 Event Schema 数据列分离。
- 所有 Event Store 类型当前 module-private，外部 crate 无直接 API；但私有可见性不等于 approved authority binding，见 F-001。
- optional user 采用 `(user_present,user_id)` 非 NULL 规范编码并进入 sequence/query key，避免 SQLite NULL-distinct 绕过。
- Run/Stream SQL 均绑定 tenant、user presence/value、workspace、run、agent；payload 没有参与这些 query predicates。
- persisted reader identity 目前由调用者传入 SchemaSet 后与行比较，不是 RFC 所述 Kernel registry 按 persisted exact identity 恢复；相关 compatibility negative evidence 缺失。
- causation append 要求同完整 scope/run/stream 已存在，失败在事务内 rollback；但持久化 causation/correlation 漂移未 fail closed，见 F-003。

# Regression and test review

- 原始 VALIDATION 记录 Focused、Impacted、Core、governance、clippy 与 diff-check 通过；性能数字明确标为 observation，不是收益声明，Token/Provider cost 记为不适用。
- crate 测试使用真实临时 SQLite 文件而非 mock，覆盖的 7 个场景本身与代码一致。
- 测试函数未按 PLAN 中的 migration/append/atomicity/isolation/compatibility scopes 分组；更重要的是多项 planned scenarios 未实现，见 F-005。
- 本 review 不以工作记录的“覆盖”叙述替代源码测试断言；未发现 cancellation/timeout、旧 schema reader 或 external API surface 的原始证据。
- 锁定的 `sqlx 0.8.6`、Tokio、SHA-256 与 tempfile 属于 approved SPEC/RFC 已识别依赖；未发现 protocol dependency growth 或网络/provider 调用。

# Scope and unrelated changes

精确提交的产品代码集中于新增 `pareto-kernel::event_store`、workspace dependency/lockfile 和 REQ-0004 设计/工作记录；未发现 Runtime、状态机、Projection、Replay executor、CLI 或通用 SQL escape hatch。`Cargo.lock` 大幅增长与 sqlx/Tokio 传递依赖一致。提交同时包含实施前设计记录，但均属于 REQ-0004 范围；未发现明显无关产品行为修改。

发布前 rollback 可 revert 新 crate/workspace member；若数据库已被使用，必须保留 v1 reader/migration 且不得降 `user_version` 或改写历史。当前 review 不批准进入 verified/done，因此尚不应执行发布后 rollback 声明或归档。

# Re-review history

- 2026-08-23：fresh-agent 独立评审精确提交 `9aab8614a7083958659f5969f2caec38b7c597cb`；0 Blocker、5 Major，结论 changes requested。
