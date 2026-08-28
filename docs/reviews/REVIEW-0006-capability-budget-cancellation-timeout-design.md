---
id: REVIEW-0006
title: REQ-0007 Capability、预算、取消与超时独立设计评审
status: approved
owners: [independent-reviewer]
created: 2026-08-25
updated: 2026-08-25
links: [REQ-0007, SPEC-0006, RFC-0006, ADR-0007, REQ-0003, REQ-0004, REQ-0005, REQ-0006, ARCH-0002, ARCH-0003]
independence: independent
reviewed_revision: 3318cbc6fe8bc8c9717a5a2b4aea1153f0d281d6
open_blockers: 0
open_majors: 0
---

# Verdict

批准进入实现。第二次 focused independent design re-review 固定候选提交
`a4e34785908207e622365250ae1466b85b4baecb`，重点审查设计差异
`cfa7a06c3588a6ad975a9511140d0984f5eb1b8f..a4e3478`，并确认上一轮已关闭的
F-001、F-002、F-003、F-005、F-006 没有回退。F-004 required proof 现已由一致、durable、
可测试的 Requirement/Spec/RFC/ADR 合同关闭：reserve event 固定完整 `TimeoutKeyV1`；Kernel recovery
authority 从该key、canonical Clock sample和构造时冻结的verified/unknown usage evidence确定性产生
domain-separated canonical fingerprint及command/event ID；事务按integrity/isolation、same-ID exact或mutation、
different-ID existing terminal、Clock due的固定优先级处理；`not_due`不append或消费identity；commit-response-loss
以exact command bytes重试，reopen丢失command时用新sample/new ID取得既有terminal且不重复append或核算。

