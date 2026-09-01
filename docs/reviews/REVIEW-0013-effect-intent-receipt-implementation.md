---
id: REVIEW-0013
title: REQ-0009 Effect Intent/Receipt 独立实现评审
status: changes-requested
owners: [runtime-kernel]
created: 2026-08-31
updated: 2026-09-01
links: [REQ-0009, SPEC-0008, RFC-0009, ADR-0010, REVIEW-0012, FIX-0002]
independence: independent
reviewed_revision: f3bf18e0129f397c998032979b0bf19dc055ca56
open_blockers: 0
open_majors: 1
---

# Verdict

`changes-requested`。最终 focused re-review 固定 exact candidate
`f3bf18e0129f397c998032979b0bf19dc055ca56`，基线为
`fc1e3d968b87a8cfea987bea306f53b6a5b8c468`。新端到端路径已经证明
`CrashAfterReturn → close/reopen → Kernel recovery Unknown → Manifest-pinned sealed query producer
→ reconcile` 可达，且 external executor 只调用一次；因此 F-005 上轮发现的“recovery Unknown 永远不可对账”
残余已修复。

但 F-005 尚不能关闭。source admission 对 Receipt-backed lineage 的谓词不是完整、互斥的合法形状：只要求
producer/adapter、`receipt_digest` 与 `observed_at` 存在，不要求 `result_digest` 存在，也不限制
`reason_code` 为 Receipt terminal 的合法 reason。于是含 `effect-recovery-after-claim` reason 与 Receipt 字段的
混合 terminal payload 会被误当 Receipt-backed source。pure fold 同样没有验证两类 terminal payload 的互斥
语义。正常 writer 当前不会产生该形状，但 exact validated Event history 是权威输入，REQ-0009 AC-15/17
要求非法历史 fail closed；已有 pair reread/reseal 威胁模型也不能假定“writer 不生成”即可。当前没有相应
hybrid/resealed-history no-write 测试。因此结论为 0 Blocker、1 open Major，REQ-0009 不得进入
`verified`/`done`。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs`; lifecycle/control tests | 初始实现的 claim authority、cancel/deadline 与 retry lease 不闭合。整改已在 writer transaction 内 refold exact lifecycle/control/effect，且 already-claimed 不再发 lease。 | writer-lock race 与 no-reinvoke 证明。 | closed |
| F-002 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:403-452,1003-1103`; `tests.rs:1367-1450` | 初始实现允许 executor 自报 implementation identity，且边界故障不闭合。整改改为 module-private concrete resolver，固定 implementation digest；claim→invoke→terminal/recovery 由 Kernel orchestration 掌握。 | wrong implementation zero-call；所有 Fake outcome terminal/recovery；crash 后 executor 不重入。 | closed |
| F-003 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:1450-1740`; Receipt tests | 初始实现 wrong producer 会写目标 stream，authenticated-invalid Receipt 的 audit 与 conservative settlement 不原子。整改实现 identity mismatch no-write，并将 rejection audit 与双-stream terminal pair 纳入同一 transaction。 | wrong producer no-write；invalid trusted Receipt 为完整三 Event 或零 Event。 | closed |
| F-004 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:1900-2150`; recovery tests | 初始 recovery epoch/liveness 可自报且 ExistingTerminal 顺序错误。整改使用 module-private `KernelRecoveryClock` 签发用途受限、purpose-bound authority，保留 same-ID exact/mutation 与 different-ID ExistingTerminal 优先级。 | forged authority no-write；epoch/new-sample/reopen bounded model。 | closed |
| F-005 | Major | `crates/pareto-kernel/src/event_store/effect_runtime.rs:2258-2289,3875-3920`; `tests.rs:1367-1450,2400-2570` | sealed、Manifest-pinned reconciliation producer和 recovery-backed source 已实现；最终候选也使 recovery Unknown 可被对账。但 source predicate 并未严格区分完整 Receipt-backed 与 Kernel recovery-backed payload：Receipt 分支不要求 `result_digest` 且不限制 Receipt reason，fold 也未验证两类 terminal shape。重封的 hybrid terminal 可作为合法 source，而现有 forged-source 测试只覆盖 wrong producer/resolution/nonexistent Event，不覆盖 hybrid payload。此缺口破坏 AC-11/15/17 的 closed evidence 与非法历史 fail-closed。 | 提取 writer、fold、reconcile 共用的 exact、互斥 lineage validator。Receipt-backed 至少要求合法 Receipt reason/outcome、pinned producer/adapter、receipt/result digest、observed time及其 outcome-specific字段；recovery-backed 只接受 exact recovery reason/Unknown 且所有 Receipt-derived字段缺失/空。对重封 hybrid（recovery reason + Receipt fields、Receipt reason + missing result/identity、两类可选字段混配）证明 projection/read fail closed，reconcile 调用零写入。 | open |
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
- F-005 已修部分：最终 diff 在 `effect_runtime.rs:2270-2289` 增加 recovery-backed predicate；
  `fake_outcomes` 建立真实 CrashAfterReturn claim-only history，close/reopen 后用 Kernel recovery写 Unknown，
  再由固定 reconciliation implementation生成 sealed observation并结为 ResolvedNotApplied。测试断言 external
  executor counter仍为1，query producer counter为1。
