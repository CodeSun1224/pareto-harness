---
id: REVIEW-0012
title: REQ-0009 Effect Intent/Receipt 与幂等效果独立设计评审
status: changes-requested
owners: [independent-reviewer]
created: 2026-08-30
updated: 2026-08-30
links: [REQ-0009, SPEC-0008, RFC-0009, REQ-0004, REQ-0007, REQ-0008, RFC-0002, RFC-0005, RFC-0006, RFC-0008, ADR-0003, ADR-0006, ADR-0007, ADR-0009, ARCH-0002, ARCH-0003]
independence: independent
reviewed_revision: 9f8bf23f8a5737e3c6744662dbcd15ecabbca16f
open_blockers: 0
open_majors: 4
---

# Verdict

`changes-requested` for exact revision `9f8bf23f8a5737e3c6744662dbcd15ecabbca16f`
(parent `d2de39e2c49f76659e2d2805a59258b782aef065`)：0 Blocker、4 open Major。
Intent-before-dispatch、双 stream atomic pair、默认拒绝、保守 unknown settlement、lifecycle success guard
和 live/Recorded 类型隔离的方向正确，但四个跨边界合同仍未闭合：Recorded replay 会读取未被派生
Manifest 固定的后续 source facts；既有 Boundary Inventory 类型会把可能已发生的 partial/unknown 错记为
“receipt 前失败”；Fake executor 没有明确的 immutable revision pin；claim/crash 后也没有可重放的 recovery
command identity。任一缺口都可能让相同 Manifest 产生不同结果，丢失 partial/unknown 语义，或让恢复路径
长期占用 reservation/由临时调用方重建 authority，因此 RFC/SPEC 在修订并由同一独立 Reviewer 复审前不得接受或实施。

本评审只评价固定 revision 的三份新增设计文档及其引用的 accepted 合同和实际代码基线；没有评价或采纳
提交后的未提交工作区结论，也没有把当前不存在的 Effect Runtime、Schema 或测试当作证据。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `RFC-0009:153,167-169`；`SPEC-0008:75-77`；REQ-0009 AC-10/17/18/19 | Recorded replay 只固定 finalized `BoundaryInventoryRevision`，却要求读取 source Effect 的完整历史；source terminal 后又可追加 reconciliation/late facts并生成另一个未被 derived Manifest 固定的 `BoundaryReconciliationRevision`。现有 `ExecutionMode` 只固定 source inventory，故相同 derived Manifest 在后续 source reconciliation 前后可重建出不同 Projection，违反 Manifest-pinned identity、Recorded determinism 与 source horizon 隔离。 | 二选一冻结为 durable 合同并做 golden/negative proof：Replay 只读取 inventory 固定的 exact event horizon，且后续 facts 完全不参与该 replay；或 Manifest/ExecutionMode 精确固定受验证的 reconciliation revision及最终 Effect cursor/digest。测试须在 derived Manifest 建立后追加 source late/reconciliation Event，证明同一 pin 的结果不变；alternate/current/unpinned revision必须拒绝。 | open |
| F-002 | Major | `RFC-0009:167`；`SPEC-0008:75`；`crates/pareto-protocol/src/types.rs:377-404`；RFC-0002 boundary finalization contract；REQ-0009 AC-08/09/10/18 | 设计把 `partial/unknown` 映射到既有 `BoundaryOutcome::Failed(reason)`；但 accepted/current 类型把 `Failed` 明确定义为“request failed before a receipt”，且 record 只能保存 reason code，无法表达 confirmed components、unknown components、limitations、attempt/external identity与 open reconciliation。可能已发生的外部效果因此在 replay inventory 中被语义降格成 effect-before failure，正是 AC-09 禁止的伪装，也无法忠实核对 Effect Projection。 | 对 Boundary Inventory 做闭合、versioned 的 major 演进（或定义不丢信息的独立 Effect boundary record），显式表达 `not_applied`、`applied`、`partial`、`unknown`、receipt/attempt identity、confirmed/unknown摘要、limitations与reconciliation binding；旧 inventory/reader bytes保持不变。golden/compatibility测试须证明 partial/unknown 不可序列化为旧 `Failed-before-receipt`，Recorded fold与 inventory逐项等价。 | open |
| F-003 | Major | REQ-0009 AC-01/05/19；`RFC-0009:34-43,85,98-106,124-126`；`SPEC-0008:23-26,59-61` | AC-01要求固定 executor，但 registration 只列 effect/adapter/producer/operation revisions 和 `handler_compatibility_digest`；claim/lease只明确绑定 producer/adapter/process epoch，Fake executor则从未定义的 compatibility identity解析。digest 既没有明确的 descriptor Schema/revision role，也没有被写入 Effect ID、request digest、claim/lease/Receipt/Projection的完整绑定。registry 不变但 executor实现替换时，live dispatch及恢复证据可漂移。 | 增加明确的 immutable executor revision与内容地址 descriptor/config digest，定义它与 `handler_compatibility_digest` 的唯一关系，并绑定 Manifest registry、Effect ID/request digest、Intent、claim、lease、Receipt admission、Projection和reopen；不同executor revision的same-key请求稳定冲突。protocol golden、old/current substitution、wrong-executor lease/receipt和Recorded零executor测试须命中非零。 | open |
| F-004 | Major | REQ-0009 AC-10/12/13/20；`RFC-0009:118,122-124,145,157,184-188`；`SPEC-0008:53,59,65,93,130` | 设计声称 claim 后进程崩溃/响应丢失会原子 settle unknown 并进入 reconciliation，但只定义 reserve/terminal pair command，没有定义 reopen 时谁、凭什么事实、以何种稳定 identity 产生 terminal pair。丢失 live lease/observation 后，仅凭“explicit recovery”无法区分 command exact retry/mutation，也没有 recovery key、process epoch判定、canonical Clock/evidence、fingerprint/Event IDs、`not_due`行为或 commit-response-loss优先级。实现要么永久保留 reserved operation并阻塞 success，要么让临时调用方重建 terminal authority。 | 冻结 Kernel-private `EffectRecoveryCommand`（或明确复用并扩展 accepted timeout recovery contract）：persisted recovery key必须绑定scope/effect/attempt/claim/process epoch/operation/reservation/policy；定义 canonical Clock/evidence、domain-separated fingerprint与双Event IDs；锁内按 integrity/isolation→same-ID exact/mutation→existing terminal→eligibility/due 顺序处理；not-eligible不消费identity，response-loss/reopen exact或新sample retry不重复settle。覆盖 claim-before-call、applied-before-response、before/after pair commit、timeout equality、cancel和success race的 bounded model及真实SQLite reopen测试。 | open |

