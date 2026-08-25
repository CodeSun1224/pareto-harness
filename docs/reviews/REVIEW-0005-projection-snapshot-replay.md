---
id: REVIEW-0005
title: REQ-0006 Projection、Snapshot 与 Replay 独立代码评审
status: changes-requested
owners: [independent-reviewer]
created: 2026-08-25
updated: 2026-08-25
links: [REQ-0006, SPEC-0005, RFC-0005, ADR-0006, REQ-0003, REQ-0004, REQ-0005, REVIEW-0002, REVIEW-0003, REVIEW-0004]
independence: independent
reviewed_revision: 5c4f6e7f304c55fb61b6cc7e08d5bbe902b8d82c
open_blockers: 0
open_majors: 3
---

# Verdict

Changes requested。独立评审固定 exact implementation commit
`5c4f6e7f304c55fb61b6cc7e08d5bbe902b8d82c`，baseline 为
`bb395ad78f762b53d5f486c742194dd8d551dc61`。当前 0 open Blocker、3 open
Major；REQ-0006 不得进入 `verified` 或 `done`。

评审确认了几个重要的正确方向：Recorded replay 的接口没有 Effect/Provider/Tool executor
或 append capability；Snapshot-assisted load 先由 Event Store exact reader 重读并验证完整
history，再以权威 prefix 重算 rolling chain；Projection digest 与 comparability gate 包含 store、完整
scope、stream、cursor、source/output identity、history 和 reducer provenance；Snapshot 创建使用
`BEGIN IMMEDIATE`，并且 Snapshot UPDATE/DELETE trigger 均存在。但这些正确方向不能关闭下列
exact reducer retention、v2 migration integrity 和验证证据缺口。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `crates/pareto-kernel/src/event_store/projection.rs:125-189,216-279,549-557,1025-1067` | `ProjectionRegistry::retained` 遍历所有 supplied source set，只要能找到旧的四个 1.0 lifecycle bindings 就动态创建 registration；它又按 `outputs` 顺序选择第一个含 Projection member 的 set。registration 只携带 descriptor/ref，没有与 ref 绑定的 implementation dispatch；full 与 suffix 路径无论解析到哪个 ref 都调用同一份当前 `fold_lifecycle` / `apply_lifecycle_event`。因此 evolved source set 可被旧 reducer 自动接纳，output registry 顺序可改变同一历史 source 的 reducer/output identity，未来所谓 retained reducer 仍会执行当前实现。这违反 AC-02/AC-12 及 RFC-0005 的 persisted source contract → exact immutable reducer/output/implementation allowlist、current-substitution 拒绝和历史 retention 合同。现有 `reducer_resolution` 只清空 vector，不能证明 wrong/current/evolved substitution 被拒绝。 | 用纳入版本控制的显式 registration 固定 exact source key（并明确 source-set evolution政策）、exact reducer ref、descriptor、output set/limits和对应 implementation；执行必须按 exact ref dispatch，不得按 registry 顺序或当前共享函数隐式选择。真实测试至少构造第二个 evolved source、第二个含 Projection 的 output set及第二个 reducer implementation，证明 registry 顺序不影响旧 Run，old Run/Snapshot exact resolve旧实现，new source没有显式mapping时 fail closed，missing/wrong/current substitution拒绝。 | open |
| F-002 | Major | `crates/pareto-kernel/src/event_store.rs:403-424`; `crates/pareto-kernel/src/event_store/tests.rs:237-267,681-715`; `crates/pareto-kernel/src/event_store/projection/snapshot.rs:352-430` | v2 open 只把固定 `V2_MIGRATION_DDL` 的 fingerprint 与 migration ledger 自报值比较；对实际 schema 只检查 writer-epoch列的少数字段、Snapshot列数及同名index存在。保持26列和同名index但删除/改变 Snapshot CHECK、UNIQUE、type、index columns/order的数据库仍可通过 open，ledger checksum不能证明实际DDL。测试只证明新库直接到v2、空v1 old connection被epoch trigger拒绝，以及v0初始化失败rollback；没有含历史Event/Manifest/Snapshot的真实v1→v2 bytes-preservation、v2中途失败rollback或actual table/index drift矩阵。这不足以证明 AC-11 的原子 migration、完整schema/trigger identity、历史保留和rollback合同。 | 校验 actual `sqlite_master` table/index SQL 的冻结canonical identity，或逐列/constraint/index/trigger执行等价的完整结构证明；同列数/同名index的CHECK/UNIQUE/index-order漂移必须在open时拒绝。加入真实v1 fixture（含多个exact旧Schema lifecycle events），迁移后证明Event JSON/fingerprint/sequence/ordinal/store ID/Manifest逐字节未改、旧连接写拒绝且v2写成功；在v2各DDL阶段注入失败并证明完整rollback到v1，随后成功迁移、reopen及newer/old-binary拒绝。 | open |
| F-003 | Major | `docs/specs/SPEC-0005-projection-snapshot-replay.md:135-151`; `.agents/work/active/REQ-0006-projection-snapshot-replay/VALIDATION.md`; `crates/pareto-protocol/tests/protocol_contract.rs:150-206`; `crates/pareto-kernel/src/event_store/projection.rs:1109-1363`; `crates/pareto-kernel/src/event_store/projection/{snapshot,replay}.rs` | AC→test 矩阵和提交中的通过声明显著超过实际断言。独立运行计划中的 `cargo test -p pareto-kernel projection::authority --offline` 为 **0 tests**。golden未覆盖承诺的scope/source mutation和真正的N/prefix accumulator matrix；resolver/compatibility只覆盖“清空全部”；invalid history只覆盖两类；comparison只覆盖cross-store；Snapshot output/isolation/fallback/migration测试没有old/current/alternate reader、完整scope/cursor/version/digest corruption、真实v1数据migration或失败rollback；`projection::concurrency`是先读、后append、再读的顺序测试，不是并发point-in-time证明。默认并行门禁还不稳定：独立首次 `projection::` 失败于 `snapshot::fallbacks`；`event_store` 随后同时失败于 `fallbacks` 和 `prefix_corruption`，均因pool上的DROP后CREATE trigger报告“already exists”；单线程运行才稳定。一次后续workspace运行通过不能抹去可复现的默认门禁失败。AC-01/02/03/04/06/08/09/10/11/12/13因此缺少高风险负证据，当前不能验证质量或回归结论。 | 为每个AC补充有实质断言且过滤命令命中非零测试的矩阵：两连接/barrier point-in-time read与snapshot/append顺序；tenant/user presence-value/workspace/run/actor/stream/store/source/output/cursor/history/reducer逐字段swap；unknown major/schema、0/gap/reuse/MAX及非法lifecycle；candidate format/schema/version/digest/cursor各类fallback与prefix corruption fail-closed区分；equal/divergent/not-comparable全矩阵；empty/1/N/prefix+suffix和scope/source mutation golden；F-001/F-002要求的compatibility/migration fixtures。corruption tests须在单一受控connection/transaction中改写并恢复trigger，默认并行命令重复运行稳定通过；VALIDATION只记录reviewer可复现的实际结果。 | open |
| F-004 | Minor | `crates/pareto-kernel/src/event_store/projection.rs:30-47,60-100,366-398,912-1021`; `SPEC-0005` Errors/fallback contract | 稳定错误合同没有完整落地：`UnsupportedEvent` 与 error-form `NotComparable` 从未构造，unknown event当前通常被映射为 `AggregateCorrupt`；candidate reducer/source不兼容常因先用当前reducer执行 `validate_snapshot_record` 而归入 `RejectedIntegrity`，而不是稳定的 incompatible disposition。路径均 fail closed，故当前不是越权或状态损坏，但会让诊断、兼容测试和未来调用方依赖偶然分支。 | 固定并测试 unknown event/schema、candidate format/digest/cursor/source/reducer/output mismatch各自稳定类别；删除不可能的类别或让实现按Spec产生它们，确保错误不泄漏其他scope数据。 | accepted |