- F-005 仍 open：同一 predicate 的 Receipt-backed 分支仅检查4个字段；`fold_effect_events` 的
  `effect-reconciliation-required-v1` 分支只检查 pair/outcome/limitations/order/state，并将任意其余 optional
  fields投影为权威事实。故 hybrid payload既不会在 fold fail closed，也可通过 source admission。现有
  reconciliation负测不构造或重封这类非法历史。
- F-006/F-007/F-008/F-009 在前轮已由 counterpart reseal、Task exact guard、lossless projection 与 retained
  golden tests独立关闭；最终5文件diff未触碰这些实现面。

# Acceptance trace

| Acceptance | Independent result |
|---|---|
| AC-01..10 | satisfied：Manifest/executor authority、Intent/claim、idempotency、fault terminal、Receipt、partial与recovery paths均由已关闭F-001..F-004及独立测试覆盖。 |
| AC-11 | not fully satisfied：sealed query producer与recovery Unknown路径成立，但非法hybrid source仍可被接纳；见F-005。 |
| AC-12..14 | satisfied：双stream terminal/settlement/rejection原子，cancel/deadline与late Receipt规则已覆盖。 |
| AC-15 | not fully satisfied：连续Event stream存在，但 fold未拒绝非法hybrid reconciliation-required payload；见F-005。 |
| AC-16 | satisfied：scope、wrong implementation/producer/source均在写入前拒绝；未发现跨域存在性泄漏。 |
| AC-17 | not fully satisfied：projection字段无损且pair完整性闭合，但重封hybrid terminal不会fail closed；见F-005。 |
| AC-18 | satisfied：Inventory V2、explicit fixed horizon与Recorded replay零写入/零执行由Kernel/Protocol tests覆盖。 |
| AC-19 | satisfied：retained v1/v2 SchemaSet、reader/reducer/projection/snapshot identity保持 byte-identical，v3隔离。 |
| AC-20 | not fully satisfied：Effect 23/23与既有 impacted/core/full gates通过，但缺F-005 required hybrid fault test。 |
| AC-21 | satisfied：外部接入只持 proposal/observation，不获得Event、lease、terminal或reconciliation producer authority。 |
| AC-22 | satisfied：Run/Task exact success guards均在 writer transaction 内。 |

# Compatibility, permission, isolation, and regression review

- 最终增量没有修改Protocol、Schema、SQLite DDL、Cargo dependency或retained identity；F-009兼容闭合不受影响。
- executor、recovery authority、reconciliation producer均为 module-private sealed construction；wrong
  implementation/producer/observation在目标 stream保持no-write。F-005是权威历史语义校验缺口，不是公开API权限扩张。
- terminal pair、rejection audit与counterpart reread/reseal仍保持原子/双向验证；最终增量只放宽合法的
  recovery-backed reconciliation source。

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

## 最终 focused re-review 原始证据

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

最终候选没有重跑完整workspace suite；其基线`fc1e3d`完整suite为绿，最终diff仅5文件且focused suite覆盖
全部新增运行时路径。该取舍不影响F-005结论：绿灯测试没有构造hybrid/resealed source。

# Scope and unrelated changes

最终增量
`fc1e3d968b87a8cfea987bea306f53b6a5b8c468..f3bf18e0129f397c998032979b0bf19dc055ca56`
包含5文件、94 insertions、8 deletions。运行时改动仅为reconciliation source lineage与CrashAfterReturn端到端回归；
TASKS、VALIDATION、FIX-0002同步记录证据。未发现无关运行时功能、依赖增长、Schema/SQLite或旧字节变化。

# Re-review conditions

仅F-005保持open。最小修复是让 writer、fold 与 reconcile source admission共用 exact、互斥的
Receipt-backed/recovery-backed terminal shape validator，并补 hybrid/resealed-history fail-closed与reconcile
no-write负测。不要求改变合法CrashAfterReturn recovery链路、Schema或外部API。Blocker/Major不能由实现者
自关；同一 independent Reviewer须对新的exact commit与增量diff复审。只有`open_blockers: 0`且
`open_majors: 0`时才可改为`approved`。

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
