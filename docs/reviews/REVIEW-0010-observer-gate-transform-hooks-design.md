---
id: REVIEW-0010
title: REQ-0008 Observer、Gate 与 Transform Hook 独立设计评审
status: approved
owners: [independent-reviewer]
created: 2026-08-28
updated: 2026-08-28
links: [REQ-0008, SPEC-0007, RFC-0008, RFC-0007, ADR-0008, REQ-0004, REQ-0007, REVIEW-0006, REVIEW-0007, REVIEW-0009, FIX-0001]
independence: independent
reviewed_revision: e3d8d805b46fb4e1e25b23bc53bead71de730853
open_blockers: 0
open_majors: 0
---

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `REQ-0008` AC-02/03/05/14；`SPEC-0007` point phases、invocation key与test trace；`RFC-0008` §§2/Interfaces | Remediation 固定 `before_proposal_admission = Transform → Gate → Observer`、`before_authoritative_commit = Gate → Observer`、`after_* = Observer`，priority仅在phase内排序；每步持久化initial/input/predecessor/final digest与point finalization。所有Gate读取同一final Transform输出，Observer只在business decision固定后读取只读view；其failure只能改变分离execution status，不能回写allow/deny。 | `phase_order_lineage`、`ordering`、`observer_non_authority`及Recorded/reopen等价的命名非零计划覆盖原required proof；实现仍须提交实际测试证据。 | closed |
| F-002 | Major | `REQ-0008` AC-04；`SPEC-0007` Gate-bearing rule；`RFC-0008` §3 | Remediation 删除`gate_requirement=none`及同义字段；两个Gate-bearing point无条件要求至少一个required Gate，零Gate或仅optional Gate稳定deny；`after_*`由类型矩阵禁止Gate而不适用该检查。 | `gate_composition`与`default_deny`计划明确覆盖零Gate、仅optional、failure/timeout/invalid/unknown，并要求所有filter非零；旧/unknown reader由compatibility矩阵覆盖。 | closed |
| F-003 | Major | `REQ-0008` AC-09/12/14/15/20；`SPEC-0007` Atomic pair commands；`RFC-0008` §5/Failure modes | Remediation 冻结reserve与terminal两类crate-private pair command：pair ID/kind、full-command fingerprint、双stream expected cursor/next sequence、两个确定性Event ID/sequence/prepared bytes及交叉引用；固定zero写两者、two仅exact retry、mutation conflict、one即`corrupt_partial_pair`、任一validation/first-or-second insert/commit fault rollback、response-loss exact bytes retry。Hook terminal只能走pair入口，现有通用single-stream terminal遇Hook binding必须fail closed；不存在补写/catch-up合法态。 | `reserve_pair_atomicity`、`pair_fault_injection`、`terminal_pair_atomicity`、`pair_corruption`、`terminal_race`与Recorded测试覆盖two-writer、commit loss、validly-resealed one-sided history、预算守恒和零执行/零核算；当前单Event baseline明确要求transaction-local重构且不扩大public SQL/API。 | closed |
| F-004 | Major | `REQ-0008` AC-01/05；`SPEC-0007` registration/failure semantics；`RFC-0008` §7 | Remediation 统一为仅Observer注册warn-and-continue或fail-closed；Gate固定fail closed；Transform没有policy字段并固定reject-whole-proposal，失败后skip后续Transform/Gate/Observer，不能返回原proposal继续。 | protocol `hook_contract`拒绝Gate/Transform policy字段；`failure_policy`与`transform_chain_failure`覆盖中间失败、零partial authority和Recorded结果。 | closed |

# Verdict

`approved` for exact remediation `3aee02adf8815466b02f51de247ae19922efc126`
(parent `43f3a5bc4bc44fe103856565a238105837c67c6e`；initial candidate
`8507bae4ad979232e69ba282ee9c97ee71e3520e`)。同一 fresh independent Reviewer 逐项复核
`43f3a5b..3aee02a`，没有采纳实现者closure结论。F-001至F-004 required proof已成为一致、durable、
可测试的Requirement/Spec/RFC合同；0 Blocker、0 open Major。批准仅解除设计门禁，不是产品实现或测试批准；
仍须先接受RFC、建立ADR-0009、批准Spec/Requirement并创建Plan/Tasks/Handoff，之后才可编写Runtime代码。

# Acceptance trace

