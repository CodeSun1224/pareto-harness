---
id: REVIEW-0012
title: REQ-0009 Effect Intent/Receipt 与幂等效果独立设计评审
status: changes-requested
owners: [independent-reviewer]
created: 2026-08-30
updated: 2026-08-30
links: [REQ-0009, SPEC-0008, RFC-0009, REQ-0004, REQ-0007, REQ-0008, RFC-0002, RFC-0005, RFC-0006, RFC-0008, ADR-0003, ADR-0006, ADR-0007, ADR-0009, ARCH-0002, ARCH-0003]
independence: independent
reviewed_revision: aba3a33703e681c542fd58b32f3d0ae41cff369d
open_blockers: 0
open_majors: 1
---

# Verdict

Focused substantive re-review of exact revision `aba3a33703e681c542fd58b32f3d0ae41cff369d`
against initial candidate `9f8bf23f8a5737e3c6744662dbcd15ecabbca16f`：`changes-requested`，
0 Blocker、1 open Major。F-001 fixed replay horizon、F-002 Boundary Inventory/Effect Record V2与F-003
content-addressed executor identity已由一致的Requirement/RFC/SPEC合同和命名proof计划关闭。F-004的stable
Kernel recovery key/authority/fingerprint/pair identity/retry顺序大部分闭合，但terminal mapping仍把所有
`deadline winner`定义为`reconciliation-required/unknown|partial`，而recovery段又把未claim的`deadline_due`
确定为`not_applied`。同一pre-dispatch timeout仍可能产生两种权威Effect结论，因此不能批准。

本评审只评价固定 revision 的三份新增设计文档及其引用的 accepted 合同和实际代码基线；没有评价或采纳
提交后的未提交工作区结论，也没有把当前不存在的 Effect Runtime、Schema 或测试当作证据。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `RFC-0009:181,193,195-208`；`SPEC-0008:82-86`；REQ-0009 AC-17/18/19 | Remediation新增V2 inventory固定exact Effect inclusive cursor/history digest；projection API只能读取显式horizon，Recorded逐项核对该range，horizon后late/reconciliation facts不参与同一pin，alternate/current/unpinned cursor/digest拒绝。相同Manifest/inventory在追加后事实前后必须byte-identical。 | 原required proof已进入`recorded_replay`与`projection_recovery`命名计划；实现仍须提交source-late-after-pin golden/negative证据。 | closed |
| F-002 | Major | `RFC-0009:195-208,253-255`；`SPEC-0008:84-86,98,122`；REQ-0009 AC-09/18/19 | Remediation保留Boundary Inventory/Record V1 bytes/reader，新增V2 inventory与Effect record；record固定effect/request/attempt/external/executor/operation identity，并分别表达applied、not_applied、cancelled-before-claim、partial confirmed/unknown digests、limitations和reconciliation binding，禁止用V1 Failed承载partial/unknown。 | 原required proof已进入Inventory V1/V2 compatibility与Recorded逐项等价计划；实现仍须证明旧bytes不变及partial/unknown不能降格。 | closed |
| F-003 | Major | `RFC-0009:34-48,81-89,120,128-138,193`；`SPEC-0008:23-33,52,60-62,82`；REQ-0009 AC-01/05/19 | Remediation新增独立内容地址`EffectExecutorDescriptorV1`，registration exact绑定revision/content/config；compatibility digest仅证明实现匹配而不替代identity。executor identity进入Effect/request ID、Intent、claim、recovery key、lease、Receipt admission、Projection及reopen；same-key executor替换稳定冲突。 | 原required proof已进入protocol golden、wrong-executor/substitution、lease与compatibility命名计划；实现仍须证明descriptor/current substitution fail closed。 | closed |
| F-004 | Major | REQ-0009 AC-08/10/12/13/20；`RFC-0009:140-173`；`SPEC-0008:54-70` | Remediation已冻结Kernel-private recovery authority、base/full key、epoch/Clock/evidence、domain-separated fingerprint、pair/Event IDs、锁内exact/mutation/ExistingTerminal/eligibility顺序及response-loss retry。但terminal矩阵仍规定所有`deadline winner`产生`reconciliation-required / unknown或partial`，recovery合同却规定未claim的`deadline_due`由history证明未交付lease并结论`not_applied`。这会让同一pre-dispatch timeout按入口不同形成open reconciliation或确定未执行，破坏唯一terminal/预算/lifecycle fold。 | 在Requirement/RFC/SPEC统一按claim boundary冻结cause→Runtime outcome/accounting→Effect conclusion矩阵：pre-claim deadline必须唯一映射`timed_out + verified not_applied`且不打开reconciliation；post-claim deadline才是`timed_out + partial/unknown + reconciliation-required`。同样显式冻结process-loss/cancellation的pre/post-claim映射。`crash_recovery`、`cancellation_timeout`和bounded reverse-race测试须覆盖deadline equality与两个入口并证明同一唯一pair。 | open |

# Constitutional effect trace

