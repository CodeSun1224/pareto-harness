---
id: REVIEW-0013
title: REQ-0009 Effect Intent/Receipt 独立实现评审
status: changes-requested
owners: [runtime-kernel]
created: 2026-08-31
updated: 2026-08-31
links: [REQ-0009, SPEC-0008, RFC-0009, ADR-0010, REVIEW-0012]
independence: independent
reviewed_revision: 6cad604ffe5ec2126f9745bf22ece713f2c0ce85
open_blockers: 0
open_majors: 9
---

# Verdict

`changes-requested`。fixed implementation commit `6cad604ffe5ec2126f9745bf22ece713f2c0ce85`
的现有测试全部为绿，但独立源码/调用链审查确认 9 个 open Major。实现尚未兑现 claim 前的
Kernel writer 内准入、不可重发 dispatch lease、Manifest-pinned executor implementation、Receipt
拒绝后的保守结算、recovery ExistingTerminal/epoch proof、受信 reconciliation evidence、双 stream
pair 的读取侧完整性、Task success guard、完整 Effect Projection 和历史 reducer identity 不变性。
因此 AC-01..AC-22 尚不能验证，REQ-0009 不得进入 `verified` 或 `done`。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:850-1039`; `tests.rs:935-980` | `claim_effect_dispatch` 只检查 Effect intent/descriptor/cursor；没有在 `BEGIN IMMEDIATE` 内验证 Run/Task 仍为 `running`、operation 仍 reserved、effective cancellation、deadline equality 或 claim Clock。因而 Intent 后 pause/cancel/deadline 发生时仍可提交 claim 并跨过可能执行边界。exact retry 还重新构造并返回可执行 `EffectDispatchLease`，测试明确要求 retry lease 等于首个 lease；这与 AC-03/05/13 及批准设计“already-claimed 不再发 lease”相反。 | 在 claim writer transaction 内 refold exact lifecycle/control/effect 并加入 paused/cancel/deadline/equality/race 负测；retry API 不得返回可执行 lease，且证明 executor counter 不变。 | open |
| F-002 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:139-153,403-452,1003-1103` | `FakeEffectExecutor` 没有 implementation identity，`execute_fake_effect` 只验证 descriptor 与自算 lease seal，完全未核对 descriptor 的 `implementation_compatibility_digest`；调用方可把任意 trait implementation 注入同一 Manifest descriptor。`dispatch_effect` 也只把 `ResponseLost`/`FailedBeforeApply`/`CrashedAfterReturn` 枚举返回给调用方，不执行 Kernel-owned terminal admission，测试仅断言 enum/counter 后留下 claimed operation。executor substitution 和边界故障因此没有受 Manifest 固定的闭合权威路径，违反 AC-01/05/06/08/21。 | 建立 sealed、Manifest-pinned Fake implementation resolver/identity；提供 Kernel-owned claim→invoke→admit/terminal orchestration，并对 wrong implementation、response loss、pre-effect failure、return-before-terminal crash 做 Event/预算终态断言。 | open |
| F-003 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:1040-1171`; `tests.rs:1185-1243` | Receipt trust 分支与批准合同相反：wrong producer/adapter 被当作可写目标 stream 的 `effect-message-rejected`，而 AC-07/16 要求 wrong producer/cross-domain no-write；正确 producer 的 malformed/oversized/unsorted output 也只写 rejection 后返回，operation 仍 reserved、Effect 仍 claimed，而合同要求按 unknown 保守 settlement + reconciliation。代码还未校验 `observed_at`、`observed_usage`/meter evidence或 `max_result_summary_bytes`。现有 `receipt_admission` 测试明确把 wrong producer 写 audit 当作成功证据。 | 区分 identity/source mismatch no-write 与 authenticated producer invalid-output audit+atomic unknown settlement；覆盖 adapter/producer/scope/epoch/schema/bytes/depth/count/result-summary/meter/Clock，断言目标 Event 数、gross budget 和 reconciliation。 | open |
| F-004 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:1492-1749`; `tests.rs:1420-1550` | recovery 的 `ProcessEpochLost` 证明只是调用方提供的两个 digest 不相等；`current_process_epoch_digest` 不从 canonical `ClockSample.process_epoch` 派生，也没有 Kernel-authenticated liveness observation。终态分支只接受同 command IDs/fingerprint；不同 ID 的 reopen/new Clock sample 在 terminal 已存在时返回 `IdempotencyConflict`，没有 SPEC/AC-10 要求的 `ExistingTerminal` no-op。测试没有 not-eligible→new-sample、commit-response-loss lost-command/reopen 或伪造 epoch proof。 | 引入不可伪造 process-epoch observation并绑定 Clock/recovery key；按批准优先级实现 same-ID exact/mutation→different-ID ExistingTerminal→eligibility，覆盖 pre/post-claim、response-loss和 reopen bounded model。 | open |
| F-005 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:1752-1906`; `tests.rs:1643-1744` | reconciliation 没有 producer/evidence authority。命令只需把 `producer_revision` 字符串设为 `reconciliation_policy_revision`；`source_observation_event_ids` 只要求非空排序，不验证 Event 存在、属于 exact Effect/attempt/external key、是 admitted query/Receipt observation或与 evidence fingerprint 相符。测试甚至用 terminal `effect-reconciliation-required` Event 作为 source，就能关闭 reconciliation。任意 crate 内调用者可自证 `ResolvedApplied/NotApplied/Partial`，违反 AC-07/11/16/21。 | 使用 Manifest-pinned query adapter/reconciliation producer handle和闭合 evidence admission；逐 source Event 验证存在性、类型、scope/effect/attempt/external identity与 fingerprint，并加入 owner-only request、wrong producer/source/mutation/unresolved 负测。 | open |
| F-006 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:2144-2345,2771-3109`; `crates/pareto-kernel/src/event_store/runtime_control.rs:2185-2428,2733-2757` | pair writer 计算了 prepared digests/fingerprint，但 Effect/Control fold 从不重算，也不读取并核对 counterpart Event bytes/presence。Effect Projection 可只凭 Effect stream fold；Control fold也只验证 payload 局部字段。删除/替换一侧后重新封 Event row，读取、projection/success guard 不会检测 `one-existing`/cross-prepared drift，违反 AC-12/15/17 与单边终态损坏 fail-closed 合同。 | 在权威读取/fold/reopen 中双向核对 counterpart presence、pair ID/kind/fingerprint、prepared digests和业务 binding；增加 reserve/terminal 任一侧缺失、reseal、异 bytes、wrong counterpart 的 reopen/projection/success/replay 负测。 | open |
| F-007 | Major | `crates/pareto-kernel/src/event_store/lifecycle.rs:326-339`; `effect_runtime.rs:2674-2713`; `tests.rs:1747-1843` | success guard 只接入 `transition_run`；`transition_task` 没有 Effect guard。Task-scoped pending/claimed/open-reconciliation Effect 的 Task 仍可迁移 `succeeded`。唯一 helper 也只做整 Run `all()`，没有目标 Task 过滤语义。AC-22 明确要求 Run/Task 同 writer transaction 拒绝成功。 | 新增 Task exact-scope guard并在 `transition_task` writer transaction refold；覆盖其他 Task 不阻塞、目标 Task pending/claimed/partial/unknown/open reconciliation 阻塞，以及 transition/terminal race 双顺序。 | open |
| F-008 | Major | `crates/pareto-protocol/src/effects.rs:650-749`; `crates/pareto-kernel/src/event_store/effect_runtime.rs:2839-2858,3112-3166` | `EffectProjectionEntryV1` 无 subject/Task、executor config、base recovery key（未claim无法恢复）、reserved/accounted usage、pair/terminal identity、partial confirmed/unknown digests、limitations、reconciliation source/evidence；projection top-level 也不固定 output reader/history revision或 protocol limits。AC-17 要求 crash/reopen 恢复这些 exact facts，当前 public projection 只能表达简化状态，命名 `projection_recovery` 测试没有逐字段验证。 | 扩展版本化 projection/hash view以无损表达批准合同全部 identity/state/budget/evidence，并加入 unclaimed/claimed/partial/unknown/reconciled reopen exact equality、unknown reader/reducer/current substitution 和 fixed horizon tests。 | open |
| F-009 | Major | `crates/pareto-kernel/src/event_store/projection.rs:201-245,1544-1620` | 实现把 v3 source key加入共享 `ProjectionRegistry` 后，直接更新了既有 lifecycle reducer contract digest、v1 projection digest、snapshot digest和history chain golden 值（例如 reducer `898790...`→`5549c7...`）。批准 SPEC/RFC/ADR 明确要求旧 reader/reducer/snapshot bytes/identity 保持不变，并把任何旧 reducer identity 变化列为 implementation blocker；改测试期望不能构成兼容迁移。 | 恢复全部旧 golden bytes/identities，隔离新增 v3 reducer registration使 v1/v2 descriptor不变；对 accepted baseline逐 byte/digest比较并保留 old replay/snapshot tests。 | open |