# Constitutional effect trace

| Effect path | Fixed-revision evidence | Independent result |
|---|---|---|
| Proposal and admission | principal + persisted v3 Manifest/Task + exact registry/history + Capability/trusted envelope + cancel/deadline → atomic reserve/Intent pair | 默认拒绝、scope与预算authority留在Kernel，设计满足；executor pin仍受F-003阻塞。 |
| Dispatch | committed Intent → writer内refold → dispatch claim → opaque single-attempt lease → Fake executor | Intent-before-dispatch和already-claimed不重发lease明确；crash后authority/identity未闭合，见F-004。 |
| Receipt and settlement | bounded untrusted observation → producer/adapter/lease/schema/meter admission → atomic control settlement/Effect conclusion | Receipt不是authority、unknown全额保守核算与pair零/二方向满足；reopen recovery仍缺command合同。 |
| Partial/unknown and reconciliation | partial/unknown terminal pair → open reconciliation → pinned query producer → append-only resolution，不改control/budget/lifecycle | live Effect stream保留独立轴；finalized boundary record会丢失/误述这些轴，见F-002。 |
| Lifecycle | success transaction fold control/effect，拒绝reserved、pending、partial/unknown/open reconciliation；failed/cancelled在operation settle后可保留对账 | authority与竞争方向满足；F-004未解决时crash可永久阻塞success。 |
| Projection/recovery | continuous exact Effect stream + retained SchemaSet/reducer/output/history identities → pure Projection/reopen | fail-closed目标明确；un-pinned terminal后history使Recorded结果不稳定，见F-001。 |
| Recorded replay | source inventory + source Effect history → read/fold only；API无executor/writer/settlement authority | 零live authority方向满足，但输入horizon没有完整版本固定，不能称确定性replay，见F-001/F-002。 |

# Acceptance trace

