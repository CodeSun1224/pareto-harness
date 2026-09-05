---
id: REVIEW-0013
title: REQ-0009 Effect Intent/Receipt 独立实现评审
status: approved
owners: [runtime-kernel]
created: 2026-08-31
updated: 2026-09-05
links: [REQ-0009, SPEC-0008, RFC-0009, ADR-0010, REVIEW-0012, FIX-0002]
independence: independent
reviewed_revision: 6df161ff5d5fc150cfa09f48ae54b7501cababcb
open_blockers: 0
open_majors: 0
---

# Verdict

`approved`。最终 F-005 focused re-review 固定 exact candidate
`25e84603f09c3e3c47c29846e9cc3ef1fe6a4d72`，基线为
`f3bf18e0129f397c998032979b0bf19dc055ca56`。实现新增一个 exact、互斥的
`validate_reconciliation_required_lineage`，并同时接入 Receipt terminal writer、Kernel recovery writer、
pure fold 与 reconcile source admission。Receipt Partial/Unknown 的 reason、result、confirmed 与 producer/
adapter/observed identity形态，以及 recovery Unknown 的空Receipt形态现均闭合；missing result/identity、
recovery+Receipt hybrid 与重封history稳定fail closed，reconcile零写入。

独立focused、Effect全集、workspace、governance、fmt、clippy与scope证据均通过；最终增量未发现新的
Blocker/Major。因此 F-005 closed，本Review为 0 Blocker、0 open Major。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs`; lifecycle/control tests | 初始实现的 claim authority、cancel/deadline 与 retry lease 不闭合。整改已在 writer transaction 内 refold exact lifecycle/control/effect，且 already-claimed 不再发 lease。 | writer-lock race 与 no-reinvoke 证明。 | closed |
| F-002 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:403-452,1003-1103`; `tests.rs:1367-1450` | 初始实现允许 executor 自报 implementation identity，且边界故障不闭合。整改改为 module-private concrete resolver，固定 implementation digest；claim→invoke→terminal/recovery 由 Kernel orchestration 掌握。 | wrong implementation zero-call；所有 Fake outcome terminal/recovery；crash 后 executor 不重入。 | closed |
| F-003 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:1450-1740`; Receipt tests | 初始实现 wrong producer 会写目标 stream，authenticated-invalid Receipt 的 audit 与 conservative settlement 不原子。整改实现 identity mismatch no-write，并将 rejection audit 与双-stream terminal pair 纳入同一 transaction。 | wrong producer no-write；invalid trusted Receipt 为完整三 Event 或零 Event。 | closed |
| F-004 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:1900-2150`; recovery tests | 初始 recovery epoch/liveness 可自报且 ExistingTerminal 顺序错误。整改使用 module-private `KernelRecoveryClock` 签发用途受限、purpose-bound authority，保留 same-ID exact/mutation 与 different-ID ExistingTerminal 优先级。 | forged authority no-write；epoch/new-sample/reopen bounded model。 | closed |
| F-005 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:491-538,1770-1800,2141-2166,2323-2330,3921`; `tests.rs:2621-2775` | 前一候选未严格区分 Receipt-backed 与 recovery-backed terminal source。最终整改引入共用 validator：Receipt Partial/Unknown 必须具有各自 exact reason、receipt/result、producer/adapter/observed identity及正确 confirmed presence；Kernel recovery 只允许 exact recovery reason/Unknown且全部Receipt派生字段缺失/空。writer、fold与reconcile使用同一判定。 | missing result/identity、recovery+Receipt hybrid 直接拒绝；重封 hybrid history read/reconcile fail closed且Event count不变；合法两类 lineage 仍可完成reconciliation。 | closed |
| F-006 | Major | Effect/Control pair reread | 初始 fold 未重算 counterpart。整改已双向读取、重算 prepared digests/pair fingerprint并对缺失或 resealed drift fail closed。 | 双侧丢失/漂移 reopen tests。 | closed |
| F-007 | Major | lifecycle Task transition | 初始仅 Run success guard。整改将 Task-exact Effect guard 接入同一 writer transaction，其他 Task 不误阻塞。 | Task scope/race tests。 | closed |
| F-008 | Major | Effect Projection | 初始 projection 丢失 authority、budget、partial/unknown 和 evidence。整改 projection 无损保存并固定 reader/history horizon。 | unclaimed/partial/reconciled reopen equality。 | closed |
| F-009 | Major | retained Projection identities | 初始 v3 registration 改写 v1/v2 golden。整改隔离 v3 并恢复 retained bytes/digests。 | retained golden byte/digest identity。 | closed |

# Focused re-review evidence

以下均为同一 independent Reviewer 对 exact commits 的源码、diff 与本地执行证据，不采信实现者结论。

- F-002 closed：`ResolvedFakeEffectExecutor` 的构造与 resolver 均为 module-private，resolver 将 descriptor
  implementation compatibility digest 与 Kernel 固定 digest exact 比较；wrong implementation 在 invoke 前拒绝。
  `CrashAfterReturn` 首次只留下 claimed history，retry 返回 AlreadyClaimed；close/reopen 后只走 recovery，
  executor counter保持1。
- F-003 closed：wrong producer/adapter 在 transaction 写入前 `Unauthorized`；authenticated-invalid Receipt
  被规范化为 Unknown，`append_authenticated_receipt_rejection` 已并入同一 writer transaction。
  `AfterPairBeforeAudit` fault 回滚三 Event，正常路径同时提交 control terminal、Effect terminal 与 rejection audit。
- F-004 closed：`KernelRecoveryClock` 从 canonical Clock/current epoch签发包含 scope/effect/attempt/cause 的
  purpose-bound `EffectRecoveryAuthority`；字段私有且 seal 验证。伪造 Clock/authority no-write；same-ID
  exact/mutation 与 new-sample ExistingTerminal 顺序符合 SPEC。
- F-005 closed：`validate_reconciliation_required_lineage` 是唯一共用判定。Receipt lineage要求
  receipt/result/producer/adapter/observed identity全部存在；Partial只接受`effect-partial`且confirmed存在，
  Unknown只接受`effect-unknown`且confirmed缺失。Recovery lineage只接受
  `effect-recovery-after-claim`/Unknown，并要求Receipt identity、usage、limitations与confirmed全部缺失/空；
  两个谓词必须恰有一个成立。Receipt admission与recovery writer先调用validated wrapper，fold与reconcile
  source admission直接调用同一validator。
- 新增`hybrid_reconciliation_lineage_fails_closed_without_writes`先证明合法Receipt与recovery两类，再逐项拒绝
  missing result、missing Receipt identity与recovery+Receipt hybrid；随后重封hybrid terminal row，权威读取/
  reconcile返回`AggregateCorrupt`，数据库Event count前后相同。既有`fake_outcomes`继续证明合法recovery
  Unknown经sealed query producer可结为ResolvedNotApplied且external executor不重入。
- F-006/F-007/F-008/F-009 在前轮已由 counterpart reseal、Task exact guard、lossless projection 与 retained
  golden tests独立关闭；最终5文件diff未触碰这些实现面。

# Acceptance trace

| Acceptance | Independent result |
|---|---|
| AC-01..10 | satisfied：Manifest/executor authority、Intent/claim、idempotency、fault terminal、Receipt、partial与recovery paths均由已关闭F-001..F-004及独立测试覆盖。 |
| AC-11 | satisfied：sealed query producer、两类exact lineage与hybrid source拒绝均由共用validator和no-write测试覆盖。 |
| AC-12..14 | satisfied：双stream terminal/settlement/rejection原子，cancel/deadline与late Receipt规则已覆盖。 |
| AC-15 | satisfied：连续Event stream pure fold调用共用lineage validator，非法hybrid terminal fail closed。 |
| AC-16 | satisfied：scope、wrong implementation/producer/source均在写入前拒绝；未发现跨域存在性泄漏。 |
| AC-17 | satisfied：projection字段无损、pair完整性闭合，重封hybrid terminal读取稳定fail closed。 |
| AC-18 | satisfied：Inventory V2、explicit fixed horizon与Recorded replay零写入/零执行由Kernel/Protocol tests覆盖。 |
| AC-19 | satisfied：retained v1/v2 SchemaSet、reader/reducer/projection/snapshot identity保持 byte-identical，v3隔离。 |
| AC-20 | satisfied：新增hybrid fault test命中1/1，Effect 24/24与workspace/core gates通过。 |
| AC-21 | satisfied：外部接入只持 proposal/observation，不获得Event、lease、terminal或reconciliation producer authority。 |
| AC-22 | satisfied：Run/Task exact success guards均在 writer transaction 内。 |

# Compatibility, permission, isolation, and regression review

- 最终增量没有修改Protocol、Schema、SQLite DDL、Cargo dependency或retained identity；F-009兼容闭合不受影响。
- executor、recovery authority、reconciliation producer均为 module-private sealed construction；wrong
  implementation/producer/observation在目标 stream保持no-write。F-005整改只收紧既有权威Event语义，不扩张公开权限。
- terminal pair、rejection audit与counterpart reread/reseal仍保持原子/双向验证；最终增量只放宽合法的
  recovery-backed reconciliation source，并拒绝所有Receipt/recovery混合形态。

# Execution evidence

## 初始与前轮证据

- exact `6cad604ffe5ec2126f9745bf22ece713f2c0ce85`：Effect 19/19；Kernel 179 passed/1 ignored；
  Protocol unit 9、contract 25、baseline 1 ignored；governance 27；scope、fmt、clippy、schema byte identity、
  diff check通过。结论9 open Major。
- exact `7eeb5f6d4095b7d2fdc6cc225e9b60c89482063f`：Effect 23/23；Event Store 184 passed/1 ignored；
  workspace Kernel 184 passed/1 ignored、Protocol 9+25 passed/1 ignored；governance 27、scope、fmt、clippy通过。
  F-001/F-006/F-007/F-008/F-009关闭，4 open Major。
- exact `fc1e3d968b87a8cfea987bea306f53b6a5b8c468`：独立静态检查关闭F-002/F-003/F-004；
  Effect 23/23；workspace Kernel 184 passed/1 ignored、Protocol 9+25 passed/1 ignored；fmt/clippy/governance/scope通过。
  F-005因recovery Unknown lineage不可达保持open。

## `f3bf18e` focused re-review 原始证据

Reviewer在Windows PowerShell、2026-09-01、exact
`f3bf18e0129f397c998032979b0bf19dc055ca56`执行：

- `git rev-parse f3bf18e0129f397c998032979b0bf19dc055ca56`：输出同一完整hash，exit 0。
- `git diff --stat fc1e3d968b87a8cfea987bea306f53b6a5b8c468..f3bf18e0129f397c998032979b0bf19dc055ca56`：
  5 files changed, 94 insertions, 8 deletions；仅Effect runtime/test及TASKS/VALIDATION/FIX-0002。
- `git diff --check fc1e3d968b87a8cfea987bea306f53b6a5b8c468..f3bf18e0129f397c998032979b0bf19dc055ca56`：无输出，exit 0。
- `cargo test -p pareto-kernel event_store::effect_runtime::tests:: --offline`：
  `23 passed; 0 failed; 0 ignored; 162 filtered out`，exit 0，约27.9秒。
- `cargo fmt --all -- --check`：无输出，exit 0。
- `cargo clippy -p pareto-kernel --all-targets --all-features --offline -- -D warnings`：完成，exit 0。
- 一次误用不存在的`python scripts/check_requirement_scope.py REQ-0009`：Python报告文件不存在，exit 2；
  该操作没有修改文件。
- `python scripts/check_req0009_scope.py`：
  `REQ-0009 scope check passed: DB v2 and retained sets frozen; Fake Effect only; replay read-only; dependencies unchanged`，exit 0。
- `python scripts/check_docs.py`：exit 1；仅报告 REVIEW-0001..0012 reviewed revision freshness
  相对REQ-0009 substantive paths stale。本Review已绑定最终exact candidate，但Reviewer未被授权机械前移其他Review；
  故如实记录为未通过，不表示文档门禁为绿。
- `git diff --check`：无输出，exit 0；`git status --short`仅显示
  `M docs/reviews/REVIEW-0013-effect-intent-receipt-implementation.md`。

## `25e8460` 最终 focused re-review 原始证据

Reviewer在Windows PowerShell、2026-09-01、exact
`25e84603f09c3e3c47c29846e9cc3ef1fe6a4d72`执行：

- `git rev-parse 25e84603f09c3e3c47c29846e9cc3ef1fe6a4d72`：输出同一完整hash，exit 0。
- `git diff --stat f3bf18e0129f397c998032979b0bf19dc055ca56..25e84603f09c3e3c47c29846e9cc3ef1fe6a4d72`：
  6 files changed, 394 insertions, 226 deletions；其中Review文本来自独立Review record commit `644b784`，
  其完整hash为`644b784fcac6426551062cef6dd56f1b4023f182`；排除Review后产品整改为5文件、
  267 insertions、59 deletions，限于Effect runtime/test及TASKS/VALIDATION/FIX-0002。
- `git diff --check f3bf18e0129f397c998032979b0bf19dc055ca56..25e84603f09c3e3c47c29846e9cc3ef1fe6a4d72`：无输出，exit 0。
- `cargo test -p pareto-kernel event_store::effect_runtime::tests::hybrid_reconciliation_lineage_fails_closed_without_writes --offline -- --exact`：
  `1 passed; 0 failed; 185 filtered out`，exit 0，约0.91秒。
- `cargo test -p pareto-kernel event_store::effect_runtime::tests:: --offline`：
  `24 passed; 0 failed; 0 ignored; 162 filtered out`，exit 0，约25.58秒。
- `cargo test --workspace --all-targets --all-features --offline`：Kernel
  `185 passed; 0 failed; 1 ignored`；Protocol unit `9 passed`、contract `25 passed`、baseline `1 ignored`，
  exit 0。ignored项仍为无阈值performance observation；schema drift rejection测试的预期stderr不影响pass。
- `python -m unittest discover -s scripts/tests -p "test_*.py"`：`Ran 27 tests ... OK`，exit 0。
- `cargo fmt --all -- --check`：无输出，exit 0。
- `cargo clippy -p pareto-kernel --all-targets --all-features --offline -- -D warnings`：完成，exit 0。
- `python scripts/check_req0009_scope.py`：
  `REQ-0009 scope check passed: DB v2 and retained sets frozen; Fake Effect only; replay read-only; dependencies unchanged`，exit 0。
- 更新本Review后`python scripts/check_docs.py`：exit 1；仅报告REVIEW-0001..0012相对REQ-0009
  substantive paths的既有freshness缺口。本Review已绑定exact candidate；Reviewer未被授权修改其他Review，
  因而不把docs gate表述为通过。
- 更新本Review后`git diff --check`：无输出，exit 0；`git status --short`仅显示
  `M docs/reviews/REVIEW-0013-effect-intent-receipt-implementation.md`。

# Scope and unrelated changes

最终增量
`f3bf18e0129f397c998032979b0bf19dc055ca56..25e84603f09c3e3c47c29846e9cc3ef1fe6a4d72`
除携带Review record外，产品改动仅为共用lineage validator、其四处接入、hybrid/resealed负测与对应
TASKS/VALIDATION/FIX-0002证据。未发现无关运行时功能、依赖增长、Schema/SQLite或旧字节变化。

# Re-review conditions

无open re-review condition。F-001..F-009均由同一 independent Reviewer在绑定的exact revisions上关闭；
若后续再修改受信Effect authority、Event shape/fold、pair、reconciliation或retained identity，须重新执行独立评审。

# Re-review history

- 2026-08-31：fresh independent review exact `6cad604ffe5ec2126f9745bf22ece713f2c0ce85`
  against accepted baseline `60cee6ed44d150185bf99ca3095a8ce803bcc0d3`；0 Blocker、9 Major，
  `changes-requested`。
- 2026-09-01：focused re-review exact `7eeb5f6d4095b7d2fdc6cc225e9b60c89482063f`；
  F-001/F-006/F-007/F-008/F-009 closed，0 Blocker、4 Major，`changes-requested`。
- 2026-09-01：focused re-review exact `fc1e3d968b87a8cfea987bea306f53b6a5b8c468`；
  F-002/F-003/F-004 closed，发现F-005 recovery Unknown lineage残余；0 Blocker、1 Major，`changes-requested`。
- 2026-09-01：final focused re-review exact `f3bf18e0129f397c998032979b0bf19dc055ca56`
  against `fc1e3d968b87a8cfea987bea306f53b6a5b8c468`；合法recovery Unknown链路与no-reinvoke已证明，
  但F-005因hybrid terminal/source形状未fail closed保持open；0 Blocker、1 Major，`changes-requested`。
  Reviewer仅修改本Review文件，未修改实现、Requirement状态或其他文件。
- 2026-09-01：最终F-005 focused re-review exact
  `25e84603f09c3e3c47c29846e9cc3ef1fe6a4d72` against
  `f3bf18e0129f397c998032979b0bf19dc055ca56`（前次Review record commit
  `644b784fcac6426551062cef6dd56f1b4023f182`）。共用exact/互斥
  lineage validator已接入writer、fold、reconcile；missing/hybrid/resealed history proof与全workspace门禁通过。
  F-005 closed；0 Blocker、0 Major，`approved`。Reviewer仅修改本Review文件，未修改实现或提交。
- 2026-09-01：closure freshness exact `62bc44e250587594912f7ef16b431be6b1c12103`。相对本Review批准的runtime exact `25e84603f09c3e3c47c29846e9cc3ef1fe6a4d72`，后续仅提交Review文本和行为中立done/archive/fact sync；无runtime、Schema、权限或finding变化，approved 0/0保持。
- 2026-09-05：Verified Procedure 路线 freshness re-review exact `660cfca9e230f1440505c8e3bfd9a07bf17529ab`。candidate对Effect runtime、Protocol、Schema、DB、Cargo、tests、pair/reconciliation authority与retained identity零差异；ARCH-0003现准确记录REQ-0009 Schema/Kernel Runtime/Projection/Boundary Inventory V2已实现，未来Procedure/Node仅以前向identity绑定消费既有边界。REVIEW-0013保持approved、0/0。
- 2026-09-05：路线接受 closure freshness exact `6df161ff5d5fc150cfa09f48ae54b7501cababcb`。closure对Effect Runtime、Protocol、Schema、DB、Cargo、tests、pair/reconciliation authority与retained identity零差异；ADR-0012不重释已发生Effect或授予外部writer，ARCH-0003 implemented事实保持准确。REVIEW-0013保持approved、0/0。