# Acceptance trace

| Acceptance | Independent result |
|---|---|
| AC-01 | not satisfied：executor implementation未固定，Effect ID helper也未显式覆盖executor descriptor/config；见F-002。 |
| AC-02 | partially evidenced：request path有Manifest/Capability/budget准入，但claim/reconciliation authority不闭合；见F-001、F-005。 |
| AC-03 | reserve/Intent atomic path存在；claim前lifecycle/control/cancel/deadline未重查，不能整体满足；见F-001。 |
| AC-04 | exact request retry/same-key mutation有测试；executor mutation依赖不完整且Effect ID preimage未覆盖executor identity，未关闭。 |
| AC-05 | not satisfied：retry重发lease，lease/implementation identity不完整；见F-001、F-002。 |
| AC-06 | outcome enum有Fake测试，但缺Kernel-owned fault→terminal路径及timeout/malformed完整矩阵；见F-002、F-003。 |
| AC-07 | not satisfied：wrong producer写目标、invalid trusted producer不settle、meter/Clock/limits admission不完整；见F-003。 |
| AC-08 | fold枚举了状态，但边界故障可长期停在claimed，权威结论不闭合；见F-002、F-003。 |
| AC-09 | partial Event/Inventory保留部分摘要；Projection不保留，测试未证明usage/evidence上界；见F-008。 |
| AC-10 | not satisfied：epoch proof可自报且缺ExistingTerminal new-sample no-op；见F-004。 |
| AC-11 | not satisfied：无受信producer/evidence admission；见F-005。 |
| AC-12 | writer pair存在，但读取侧不验证counterpart；invalid Receipt不原子settle；见F-003、F-006。 |
| AC-13 | not satisfied：claim可越过cancel/deadline/pause且race矩阵未实现；见F-001。 |
| AC-14 | late audit基本路径存在；wrong producer audit/no-write边界和矛盾/乱序矩阵不足；见F-003。 |
| AC-15 | 使用单Effect stream且无新mutable表，但pair cross-stream完整性不能从历史验证；见F-006。 |
| AC-16 | request scope矩阵存在；Receipt/reconciliation/lease身份矩阵不满足；见F-003、F-005。 |
| AC-17 | not satisfied：Projection无损恢复合同缺失；见F-006、F-008。 |
| AC-18 | V2 fixed-horizon inventory/replay happy path存在；Projection证据不完整，post-inventory reconciliation lineage仅协议类型、无Kernel finalizer证明。 |
| AC-19 | not satisfied：retained reducer/projection/snapshot identities被改写；见F-009。 |
| AC-20 | 19个命名filter均非零且通过，但缺上述负测/race/model/compatibility proof，不能以命名绿灯替代覆盖。 |
| AC-21 | not satisfied：crate-private可见性存在，但executor/reconciliation/recovery authority接口仍可由普通内部值自组；见F-002、F-004、F-005。 |
| AC-22 | Run guard happy path存在；Task guard缺失；见F-007。 |

