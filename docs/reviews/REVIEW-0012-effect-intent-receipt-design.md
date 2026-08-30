---
id: REVIEW-0012
title: REQ-0009 Effect Intent/Receipt 与幂等效果独立设计评审
status: approved
owners: [independent-reviewer]
created: 2026-08-30
updated: 2026-08-30
links: [REQ-0009, SPEC-0008, RFC-0009, ADR-0010, REQ-0004, REQ-0007, REQ-0008, RFC-0002, RFC-0005, RFC-0006, RFC-0008, ADR-0003, ADR-0006, ADR-0007, ADR-0009, ARCH-0002, ARCH-0003]
independence: independent
reviewed_revision: 46772c7fbb30e82f0e8fd4fb50915e8414acaa65
open_blockers: 0
open_majors: 0
---

# Verdict

`approved` for exact revision `46772c7fbb30e82f0e8fd4fb50915e8414acaa65`：0 Blocker、0 open Major。
Focused planning re-review确认REQ-0009仅推进为`planned`并新增PLAN/TASKS/HANDOFF；`46772c7`已关闭
`04a5613` planning Major，在任何行为编辑前强制`planned→implementing`，并为AC-21补齐`cargo tree`与相对
`60cee6e`的Cargo manifest/lock exact diff命令。规划明确Runtime、Schema、测试均未实现，不替代后续fresh code review。
`b7acbd8..60cee6e`仅完成设计接受闭环：ADR-0010忠实采用本Review在`b7acbd8`批准的合同，
REQ/RFC/SPEC分别推进为approved/accepted/approved，导航、Epic、backlog与ARCH-0001/0003同步且明确尚未规划或实现。
没有新增authority、state或replay语义，也没有代码、Schema、Runtime、测试或旧Requirement合同变化。
最终一行RFC修订强制未claim old-epoch/deadline/cancel recovery只能`not_applied + verified zero usage + full
reservation release`并禁止partial/unknown；已claim仍只能partial/unknown + reconciliation。该规则与REQ AC-10/13、
terminal table、SPEC Explicit recovery/terminal mapping和stable command eligibility/priority完全一致，关闭F-004且无新finding。
批准仅解除设计门禁，不代表REQ-0009已接受、规划、实现或通过Runtime测试。

本评审只评价固定 revision 的三份新增设计文档及其引用的 accepted 合同和实际代码基线；没有评价或采纳
提交后的未提交工作区结论，也没有把当前不存在的 Effect Runtime、Schema 或测试当作证据。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `RFC-0009:181,193,195-208`；`SPEC-0008:82-86`；REQ-0009 AC-17/18/19 | Remediation新增V2 inventory固定exact Effect inclusive cursor/history digest；projection API只能读取显式horizon，Recorded逐项核对该range，horizon后late/reconciliation facts不参与同一pin，alternate/current/unpinned cursor/digest拒绝。相同Manifest/inventory在追加后事实前后必须byte-identical。 | 原required proof已进入`recorded_replay`与`projection_recovery`命名计划；实现仍须提交source-late-after-pin golden/negative证据。 | closed |
| F-002 | Major | `RFC-0009:195-208,253-255`；`SPEC-0008:84-86,98,122`；REQ-0009 AC-09/18/19 | Remediation保留Boundary Inventory/Record V1 bytes/reader，新增V2 inventory与Effect record；record固定effect/request/attempt/external/executor/operation identity，并分别表达applied、not_applied、cancelled-before-claim、partial confirmed/unknown digests、limitations和reconciliation binding，禁止用V1 Failed承载partial/unknown。 | 原required proof已进入Inventory V1/V2 compatibility与Recorded逐项等价计划；实现仍须证明旧bytes不变及partial/unknown不能降格。 | closed |
| F-003 | Major | `RFC-0009:34-48,81-89,120,128-138,193`；`SPEC-0008:23-33,52,60-62,82`；REQ-0009 AC-01/05/19 | Remediation新增独立内容地址`EffectExecutorDescriptorV1`，registration exact绑定revision/content/config；compatibility digest仅证明实现匹配而不替代identity。executor identity进入Effect/request ID、Intent、claim、recovery key、lease、Receipt admission、Projection及reopen；same-key executor替换稳定冲突。 | 原required proof已进入protocol golden、wrong-executor/substitution、lease与compatibility命名计划；实现仍须证明descriptor/current substitution fail closed。 | closed |
| F-004 | Major | REQ-0009 AC-10/12/13/20；`RFC-0009:142-173,186-188`；`SPEC-0008:54-74` | `b7acbd8`最终明确未claim old-epoch/deadline/cancel recovery必须not_applied、verified zero usage、释放全部reservation并禁止partial/unknown；已claim才允许verified partial或unknown全额并打开reconciliation。stable recovery key/authority/fingerprint/pair/retry/eligibility顺序未回退，terminal table、REQ和SPEC完全一致。 | Required proof已进入`crash_recovery`、`atomic_settlement`与`cancellation_timeout`计划：未claim三cause零consume/full release，已claim才可partial/unknown；实现仍须提供实际非零测试证据。 | closed |