| Acceptance | Review result | Independent evidence / gap |
|---|---|---|
| AC-01 | design-satisfied | registration明确Observer-only policy；Gate/Transform无policy字段且固定失败语义，见F-004。 |
| AC-02 | design-satisfied | kind × point矩阵与固定phase闭合；Observer只读且业务决定先固定，见F-001。 |
| AC-03 | design-satisfied | phase ordinal由point/kind推导，phase内priority/ID/revision稳定排序；lineage逐Event持久化。 |
| AC-04 | design-satisfied | deny优先、required explicit allow、abstain不放行；Gate-bearing空required集无条件deny，见F-002。 |
| AC-05 | design-satisfied | Observer两策略不改business decision；Gate fail closed；Transform固定reject-whole，见F-001/F-004。 |
| AC-06 | design-satisfied | allow mask之外再比较保护 hash view，覆盖 identity、authority、budget、Receipt、Evidence、terminal及 unknown field。 |
| AC-07 | design-satisfied | principal/Manifest/lifecycle/control重建和不可序列化、不可委托的 bounded lease保持 Kernel-private。 |
| AC-08 | design-satisfied | tenant/user presence-value/workspace/run/task/owner/subject及业务 ID 均要求 exact；未授权 no-write。 |
| AC-09 | design-satisfied | reserve pair固定双cursor/Event/fingerprint与zero/one/two/rollback/response-loss语义，见F-003。 |
| AC-10 | design-satisfied | verified/unknown settlement继承REQ-0007并只能通过terminal pair结算，exact retry不重复核算。 |
| AC-11 | design-satisfied | absolute/monotonic deadline、FakeClock、cooperative probe、hung recovery均复用REQ-0007且禁止真实 sleep。 |
| AC-12 | design-satisfied | completion/cancel/timeout共享terminal pair；common single-stream terminal拒绝，late不反转。 |
| AC-13 | design-satisfied | pre-decode limits、closed output、Kernel-only redaction和安全摘要覆盖 injection/secret/path/SQL 泄漏。 |
| AC-14 | design-satisfied | point-start/lineage/invocation/terminal/skip/finalization均为版本化Hook Event并交叉验证pair。 |
| AC-15 | design-satisfied | reopen恢复完整pending/terminal pair；one-sided reseal与unknown/current substitution均fail closed。 |
| AC-16 | design-satisfied | Recorded只读 fold、不持有handler/writer/timeout authority；Simulated/Reexecute dispatch前拒绝。 |
| AC-17 | design-satisfied at plan level | 新内容地址set、RunManifest新major倾向、旧Run不升级、SQLite v2和既有reducer保留均明确；实现须证明retained bytes。 |
| AC-18 | design-satisfied | 仅进程内Rust Fake reference，不暴露Rust ABI/SQLite布局，也未选择外部transport。 |
| AC-19 | design-satisfied | 下游只获得proposal/observation/result，Effect/Evidence/terminal authority仍由后续Requirement批准。 |
| AC-20 | design-satisfied at plan level | 命名矩阵已补phase/lineage、Observer隔离、Transform chain、pair fault/response-loss/one-sided reseal/common terminal拒绝；所有filter要求list非零。 |

# Compatibility, permission, and isolation review

- Hook handler没有Event transaction、Manifest mutable handle、Capability/lease constructor、budget、timeout recovery、Effect/Evidence或lifecycle terminal authority；RFC-0007/ADR-0008的Rust authority边界未被绕过。
- Gate与Transform输出在Kernel重新验证后才可进入组合或后续权威提交；Observer annotation不自动成为业务决定、Memory或Evidence。
- Event envelope仍由Manifest owner作为Kernel signer，subject/producer在闭合payload中验证；Task与Hook业务ID不被误当Event Store isolation authority。
- 新Hook SchemaSet/RunManifest major的方向与当前v1八角色闭合验证相容，但实现必须按exact schema reader分支，不能全局把旧Manifest改成九角色。
- Remediation 删除F-002默认拒绝回退并闭合F-001/F-003/F-004；实现不得弱化fixed phase、empty-required deny、atomic pair或fixed Transform failure语义。

# Regression and test review

候选是纯设计提交，没有Runtime、Protocol、Schema、DB、Cargo、依赖或测试实现，因而本评审不把任何现有
REQ-0003..0007绿灯当作Hook行为证据。现有代码抽查确认：