| Acceptance | Review result | Independent evidence or gap |
|---|---|---|
| AC-01 | not satisfied | scope、request、operation、policy多数闭合，但executor identity未明确固定，F-003。 |
| AC-02 | design-satisfied | admission由principal、persisted Manifest/lifecycle/control/effect、Capability/reservation与retained SchemaSet构造；proposal/Hook/Receipt不是authority。 |
| AC-03 | design-satisfied at plan level | reserve/Intent同事务，claim提交后才执行，且明确不宣称跨外部边界事务；实现仍需fault proof。 |
| AC-04 | design-satisfied at plan level | Effect ID域分离、request digest mutation冲突、exact retry无重复reserve/claim/settle已冻结；executor drift须由F-003纳入digest。 |
| AC-05 | not satisfied | opaque lease及scope/attempt/epoch约束明确，但缺exact executor revision binding，F-003。 |
| AC-06 | design-satisfied at plan level | outcome矩阵仅使用进程内Fake且禁止真实I/O；实现测试尚不存在。 |
| AC-07 | design-satisfied at plan level | Receipt observation经过producer/adapter/lease/schema/limits/meter admission，非法受信producer输出走unknown保守结算。 |
| AC-08 | design-satisfied for live stream | dispatch/external/reconciliation三轴和唯一terminal pair能区分已知与未知；inventory层回退语义受F-002阻塞。 |
| AC-09 | not satisfied end-to-end | Effect Event保留partial摘要且不redispatch，但 finalized inventory把partial压成旧Failed，F-002。 |
| AC-10 | not satisfied | 默认不盲重试正确；claim/crash/reopen后缺稳定recovery command/identity，F-004。 |
| AC-11 | design-satisfied at plan level | reconciliation要求pinned producer/evidence fingerprint、exact/mutation和append-only；其revision未成为Replay pin，F-001。 |
| AC-12 | not satisfied for recovery | live terminal pair、verified/unknown accounting方向满足；crash后如何唯一生成该pair未定义，F-004。 |
| AC-13 | not satisfied for recovery | writer race与deadline equality已规定；timeout/recovery command identity和commit-loss语义未闭合，F-004。 |
| AC-14 | design-satisfied at plan level | terminal后late/audit不反转control/lifecycle/budget，Event仅存redacted摘要；Replay pin仍需F-001修复。 |
| AC-15 | design-satisfied at plan level | 单append-only Effect stream、无第二mutable authority table、纯fold和cross-pair验证方向明确。 |
| AC-16 | design-satisfied at plan level | tenant/user presence-value/workspace/run/task/actor及业务ID exact矩阵已计划，跨域错误不写目标。 |
| AC-17 | not satisfied end-to-end | Projection fail-closed字段与非法序列矩阵明确；reopen terminal recovery和Recorded source horizon分别受F-004/F-001阻塞。 |
| AC-18 | not satisfied | API零executor/writer是必要条件但不充分；读取未固定的完整source history会使相同derived Manifest漂移，F-001/F-002。 |
| AC-19 | not satisfied | v3/new SchemaSet、旧bytes/reader与SQLite v2保留方向正确；Boundary major、reconciliation pin及executor revision仍未闭合，F-001至F-003。 |
| AC-20 | not satisfied at design gate | planned filter覆盖广，但缺F-001至F-004的pin/inventory/executor/recovery required proof；当前没有Effect tests可运行。 |
| AC-21 | design-satisfied at plan level | proposal/intent/dispatch/receipt/reconciliation接口保持Kernel-private且不提前授予Provider/Tool/Sandbox/Loop authority。 |
| AC-22 | design-satisfied subject to F-004 | success同事务fold Effect并拒绝未决/open reconciliation；failed/cancelled保留对账且不重开lifecycle。crash recovery不闭合可造成永久reserved。 |

# Compatibility, permission, and isolation review

- Manifest v3而不是重释v1/v2，是与当前`validate_run_manifest`按major闭合roles及Hook v2 pin相容的最小方向；实现必须新增v3分支，不能把`major == 2`判断泛化成“current”。
- `hook_pair`与`effect_pair`互斥的 additive 新SchemaSet方向可行：retained旧payload Schema/reader/bytes不变，新set必须同时拒绝both-present，通用single-stream callback/timeout必须对任一pair fail closed。实现仍需证明control-only或effect-only reseal在cross-stream recovery中被检测。
- Event envelope继续由persisted Manifest owner代表Kernel signer；requester、subject、producer/reconciler只作为闭合payload事实，不取得Event Store、Capability、budget、terminal或replay authority。
- full scope及Task/subject/effect/attempt/operation/reservation/key/pair identity矩阵充分覆盖主要confused-deputy边界；错误producer/lease/epoch不得借unknown policy写目标或消耗他人预算。
- SQLite v2、accepted DDL/trigger/checksum和历史SchemaSet保持不变的rollback方向合理；首次v3/Effect事实后只能stop writer并保留reader/forward repair，不能用旧writer忽略新stream。