# Constitutional effect trace

| Effect path | Fixed-revision evidence | Independent result |
|---|---|---|
| Proposal and admission | principal + persisted v3 Manifest/Task + exact registry/executor/history + Capability/trusted envelope + cancel/deadline → atomic reserve/Intent pair | 默认拒绝、executor pin、scope与预算authority均留在Kernel，F-003 closed。 |
| Dispatch | committed Intent → writer内refold → claim/recovery key → executor-bound opaque lease → Fake executor | Intent-before-dispatch、executor exact与already-claimed不重发lease闭合；crash recovery identity已持久化。 |
| Receipt and settlement | bounded untrusted observation → executor/producer/adapter/lease/schema/meter admission → atomic control settlement/Effect conclusion | pre/post-claim Effect结论和accounting均唯一，F-004 closed。 |
| Partial/unknown and reconciliation | partial/unknown terminal pair → V2 record保留confirmed/unknown/limitations → pinned query producer → append-only resolution | live Event与finalized V2 inventory均不再降格partial/unknown，F-002 closed。 |
| Lifecycle | success transaction fold control/effect，拒绝reserved、pending、partial/unknown/open reconciliation；failed/cancelled在operation settle后可保留对账 | pre/post-claim reconciliation与budget fold均闭合。 |
| Projection/recovery | explicit inclusive cursor + retained identities → pure Projection；persisted key → Kernel recovery command → atomic terminal pair | stable recovery identity/priority/eligibility/accounting闭合。 |
| Recorded replay | V2 inventory-fixed cursor/history → source Effect fold/read only；horizon后facts排除；API无live authority | 相同pin byte-identical、无executor/writer/settlement authority，F-001/F-002 closed。 |

# Acceptance trace

| Acceptance | Review result | Independent evidence or gap |
|---|---|---|
| AC-01 | design-satisfied | 内容地址executor revision/descriptor/config贯穿Manifest registry、Effect/request identity和持久事实，F-003 closed。 |
| AC-02 | design-satisfied | admission由principal、persisted Manifest/lifecycle/control/effect、Capability/reservation与retained SchemaSet构造；proposal/Hook/Receipt不是authority。 |
| AC-03 | design-satisfied at plan level | reserve/Intent同事务，claim提交后才执行，且明确不宣称跨外部边界事务；实现仍需fault proof。 |
| AC-04 | design-satisfied at plan level | Effect ID域分离、executor纳入preimage、same-key executor/request mutation冲突及exact retry语义已冻结。 |
| AC-05 | design-satisfied at plan level | opaque lease exact绑定executor/scope/effect/attempt/operation/reservation/epoch/claim/recovery key。 |
| AC-06 | design-satisfied at plan level | outcome矩阵仅使用进程内Fake且禁止真实I/O；实现测试尚不存在。 |
| AC-07 | design-satisfied at plan level | Receipt observation经过producer/adapter/lease/schema/limits/meter admission，非法受信producer输出走unknown保守结算。 |
| AC-08 | design-satisfied | 未claim明确not_applied且无reconciliation，已claim明确partial/unknown并打开reconciliation。 |
| AC-09 | design-satisfied at plan level | Event与V2 record都保留partial confirmed/unknown摘要、limitations和reconciliation binding，不自动retry。 |
| AC-10 | design-satisfied | stable recovery authority/key/identity/eligibility/retry及pre/post-claim结论/accounting闭合。 |
| AC-11 | design-satisfied at plan level | pinned reconciliation producer、exact/mutation、append-only及inventory fixed horizon均闭合。 |
| AC-12 | design-satisfied at plan level | terminal pair原子且未claim只允许zero consume/full release，已claimverified partial或unknown全额。 |
| AC-13 | design-satisfied at plan level | deadline/cancel的pre/post-claim结论、accounting、writer priority与deadline equality均唯一。 |
| AC-14 | design-satisfied at plan level | terminal后late/audit不反转control/lifecycle/budget，horizon后facts不改变同一Recorded pin。 |
| AC-15 | design-satisfied at plan level | 单append-only Effect stream、无第二mutable authority table、纯fold和cross-pair验证方向明确。 |
| AC-16 | design-satisfied at plan level | tenant/user presence-value/workspace/run/task/actor及业务ID exact矩阵已计划，跨域错误不写目标。 |
| AC-17 | design-satisfied at plan level | explicit cursor/history、executor/recovery identity及Effect/budget terminal fold均明确。 |
| AC-18 | design-satisfied at plan level | V2 inventory固定cursor/history，horizon后facts不参与，Recorded类型上无live authority。 |
| AC-19 | design-satisfied at plan level | Manifest v3、Executor V1、Inventory/Record V2均新major/type并保留v1/v2、SQLite v2及旧bytes/readers。 |
| AC-20 | design-satisfied at plan level | 命名计划覆盖未claim三cause zero/full-release、已claim partial/unknown及recovery/settlement/race一致性；实现证据仍待后续代码评审。 |
| AC-21 | design-satisfied at plan level | proposal/intent/dispatch/receipt/reconciliation接口保持Kernel-private且不提前授予Provider/Tool/Sandbox/Loop authority。 |
| AC-22 | design-satisfied | success同事务fold并拒绝未决/open reconciliation；021b353已统一pre/post-claim reconciliation状态。 |

