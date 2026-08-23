---
id: REVIEW-0003
title: REQ-0004 SQLite append-only Event Store 独立代码评审
status: approved
owners: [maintainers]
created: 2026-08-23
updated: 2026-08-24
links: [REQ-0004, SPEC-0003, RFC-0003, ADR-0004]
independence: independent
reviewed_revision: b7cf277f4232515bebbe15d6a237654336b95271
open_blockers: 0
open_majors: 0
---

# Verdict

批准。第二次 focused independent re-review 的精确修复提交 `b7cf277f4232515bebbe15d6a237654336b95271` 有 0 个 open Blocker、0 个 open Major。F-005 已由 trusted authority SchemaSet/limits pins、registry exact resolution、retained set 跨重启读取、wrong-digest/alternate admitted-set substitution 负测，以及 RAII rollback、bounded busy 和跨重启同 event-id retry 证据关闭；本 reviewer 独立复跑 Focused/Core/doc/clippy/diff-check 均通过。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `crates/pareto-kernel/src/event_store.rs:82-183`; `crates/pareto-kernel/src/lib.rs` | 初审发现 admission 从 envelope/查询自报身份构造。修复新增私有 `KernelAuthority`，append 使用其独立 scope/actor/stream 做协议边界验证，read 只能经 authority + registry 构造；scope/actor/stream mismatch 负测及外部 compile-fail doctest 均通过。当前切片仍不宣称 REQ-0007 完整 capability。 | 增加独立于 envelope/查询请求的可信 principal/target 输入及不可绕过的私有 constructor，负测证明 scope/actor/stream 任一自报差异被拒绝，并以外部 compile-fail/API-surface 测试证明裸 `ValidatedEvent` 或 scope 无读写入口。 | closed |
| F-002 | Major | `crates/pareto-kernel/src/event_store.rs:185-198,415-460,513-530`; `crates/pareto-kernel/src/event_store/tests.rs:377-432` | 初审发现 cursor 未绑定 horizon/keyset position。修复的私有 opaque `Cursor` seal 覆盖 kind、query binding、horizon 和全部三个 position 字段；read 先验证 seal，逐字段篡改、scope binding、并发较小 stream、reopen/VACUUM 测试均通过。authority API 未公开，因此调用者不能重算或构造内部 seal。 | 将全部 cursor state 纳入可验证的 opaque encoding/MAC 或等价不可伪造表示；增加 horizon 与三个 position 字段逐项篡改、跨 scope/kind/schema-set、重启/VACUUM/并发 append 的负测。 | closed |
| F-003 | Major | `crates/pareto-kernel/src/event_store.rs:329-353,464-510,532-555`; `crates/pareto-kernel/src/event_store/tests.rs:707-746` | 初审发现 causation/correlation 与 stored bytes 漂移漏检。修复在 open/read 双向比较 causation/correlation，并在 open 重算 envelope/SchemaSet/limits 三组 fingerprints；逐项离线 drift 的真实文件负测均 fail closed。 | open 与逐行 read 对所有持久化身份列及三个 JSON/fingerprint 对进行完整双向校验；增加 causation、correlation、fingerprint、SchemaSet/limits JSON 与 digest 单独漂移的真实文件负测。 | closed |
| F-004 | Major | `crates/pareto-kernel/src/event_store.rs:208-354`; `crates/pareto-kernel/src/event_store/tests.rs:573-622` | 初审发现 store identity 未固定且 v1 接受 application id 0。修复区分只创建新库的 `open` 与必须携带 expected store id 的 `open_pinned`，v1 严格要求 application id；合法 store 换入、application id 清零和 metadata identity 漂移测试均拒绝。 | open 返回并由可信配置持久绑定 expected store identity（或提供等价不可替换机制），v1 非零 application id 严格匹配；真实文件负测覆盖换入另一合法 store、清零/篡改 application id、identity metadata 漂移和 reopen。 | closed |
| F-005 | Major | `crates/pareto-kernel/src/event_store.rs:82-190`; `crates/pareto-kernel/src/event_store/tests.rs:230-275,675-720,840-909`; `.agents/work/active/REQ-0004-sqlite-event-store/VALIDATION.md` | 第二轮 remediation 将 SchemaSetRef 与 limits pins 移入 private trusted authority；append 拒绝 supplied set/limits 与 pin 不同，read 只能让 retained registry 按 authority exact ref 解析。持久旧 set 在 reopen 后可读；missing/wrong digest 与另一个已 admitted set 替换均 fail closed。RAII transaction drop 保持原值、busy 有界分类、成功提交后跨重启同 event-id retry 返回 `AlreadyCommitted`，共同证明取消/commit-response uncertainty 的规定恢复路径。当前 15 个真实 SQLite tests 与 compile-fail、Core regression 已由 reviewer 独立复跑。 | 按 SPEC-0003 traceability 补齐旧 Schema fixture、persisted-identity-driven registry、missing/wrong/alternate-compatible reader 负测，以及 cancellation/commit-uncertainty 可行的确定性证据；更新 VALIDATION 并重跑 Focused/Impacted/Core/full gates。 | closed |

