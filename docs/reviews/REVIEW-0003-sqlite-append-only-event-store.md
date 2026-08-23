---
id: REVIEW-0003
title: REQ-0004 SQLite append-only Event Store 独立代码评审
status: changes-requested
owners: [maintainers]
created: 2026-08-23
updated: 2026-08-23
links: [REQ-0004, SPEC-0003, RFC-0003, ADR-0004]
independence: independent
reviewed_revision: a7c73b317c57813a2a1d29b35ef09a2773c6e17b
open_blockers: 0
open_majors: 1
---

# Verdict

不批准。focused re-review 的精确修复提交 `a7c73b317c57813a2a1d29b35ef09a2773c6e17b` 有 0 个 open Blocker、1 个 open Major。F-001 至 F-004 已由代码、真实 SQLite 负测和本 reviewer 独立复跑关闭；F-005 仍 open，因为 AC-10 的旧 Schema fixture、wrong-digest/alternate-compatible registry 与 persisted-identity reader 证据仍不存在，其他部分验收矩阵也未完全达到 approved SPEC-0003 的 planned proof。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `crates/pareto-kernel/src/event_store.rs:82-183`; `crates/pareto-kernel/src/lib.rs` | 初审发现 admission 从 envelope/查询自报身份构造。修复新增私有 `KernelAuthority`，append 使用其独立 scope/actor/stream 做协议边界验证，read 只能经 authority + registry 构造；scope/actor/stream mismatch 负测及外部 compile-fail doctest 均通过。当前切片仍不宣称 REQ-0007 完整 capability。 | 增加独立于 envelope/查询请求的可信 principal/target 输入及不可绕过的私有 constructor，负测证明 scope/actor/stream 任一自报差异被拒绝，并以外部 compile-fail/API-surface 测试证明裸 `ValidatedEvent` 或 scope 无读写入口。 | closed |
| F-002 | Major | `crates/pareto-kernel/src/event_store.rs:185-198,415-460,513-530`; `crates/pareto-kernel/src/event_store/tests.rs:377-432` | 初审发现 cursor 未绑定 horizon/keyset position。修复的私有 opaque `Cursor` seal 覆盖 kind、query binding、horizon 和全部三个 position 字段；read 先验证 seal，逐字段篡改、scope binding、并发较小 stream、reopen/VACUUM 测试均通过。authority API 未公开，因此调用者不能重算或构造内部 seal。 | 将全部 cursor state 纳入可验证的 opaque encoding/MAC 或等价不可伪造表示；增加 horizon 与三个 position 字段逐项篡改、跨 scope/kind/schema-set、重启/VACUUM/并发 append 的负测。 | closed |
| F-003 | Major | `crates/pareto-kernel/src/event_store.rs:329-353,464-510,532-555`; `crates/pareto-kernel/src/event_store/tests.rs:707-746` | 初审发现 causation/correlation 与 stored bytes 漂移漏检。修复在 open/read 双向比较 causation/correlation，并在 open 重算 envelope/SchemaSet/limits 三组 fingerprints；逐项离线 drift 的真实文件负测均 fail closed。 | open 与逐行 read 对所有持久化身份列及三个 JSON/fingerprint 对进行完整双向校验；增加 causation、correlation、fingerprint、SchemaSet/limits JSON 与 digest 单独漂移的真实文件负测。 | closed |
| F-004 | Major | `crates/pareto-kernel/src/event_store.rs:208-354`; `crates/pareto-kernel/src/event_store/tests.rs:573-622` | 初审发现 store identity 未固定且 v1 接受 application id 0。修复区分只创建新库的 `open` 与必须携带 expected store id 的 `open_pinned`，v1 严格要求 application id；合法 store 换入、application id 清零和 metadata identity 漂移测试均拒绝。 | open 返回并由可信配置持久绑定 expected store identity（或提供等价不可替换机制），v1 非零 application id 严格匹配；真实文件负测覆盖换入另一合法 store、清零/篡改 application id、identity metadata 漂移和 reopen。 | closed |
| F-005 | Major | `crates/pareto-kernel/src/event_store/tests.rs`; `.agents/work/active/REQ-0004-sqlite-event-store/VALIDATION.md` | remediation 将 7 个测试扩展为 14 个，并补齐 external compile-fail、migration rollback、transaction drop/fresh visibility/busy/I/O、scope/payload isolation、两个 store 竞争及漂移矩阵；这些部分证据已独立复跑通过。但 AC-10 仍没有旧 Schema fixture、wrong-digest/alternate-compatible SchemaSet registry 负测；`AdmittedRead::admit` 由调用方传入 `expected_schema_set`，未证明按 persisted exact identity 恢复 reader 且替代 reader 不可选择。AC-06 的 cancellation/commit-uncertainty 也仅有 RAII/重试设计推断而无故障注入。因此 planned compatibility/recovery proof 尚不完整。 | 按 SPEC-0003 traceability 补齐旧 Schema fixture、persisted-identity-driven registry、missing/wrong/alternate-compatible reader 负测，以及 cancellation/commit-uncertainty 可行的确定性证据；更新 VALIDATION 并重跑 Focused/Impacted/Core/full gates。 | open |

# Acceptance trace