| Effect path | Fixed-revision evidence | Independent result |
|---|---|---|
| Proposal and admission | principal + persisted v3 Manifest/Task + exact registry/executor/history + Capability/trusted envelope + cancel/deadline → atomic reserve/Intent pair | 默认拒绝、executor pin、scope与预算authority均留在Kernel，F-003 closed。 |
| Dispatch | committed Intent → writer内refold → claim/recovery key → executor-bound opaque lease → Fake executor | Intent-before-dispatch、executor exact与already-claimed不重发lease闭合；crash recovery identity已持久化。 |
| Receipt and settlement | bounded untrusted observation → executor/producer/adapter/lease/schema/meter admission → atomic control settlement/Effect conclusion | Receipt不是authority、unknown保守核算与pair零/二方向满足；pre-claim deadline mapping仍受F-004阻塞。 |
| Partial/unknown and reconciliation | partial/unknown terminal pair → V2 record保留confirmed/unknown/limitations → pinned query producer → append-only resolution | live Event与finalized V2 inventory均不再降格partial/unknown，F-002 closed。 |
| Lifecycle | success transaction fold control/effect，拒绝reserved、pending、partial/unknown/open reconciliation；failed/cancelled在operation settle后可保留对账 | authority方向满足；F-004需统一pre/post-claim timeout结论以确保所有入口fold一致。 |
| Projection/recovery | explicit inclusive cursor + retained identities → pure Projection；persisted key → Kernel recovery command → atomic terminal pair | fixed horizon和stable recovery大体闭合；terminal cause matrix仅剩F-004。 |
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
| AC-08 | not satisfied | 三轴与唯一pair方向正确，但pre-claim deadline在terminal矩阵和recovery段有两种结论，F-004。 |
| AC-09 | design-satisfied at plan level | Event与V2 record都保留partial confirmed/unknown摘要、limitations和reconciliation binding，不自动retry。 |
| AC-10 | not satisfied | stable recovery authority/key/identity/retry已闭合；pre-claim deadline/process-loss/cancel terminal映射仍须统一，F-004。 |
| AC-11 | design-satisfied at plan level | pinned reconciliation producer、exact/mutation、append-only及inventory fixed horizon均闭合。 |
| AC-12 | not satisfied | pair与核算原子性明确，但pre-claim timeout究竟是not_applied还是unknown会改变release/consume与reconciliation，F-004。 |
| AC-13 | not satisfied | writer priority、deadline equality与recovery identity明确；pre/post-claim deadline outcome矩阵矛盾，F-004。 |
| AC-14 | design-satisfied at plan level | terminal后late/audit不反转control/lifecycle/budget，horizon后facts不改变同一Recorded pin。 |
| AC-15 | design-satisfied at plan level | 单append-only Effect stream、无第二mutable authority table、纯fold和cross-pair验证方向明确。 |
| AC-16 | design-satisfied at plan level | tenant/user presence-value/workspace/run/task/actor及业务ID exact矩阵已计划，跨域错误不写目标。 |
| AC-17 | design-satisfied subject to F-004 | explicit cursor/history、executor/recovery identity和fail-closed矩阵明确；terminal cause fold仍须唯一。 |
| AC-18 | design-satisfied at plan level | V2 inventory固定cursor/history，horizon后facts不参与，Recorded类型上无live authority。 |
| AC-19 | design-satisfied at plan level | Manifest v3、Executor V1、Inventory/Record V2均新major/type并保留v1/v2、SQLite v2及旧bytes/readers。 |
| AC-20 | not satisfied at design gate | F-001至F-003 proof已进入命名计划；F-004仍缺pre/post-claim cause matrix及两入口一致性的明确测试。 |
| AC-21 | design-satisfied at plan level | proposal/intent/dispatch/receipt/reconciliation接口保持Kernel-private且不提前授予Provider/Tool/Sandbox/Loop authority。 |
| AC-22 | design-satisfied subject to F-004 | success同事务fold并拒绝未决/open reconciliation；F-004必须保证timeout入口不会对同一history产生不同open-reconciliation状态。 |

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

focused exact `9f8bf23..aba3a33`只修订上述同三份REQ-0009 proposed/impact-analyzed/draft设计并链接本Review；
没有修改REQ-0002..0008 accepted/implemented Requirement、Spec、RFC、ADR或其产品代码、Schema、SQLite、Cargo、测试与依赖。
因此本Reviewer可对REVIEW-0001..0007、0010、0011做限定freshness前移；这不接受或实现REQ-0009，也不改变这些Review的原findings/verdict。

# Re-review conditions

同一独立Reviewer应在新的固定revision上确认F-004的pre/post-claim recovery cause矩阵在Requirement/RFC/SPEC完全一致，
尤其deadline equality从callback/timeout/recovery任一入口只能产生同一Runtime outcome、Effect conclusion、budget accounting和
reconciliation状态；命名bounded race计划同步。只有0 open Blocker、0 open Major时才可把本Review改为`approved`并进入
RFC接受/ADR/Spec批准与planning，设计批准仍不等于实现证据。

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
