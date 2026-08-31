---
id: REVIEW-0013
title: REQ-0009 Effect Intent/Receipt 独立实现评审
status: changes-requested
owners: [runtime-kernel]
created: 2026-08-31
updated: 2026-09-01
links: [REQ-0009, SPEC-0008, RFC-0009, ADR-0010, REVIEW-0012]
independence: independent
reviewed_revision: 7eeb5f6d4095b7d2fdc6cc225e9b60c89482063f
open_blockers: 0
open_majors: 4
---

# Verdict

`changes-requested`。focused re-review 固定整改 commit
`7eeb5f6d4095b7d2fdc6cc225e9b60c89482063f`，相对初始实现
`6cad604ffe5ec2126f9745bf22ece713f2c0ce85`。独立源码、增量 diff 与门禁复核关闭
F-001、F-006、F-007、F-008、F-009；F-002、F-003、F-004、F-005 仍为 open Major。
当前测试全部为绿，但 executor implementation/recovery authority 仍可由普通 Kernel 内部值自报，
authenticated-invalid Receipt 的 mandatory audit 不与 unknown terminal pair 原子提交，reconciliation
也仍未由 Manifest-pinned query producer observation 决定。故 REQ-0009 仍不得进入 `verified` 或 `done`。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:850-1039`; `tests.rs:935-980` | `claim_effect_dispatch` 只检查 Effect intent/descriptor/cursor；没有在 `BEGIN IMMEDIATE` 内验证 Run/Task 仍为 `running`、operation 仍 reserved、effective cancellation、deadline equality 或 claim Clock。因而 Intent 后 pause/cancel/deadline 发生时仍可提交 claim 并跨过可能执行边界。exact retry 还重新构造并返回可执行 `EffectDispatchLease`，测试明确要求 retry lease 等于首个 lease；这与 AC-03/05/13 及批准设计“already-claimed 不再发 lease”相反。 | 在 claim writer transaction 内 refold exact lifecycle/control/effect 并加入 paused/cancel/deadline/equality/race 负测；retry API 不得返回可执行 lease，且证明 executor counter 不变。 | closed |
| F-002 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:139-153,403-452,1003-1103` | `FakeEffectExecutor` 没有 implementation identity，`execute_fake_effect` 只验证 descriptor 与自算 lease seal，完全未核对 descriptor 的 `implementation_compatibility_digest`；调用方可把任意 trait implementation 注入同一 Manifest descriptor。`dispatch_effect` 也只把 `ResponseLost`/`FailedBeforeApply`/`CrashedAfterReturn` 枚举返回给调用方，不执行 Kernel-owned terminal admission，测试仅断言 enum/counter 后留下 claimed operation。executor substitution 和边界故障因此没有受 Manifest 固定的闭合权威路径，违反 AC-01/05/06/08/21。 | 建立 sealed、Manifest-pinned Fake implementation resolver/identity；提供 Kernel-owned claim→invoke→admit/terminal orchestration，并对 wrong implementation、response loss、pre-effect failure、return-before-terminal crash 做 Event/预算终态断言。 | open |
| F-003 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:1040-1171`; `tests.rs:1185-1243` | Receipt trust 分支与批准合同相反：wrong producer/adapter 被当作可写目标 stream 的 `effect-message-rejected`，而 AC-07/16 要求 wrong producer/cross-domain no-write；正确 producer 的 malformed/oversized/unsorted output 也只写 rejection 后返回，operation 仍 reserved、Effect 仍 claimed，而合同要求按 unknown 保守 settlement + reconciliation。代码还未校验 `observed_at`、`observed_usage`/meter evidence或 `max_result_summary_bytes`。现有 `receipt_admission` 测试明确把 wrong producer 写 audit 当作成功证据。 | 区分 identity/source mismatch no-write 与 authenticated producer invalid-output audit+atomic unknown settlement；覆盖 adapter/producer/scope/epoch/schema/bytes/depth/count/result-summary/meter/Clock，断言目标 Event 数、gross budget 和 reconciliation。 | open |
| F-004 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:1492-1749`; `tests.rs:1420-1550` | recovery 的 `ProcessEpochLost` 证明只是调用方提供的两个 digest 不相等；`current_process_epoch_digest` 不从 canonical `ClockSample.process_epoch` 派生，也没有 Kernel-authenticated liveness observation。终态分支只接受同 command IDs/fingerprint；不同 ID 的 reopen/new Clock sample 在 terminal 已存在时返回 `IdempotencyConflict`，没有 SPEC/AC-10 要求的 `ExistingTerminal` no-op。测试没有 not-eligible→new-sample、commit-response-loss lost-command/reopen 或伪造 epoch proof。 | 引入不可伪造 process-epoch observation并绑定 Clock/recovery key；按批准优先级实现 same-ID exact/mutation→different-ID ExistingTerminal→eligibility，覆盖 pre/post-claim、response-loss和 reopen bounded model。 | open |
| F-005 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:1752-1906`; `tests.rs:1643-1744` | reconciliation 没有 producer/evidence authority。命令只需把 `producer_revision` 字符串设为 `reconciliation_policy_revision`；`source_observation_event_ids` 只要求非空排序，不验证 Event 存在、属于 exact Effect/attempt/external key、是 admitted query/Receipt observation或与 evidence fingerprint 相符。测试甚至用 terminal `effect-reconciliation-required` Event 作为 source，就能关闭 reconciliation。任意 crate 内调用者可自证 `ResolvedApplied/NotApplied/Partial`，违反 AC-07/11/16/21。 | 使用 Manifest-pinned query adapter/reconciliation producer handle和闭合 evidence admission；逐 source Event 验证存在性、类型、scope/effect/attempt/external identity与 fingerprint，并加入 owner-only request、wrong producer/source/mutation/unresolved 负测。 | open |
| F-006 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:2144-2345,2771-3109`; `crates/pareto-kernel/src/event_store/runtime_control.rs:2185-2428,2733-2757` | pair writer 计算了 prepared digests/fingerprint，但 Effect/Control fold 从不重算，也不读取并核对 counterpart Event bytes/presence。Effect Projection 可只凭 Effect stream fold；Control fold也只验证 payload 局部字段。删除/替换一侧后重新封 Event row，读取、projection/success guard 不会检测 `one-existing`/cross-prepared drift，违反 AC-12/15/17 与单边终态损坏 fail-closed 合同。 | 在权威读取/fold/reopen 中双向核对 counterpart presence、pair ID/kind/fingerprint、prepared digests和业务 binding；增加 reserve/terminal任一侧缺失、reseal、异 bytes、wrong counterpart 的 reopen/projection/success/replay负测。 | closed |
| F-007 | Major | `crates/pareto-kernel/src/event_store/lifecycle.rs:326-339`; `effect_runtime.rs:2674-2713`; `tests.rs:1747-1843` | success guard 只接入 `transition_run`；`transition_task` 没有 Effect guard。Task-scoped pending/claimed/open-reconciliation Effect 的 Task 仍可迁移 `succeeded`。唯一 helper 也只做整 Run `all()`，没有目标 Task 过滤语义。AC-22 明确要求 Run/Task 同 writer transaction 拒绝成功。 | 新增 Task exact-scope guard并在 `transition_task` writer transaction refold；覆盖其他 Task 不阻塞、目标 Task pending/claimed/partial/unknown/open reconciliation 阻塞，以及 transition/terminal race 双顺序。 | closed |
| F-008 | Major | `crates/pareto-protocol/src/effects.rs:650-749`; `crates/pareto-kernel/src/event_store/effect_runtime.rs:2839-2858,3112-3166` | `EffectProjectionEntryV1` 无 subject/Task、executor config、base recovery key（未claim无法恢复）、reserved/accounted usage、pair/terminal identity、partial confirmed/unknown digests、limitations、reconciliation source/evidence；projection top-level 也不固定 output reader/history revision或 protocol limits。AC-17 要求 crash/reopen 恢复这些 exact facts，当前 public projection 只能表达简化状态，命名 `projection_recovery` 测试没有逐字段验证。 | 扩展版本化 projection/hash view以无损表达批准合同全部 identity/state/budget/evidence，并加入 unclaimed/claimed/partial/unknown/reconciled reopen exact equality、unknown reader/reducer/current substitution 和 fixed horizon tests。 | closed |
| F-009 | Major | `crates/pareto-kernel/src/event_store/projection.rs:201-245,1544-1620` | 实现把 v3 source key加入共享 `ProjectionRegistry` 后，直接更新了既有 lifecycle reducer contract digest、v1 projection digest、snapshot digest和history chain golden 值（例如 reducer `898790...`→`5549c7...`）。批准 SPEC/RFC/ADR 明确要求旧 reader/reducer/snapshot bytes/identity 保持不变，并把任何旧 reducer identity 变化列为 implementation blocker；改测试期望不能构成兼容迁移。 | 恢复全部旧 golden bytes/identities，隔离新增 v3 reducer registration使 v1/v2 descriptor不变；对 accepted baseline逐 byte/digest比较并保留 old replay/snapshot tests。 | closed |