本评审及本轮复审均为 independent design review。Reviewer 未参与设计或实现，不采信历史self-review作为批准
证据。候选worktree开始时clean，`cfa7a06..a4e3478`只含REQ-0007设计、active work和Review记录，
没有Runtime、Schema、DB、Cargo、public API或依赖实现变更；本轮没有运行或评价不存在的Runtime实现。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `SPEC-0006:39-52,188,192,198`; `RFC-0006:76-80,102-119,221,264`; REQ-0007 AC-02/06/12 | 首轮缺少Run/Task lifecycle状态准入，可能在paused/terminal后reserve/dispatch。候选冻结init/manage/reserve/settle矩阵；reserve与control-capable lifecycle pause/terminal transition均在同一writer transaction fold两条stream，pending operation阻止transition，旧set/no-control Run保持REQ-0005语义。 | 全状态表与two-pool `lifecycle_reserve_race`计划证明任一提交顺序只有一个合法结果，terminal/pause后无新dispatch。 | closed |
| F-002 | Major | `SPEC-0006:86-96,125,169`; `RFC-0006:127-135,177-193,227,231`; REQ-0007 AC-09/10/18 | 首轮没有cancel request/ack authority。候选冻结Run/Task owner-only、operation owner或persisted subject、probe只读、ack仅approved producer+opaque lease或Kernel recovery authority；Gate/Hook/plugin只能proposal，未授权/跨域不写目标，并新增稳定request/ack接口。 | `cancellation_authority`和propagation矩阵覆盖owner/subject/issuer/同域无关actor/跨域、ancestor、重复/乱序ack；未来Capability management须新RFC/Schema。 | closed |
| F-003 | Major | `SPEC-0006:24-26,62,76-82,113-125`; `RFC-0006:36,88,121-123,156-165,226-227`; REQ-0007 AC-08/13/16 | 首轮callback/settlement producer与evidence authority未定义。候选以retained `TrustedOperationContractV1`、不可序列化opaque lease、producer/operation/reservation/namespace/epoch绑定和首片唯一`kernel_meter_v1`闭合；同scope ID holder不能settle、触发unknown或污染late audit。 | `callback_authority`/usage/idempotency/late矩阵覆盖错误binding、stale/revoked producer、伪造verified evidence、unknown触发权、late与跨域无写入。 | closed |
| F-004 | Major | `SPEC-0006` Timeout/recovery contract与AC-08/11/12；`RFC-0006` §5/Failure modes/Evaluation；`ADR-0007`；REQ-0007 AC-11/12/18/19 | 候选把`TimeoutKeyV1`逐字段绑定persisted operation/source/clock policy；`TimeoutRecoveryCommandV1`固定key、canonical Clock sample、冻结usage evidence fingerprint并用版本化domain的canonical SHA-256派生fingerprint及command/event ID。锁内处理顺序先验证integrity/isolation/key，再处理same-ID exact/mutation，再处理different-ID terminal no-op，最后判断Clock；`not_due`不append/不消费identity，commit-response-loss exact bytes重试，reopen新sample/new ID不重复event或budget。无callback settlement、verified partial/unknown、deadline equality及winner race语义保持。 | `timeout_recovery`、`idempotency`、`terminal_race`及golden矩阵已明确覆盖key/fingerprint/event ID、not-due、新sample、response loss、same-ID mutation、different-ID terminal no-op、reopen和零重复核算；实现证据由后续独立代码评审验证。 | closed |
| F-005 | Major | `SPEC-0006:24,119,125,179-181,203`; `RFC-0006:27,169-175,219,234,254-260`; REQ-0007 AC-17 | 首轮Manifest与control source SchemaSet/limits关系不明确。候选唯一选择Manifest-pinned规则：只有新control-capable set Run可在created初始化，sequence-1、每个row、limits与reducer source均exact；四个完整hash旧set Run拒绝后加control但保留既有lifecycle/Projection/Snapshot；current/alternate/漂移/unknown均fail closed并保留reader/reducer。 | `schema_manifest_binding`、protocol retained-set与Event Store回归覆盖新/四旧set、mismatch/current/alternate、row/limits drift、unknown major、DB v2和rollback。 | closed |
| F-006 | Major | `SPEC-0006:25,62-66,123,156-165,191-192`; `RFC-0006:88,100-123,203-213,223-226`; REQ-0007 AC-05/06/08 | 首轮不可信requested vector可低报，hard limit可能effect后才发现。候选将其降为observation，retained operation contract确定性产生完整finite trusted envelope；全部Fake资源由Kernel meter在下一单位越界前中止，无法证明/enforce上界则effect前拒绝，reserve/Capability/accounts均按trusted vector。真实Provider/Tool注册明确延期。 | `resource_envelope`/budget/producer测试覆盖低报、零/漏维度、无retained contract、meter violation、unknown、partial/cancel/timeout，并证明no real Provider/Tool scope。 | closed |
| F-007 | Minor | `SPEC-0006:183-206`; active `PLAN.md` Validation | 首轮过滤命令可0命中且static inspection不可复算。候选现计划新增`assert_cargo_test_filter.py`先list并拒绝0命中，为每个Major场景给出稳定filter，并用`check_req0007_scope.py`检查DB/clock/scope/replay依赖；设计层可复算性已改善。脚本和测试尚属实现任务，本设计复审不宣称已运行。 | 实施时提交helper/static脚本，Validation逐filter记录非零count/result；独立代码评审验证脚本本身不漏检且所有命令可复跑。 | accepted |
| F-008 | Note | `.agents/work/active/REQ-0007-capability-budget-cancellation-timeout/TASKS.md:12` | 固定提交把后续独立代码评审编号预写为 `REVIEW-0006`；本正式独立设计评审按当时 next available ID 已占用该编号。编号不影响产品合同，但后续不得覆盖本记录。 | 实施后的 fresh independent code review 使用下一个可用 ID（预期 `REVIEW-0007`），并在 Plan/Tasks 正常修订阶段更新引用；保留本评审历史。 | accepted |

# Constitutional effect trace

