---
id: REVIEW-0010
title: REQ-0008 Observer、Gate 与 Transform Hook 独立设计评审
status: changes-requested
owners: [independent-reviewer]
created: 2026-08-28
updated: 2026-08-28
links: [REQ-0008, SPEC-0007, RFC-0008, RFC-0007, ADR-0008, REQ-0004, REQ-0007, REVIEW-0006, REVIEW-0007, REVIEW-0009, FIX-0001]
independence: independent
reviewed_revision: 8507bae4ad979232e69ba282ee9c97ee71e3520e
open_blockers: 0
open_majors: 4
---

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `SPEC-0007:21-29,65-69`; `RFC-0008:25-35,47`; REQ-0008 AC-02/03 | 一个 point 可同时注册 Observer、Gate、Transform，但唯一排序 tuple 不含 kind/phase，设计只分别定义 Gate 串行组合和 Transform 串行 pipeline，没有定义跨类型执行 phase、每个 Observer/Gate 看到原 proposal 还是前一 Transform 输出、以及 input digest/source cursor 何时固定。实现可合法地产生“先 Gate 后 Transform”或“先 Transform 后 Gate”等不同权威准入结果，违反相同 Manifest/输入/历史产生相同顺序和决定的合同。 | 冻结版本化的 point phase/order 与逐 invocation input lineage；明确 Transform、Gate、Observer 的跨类型可见性和 point finalization，或禁止同一 point 混合类型。增加至少覆盖 Observer/Gate/Transform 优先级交错、输入摘要、重启和 Recorded replay 等价的命名非零测试。 | open |
| F-002 | Major | `REQ-0008:30`; `SPEC-0007:21,67`; `RFC-0008:39-47` | Requirement 明确“空 required gate 集默认 deny”，RFC/SPEC 却允许 registry 声明 `gate_requirement=none` 后放行；而列出的 `HookRegistryRevisionV1`/`HookRegistrationV1` 字段中也没有这个 point-level policy。该未定义例外既削弱 AC-04 的默认拒绝硬门禁，也无法被 Schema、Manifest pin 或 retained reader 精确验证。 | 删除该例外并证明所有空 required 集稳定 deny；或先修订 Requirement，再把 point-level gate policy 定义为闭合、版本化、Manifest-pinned 字段，说明谁有权选择、兼容/升级语义和为何不构成 required Gate 绕过。测试须覆盖无 Gate、仅 optional Gate、显式 none、unknown/missing policy 和旧 reader。 | open |
| F-003 | Major | `SPEC-0007:38-59,73-79,96-102,136-142`; `RFC-0008:55-63,100-111`; REQ-0008 AC-09/10/12/14/15/20 | 设计要求 control reservation 与 Hook invocation 在同一 SQLite transaction 成对追加，但没有冻结 pair-command identity/fingerprint、两条 event ID/sequence 的 exact retry 判定、单边已存在历史的 corruption 规则、第二次 insert/validation/commit 失败的 rollback结果，或 settlement/timeout 后 control terminal 与 Hook terminal 的 catch-up/reconcile command。现有 `reserve_protected_operation`/`append_control` 会自行 commit 单一 control Event，底层 `insert_prepared` 也只判断单 Event；实现需要实质重构。当前测试矩阵只有余额竞争和普通 idempotency，没有第二 Hook append 失败、pair exact/mutation retry、单边 validly re-sealed history或 timeout 后 Hook terminal 恢复证明，因此不能证明“无部分成功”。 | 冻结一个 Kernel-private atomic pair command：两个 stream 的预期 cursor、两个完整 Event/fingerprint、检查优先级、exact/mutation retry、zero/one/two-existing 状态和 rollback结果；同样冻结 control terminal 到 Hook decision/terminal 的原子或显式幂等 reconciliation。加入真实 SQLite fault/validation injection、commit-response-loss、close/reopen、two-writer reverse winner、单边重封历史及 Recorded replay 零执行/零核算测试。 | open |
| F-004 | Major | `REQ-0008:27,31`; `SPEC-0007:21,65-69`; `RFC-0008:92-95` | AC-01 要求每个注册固定 failure policy，AC-05 又要求 Transform 按注册策略选择“拒绝原 proposal”或“整体失败”；候选 registration 只列 Observer failure policy，Gate 固定 fail closed，Transform 固定 reject-whole-proposal。因而 Transform 的两个允许结果没有版本化选择字段或精确下游语义，Manifest/registry digest也无法 pin该行为。实现者会被迫自行决定这是固定策略、缺失字段还是把两种结果合并。 | 选择并统一 Requirement/Spec/RFC：若 Transform 策略可配置，定义闭合 enum、合法 point、状态/decision/reason、组合和 replay 语义；若首片固定一种行为，修订 AC-01/05 明确无选择。测试须逐策略覆盖 chain 中间失败、零 partial authority、下游最终结果和 Recorded replay。 | open |

# Verdict

`changes-requested` for exact candidate `8507bae4ad979232e69ba282ee9c97ee71e3520e`
(parent `754798de3a7f0f09d38c466b8f09199c7ebda9d1`)。本评审由未参与设计的 fresh
independent Reviewer 执行。候选正确保持 Rust Kernel authority、Observer 非权威、Transform 保护字段、
全隔离、REQ-0007 预算/取消/deadline、Recorded replay 零执行和 transport neutrality，但上述四项仍会让
不同实现对准入、默认拒绝、跨 stream 原子性或失败结果作出不同解释。最终 0 Blocker、4 open Major；不得批准
RFC/SPEC/Requirement，也不得进入实现。