# Regression and test review

固定提交只新增REQ/RFC/SPEC共437行；`git diff --quiet 9f8bf23^ 9f8bf23 -- crates schemas Cargo.toml Cargo.lock scripts .agents AGENTS.md`
为exit 0，未提前修改Runtime、Schema、DB、Cargo、测试、agent规则或依赖。实际代码抽查确认：

- `RunManifest`当前只有v2 Hook digest，semantic validation只对major 2增加`hook_registry`；v3必须是retained major分支，不能修改v1/v2闭合角色。
- `OperationReservedPayloadV1`/`OperationSettledPayloadV1`当前只有optional `hook_pair`，generic `append_atomic_pair`已提供two-existing exact、one-existing corrupt和transaction rollback基础，但Effect binding/cross-fold尚不存在。
- `BoundaryOutcome::Failed`当前合同明确是receipt前失败，`BoundaryReconciliationRevision`只固定inventory revision与late Event IDs；这直接支持F-001/F-002，不是图示推断。
- lifecycle success guard目前只读取Runtime Control/Hook相关状态；Effect同事务guard、Projection、Fake executor和Recorded API均尚不存在，不能用现有回归绿灯证明AC-01..22。

Independent Reviewer 在Windows/PowerShell、2026-08-30执行：

- `python -m unittest discover -s scripts/tests -p "test_*.py"`：24 tests passed，exit 0；这些是治理脚本测试，不是Effect行为证据。
- `git diff --check 9f8bf23^ 9f8bf23`：无输出，exit 0。
- `python scripts/check_docs.py`：exit 1；独立运行没有复现候选所述绿灯，只报告REVIEW-0001..0007、0010、0011因三份REQ-0009设计文档而stale。按任务边界，本Reviewer没有修改旧Review；该freshness门禁须由维护流程另行处理，不能把缓存结果写成fixed-revision通过。
- 本Review落盘后再次执行docs check和`git diff --check`，结果记录在提交前验证中；所有Effect命名Cargo filter在固定revision均尚不存在，未伪造non-zero结果。

实施前的修订test trace至少应新增F-001 source-late-after-pin、F-002 boundary partial/unknown major compatibility、
F-003 executor substitution/lease binding、F-004 recovery key/response-loss/reopen bounded model，并继续保留现有atomic pair、
isolation、budget/cancel/deadline/lifecycle、Recorded零执行及全workspace回归。Q/C/L需分开记录：质量为上述硬门禁；
费用只记录Fake reserved/verified/unknown且外部费用为零；延迟用具体可复算命令记录Intent pair、claim、terminal pair、
recovery/reconciliation、1/10/100 fold、contention和Recorded replay，不作无baseline优化声明。

# Scope and unrelated changes

exact `d2de39e..9f8bf23` diff仅新增：

- `docs/requirements/REQ-0009-effect-intent-receipt-idempotent-effects.md`
- `docs/rfcs/RFC-0009-kernel-governed-effect-intent-receipt.md`
- `docs/specs/SPEC-0008-effect-intent-receipt-idempotent-effects.md`

没有修改accepted Requirement/RFC/ADR、旧Review、README/index、active handoff、产品代码、Schema、SQLite、Cargo、测试或依赖。
本Reviewer只新增`REVIEW-0012`，没有修改被评审设计、代码、测试或旧Review。

# Re-review conditions

同一独立Reviewer应在新的固定revision上逐项确认：F-001 replay pin/horizon、F-002 faithful boundary major、F-003
executor immutable identity、F-004 recovery command/idempotency均成为一致的Requirement/RFC/SPEC合同；AC trace及命名
non-zero计划同步修订；v1/v2/SQLite v2/Hook pair/accepted replay合同无回退；docs freshness门禁可复现通过。
只有0 open Blocker、0 open Major时才可把本Review改为`approved`并进入RFC接受/ADR/Spec批准与planning，设计批准仍不等于实现证据。

# Re-review history

- 2026-08-30：fresh independent architecture review of exact
  `9f8bf23f8a5737e3c6744662dbcd15ecabbca16f` against
  `d2de39e2c49f76659e2d2805a59258b782aef065`。结论0 Blocker、4 open Major，
  `changes-requested`。设计和产品路径保持只读；Reviewer仅创建本Review记录。