| Effect path | Candidate design evidence | Independent result |
|---|---|---|
| Capability issue/delegate/revoke | principal → persisted owner/parent authority → lifecycle guard → subset/revocation/expiry → control Event → pure fold → Projection/reopen | 默认拒绝、owner无operation bypass、delegate收窄与management lifecycle矩阵闭合。 |
| Protected operation allow | proposal → Manifest-pinned readers → lifecycle guard → Capability → trusted operation contract/envelope → cancel/deadline → atomic reserve Event → opaque lease/Fake meter → authorized settlement Event → Projection/recovery | state、authority、pre-effect hard budget与atomic reserve链闭合，F-001/F-006 closed。 |
| Protected operation deny | integrity/isolation → lifecycle/capability/envelope/budget decision → same-aggregate safe denial Event；unauthorized/cross-scope no append | stable errors和安全audit边界可测试；cancel/callback的同域低权限主体也被独立authority table拒绝。 |
| Cancellation | owner或persisted operation subject → request authority → cancellation Event → ancestor predicate → lease probe → producer/recovery ack + settlement → Projection/reopen | request/probe/ack/settlement authority与no-write负例闭合，F-002 closed。 |
| Deadline/timeout | persisted TimeoutKey + trusted Clock sample + frozen Kernel-meter evidence → Kernel recovery authority → canonical command/event identity → lock-time integrity/idempotency/terminal/due checks → timed-out settlement → Projection/reopen | exact identity、无callback、verified/unknown核算、winner、response-loss与reopen路径闭合，F-004 closed；后续实现须证明无重复event/budget。 |
| Budget settlement/refund | trusted envelope/meter + producer/lease → checked consume/release或authorized unknown → settlement Event；owner refund → correction Event → fold | producer authority、hard limit、gross/refund/net和terminal no-reopen闭合，F-003/F-006 closed。 |
| Late/duplicate/out-of-order | producer+lease → AlreadyApplied/conflict或safe digest audit；unauthorized producer no append → counters/replay | terminal no-mutation、redaction与audit admission闭合。 |
| Recorded replay | exact source control history → retained reader/reducer → Projection/digest；无executor/append authority | read/fold-only类型隔离满足replay零执行/零重复核算的设计要求；仍需实现证据证明counter/event/budget不变。 |

# Acceptance trace

| Acceptance | Review result | Evidence and gap |
|---|---|---|
| AC-01 | 设计满足 | Capability闭合字段、subject/scope/resource/operation/time/delegation/parent及“payload不等于authority”保持冻结。 |
| AC-02 | 设计满足 | exact lifecycle history进入private admission；init/manage/reserve/settle矩阵和transition/reserve serialization关闭F-001。 |
| AC-03 | 设计满足 | root owner、delegate subset、full-chain revalidation、revocation/expiry保持明确。 |
| AC-04 | 设计满足 | same-aggregate safe denial、跨域/未授权no-write及新增management/producer authority reason均闭合。 |
| AC-05 | 设计满足 | canonical dimensions与账本之外，retained trusted envelope/meter在effect前覆盖全部required dimensions，关闭F-006。 |
| AC-06 | 设计满足 | lifecycle、Capability、envelope和多scope account在单writer transaction全有或全无；余额与transition竞争均有模型计划。 |
| AC-07 | 设计满足 | consume/release/refund、partial、unknown、无callback timeout核算及exact recovery retry均已冻结。 |
| AC-08 | 设计满足 | 首片仅Kernel meter authoritative；producer/lease/evidence/namespace/epoch绑定和unknown触发权关闭F-003。 |
| AC-09 | 设计满足 | Run/Task owner-only、operation owner/subject、probe/ack authority及稳定API/no-write语义冻结。 |
| AC-10 | 设计满足 | cooperative probe/ack、uninterruptible callback或Kernel recovery及四终态互斥闭合。 |
| AC-11 | 设计满足 | 双时钟、persisted TimeoutKey、canonical sample、冻结evidence、确定性ID/fingerprint、`not_due`、exact response-loss retry、无callback/reopen和unknown结算均已冻结。 |
| AC-12 | 设计满足 | integrity/isolation、same-ID exact/mutation、different-ID terminal、Clock due优先级，以及deadline equality、cancel/callback/timeout和lifecycle/reserve winner均已冻结并映射模型测试。 |
| AC-13 | 设计满足 | 只有原producer/lease可duplicate/late；mutation conflict、unauthorized no-write和audit no-mutation明确。 |
| AC-14 | 设计满足 | 单control stream、exact validated fold、无第二权威表及corruption fail-closed保持。 |
| AC-15 | 设计满足 | full-provenance Projection、完整历史Recorded replay和无executor/writer/account mutation保持。 |
| AC-16 | 设计满足 | 全scope/ID exact之外，同域低权限cancel/callback、错误lease/account/set均有负测计划。 |
| AC-17 | 设计满足 | Manifest-pinned新set唯一规则、四旧set拒绝升级、row/limits/reducer exact、DB v2与retention关闭F-005。 |
| AC-18 | 设计满足 | stable private interfaces含cancel/probe/ack/callback/envelope及版本化timeout key/recovery command，且不提前实现下游Provider/Tool框架。 |
| AC-19 | 设计追踪满足 | 矩阵覆盖F-001..F-006及key/ID golden、not-due、response-loss、same-ID mutation、different-ID terminal、reopen零重复核算，并要求helper证明filter非零；测试尚待实现。 |
| AC-20 | 仅为计划 | `907eee7..a4e3478`无Runtime/Schema/DB/API/依赖变化；完整REQ-0003..0006回归仍属于实施后验证，不能由设计复审宣称通过。 |