- Event Store的`PreparedEvent`、`check_prepared_idempotency`与`insert_prepared`按单Event工作；child module可复用同一SQLite transaction，但没有现成atomic pair primitive。
- Runtime Control的`reserve_protected_operation`与`append_control`拥有并commit自己的事务；Hook实现若要满足RFC，必须建立不暴露SQL/authority的transaction-local组合路径。
- lifecycle/control已能在同一writer transaction读取并fold两条历史，Runtime Control已有FakeClock、opaque lease、timeout recovery、Projection/Recorded replay和真实SQLite竞态测试，可作为实现基础，但不能替代F-003的Hook pair证据。
- SPEC的命名filter在代码尚不存在时不能运行；实施Plan必须把每个filter写成`assert_cargo_test_filter.py`等可复算命令并记录非零count。尤其要补F-001至F-004 required proof，而不能只依赖workspace全量绿灯。

Independent Reviewer 在 Windows/PowerShell、2026-08-28 执行：

- `python -m unittest discover -s scripts/tests -p "test_*.py"`：21 tests passed，exit 0。
- 首次 `python scripts/check_docs.py`：exit 1；唯一错误为 REVIEW-0001..0007 的 `reviewed revision is stale`，均指向本候选五个设计/导航文件。
- Reviewer 实质确认候选没有修改 REQ-0002治理、Protocol/Schema、Event Store、Lifecycle/Manifest、Projection/Snapshot/Replay或Runtime Control既有产品合同，且本Review以4 open Major阻塞新设计后，仅前移 REVIEW-0001..0007 的`reviewed_revision`并追加freshness叙述。
- 再次 `python scripts/check_docs.py`：`Document validation passed: 180 Markdown files, 59 formal IDs.`，exit 0。
- `git diff --check`：无输出，exit 0。
- `git status --short`：仅 REVIEW-0001..0007 reviewer-owned freshness修改及新增 REVIEW-0010；设计和产品代码保持只读。

Focused remediation re-review 在 Windows/PowerShell、2026-08-28 对 exact `3aee02ad` 独立执行：

- `git diff --quiet 43f3a5b 3aee02a -- crates schemas Cargo.toml Cargo.lock scripts`：无产品、Schema、Cargo或script diff，exit 0。
- `python -m unittest discover -s scripts/tests -p "test_*.py"`：21 tests passed，exit 0。
- 首次 `python scripts/check_docs.py`：除本Review表格中一个未转义竖线外，只报告 REVIEW-0001..0007 stale；没有产品合同或finding计数错误。
- 修正Review表格，并在逐项确认旧治理/Protocol/Event Store/Lifecycle/Projection/Runtime Control批准合同未回退后，Reviewer仅前移 REVIEW-0001..0007 freshness。
- 最终 `python scripts/check_docs.py`：`Document validation passed: 180 Markdown files, 59 formal IDs.`，exit 0。
- `git diff --check`：无输出，exit 0。
- `git status --short`：仅 REVIEW-0001..0007 freshness与 REVIEW-0010 focused closure；REQ/SPEC/RFC及产品代码保持只读。

Focused accepted-doc freshness re-review 在 Windows/PowerShell、2026-08-28 对 exact
`3318cbc6fe8bc8c9717a5a2b4aea1153f0d281d6` 独立执行：

- `git diff --quiet ea9633c8883353289f32ea90757cee0ed20a545f 3318cbc6fe8bc8c9717a5a2b4aea1153f0d281d6 -- crates schemas Cargo.toml Cargo.lock scripts .agents`：无产品、Schema、Cargo、script或agent diff，exit 0。
- `python -m unittest discover -s scripts/tests -p "test_*.py"`：21 tests passed，exit 0。
- `python scripts/check_docs.py`：`Document validation passed: 181 Markdown files, 60 formal IDs.`，exit 0。
- `git diff --check`：无输出，exit 0。
- `git status --short`：仅 REVIEW-0001..0007 与 REVIEW-0010 reviewer-owned freshness；accepted design与产品代码保持只读。

Focused planning freshness re-review 在 Windows/PowerShell、2026-08-28 对 exact
`e3d8d805b46fb4e1e25b23bc53bead71de730853` 独立执行：