## Focused re-review evidence

以下证据由同一 independent Reviewer 对 exact remediation revision 独立读取和执行；不是对
FIX-0002或VALIDATION结论的转述。

- F-001 closed：`claim_effect_dispatch` 在 `BEGIN IMMEDIATE` 内调用
  `ensure_effect_dispatch_admissible`，后者refold lifecycle/control并检查exact reservation、Task、
  principal、settlement、cancellation与deadline equality；already-claimed返回`lease: None`。
  `claim_revalidates_cancellation_and_deadline_under_writer_lock`与23-test Effect suite通过。
- F-002 open：`execute_effect_to_terminal`补上了claim→invoke→terminal orchestration，但
  `FakeEffectExecutor::implementation_compatibility_digest()`仍由被注入的trait object自行返回；任意实现
  可以回报descriptor中的digest后进入`invoke`，没有sealed resolver或由Manifest identity解析出的
  implementation handle。`CrashedAfterReturn`也在同一调用中直接进入Receipt admission，测试只是让
  executor返回`Unknown`后同步结清，没有制造“external return后、terminal pair前进程退出”的history/
  recovery边界。最小修复：由Kernel按descriptor解析sealed Fake implementation，禁止调用对象自证identity；
  crash-after-return测试必须在terminal admission前中断，reopen后只走保守recovery且counter不增加。