# Compatibility, permission, and isolation review

- control Event envelope恒为Manifest owner；requester、subject、producer与cancellation requester是闭合payload事实，只有owner/parent、opaque lease+registered producer或Kernel recovery authority能形成对应admission。公开Schema值、ID、Projection、Replay和同scope身份都不取得authority。
- tenant、user presence/value、workspace、run、owner、subject、Task、stream和全部业务ID仍exact隔离；取消和callback另有同域权限矩阵，未授权及跨域都不写目标。
- lifecycle/control两stream共享SQLite writer：reserve与pause/terminal transition锁内fold两条history，既避免terminal后dispatch，也保持旧set/no-control Run的REQ-0005状态合同。该additive guard不重释lifecycle `cancelled`或既有Event。
- 新control source只能来自immutable Manifest固定的新control-capable set/limits；四个retained旧set完整hash已列出且不升级。RunTask reducer仍只抽取四个lifecycle binding；SQLite v2、Snapshot DDL/trigger/checksum和已有reader identity不变。
- retained trusted operation contract与Kernel meter把hard budget enforcement前移至dispatch前；真实Provider/Tool/Effect/Receipt authority明确延期，不在候选中实现。
- Projection仍是Event派生，Recorded replay没有executor、append、timeout/recovery writer或mutable budget authority；quality、cost、latency继续分开且没有优化声明。

# Regression and test review

本轮是固定提交上的设计复审，不是实现或代码评审。候选没有Runtime、Schema或DB实现，因此未把Cargo、Schema
生成、性能或active VALIDATION baseline当成REQ-0007行为证据。后续实现至少需要：

- Focused：落实F-001..F-006所列状态、authority、callback、timeout identity、SchemaSet和resource-envelope表/模型测试；每个filter记录非零命中。
- Impacted：REQ-0005 lifecycle terminal/concurrency、REQ-0006 reducer/source identity/Recorded replay、Event Store transaction/idempotency/isolation与所有retained SchemaSet回归。
- Core：跨tenant/user presence-value/workspace/run/actor/Task全矩阵、同域低权限主体、two-pool race、real SQLite close/reopen、unknown/current substitution、replay零dispatch/append/accounting。
- Compatibility：明确Manifest/control source set规则后，证明四个旧set byte-identical、旧Run的受支持/拒绝行为、新set重复生成、DB v2 actual DDL/trigger bytes和RunTask reducer identity不变。
- 静态与治理：`python scripts/check_docs.py`、completion gates、可复现no-real-sleep/no-Provider/Tool/Effect/API/dependency检查及`git diff --check`。