# Compatibility, permission, and isolation review

- Manifest v3、独立SchemaSet和SQLite v2方向符合ADR；`python scripts/check_req0009_scope.py`也确认DB v2、retained
  schema directories、依赖和Fake-only静态范围。但该脚本没有检查历史 reducer/projection/snapshot golden identity，F-009仍成立。
- authority值均未公开到crate外，但“private struct”不等于闭合authority：executor trait无pinned implementation seal，recovery epoch
  proof由命令字段自报，reconciliation无producer/evidence handle。后续Provider/Tool接入前必须修复，不能把crate visibility当作proof。
- request isolation测试覆盖tenant/user presence/value/workspace/run/agent；没有覆盖Task/operation/reservation/attempt/lease/Receipt/
  reconciliation source的完整no-write矩阵。wrong producer当前明确会向目标Effect stream写audit。
- generic atomic pair保证正常writer的zero/two transaction原子性；读取/fold缺counterpart validation使raw corruption/reseal与reopen不满足fail closed。

# Regression and test review

Independent reviewer在Windows PowerShell、2026-08-31、exact
`6cad604ffe5ec2126f9745bf22ece713f2c0ce85`执行的原始证据：

- `cargo test -p pareto-kernel event_store::effect_runtime::tests:: --offline`：`19 passed; 0 failed; 0 ignored; 161 filtered out`，exit 0。
- `cargo test -p pareto-protocol --test protocol_contract effect_contract_manifest_events_and_inventory_v2_are_closed --offline -- --exact`：`1 passed; 24 filtered out`，exit 0。
- `cargo test -p pareto-kernel lifecycle:: --offline`：`18 passed; 0 failed; 162 filtered out`，exit 0。
- `python -m unittest discover -s scripts/tests -p "test_*.py"`：`Ran 27 tests ... OK`，exit 0。
- `cargo test --workspace --all-targets --all-features --offline`：Kernel `179 passed; 1 ignored`；Protocol unit `9 passed`；contract `25 passed`；baseline `1 ignored`，exit 0。ignored项均为既有非阈值performance observation。
- `python scripts/check_req0009_scope.py`：`REQ-0009 scope check passed: DB v2 and retained sets frozen; Fake Effect only; replay read-only; dependencies unchanged`，exit 0。
- `git diff --check 60cee6ed44d150185bf99ca3095a8ce803bcc0d3..6cad604ffe5ec2126f9745bf22ece713f2c0ce85`：无输出，exit 0。
- 新增本Review后运行`python scripts/check_docs.py`：exit 1；报告REVIEW-0001..0011的
  `reviewed_revision: 46772c7...`相对REQ-0009实现diff stale。此结果与候选VALIDATION的expected pre-review failure一致；
  本Review为`changes-requested`且Reviewer未把旧Review freshness前移，故不得表示docs gate通过。