# Compatibility, permission, and isolation review

- Manifest v3而不是重释v1/v2，是与当前`validate_run_manifest`按major闭合roles及Hook v2 pin相容的最小方向；实现必须新增v3分支，不能把`major == 2`判断泛化成“current”。
- `hook_pair`与`effect_pair`互斥的 additive 新SchemaSet方向可行：retained旧payload Schema/reader/bytes不变，新set必须同时拒绝both-present，通用single-stream callback/timeout必须对任一pair fail closed。实现仍需证明control-only或effect-only reseal在cross-stream recovery中被检测。
- Event envelope继续由persisted Manifest owner代表Kernel signer；requester、subject、producer/reconciler只作为闭合payload事实，不取得Event Store、Capability、budget、terminal或replay authority。
- full scope及Task/subject/effect/attempt/operation/reservation/key/pair identity矩阵充分覆盖主要confused-deputy边界；错误producer/lease/epoch不得借unknown policy写目标或消耗他人预算。
- SQLite v2、accepted DDL/trigger/checksum和历史SchemaSet保持不变的rollback方向合理；首次v3/Effect事实后只能stop writer并保留reader/forward repair，不能用旧writer忽略新stream。

# Regression and test review

Focused remediation `9f8bf23..aba3a33`只修订REQ-0009 Requirement/RFC/Spec共100 insertions、50 deletions；
`git diff --quiet 9f8bf23 aba3a33 -- crates schemas Cargo.toml Cargo.lock scripts .agents AGENTS.md docs/requirements/REQ-0002-sdd-review-gates.md docs/requirements/REQ-0003-versioned-protocol-types-json-schema.md docs/requirements/REQ-0004-sqlite-append-only-event-store.md docs/requirements/REQ-0005-run-task-state-machine-run-manifest.md docs/requirements/REQ-0006-projection-snapshot-replay.md docs/requirements/REQ-0007-capability-budget-cancellation-timeout.md docs/requirements/REQ-0008-observer-gate-transform-hooks.md`
为exit 0，未修改既有accepted合同、Runtime、Schema、DB、Cargo、测试、agent规则或依赖。实际代码基线抽查确认：

- `RunManifest`当前只有v2 Hook digest，semantic validation只对major 2增加`hook_registry`；v3必须是retained major分支，不能修改v1/v2闭合角色。
- `OperationReservedPayloadV1`/`OperationSettledPayloadV1`当前只有optional `hook_pair`，generic `append_atomic_pair`已提供two-existing exact、one-existing corrupt和transaction rollback基础，但Effect binding/cross-fold尚不存在。
- `BoundaryOutcome::Failed`当前V1合同仍明确是receipt前失败；remediation选择新V2而非原位重释，符合retained reader边界。
- lifecycle success guard目前只读取Runtime Control/Hook相关状态；Effect同事务guard、Projection、Fake executor和Recorded API均尚不存在，不能用现有回归绿灯证明AC-01..22。