- F-003 open：wrong producer/adapter已在写入前返回`Unauthorized`，trusted invalid output也会转成
  unknown conservative terminal；Clock、result-summary、canonical usage/limitations和Schema检查已加入。
  但`append_effect_terminal_pair`在`effect_runtime.rs:1334-1451`先提交control/Effect terminal pair，随后
  `append_authenticated_receipt_rejection`在`1452-1515`开启第二个独立transaction写mandatory rejection
  audit。两次commit之间崩溃或第二次写失败会留下已结算unknown但无拒绝审计，未满足原finding要求的
  “audit + atomic unknown settlement”；现有测试只覆盖两次写均成功。最小修复：把安全reject Event与
  terminal pair纳入一个writer transaction（或采用可证明不会丢失mandatory audit的等价原子协议），
  并注入terminal后/audit前故障证明零或完整提交。
- F-004 open：different-ID/new-sample在terminal存在且两个新ID均不存在时已返回ExistingTerminal no-op，
  same-ID mutation优先级和Clock派生current epoch也有测试。但`RecoverEffectCommandV1`仍直接携带
  `ClockSample`和epoch digest，`ClockSample`字段为`pub(super)`，`RuntimeClock`也可由任意Kernel sibling
  实现；没有recovery authority或Kernel-authenticated process-liveness observation。把自报digest改成
  自报Clock字段的hash不构成不可伪造epoch-loss证明。最小修复：由Kernel-owned clock/process epoch
  service签发用途受限recovery observation/authority并在command builder中封闭构造；加入伪造Clock/
  liveness proof no-write负测。
- F-005 open：代码现在验证source Event存在、位于exact Effect history、类型为
  `effect-reconciliation-required`，并核对Effect/attempt、原Receipt producer/adapter和payload digest。
  但该Event是原terminal conclusion，不是Manifest-pinned Fake query adapter/reconciliation producer
  生成的query observation；测试仍以`harness.command.effect_event_id`作为唯一source。调用者只需在
  `ReconcileEffectCommandV1`写入`reconciliation_policy_revision`和任选resolution，Kernel便自行构造
  `effect-reconciliation-observed`并关闭对账，故source/fingerprint校验不能证明resolution来源。
  最小修复：引入sealed、Manifest-pinned reconciliation producer/query observation admission，绑定exact
  external key与查询证据，由admitted observation决定resolution；terminal Event只能作为lineage，不能
  单独授权关闭，并加入wrong implementation/producer/resolution no-write测试。
- F-006 closed：Effect read逐pair读取Control counterpart、清空seal后重算两侧prepared digest和pair
  fingerprint；Control read又验证Effect counterpart并触发完整Effect history validation。缺任一侧和
  resealed drift负测通过。