- 新增本Review后运行`git diff --check`：无输出，exit 0；`git status --short`仅显示
  `?? docs/reviews/REVIEW-0013-effect-intent-receipt-implementation.md`。

这些绿灯证明当前实现自洽于当前测试断言，不证明批准合同。例如`dispatch_lease`测试要求retry返回相同lease，
`receipt_admission`测试要求wrong producer写rejected audit，`reconciliation`测试以terminal Event充当source evidence，
`projection::digest_golden`直接接受旧identity变化；它们分别固化了F-001/F-003/F-005/F-009所述错误行为。

# Scope and unrelated changes

exact diff `60cee6ed44d150185bf99ca3095a8ce803bcc0d3..6cad604ffe5ec2126f9745bf22ece713f2c0ce85`
包含124个文件、7688 insertions、137 deletions。Cargo manifests/lock、SQLite DDL/trigger和历史SchemaSet目录没有变化；新增一个
content-addressed Effect set、Effect runtime/protocol/tests/scope script及work evidence属于REQ-0009范围。旧Review freshness元数据前移属于交付门禁记录。
但既有Projection golden identity的改写不是允许的v3演进，属于F-009兼容回归，不可作为无关更新接受。

# Re-review conditions

实现者需逐项修复F-001至F-009并补充required proof。Blocker/Major不能由实现者自关；同一independent Reviewer应对
新的exact commit和增量diff进行focused re-review，并复跑19个非零命名filter、Impacted/Core/full completion gates。
只有`open_blockers: 0`且`open_majors: 0`时才可改为`approved`。

# Re-review history

- 2026-08-31：fresh independent code review of exact
  `6cad604ffe5ec2126f9745bf22ece713f2c0ce85` against accepted implementation baseline
  `60cee6ed44d150185bf99ca3095a8ce803bcc0d3`。结论0 Blocker、9 open Major、`changes-requested`；Reviewer只新增本Review记录，未修改实现源码、Schema或测试。