# Acceptance trace

| Acceptance | Review result | Independent evidence |
|---|---|---|
| AC-01 | 实现主路径部分满足，证据不足 | `load_source` 从 persisted first row exact resolve SchemaSet/limits并对所有行调用 `validate_row`；assisted也使用同一完整range。但计划的authority命令实际0 tests，完整替代reader/API负证据不足。 |
| AC-02 | 不满足 | reducer fold在当前输入上重复结果相同，但F-001证明resolver/output/implementation不是不可变exact registration，retention/current substitution没有实现证据。 |
| AC-03 | 部分满足 | 完整history能重建Manifest/Run/Task且pure lifecycle fold复用；unknown/gap会fail closed。wrong major/schema、reuse/0/MAX和完整非法历史矩阵不足。 |
| AC-04 | 部分满足 | record/digest字段和immutable row主路径存在；F-001 output身份选择及F-003 compatibility/golden缺口未关闭。 |
| AC-05 | 主实现方向满足，门禁不稳定 | create在`BEGIN IMMEDIATE`内full fold/insert/commit；drop rollback与UPDATE/DELETE trigger存在。默认并行corruption tests两次失败，不能称全部门禁完成。 |
| AC-06 | 主实现方向满足，负证据不足 | candidate通过后对权威prefix重新计算chain，history mismatch fail closed；candidate-only错误fallback。逐类corruption/version/output/reducer/cursor矩阵不完整。 |
| AC-07 | 满足当前私有切片 | Recorded调用full reader/fold，不接收Effect/Provider/Tool callback，不append或建Snapshot；Simulated同步拒绝且没有Effect入口。测试中的独立counter并未接线，但API/dependency inspection足以证明当前不可达。 |
| AC-08 | 部分满足 | Projection digest与compare gate包含完整provenance，same-store replay equality与cross-store not-comparable通过；逐字段not-comparable及provenance相同但digest不同的divergent测试缺失。 |
| AC-09 | 实现查询键覆盖完整scope，负证据不足 | source/snapshot SQL、Projection和digest绑定完整scope/store；Projection测试覆盖主要target swap。Snapshot/cursor/digest跨scope复用及source/output逐字段matrix不足。 |
| AC-10 | 实现方向满足，测试不足 | SQLite read transaction在first SELECT后固定horizon；snapshot create持write reservation；reopen路径通过。所谓Projection concurrency test并无并发操作或barrier。 |
| AC-11 | 不满足 | writer epoch阻止held-open旧SQL的机制有效；F-002证明actual v2 DDL完整性、真实历史v1 migration与中途rollback尚未得到代码/测试证明。 |
| AC-12 | 不满足 | 新内容地址set生成且三个旧set byte-identical；F-001和F-003证明old source/output/reducer retention/current substitution矩阵未交付。 |
| AC-13 | 部分满足 | protocol 9 unit + 21 contract、lifecycle 18、fmt/clippy/governance/schema generation通过；Event Store/Projection默认并行命令有独立失败记录，旧Review freshness也尚未全部关闭。 |
| AC-14 | 满足 | diff未引入Capability、Hook、Effect executor、Provider/Agent Loop、Memory、Task/Context DAG、distributed/remote store或第三方依赖。 |