- F-007 closed：`transition_task`的同一writer transaction调用Task-exact Effect guard；helper只阻塞目标
  Task的pending/claimed/open-reconciliation，其他Task不阻塞。`lifecycle_success_guard`通过。
- F-008 closed：projection entry现保留subject/Task、executor config、base/full recovery、reserve/
  accounted usage、两类pair、Receipt/partial/unknown/limitations/reconciliation evidence；top-level固定
  limits/output-reader/history revision。unclaimed与partial crash/reopen逐字段相等测试通过。
- F-009 closed：retained v1/v2 source registration已与v3 key隔离；独立静态比对确认旧golden恢复为
  reducer `898790...`、projection `1902d00...`、snapshot `72af2c...`、history `be0d9...`/`5fda2...`、
  projection-N `36bc5...`、snapshot-N `54797...`；`digest_golden`、`effect_v3_digest_golden`与retained
  compatibility tests均通过。

# Acceptance trace

| Acceptance | Independent result |
|---|---|
| AC-01 | not satisfied：descriptor贯穿identity，但implementation仍由注入对象自报；见F-002。 |
| AC-02 | partially satisfied：request/claim准入已闭合；recovery/reconciliation authority仍不闭合；见F-004/F-005。 |
| AC-03 | satisfied：reserve/Intent pair与claim前writer内再准入均有代码和负测。 |
| AC-04 | satisfied：exact retry、same-key request/executor mutation由registry config覆盖并通过测试。 |
| AC-05 | not satisfied：retry不再重发lease，但executor implementation substitution仍可能；见F-002。 |
| AC-06 | partially satisfied：Kernel orchestration覆盖outcome；return-before-terminal crash未真实建模；见F-002。 |
| AC-07 | partially satisfied：wrong source no-write与invalid unknown settlement成立；mandatory rejection audit不原子；见F-003。 |
| AC-08 | partially satisfied：状态fold闭合；crash-after-return recovery proof仍不足；见F-002/F-004。 |
| AC-09 | satisfied：partial confirmed/unknown摘要、usage与limitations保留到Event/Projection/Inventory。 |
| AC-10 | not satisfied：ExistingTerminal和mutation已实现，process-loss authority仍可自报；见F-004。 |
| AC-11 | not satisfied：source lineage校验存在，但没有受信query producer observation；见F-005。 |
| AC-12 | partially satisfied：双stream writer/read完整性闭合；invalid Receipt rejection audit不与settlement原子；见F-003。 |
| AC-13 | satisfied：claim refold、cancel/deadline equality与success guard均在writer transaction。 |
| AC-14 | partially satisfied：wrong producer no-write、late/exact duplicate路径成立；invalid trusted audit仍可能丢失；见F-003。 |
| AC-15 | satisfied：单连续Effect stream，无第二mutable authority，pair读取双向验证。 |
| AC-16 | partially satisfied：scope/source no-write显著增强；recovery/reconciliation authority仍不足；见F-004/F-005。 |
| AC-17 | satisfied：Projection无损字段、explicit horizon/history identity及counterpart fail-closed已有测试。 |
| AC-18 | satisfied：V2 inventory、fixed-horizon Recorded replay及零写入/零执行合同由protocol+Kernel tests覆盖。 |
| AC-19 | satisfied：retained v1/v2 schema/reducer/projection/snapshot bytes与golden恢复，v3隔离。 |
| AC-20 | not fully satisfied：23个Effect tests及impacted/full门禁通过，但F-002..F-005所列fault/authority负测缺失。 |
| AC-21 | not satisfied：crate-private不等于用途受限authority；executor/recovery/reconciliation仍可由普通内部值自组。 |
| AC-22 | satisfied：Run与Task exact guard均接入同一writer transaction并有目标/非目标Task测试。 |

# Compatibility, permission, and isolation review

- Manifest v3、独立SchemaSet和SQLite v2符合ADR；scope脚本确认DB v2、retained schema目录、依赖、Fake-only与replay read-only。
  retained golden还由Reviewer直接比对并通过Projection/Protocol tests，F-009关闭。
- request/claim/Receipt scope与wrong-producer no-write已显著加强；但“private struct”不等于闭合authority：executor trait、
  recovery Clock/process epoch与reconciliation resolution仍缺sealed producer/authority，见F-002/F-004/F-005。