Focused re-review的门禁结果记录在本次提交前验证中；不存在的Effect Cargo tests仍只属于未来实施proof，未作为设计closure的运行证据。F-001至F-003关闭依据是durable合同与可测试修正已进入三份设计，不是实现者自报；F-004保持open。

Focused re-review在Windows/PowerShell、2026-08-30独立执行：

- `python -m unittest discover -s scripts/tests -p "test_*.py"`：24 tests passed，exit 0。
- `python scripts/check_docs.py`：`Document validation passed: 190 Markdown files, 65 formal IDs.`，exit 0；仅在确认REQ-0002..0008 accepted/implemented事实与代码零漂移后，才把REVIEW-0001..0007、0010、0011做限定freshness前移。
- `git diff --check`及仅Review staged diff检查在提交前通过；没有修改REQ-0009设计、Requirement/Spec/RFC/ADR、代码、Schema或测试。

Final focused candidate `aba3a33..021b353`仅对三份REQ-0009设计做6 insertions、5 deletions：REQ AC-13、
RFC terminal/lifecycle文本与SPEC terminal/test trace。没有代码、Schema、Runtime、旧合同或依赖变化。门禁将在本轮Review
和限定freshness前移后重新执行；F-004保持open依据是上述固定文本中的accounting冲突，而非缺少实现测试。

Final focused re-review独立执行：24 governance tests passed；`python scripts/check_docs.py`通过
（190 Markdown files、65 formal IDs）；`git diff --check`和仅`docs/reviews/` scope检查通过。不存在的Effect
Runtime tests仍未被虚构为实现证据。

One-line closure candidate `021b353..b7acbd8`只修改RFC-0009 recovery一句，把未claim recovery从可选
zero/partial收紧为必须zero usage、full release并禁止partial/unknown；代码、Schema、Runtime、测试、旧合同和依赖零变化。
本轮独立复跑24 governance tests、docs check（190 Markdown/65 formal IDs）、diff check和仅Review scope均通过。

Initial independent Reviewer 在Windows/PowerShell、2026-08-30执行：

- `python -m unittest discover -s scripts/tests -p "test_*.py"`：24 tests passed，exit 0；这些是治理脚本测试，不是Effect行为证据。
- `git diff --check 9f8bf23^ 9f8bf23`：无输出，exit 0。
- `python scripts/check_docs.py`：exit 1；独立运行没有复现候选所述绿灯，只报告REVIEW-0001..0007、0010、0011因三份REQ-0009设计文档而stale。按任务边界，本Reviewer没有修改旧Review；该freshness门禁须由维护流程另行处理，不能把缓存结果写成fixed-revision通过。
- 所有Effect命名Cargo filter在固定revision均尚不存在，未伪造non-zero结果。

实施前的修订test trace至少应新增F-001 source-late-after-pin、F-002 boundary partial/unknown major compatibility、
F-003 executor substitution/lease binding、F-004 recovery key/response-loss/reopen bounded model，并继续保留现有atomic pair、
isolation、budget/cancel/deadline/lifecycle、Recorded零执行及全workspace回归。Q/C/L需分开记录：质量为上述硬门禁；
费用只记录Fake reserved/verified/unknown且外部费用为零；延迟用具体可复算命令记录Intent pair、claim、terminal pair、
recovery/reconciliation、1/10/100 fold、contention和Recorded replay，不作无baseline优化声明。

# Scope and unrelated changes

initial exact `d2de39e..9f8bf23` diff仅新增：

- `docs/requirements/REQ-0009-effect-intent-receipt-idempotent-effects.md`
- `docs/rfcs/RFC-0009-kernel-governed-effect-intent-receipt.md`
- `docs/specs/SPEC-0008-effect-intent-receipt-idempotent-effects.md`

focused exact `9f8bf23..b7acbd8`只修订上述同三份REQ-0009 proposed/impact-analyzed/draft设计并链接本Review；
没有修改REQ-0002..0008 accepted/implemented Requirement、Spec、RFC、ADR或其产品代码、Schema、SQLite、Cargo、测试与依赖。
因此本Reviewer可对REVIEW-0001..0007、0010、0011做限定freshness前移；这不接受或实现REQ-0009，也不改变这些Review的原findings/verdict。