# Compatibility, permission, and isolation review

- `pareto-protocol`仍不依赖Kernel/sqlx，Cargo manifests/lock及第三方依赖未变化；新SchemaSet为独立内容地址，三个旧set在baseline到reviewed revision间byte-diff为空。
- authority-bearing Store、Projection、Snapshot和Replay入口仍为crate-private；没有public raw SQL、Snapshot import、caller-selected reducer或Effect callback。
- Event读取由persisted SchemaSet/limits、row fingerprints和完整scope SQL约束；Manifest owner-only authority未扩大为REQ-0007 Capability。
- Snapshot query、record、hash view与comparison都绑定tenant、user presence/value、workspace、run、agent/owner actor、stream和store ID。未发现payload shadow成为authority的路径。
- F-001意味着兼容与版本权限仍可被trusted registry构造顺序/自动注册误用；F-002意味着raw DB drift的定义内检测不完整。二者是fail-closed trusted-kernel合同问题，必须在验证前关闭。

# Regression and test review

Reviewer在Windows/PowerShell、2026-08-25、offline依赖条件下独立执行：

- `cargo test -p pareto-kernel projection:: --offline`：首次26 passed、1 failed、1 ignored；`snapshot::fallbacks`因trigger重复创建panic。立即单测该case通过，第二次整组27 passed/1 ignored，证明间歇性而非稳定门禁。
- `cargo test -p pareto-kernel event_store --offline`：58 passed、2 failed、1 ignored；`snapshot::fallbacks`与`snapshot::prefix_corruption`均因DROP后CREATE trigger报告already exists而panic。
- `cargo test -p pareto-kernel event_store --offline -- --test-threads=1`：60 passed、1 ignored；只能证明单线程规避，不满足默认完成命令。
- `cargo test --workspace --all-targets --all-features --offline`：本次运行Kernel 60 passed/1 ignored、Protocol 9 unit + 21 contract/1 observation ignored；通过不关闭前两次默认并行失败。
- `cargo test -p pareto-kernel lifecycle:: --offline`：18 passed。
- `cargo test -p pareto-protocol --all-targets --all-features --offline`：9 unit + 21 contract passed，1 observation ignored；publisher drift fixture的stderr是预期负例，process exit为0。
- `cargo test -p pareto-kernel projection::authority --offline`：0 tests；`snapshot::migration`：1 passed；`cargo test -p pareto-protocol projection_digest_golden --offline`：contract target命中1 passed，其余targets为0。
- workspace fmt、clippy `-D warnings`、18个Python governance tests、schema generator与`git diff --check`均通过；生成后worktree clean，三个retained SchemaSet相对baseline无diff。
- `python scripts/check_docs.py`失败于REVIEW-0001..0004 freshness。经本轮实质审查，REVIEW-0001、REVIEW-0002和REVIEW-0004既有批准合同可前移；REVIEW-0003因F-002/F-003影响Event Store migration/core regression而必须保持stale，不能机械批准。

现有quality oracle（full fold/recorded replay）方向正确；本地latency/storage observation只是debug环境观察，无阈值、无Provider/Token费用和无优化声明。本评审不批准任何质量、成本或延迟优化结论。

# Scope and unrelated changes

exact diff只包含REQ-0006设计/work记录、协议类型/Schema、Kernel Projection/Snapshot/Replay与必要Event Store/lifecycle重构、测试和一个新内容地址SchemaSet。没有Cargo依赖增长、外部Effect、Provider、CLI、Memory、DAG、distributed/remote store或明显无关产品修改。

# Re-review history

- 2026-08-25：fresh independent review of exact `5c4f6e7f304c55fb61b6cc7e08d5bbe902b8d82c` against baseline `bb395ad78f762b53d5f486c742194dd8d551dc61`。逐项读取REQ-0006/SPEC-0005/RFC-0005/ADR-0006、独立架构评审、Plan/Tasks/Handoff/Validation、完整源码/Schema/test diff，并对照REQ-0003/0004/0005及REVIEW-0002/0003/0004。结论0 Blocker、3 Major、1 accepted Minor，changes requested。任何修复必须产生新exact commit，由同一独立reviewer检查remediation diff和新原始证据；实现者不能自行关闭F-001至F-003。