本轮Review与REVIEW-0001..0005 substantive freshness记录落盘后独立执行的文档校验见下方；
设计批准只确认durable contract闭合，不替代后续实现、测试和fresh independent code review。

- `python scripts/check_docs.py`：passed，170 Markdown files、49 formal IDs；REVIEW-0001..0005
  的freshness只在逐项确认`907eee7..a4e3478`未改变各自已批准Runtime/Schema/DB/API事实后前移。
- `git diff --check`：passed。
- `git status --short`：仅`docs/reviews/REVIEW-0001..0006`为Reviewer-owned修改；无被评审设计或Runtime改动。

# Scope and unrelated changes

第二次focused对象为`cfa7a06..a4e3478`的REQ-0007 Requirement/Spec/RFC/ADR、active work和Review记录，
并回看上一轮`05dd7ca..cfa7a06` closure及ARCH-0002/0003、REQ/SPEC/RFC/ADR-0005/0006。完整`907eee7..a4e3478`进一步证明只新增/修订
REQ-0007设计与Review记录；`crates/`、`schemas/`、Cargo、AGENTS、scripts、skills、DB/API实现和REQ-0001..0006
durable合同零差异。本Reviewer只修改`docs/reviews/REVIEW-0001..0006`，没有修改任何被评审设计或Runtime。

# Re-review conditions

设计门禁已满足，可按批准合同进入实现。实施必须逐项兑现F-001..F-006 required proof、AC-01..20测试矩阵、
非零filter/static helper、真实SQLite close/reopen与response-loss exact retry，不得用实现选择重释TimeoutKey、identity
preimage或冲突优先级。实现仍需fresh independent code review和新的Review ID；本设计评审不能替代。

# Re-review history

- 2026-08-25：fresh independent design review of exact
  `05dd7ca7ece0d362aa96a6bb99f6c92e5d8999b2`；排除提交后的未提交Runtime改动。结论0 Blocker、
  6 Major、1 accepted Minor、1 accepted Note，changes requested。
- 2026-08-25：focused independent re-review of exact
  `cfa7a06c3588a6ad975a9511140d0984f5eb1b8f` against `05dd7ca`。逐项关闭F-001 lifecycle
  admission/two-stream guard、F-002 cancellation authority、F-003 callback/evidence producer、F-005
  Manifest/control SchemaSet binding和F-006 trusted resource envelope；F-007的非零filter/static命令已成为可复算
  实施计划，保持accepted Minor。F-004因timeout recovery command/event identity仍未定义而保持open。
  结论0 Blocker、1 Major、1 accepted Minor、1 accepted Note，仍为changes requested；Runtime继续阻塞。
- 2026-08-25：second focused independent design re-review of exact
  `a4e34785908207e622365250ae1466b85b4baecb` against `cfa7a06`。逐项验证persisted
  `TimeoutKeyV1`、canonical Clock sample、冻结usage evidence、domain-separated fingerprint及command/event ID、
  `not_due`不消费identity、response-loss exact retry、same-ID mutation、different-ID terminal no-op、reopen新ID和
  verified/unknown零重复核算，并确认F-001/F-002/F-003/F-005/F-006无回退。F-004 closed；无新finding。
  结论0 Blocker、0 Major、1 accepted Minor、1 accepted Note，设计approved；实现与测试证据留给后续独立代码评审。
- 2026-08-27：substantive design freshness confirmation exact
  `87be5391c40fdaa5b423c921747e7c941f7e2d42`。独立实现REVIEW-0007已在exact `80249cc`逐项验证
  lifecycle、cancel/callback/recovery authority、trusted envelope、Manifest-pinned SchemaSet和timeout identity，
  F-001至F-009全部closed；`f18f410..87be539`随后仅同步REQ-0007 done/archive、README/index/Epic/Requirement
  implemented facts和归档work，对REQ/SPEC/RFC/ADR及Runtime/Schema/API/DB/权限零差异。closure因归档
  Validation仍含历史failed结果而由REVIEW-0007 F-010阻塞，但该治理证据结构问题不推翻已批准设计合同。
  REVIEW-0006保持approved、0 open Blocker/Major，freshness前移至exact `87be539`。