# Acceptance trace

| Acceptance | Review result | Evidence |
|---|---|---|
| AC-01 | 满足当前 v1 合同 | 新库、pinned reopen、newer version、trigger drift、失败 migration rollback、application id 清零及 store swap 均有真实文件证据；首版无已发布旧 DB migration，不虚构历史升级。 |
| AC-02 | 满足 | authority 与 envelope 身份独立，API compile-fail；envelope/SchemaSetRef/limits 保存并重算 fingerprints，显式身份双向重验。 |
| AC-03 | 部分满足 | 固定 UPDATE/DELETE trigger 真实执行且 drift 被 open 拒绝；尚无 migration bypass/drop 的完整负测。 |
| AC-04 | 满足核心合同 | `BEGIN IMMEDIATE`、完整 non-NULL user key、sequence UNIQUE；两个独立 store/pool 竞争产生一个 commit、一个稳定 SequenceConflict，最终一行。user-present 隔离由 authority scope matrix 覆盖。 |
| AC-05 | 部分满足 | exact envelope/schema/limits fingerprints 支持 retry/conflict；测试只改变 correlation，未覆盖 scope/run/stream/sequence 与同 sequence 异 event 的完整矩阵。 |
| AC-06 | 满足 | 事务 RAII drop rollback、causation failure、fresh connection visibility、bounded busy 与 stable I/O 分类有真实证据；跨重启 identical event-id retry 返回 `AlreadyCommitted`，证明 commit response 不确定时的规定恢复路径。 |
| AC-07 | 满足 | 私有 authority reader、稳定排序、ordinal horizon 与 sealed opaque cursor 已覆盖篡改、scope mixing、页间 append、reopen/VACUUM。 |
| AC-08 | 满足主要合同 | tenant/user/workspace/run/agent/stream authority mismatch、payload shadow、causation rollback，以及所有持久 JSON identity/fingerprint drift 已覆盖。 |
| AC-09 | 满足主要 E2E | tempfile 真实 SQLite 覆盖 create/append/read/fresh connection/reopen/continue、幂等、sequence、两个 store 竞争、失败 rollback、busy 和 VACUUM。 |
| AC-10 | 满足 | authority 固定 exact SchemaSetRef/limits，retained registry 按 pin 解析；持久旧 set 跨重启可读，missing/wrong digest 与 alternate admitted set 替换均拒绝，stored row 再次要求 exact JSON/fingerprint 相等。DB migration 未改写 Event JSON。 |
| AC-11 | 满足现有原始记录 | VALIDATION 记录 protocol 9 unit + 17 contract、workspace tests 均通过；commit 依赖方向仍是 kernel -> protocol，protocol 未新增 SQLite/runtime/network 依赖。 |

# Compatibility, permission, and isolation review