design-acceptance closure exact `b7acbd8..60cee6e`只新增ADR-0010，推进REQ-0009/RFC-0009/SPEC-0008状态，
并同步index、EPIC-0002、backlog、ARCH-0001和ARCH-0003。ADR逐项保持已评审Kernel authority、atomic pair、
unclaimed/claimed recovery、fixed-horizon Recorded replay、兼容与rollback边界；共享文本明确REQ-0009尚未规划或实现。
`crates/`、`schemas/`、Cargo、scripts、REQ-0002..0008合同均无差异。

# Re-review conditions

设计门禁现为0 open Blocker、0 open Major；RFC/ADR/Spec/Requirement已完成接受闭环，可进入planning。
后续实现必须逐项兑现F-001至F-004 required proof、所有命名filter非零、完整completion gates和fresh independent code review；
本设计批准不得被表述为REQ-0009已实现或Runtime测试已通过。

# Re-review history

- 2026-08-30：fresh independent architecture review of exact
  `9f8bf23f8a5737e3c6744662dbcd15ecabbca16f` against
  `d2de39e2c49f76659e2d2805a59258b782aef065`。结论0 Blocker、4 open Major，
  `changes-requested`。设计和产品路径保持只读；Reviewer仅创建本Review记录。
- 2026-08-30：同一independent Reviewer focused substantive re-review exact
  `aba3a33703e681c542fd58b32f3d0ae41cff369d` against `9f8bf23`。fixed cursor/history及horizon后
  facts不变关闭F-001；Inventory/Effect Record V2无损partial/unknown并保留V1关闭F-002；内容地址executor
  descriptor贯穿identity/admission/reopen关闭F-003。stable recovery key/authority/fingerprint/pair/retry已补齐，
  但pre-claim `deadline_due`在terminal table与recovery段仍分别为unknown/reconciliation与not_applied，F-004保持open。
  当前0 Blocker、1 open Major，`changes-requested`；没有Runtime/Schema/测试实现可被本设计复审宣称通过。
- 2026-08-30：final focused re-review exact `021b353d0efc923ef8739e3cb97d88f586c4fe06` against
  `aba3a33703e681c542fd58b32f3d0ae41cff369d`。6+/5-修订已把未claim deadline/cancel统一为not_applied、
  已claim统一为partial/unknown + reconciliation，Effect结论与recovery eligibility/priority无新冲突；但RFC recovery
  仍允许未claim“verified zero/partial meter”，与terminal table/SPEC的verified zero + release冲突。F-004保持open，
  当前0 Blocker、1 open Major，`changes-requested`；REQ-0009仍不得接受或进入实现。
- 2026-08-30：one-line final closure re-review exact `b7acbd82824d8410d432117c89be1bd56c8ce05c`
  against `021b353d0efc923ef8739e3cb97d88f586c4fe06`。唯一RFC行强制未claim old-epoch/deadline/cancel
  recovery以not_applied、verified zero usage结清并full release，禁止partial/unknown；与REQ AC-10/13、terminal table、
  SPEC recovery/mapping和stable command priority一致，无新finding。F-004 closed，当前0 Blocker、0 Major，`approved`；
  仅解除设计门禁，REQ-0009仍未接受、规划或实现。
- 2026-08-30：design-acceptance closure freshness re-review exact `60cee6ed44d150185bf99ca3095a8ce803bcc0d3`
  against `b7acbd82824d8410d432117c89be1bd56c8ce05c`。九个文档只创建忠实的ADR-0010、接受REQ/RFC/SPEC
  并同步共享导航/架构/路线事实；所有批准声明准确引用`b7acbd8`的independent approved 0/0，且明确Schema、Runtime、
  代码与实现测试尚不存在。ADR未新增未评审authority/state/replay语义，旧REQ合同无变化；无新finding，保持approved 0/0。
- 2026-08-30：focused planning re-review exact `46772c7fbb30e82f0e8fd4fb50915e8414acaa65`。初始planning
  `04a5613`忠实承接AC-01至AC-22与设计边界，但遗漏AC-21的`cargo tree`/manifest diff具体命令及
  `planned→implementing`启动门禁；本Reviewer将其作为planning Major退回。`46772c7`只修订PLAN/HANDOFF，
  补齐两项exact proof与行为编辑前状态迁移；无Runtime、Schema、测试、旧合同或新设计语义变化。finding closed，
  无新finding，保持approved 0/0；本轮仅为planning freshness，不是实现code review。