- 2026-08-27：focused F-010 freshness confirmation exact
  `53338a836f646cdcefb6858ce07b0b0e8e12b11e`。`828f9aa..53338a8`仅把三条历史失败从归档Validation
  最终Results迁入Historical design remediation叙述，完整保留本Review首轮changes-requested、F-004修订期
  freshness失败与exact `a4e3478`最终独立批准；REQ/SPEC/RFC/ADR及产品零差异。REVIEW-0006保持approved、
  0 open Blocker/Major，freshness前移至exact `53338a8`。
- 2026-08-27：exact architecture clarification
  `8bb885bda678f5f785706e9eb335f472b5244974` substantive freshness re-review。`53338a8..8bb885b`
  未修改REQ-0007 Requirement/Spec/RFC/ADR、Runtime、Schema、权限、预算、取消、Effect或Replay合同；
  `ARCH-0004`准确重申这些机制属于Rust可信内核，非Rust扩展不能自授Capability、增加Budget或绕过Effect。
  REVIEW-0006保持approved、0 open Blocker/Major。
- 2026-08-27：exact polyglot design remediation
  `1748f69d01044a936727b3b5b7659882981b9129` substantive freshness re-review。`8bb885b..1748f69`
  未修改REQ-0007 Requirement/Spec/RFC/ADR、Runtime、Schema、Capability、Budget、Cancellation、deadline、
  late result、Effect或Replay合同；RFC-0007把这些authority及trusted envelope/evidence admission保留在Rust
  control plane，外部扩展只能request或返回observation。REVIEW-0006保持approved、0 open Blocker/Major。
- 2026-08-27：exact accepted-doc candidate `b42ccdc3216f518ff60303cec20da92b78d190a1` substantive freshness re-review。`2c80b89..b42ccdc`未修改REQ-0007/SPEC-0006/RFC-0006/ADR-0007或Runtime/Schema；ADR-0008明确Rust继续拥有Capability、Budget、Cancellation、deadline、Effect/Evidence与Replay admission，跨语言边界必须另行冻结late result和opaque bounded authority。REVIEW-0006保持approved、0 open Blocker/Major。
- 2026-08-28：exact REQ-0008 design candidate `8507bae4ad979232e69ba282ee9c97ee71e3520e` substantive freshness re-review。`754798d..8507bae`未修改REQ-0007/SPEC-0006/RFC-0006/ADR-0007、Runtime或Schema；新Hook设计继续要求default-deny Capability、trusted envelope、opaque lease、Kernel meter、cancel/deadline/timeout和Recorded零核算。REVIEW-0010 F-003指出新双stream组合尚未闭合并阻塞实现，没有放宽已批准REQ-0007合同。REVIEW-0006保持approved、0 open Blocker/Major。
- 2026-08-28：exact REQ-0008 remediation `3aee02adf8815466b02f51de247ae19922efc126` substantive freshness re-review。`43f3a5b..3aee02a`未修改REQ-0007合同、Runtime或Schema；未来Hook terminal pair明确复用transaction-local settlement admission并拒绝common single-stream terminal，保留trusted envelope、opaque lease、FakeClock和unknown核算。REVIEW-0010关闭新设计F-003不改变REQ-0007已批准语义，REVIEW-0006保持approved、0 open Blocker/Major。
- 2026-08-28：exact REQ-0008 accepted-doc `3318cbc6fe8bc8c9717a5a2b4aea1153f0d281d6` substantive freshness re-review。`ea9633c..3318cbc`未修改REQ-0007合同、Runtime或Schema；ADR-0009忠实接受transaction-local pair、single-stream terminal拒绝、trusted envelope/FakeClock/unknown核算且不声称实现。REVIEW-0006保持approved、0 open Blocker/Major。