# Acceptance trace

| Acceptance | Review result | Independent evidence / gap |
|---|---|---|
| AC-01 | blocked | registry identity/config/Schema方向明确，但通用 failure policy 没有覆盖 Transform，见 F-004。 |
| AC-02 | blocked | kind × point allow matrix明确；同 point 跨 kind 的输入可见性与 phase 未冻结，见 F-001。 |
| AC-03 | blocked | `(point, priority, ID, revision)`可稳定排序单一列表，却不足以确定跨 kind 执行语义，见 F-001。 |
| AC-04 | blocked | deny/required allow/abstain/short-circuit主体规则明确；`gate_requirement=none`与空集默认拒绝冲突，见 F-002。 |
| AC-05 | blocked | Observer/Gate安全方向明确；Transform 注册策略与最终拒绝/整体失败结果缺失，见 F-004。 |
| AC-06 | design-satisfied | allow mask之外再比较保护 hash view，覆盖 identity、authority、budget、Receipt、Evidence、terminal及 unknown field。 |
| AC-07 | design-satisfied | principal/Manifest/lifecycle/control重建和不可序列化、不可委托的 bounded lease保持 Kernel-private。 |
| AC-08 | design-satisfied | tenant/user presence-value/workspace/run/task/owner/subject及业务 ID 均要求 exact；未授权 no-write。 |
| AC-09 | blocked | trusted envelope和全账户 reserve规则明确，但 reserve + invocation 的实际原子 pair contract和失败证明缺失，见 F-003。 |
| AC-10 | blocked | verified/unknown settlement方向继承REQ-0007；跨 stream terminal/reconcile exact retry仍未闭合，见 F-003。 |
| AC-11 | design-satisfied | absolute/monotonic deadline、FakeClock、cooperative probe、hung recovery均复用REQ-0007且禁止真实 sleep。 |
| AC-12 | blocked | terminal winner与 late isolation方向明确；control terminal 与 Hook terminal 的 crash/catch-up identity仍未定义，见 F-003。 |
| AC-13 | design-satisfied | pre-decode limits、closed output、Kernel-only redaction和安全摘要覆盖 injection/secret/path/SQL 泄漏。 |
| AC-14 | blocked | 单 Hook stream和 pure fold方向正确；pair event边界、point finalization与跨 stream引用的合法前缀尚不闭合，见 F-001/F-003。 |
| AC-15 | blocked | exact reader/current substitution拒绝明确；单边 pair历史与 timeout 后显式 reconcile 未定义，见 F-003。 |
| AC-16 | design-satisfied | Recorded只读 fold、不持有handler/writer/timeout authority；Simulated/Reexecute dispatch前拒绝。 |
| AC-17 | design-satisfied at plan level | 新内容地址set、RunManifest新major倾向、旧Run不升级、SQLite v2和既有reducer保留均明确；实现须证明retained bytes。 |
| AC-18 | design-satisfied | 仅进程内Rust Fake reference，不暴露Rust ABI/SQLite布局，也未选择外部transport。 |
| AC-19 | design-satisfied | 下游只获得proposal/observation/result，Effect/Evidence/terminal authority仍由后续Requirement批准。 |
| AC-20 | blocked | 现有表覆盖多数风险，但缺跨 kind phase、atomic pair第二append失败/单边历史及Transform policy测试，见 F-001/F-003/F-004。 |

# Compatibility, permission, and isolation review

- Hook handler没有Event transaction、Manifest mutable handle、Capability/lease constructor、budget、timeout recovery、Effect/Evidence或lifecycle terminal authority；RFC-0007/ADR-0008的Rust authority边界未被绕过。
- Gate与Transform输出在Kernel重新验证后才可进入组合或后续权威提交；Observer annotation不自动成为业务决定、Memory或Evidence。
- Event envelope仍由Manifest owner作为Kernel signer，subject/producer在闭合payload中验证；Task与Hook业务ID不被误当Event Store isolation authority。
- 新Hook SchemaSet/RunManifest major的方向与当前v1八角色闭合验证相容，但实现必须按exact schema reader分支，不能全局把旧Manifest改成九角色。
- F-002是当前唯一明确的默认拒绝回退；F-001/F-003/F-004是会导致权限/结果解释分叉的未冻结合同，而非实现细节。

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

# Scope and unrelated changes

精确 `754798de..8507bae4` diff仅新增REQ-0008、SPEC-0007、RFC-0008，并更新`docs/index.md`与
`EPIC-0002`链接，共356 insertions、1 deletion。没有产品代码、Schema、Cargo、DB、测试、治理规则或依赖变化；
未发现提前实现REQ-0009/0010/0011/0013/0014/0015/0018/0026/0033或预选transport的无关范围。

# Re-review history

- 2026-08-28：fresh independent design review of exact `8507bae4ad979232e69ba282ee9c97ee71e3520e`
  against parent `754798de3a7f0f09d38c466b8f09199c7ebda9d1`。结论0 Blocker、4 open Major，
  `changes-requested`。设计文件与产品代码保持只读；Reviewer仅创建本Review记录。