- generic atomic pair及读取侧counterpart reseal验证关闭F-006；但authenticated-invalid rejection audit在terminal pair之后另行commit，
  仍存在合法单边缺audit状态，见F-003。

# Regression and test review

## Initial review evidence

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

## Focused re-review execution

Independent Reviewer在Windows PowerShell、2026-09-01、exact
`7eeb5f6d4095b7d2fdc6cc225e9b60c89482063f`执行的原始证据：

- `cargo test -p pareto-kernel event_store::effect_runtime::tests:: --offline`：
  `23 passed; 0 failed; 0 ignored; 162 filtered out`，exit 0。
- `cargo test -p pareto-kernel event_store:: --offline`：
  `184 passed; 0 failed; 1 ignored; 0 filtered out`，exit 0。
- `cargo test -p pareto-protocol --all-targets --all-features --offline`：unit `9 passed`；contract
  `25 passed`；baseline `1 ignored`，exit 0。contract中打印的
  `Error: "existing content-addressed schema set differs byte-for-byte"`是drift rejection负测的预期stderr，suite为pass。
- `cargo test --workspace --all-targets --all-features --offline`：Kernel
  `184 passed; 1 ignored`；Protocol unit `9 passed`、contract `25 passed`、baseline `1 ignored`，exit 0。
- `python -m unittest discover -s scripts/tests -p "test_*.py"`：`Ran 27 tests ... OK`，exit 0。
- `python scripts/check_req0009_scope.py`：
  `REQ-0009 scope check passed: DB v2 and retained sets frozen; Fake Effect only; replay read-only; dependencies unchanged`，exit 0。
- `cargo fmt --all -- --check`：无输出，exit 0。
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`：完成，exit 0。

上述绿灯支持已关闭finding；它们不覆盖F-002的自报implementation、F-003两次transaction间故障、F-004伪造
process-liveness observation或F-005自报reconciliation resolution，故不能把全绿解释为0 open Major。

# Scope and unrelated changes

exact diff `60cee6ed44d150185bf99ca3095a8ce803bcc0d3..6cad604ffe5ec2126f9745bf22ece713f2c0ce85`
包含124个文件、7688 insertions、137 deletions。Cargo manifests/lock、SQLite DDL/trigger和历史SchemaSet目录没有变化；新增一个
content-addressed Effect set、Effect runtime/protocol/tests/scope script及work evidence属于REQ-0009范围。旧Review freshness元数据前移属于交付门禁记录。
但既有Projection golden identity的改写不是允许的v3演进，属于F-009兼容回归，不可作为无关更新接受。

focused remediation diff
`6cad604ffe5ec2126f9745bf22ece713f2c0ce85..7eeb5f6d4095b7d2fdc6cc225e9b60c89482063f`
包含100个文件、1913 insertions、207 deletions；主体为Effect runtime/tests、Lifecycle/Runtime Control/Projection、
Protocol Effect字段、新content-addressed SchemaSet、FIX/VALIDATION与本Review。Cargo manifests/lock与SQLite版本未变化，
scope脚本确认依赖与边界未扩张；未发现与REQ-0009无关的运行时功能。

# Re-review conditions

实现者需修复F-002、F-003、F-004、F-005并补充上述required proof。Blocker/Major不能由实现者自关；同一
independent Reviewer应对新的exact commit和增量diff进行focused re-review，并复跑Effect、Impacted/Core/full gates。
只有`open_blockers: 0`且`open_majors: 0`时才可改为`approved`。

# Re-review history

- 2026-08-31：fresh independent code review of exact
  `6cad604ffe5ec2126f9745bf22ece713f2c0ce85` against accepted implementation baseline
  `60cee6ed44d150185bf99ca3095a8ce803bcc0d3`。结论0 Blocker、9 open Major、`changes-requested`；Reviewer只新增本Review记录，未修改实现源码、Schema或测试。
- 2026-09-01：同一independent Reviewer focused re-review exact remediation
  `7eeb5f6d4095b7d2fdc6cc225e9b60c89482063f` against initial implementation
  `6cad604ffe5ec2126f9745bf22ece713f2c0ce85`与accepted baseline
  `60cee6ed44d150185bf99ca3095a8ce803bcc0d3`。F-001/F-006/F-007/F-008/F-009 closed；
  F-002/F-003/F-004/F-005 open；结论0 Blocker、4 open Major、`changes-requested`。Reviewer仅修改本Review记录。