| Acceptance | Review result | Evidence |
|---|---|---|
| AC-01 | 满足当前 v1 合同 | 新库、pinned reopen、newer version、trigger drift、失败 migration rollback、application id 清零及 store swap 均有真实文件证据；首版无已发布旧 DB migration，不虚构历史升级。 |
| AC-02 | 满足 | authority 与 envelope 身份独立，API compile-fail；envelope/SchemaSetRef/limits 保存并重算 fingerprints，显式身份双向重验。 |
| AC-03 | 部分满足 | 固定 UPDATE/DELETE trigger 真实执行且 drift 被 open 拒绝；尚无 migration bypass/drop 的完整负测。 |
| AC-04 | 满足核心合同 | `BEGIN IMMEDIATE`、完整 non-NULL user key、sequence UNIQUE；两个独立 store/pool 竞争产生一个 commit、一个稳定 SequenceConflict，最终一行。user-present 隔离由 authority scope matrix 覆盖。 |
| AC-05 | 部分满足 | exact envelope/schema/limits fingerprints 支持 retry/conflict；测试只改变 correlation，未覆盖 scope/run/stream/sequence 与同 sequence 异 event 的完整矩阵。 |
| AC-06 | 基本满足 | 事务 RAII rollback、causation failure、成功后 fresh connection 可见、bounded busy 与 stable I/O 分类有真实证据；commit 结果不确定/取消仍未被故障注入直接证明，保留在 F-005 的完整门禁范围。 |
| AC-07 | 满足 | 私有 authority reader、稳定排序、ordinal horizon 与 sealed opaque cursor 已覆盖篡改、scope mixing、页间 append、reopen/VACUUM。 |
| AC-08 | 满足主要合同 | tenant/user/workspace/run/agent/stream authority mismatch、payload shadow、causation rollback，以及所有持久 JSON identity/fingerprint drift 已覆盖。 |
| AC-09 | 满足主要 E2E | tempfile 真实 SQLite 覆盖 create/append/read/fresh connection/reopen/continue、幂等、sequence、两个 store 竞争、失败 rollback、busy 和 VACUUM。 |
| AC-10 | 不满足证据门禁 | 仍没有旧 Schema fixture、wrong-digest/alternate-compatible SchemaSet registry 负测；read admission 由调用方传 `expected_schema_set`，未证明 persisted row identity 驱动 registry 恢复且不能被兼容替代 reader 选择。见 F-005。 |
| AC-11 | 满足现有原始记录 | VALIDATION 记录 protocol 9 unit + 17 contract、workspace tests 均通过；commit 依赖方向仍是 kernel -> protocol，protocol 未新增 SQLite/runtime/network 依赖。 |

# Compatibility, permission, and isolation review

- 依赖方向正确，`pareto-protocol` 未反向依赖 Kernel/SQLite；DB schema 与 Event Schema 数据列分离。
- 所有 Event Store 类型当前 module-private，外部 compile-fail 通过；私有 `KernelAuthority` 已把 authority target 与 envelope/query claim 分离，F-001 关闭。
- optional user 采用 `(user_present,user_id)` 非 NULL 规范编码并进入 sequence/query key，避免 SQLite NULL-distinct 绕过。
- Run/Stream SQL 均绑定 tenant、user presence/value、workspace、run、agent；payload 没有参与这些 query predicates。
- persisted reader identity 仍由 read admission 的 `expected_schema_set` 参数选择后与行比较；missing registry 已测，但 persisted identity 驱动选择及 wrong/alternate-compatible negative evidence 仍缺失，见 F-005。
- causation append 要求同完整 scope/run/stream 已存在，失败在事务内 rollback；causation/correlation 及所有 stored byte fingerprints 漂移现已 fail closed。

# Regression and test review

- 更新 VALIDATION 记录 Focused 14 tests、doc compile-fail、Impacted、Core、governance、clippy 与 diff-check 通过；性能数字仍明确标为 observation，Token/Provider cost 不适用。
- 本 reviewer 独立复跑 `cargo test -p pareto-kernel --all-targets --all-features --offline`（14/14）、`cargo test -p pareto-kernel --doc --offline`（1/1）、kernel clippy `-D warnings` 与修复范围 diff-check，全部通过。
- crate 测试继续使用真实临时 SQLite 文件；remediation 增加 authority/isolation、migration rollback、identity swap、cursor field tamper、两个 pool 竞争、transaction drop/busy/fresh visibility 和 stored-byte drift。
- 本 review 不以 closure 声明替代测试断言；仍未发现旧 Schema fixture、wrong/alternate-compatible registry、persisted-identity reader，以及 cancellation/commit-uncertainty 故障注入的原始证据，F-005 保持 open。
- 锁定的 `sqlx 0.8.6`、Tokio、SHA-256 与 tempfile 属于 approved SPEC/RFC 已识别依赖；未发现 protocol dependency growth 或网络/provider 调用。

# Scope and unrelated changes

精确提交的产品代码集中于新增 `pareto-kernel::event_store`、workspace dependency/lockfile 和 REQ-0004 设计/工作记录；未发现 Runtime、状态机、Projection、Replay executor、CLI 或通用 SQL escape hatch。`Cargo.lock` 大幅增长与 sqlx/Tokio 传递依赖一致。提交同时包含实施前设计记录，但均属于 REQ-0004 范围；未发现明显无关产品行为修改。

发布前 rollback 可 revert 新 crate/workspace member；若数据库已被使用，必须保留 v1 reader/migration 且不得降 `user_version` 或改写历史。当前 review 不批准进入 verified/done，因此尚不应执行发布后 rollback 声明或归档。

# Re-review history

- 2026-08-23：fresh-agent 独立评审精确提交 `9aab8614a7083958659f5969f2caec38b7c597cb`；0 Blocker、5 Major，结论 changes requested。
- 2026-08-23：focused independent re-review 精确提交 `a7c73b317c57813a2a1d29b35ef09a2773c6e17b` 及 `9aab861..a7c73b3` 修复 diff；独立复跑 14 kernel tests、1 compile-fail doctest、kernel clippy 与 diff-check。F-001/F-002/F-003/F-004 closed；F-005 保持 open。最终 0 Blocker、1 Major，仍 changes requested。