- exact parent为`5546d1fc638785f8db8eac111853f8fbe951f0cd`；candidate只新增PLAN/TASKS/HANDOFF并把REQ-0008从`approved`推进到`planned`。
- PLAN/TASKS/HANDOFF创建于REVIEW-0010批准0/0及ADR-0009正式接受之后；仅治理TASK-00完成，TASK-01至TASK-11全部未开始，且明确新的fresh independent code-review、实现者只修复、原代码Reviewer关闭Blocker/Major、REQ-0009禁启。
- AC-01至AC-20及F-001至F-004 required proof均映射到具体任务、命名filter、fault/race/replay/compatibility/isolation矩阵；PLAN第9步全局要求每个Cargo filter先由helper证明非零，覆盖Protocol `hook_contract`，kernel filters另有具体helper模板。
- `git diff --quiet 5546d1fc638785f8db8eac111853f8fbe951f0cd e3d8d805b46fb4e1e25b23bc53bead71de730853 -- crates schemas Cargo.toml Cargo.lock scripts`：无Runtime、Schema、Cargo或script diff，exit 0。
- `python -m unittest discover -s scripts/tests -p "test_*.py"`：21 tests passed，exit 0。
- 首次`python scripts/check_docs.py`仅报告REVIEW-0001..0007 stale；逐项确认治理、Protocol/Schema、Event Store、Lifecycle/Manifest、Projection/Replay与Runtime Control既有合同未回退后，仅更新reviewer-owned freshness。
- 最终`python scripts/check_docs.py`：`Document validation passed: 184 Markdown files, 60 formal IDs.`，exit 0。
- `git diff --check`：无输出，exit 0；`git status --short`仅REVIEW-0001..0007与REVIEW-0010 reviewer-owned freshness。

# Scope and unrelated changes

精确 `754798de..8507bae4` diff仅新增REQ-0008、SPEC-0007、RFC-0008，并更新`docs/index.md`与
`EPIC-0002`链接，共356 insertions、1 deletion。没有产品代码、Schema、Cargo、DB、测试、治理规则或依赖变化；
未发现提前实现REQ-0009/0010/0011/0013/0014/0015/0018/0026/0033或预选transport的无关范围。

# Re-review history

- 2026-08-28：fresh independent design review of exact `8507bae4ad979232e69ba282ee9c97ee71e3520e`
  against parent `754798de3a7f0f09d38c466b8f09199c7ebda9d1`。结论0 Blocker、4 open Major，
  `changes-requested`。设计文件与产品代码保持只读；Reviewer仅创建本Review记录。
- 2026-08-28：focused independent design re-review of exact `3aee02adf8815466b02f51de247ae19922efc126`
  against parent `43f3a5bc4bc44fe103856565a238105837c67c6e`，并回看initial candidate `8507bae4`。
  F-001 fixed phase/input lineage/Observer non-authority、F-002 Gate-bearing empty-required unconditional deny、
  F-003 reserve/terminal atomic pair及single-stream terminal拒绝、F-004 fixed Transform reject-whole均由durable合同和
  命名非零测试计划关闭；无新finding。结论0 Blocker、0 Major，`approved`，但产品代码仍禁止在后续接受/规划门禁前开始。
- 2026-08-28：focused independent accepted-doc freshness re-review of exact
  `3318cbc6fe8bc8c9717a5a2b4aea1153f0d281d6` against parent
  `ea9633c8883353289f32ea90757cee0ed20a545f`。新增ADR-0009忠实等价接受exact `3aee02ad`：fixed phase/input
  lineage与Observer decision隔离、Gate-bearing empty-required deny、reserve/terminal atomic pair及single-stream terminal
  拒绝、Transform fixed reject-whole均无漂移；REQ/SPEC/RFC只更新accepted状态/链接，ARCH/index/Epic只同步“设计已接受、
  Runtime未实现”事实。crates/schemas/Cargo/scripts/agents零差异；无新finding。REVIEW-0010保持approved、0/0。
- 2026-08-28：focused independent planning freshness re-review of exact
  `e3d8d805b46fb4e1e25b23bc53bead71de730853` against parent
  `5546d1fc638785f8db8eac111853f8fbe951f0cd`。REQ只从approved推进planned；PLAN/TASKS/HANDOFF在设计批准0/0和ADR接受后创建，完整承接AC-01至AC-20及F-001至F-004 proof，只有治理TASK-00完成。新的fresh code Review、原代码Reviewer关闭Major/Blocker和REQ-0009禁启门禁明确；产品路径零差异。无新finding，保持approved、0/0。