- 依赖方向正确，`pareto-protocol` 未反向依赖 Kernel/SQLite；DB schema 与 Event Schema 数据列分离。
- 所有 Event Store 类型当前 module-private，外部 compile-fail 通过；私有 `KernelAuthority` 已把 authority target 与 envelope/query claim 分离，F-001 关闭。
- optional user 采用 `(user_present,user_id)` 非 NULL 规范编码并进入 sequence/query key，避免 SQLite NULL-distinct 绕过。
- Run/Stream SQL 均绑定 tenant、user presence/value、workspace、run、agent；payload 没有参与这些 query predicates。
- persisted reader identity 由 trusted authority pin 固定，retained registry 仅 exact resolve；读取再与行内 persisted SchemaSet/limits identity 双向比较。missing、wrong digest 与 alternate admitted set substitution 均有负测。
- causation append 要求同完整 scope/run/stream 已存在，失败在事务内 rollback；causation/correlation 及所有 stored byte fingerprints 漂移现已 fail closed。

# Regression and test review

- 更新 VALIDATION 记录 Focused 15 tests、doc compile-fail、Impacted、Core、governance、clippy 与 diff-check；其中 Core 行的 kernel 14 计数是第二轮 remediation 前的陈旧记录，本 reviewer 已在精确提交独立复跑并确认 kernel 15 与全 workspace 通过。
- 本 reviewer 第二轮独立复跑 `cargo test -p pareto-kernel --all-targets --all-features --offline`（15/15）、`cargo test -p pareto-kernel --doc --offline`（1/1）、kernel clippy `-D warnings`、`cargo test --workspace --all-targets --all-features --offline`（kernel 15、protocol 9+17，observation 1 ignored）与修复范围 diff-check，全部通过。
- crate 测试继续使用真实临时 SQLite 文件；remediation 增加 authority/isolation、migration rollback、identity swap、cursor field tamper、两个 pool 竞争、transaction drop/busy/fresh visibility 和 stored-byte drift。
- 本 review 不以 closure 声明替代测试断言；第二轮源码断言与 reviewer 独立执行已补齐 retained old set、wrong/alternate registry、authority-pinned reader，以及 RAII/commit-uncertainty retry 证据，F-005 关闭。
- 锁定的 `sqlx 0.8.6`、Tokio、SHA-256 与 tempfile 属于 approved SPEC/RFC 已识别依赖；未发现 protocol dependency growth 或网络/provider 调用。

# Scope and unrelated changes

精确提交的产品代码集中于新增 `pareto-kernel::event_store`、workspace dependency/lockfile 和 REQ-0004 设计/工作记录；未发现 Runtime、状态机、Projection、Replay executor、CLI 或通用 SQL escape hatch。`Cargo.lock` 大幅增长与 sqlx/Tokio 传递依赖一致。提交同时包含实施前设计记录，但均属于 REQ-0004 范围；未发现明显无关产品行为修改。

发布前 rollback 可 revert 新 crate/workspace member；若数据库已被使用，必须保留 v1 reader/migration 且不得降 `user_version` 或改写历史。本 review 已批准精确提交，后续仍需完成仓库 completion gates、durable docs 同步与归档。

# Re-review history

- 2026-08-23：fresh-agent 独立评审精确提交 `9aab8614a7083958659f5969f2caec38b7c597cb`；0 Blocker、5 Major，结论 changes requested。
- 2026-08-23：focused independent re-review 精确提交 `a7c73b317c57813a2a1d29b35ef09a2773c6e17b` 及 `9aab861..a7c73b3` 修复 diff；独立复跑 14 kernel tests、1 compile-fail doctest、kernel clippy 与 diff-check。F-001/F-002/F-003/F-004 closed；F-005 保持 open。最终 0 Blocker、1 Major，仍 changes requested。
- 2026-08-24：第二次 focused independent re-review 精确提交 `b7cf277f4232515bebbe15d6a237654336b95271` 及 `a7c73b3..b7cf277` 修复 diff；独立复跑 15 kernel tests、1 compile-fail doctest、kernel clippy、全 workspace tests 与 diff-check。F-005 closed；最终 0 Blocker、0 Major，批准。
