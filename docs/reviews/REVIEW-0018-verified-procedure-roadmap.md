---
id: REVIEW-0018
title: Verified Procedure 路线重设计独立审查
status: changes-requested
owners: [independent-reviewer]
created: 2026-09-05
updated: 2026-09-05
links: [REQ-0034, SPEC-0010, RFC-0013]
independence: independent
reviewed_revision: 1206ad9eb074763988609999e67962ec59a0c1b7
open_blockers: 0
open_majors: 1
---

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Blocker | `docs/requirements/REQ-0034-verified-procedure-revision.md:31-42`; `docs/specs/SPEC-0010-verified-procedure-revision.md:17,27,104-111`; `docs/roadmap/requirement-backlog.md:31-35,55-57` | **Violated invariant:** 每个 high-risk Requirement 必须能在其 prerequisite 已 verified 后独立满足全部 AC，且每个 AC 映射到该 Requirement 可执行的测试。REQ-0034 的 AC-05/06 需要尚由 REQ-0018 定义的 PlanRevision，AC-07/08/10/16 的证明又明确推迟到 REQ-0035、REQ-0016 或 REQ-0014；但这些 Requirement 都以 REQ-0034 `verified` 为 prerequisite。依照 activation rule，REQ-0034 无法验证，后继项也不能激活，路线虽表面无环却在验收语义上形成闭环。 | 重新划分 Requirement ownership：要么把 REQ-0034 收窄为其自身可实现、可测试的 identity/registry 合同，并将 Plan/Node/Evidence/executor AC 移至各自 Requirement；要么把满足这些 AC 的最小实现前移进 REQ-0034 并同步 prerequisite、scope、Plan/Tasks。提交逐 AC 的唯一 owner、具体命令和非零命中测试矩阵，并用拓扑检查证明每项只依赖已可 verified 的前置项。 | closed |
| F-002 | Major | `docs/requirements/REQ-0034-verified-procedure-revision.md:28,32`; `docs/rfcs/RFC-0013-verified-procedure-execution-architecture.md:52,60,124`; `docs/specs/SPEC-0010-verified-procedure-revision.md:27,60,103` | **Violated invariant:** Procedure 是允许流程的唯一 authority，Plan 只能进行闭合、不可扩权的 Task-specific 实例化。候选同时让 Procedure 定义 nodes/dependencies/transitions，让 Plan “展开具体 DAG”，但没有冻结模板到实例的映射、可选/重复节点、分支、参数、预算、Capability、Evidence、retry/recovery/compensation 的单调约束；AC-06 的“偏离产生新 Plan 并重新准入”还可能被解释为只换 Plan 即可偏离已验证 Procedure。Kernel 因而没有可测试的 conformance 判定，Planner 仍可能通过 Plan 删除必需节点、弱化 Evidence 或扩大能力。 | 在 RFC/SPEC 中定义 closed instantiation relation：Plan 只能从 Procedure 声明的模板、分支和参数域实例化，不能新增/删除强制节点、放宽依赖/终止条件、弱化 Evidence 或扩大 Capability/预算/重试/补偿；超出 envelope 必须产生新的 ProcedureRevision、VerifiedProcedureRevision 和审批，而不只是新 Plan。负测至少覆盖删节点、改边、复制 effectful node、放宽 schema/evidence、扩大 capability/budget 及 cross-task binding，全部在 lease/effect 前 zero-write 拒绝。 | closed |
| F-003 | Major | `docs/rfcs/RFC-0013-verified-procedure-execution-architecture.md:72-82`; `docs/specs/SPEC-0010-verified-procedure-revision.md:13,30-36` | **Violated invariant:** 每个行为影响输入与外部效果必须先由权威 Manifest、Capability/Budget、Event 与 replay boundary 约束，模型不得在流程外调用 Provider。RFC 的数据流先由 Planner 提出 Plan，随后才创建固定 Behavior/Context/Model/Tool/Budget/Boundary 的 RunManifest；若 Planner 使用模型、检索或工具，规划调用没有 Run/Node lineage、预算、效果记录或 replay 合同。若禁止这些调用，候选又没有明确 Plan proposal 的受支持 bootstrap 方式。 | 冻结唯一 bootstrap：例如以独立 planning Run/phase 先原子固定 Task、候选 Procedure、Behavior、Provider/Tool/Workspace/Environment/Schema/Budget/Boundary，再通过受治理 Effect 产出带 exact provenance 的 Plan proposal；或明确第一版只接受离线、已签名且无运行时外部调用的 Plan artifact。测试必须证明 sequence-1 权威固定之前的 Provider/Tool/Workspace/Sandbox 请求零 dispatch/零费用，并证明 execution Run 的 Plan 可追溯到 exact planning authority。 | closed |
| F-004 | Major | `docs/requirements/REQ-0034-verified-procedure-revision.md:29-30,59`; `docs/specs/SPEC-0010-verified-procedure-revision.md:24,32,101,116`; `docs/rfcs/RFC-0013-verified-procedure-execution-architecture.md:58,131` | **Violated invariant:** VerifiedProcedure 的独立批准必须是 Kernel 可判定且运行者不可自签的 authority，而不是一个声称“independent”的引用。候选固定 review/approval revision 与 authority revision，却未定义不可削弱的角色分离、review subject 绑定、actor alias 判定、freshness/invalidation 或最低 quorum；这些最小规则被推迟到 REQ-0036，但 REQ-0034 已要求注册并准入 VerifiedProcedureRevision。一个 retained authority 仍可能批准自己创建、执行或验证的候选，Kernel 无从证明“独立”。 | 在 REQ-0034 admission 合同中冻结不可被 policy 放宽的最小独立性：review decision 绑定 exact Procedure digest、TaskClass、evidence set、limitations、compatibility 和 approval policy；定义 creator/proposer/runner/evidence producer/verifier/reviewer/approver 的禁止重合或明确 quorum；定义撤销和 evidence invalidation 的 admission 效果。用同 actor/别名、角色重叠、旧 review 复用、内容或 evidence 变更、跨 scope approval 负测证明全部在 Run/Event/Effect 前拒绝。REQ-0036 可编排 promotion，但不能延后 REQ-0034 准入所需的判定语义。 | closed |
| F-005 | Minor | `docs/architecture/version-and-event-model.md:81` | 候选修改了该 accepted 权威文档，却保留“REQ-0009 尚未实现 Schema 或 Runtime”的旧事实。基线 exact `e7a939c` 已包含 `effect_runtime.rs`、Effect payload 类型/Schema 生成与 retained schema sets；该陈述也与 README、ARCH-0001、REDESIGN-IMPACT 和 VALIDATION 冲突，可能误导后续 Node/Effect 迁移设计。 | 将该行改为与 exact 基线一致的 implemented/deferred 边界，并增加或运行跨文档实现状态一致性检查。 | closed |
| F-006 | Minor | `.agents/work/active/REQ-0034-verified-procedure-revision/VALIDATION.md:6,15,34-39`; `.agents/work/active/REQ-0034-verified-procedure-revision/TASKS.md:7-11` | 原始验证仍把 subject 写成 commit 前 working tree，`check_docs` 标为 skipped，TASK-05/06 也未完成；因此实现者证据未精确 pin 到本次 candidate。候选已诚实列出 remaining gates，且独立检查能确认 commit/scope，所以该问题不改变上述设计结论。 | closure revision 中把 validation subject、实际命令/exit code、TASK 状态和 reviewed revision 对齐；不得把本 Review 或旧结果当成整改后 exact revision 的 freshness 证明。 | closed |
| F-007 | Minor | `docs/architecture/version-and-event-model.md:79` | REQ-0034 已明确不创建 Event 或 lifecycle，但 ARCH-0003 仍称 Procedure/Plan/Node lifecycle 分别由 REQ-0034/REQ-0018/REQ-0035 交付。核心 ownership 在 Requirement、Spec 和 Backlog 中已闭合，因此这是同步瑕疵而非新的依赖闭环。 | 将 Procedure 项改写为 identity/registry admission，并把 lifecycle ownership 仅归给实际拥有 Event/state 的 Requirement。 | closed |
| F-008 | Minor | `docs/rfcs/RFC-0013-verified-procedure-execution-architecture.md:165,167` | Compatibility 段落逐字重复，增加正式合同噪声但不改变语义。 | 删除重复段落并运行文档/hygiene 检查。 | closed |
| F-009 | Major | `.agents/work/active/REQ-0034-verified-procedure-revision/VALIDATION.md:29`; `.agents/work/active/REQ-0034-verified-procedure-revision/HANDOFF.md:3,15`; `.agents/work/active/REQ-0034-verified-procedure-revision/TASKS.md:12` | **Violated invariant:** final Validation/Handoff 必须描述一个当前一致且按记录命令可复算的完成状态，不能把历史时点的命令结果写成当前可重跑结果。`VALIDATION.md` 将无右端 revision 的 `git diff --exit-code 72a0f8b... -- REVIEW-0018` 记为 exit 0；在 exact `1206ad9` 重跑实际 exit 1，因为同一 Reviewer 已合法更新 verdict/findings/history。`HANDOFF.md` 又同时声称 current phase 仍是 REVIEW-0018 remediation 和 route design 已停止，TASK-10 因而在 handoff 当前状态仍矛盾时提前标记完成。设计/authority 本身未回退，但 evidence-only closure 不能作为准确最终交接。 | 新 evidence-only revision 只修正 work records：把 preservation proof 固定为实际成立的 author-only区间（例如 original Review commit 到 designer remediation/evidence revision），另列 Reviewer-owned后续更新；将 Handoff 的 current phase 与 Plan/Tasks 统一，并在本 finding 经同一 Reviewer 复审前保持 final handoff task 未完成。重跑记录中的 exact command，证明 preservation exit 0、四文件 diff scope、无 runtime/schema/REQ-0010 实现、docs/governance/fmt/diff gates；再提交同一 Reviewer focused re-review。 | open |

# Verdict

Evidence-only candidate `1206ad9eb074763988609999e67962ec59a0c1b7` 为 `changes-requested`，0 open Blocker、1 open Major。F-009 仅阻塞 final work evidence/handoff freshness；final design closure exact `6df161ff5d5fc150cfa09f48ae54b7501cababcb` 的 `approved` 结论、F-001..F-008 closure 与 0/0 architecture verdict 不回退。

核心产品目标仍是 Kernel 强制的 Verified Procedure；Memory 保持非权威，流程遵循不冒充现实正确性，版本回退/Run recovery/Workspace recovery/Effect reconciliation-compensation 与 recorded replay/reexecute/simulated 保持分离，质量、Token/费用、延迟也继续独立报告。REQ-0034/SPEC-0010/RFC-0013 的 `approved/approved/accepted` 仅接受设计，正文和 work state 均明确 runtime/schema/Manifest/Plan/Node/Evidence executor 尚未实现。

# Acceptance trace

| Acceptance | Review result |
|---|---|
| Current AC-01/02 | canonical Procedure identity 与闭合字段由 REQ-0034 自有 protocol filter 覆盖；Plan instantiation 已明确移交 REQ-0018。 |
| Current AC-03 | TaskClass 仅允许闭合 Schema/predicate/limits，拒绝模型、代码和 caller label classification。 |
| Current AC-04/05/06 | approval subject、PrincipalRoleAssignment、PrincipalRootId 最低角色分离/quorum、freshness 与 verdict 均由 REQ-0034 admission 判定。 |
| Current AC-07/08/09/10 | retained registry、opaque pure admission、default-deny/no-leak 与零 Event/Capability/reservation/Effect/mutation 均有本 Requirement 自有命名负测。 |
| Current AC-11/12 | forward-only Schema、旧 bytes 保留、filter preflight 和 offline completion gates 映射完整。 |
| Downstream | REQ-0018/0035/0016/0014/0036 分别唯一拥有 Manifest/Plan、Node、Evidence/completion、executor 和 promotion；不再作为 REQ-0034 验收条件。 |

# Compatibility, permission, and isolation review

- 候选正确保留旧 Requirement ID，并把 Procedure、Plan、Behavior 建模为正交 Revision；旧 Run 不补写 Procedure、不被重新解释为 verified execution。
- Provider/Tool/Workspace/Sandbox 被定义为 observation/execution boundary，不能取得 Event Store、mutable Manifest、Budget/Evidence/terminal writer 或 unrestricted OS/network/secret handle；Node-bound activation 的最终方向正确。
- admission failure 的 zero Run Event/Capability/reservation/Effect 和有界错误分类能避免跨域存在性泄漏。
- remediation 已将 Plan conformance、pre-Run planning 和 approval independence 固化为 closed/default-deny 合同；未发现 strategy、adapter 或 runtime principal 可直达 authority 的剩余路径。

# Regression and test review

本 Reviewer 对 exact baseline `e7a939cad71a85ada97c3b60d61ba5c024d85ab9` 与 exact candidate `cfdc65af64675b8066b9bc429fbf998d588231bc` 独立执行了以下只读检查：

- `git merge-base <baseline> <candidate>` 返回 exact baseline；`HEAD` 为 exact candidate；审查前工作树 clean。
- `git diff --name-status` / `--stat` / `--check`：24 个文档或工作记录路径，716 insertions、104 deletions，whitespace check 通过。
- `git diff --exit-code <baseline>...<candidate> -- Cargo.toml Cargo.lock crates schemas scripts`：exit 0，无 runtime、dependency、schema 或 script diff。
- `git grep` exact baseline：无 `ProcedureRevision`/`VerifiedProcedureRevision`；`plan_revision` 仅为可选 Manifest ID/equality；`EvidenceRecord` 只有协议/验证；Effect payload、Schema 与 Kernel runtime 已存在。
- `python -B -m unittest discover -s scripts/tests -p "test_*.py"`：27 tests passed。
- `cargo fmt --all -- --check` 与 exact diff whitespace check：exit 0。
- `python -B scripts/check_docs.py`：exit 1。实际失败仅为 REVIEW-0001..0007、REVIEW-0010..0013 对本次 substantive docs/work changes 的 freshness；候选已在 VALIDATION remaining gates 中承认该门禁尚未闭合。当前 Review 不修改旧 Review，也不把该非零结果描述为通过。

未独立重跑会写入 build/schema 输出的 clippy、cargo test 或 schema generation；其结果仅作为候选 VALIDATION 中的实现者证据阅读，不作为本 Review 的独立通过结论。

# Scope and unrelated changes

exact diff 未引入产品代码、Cargo 依赖、Schema、脚本或归档 REQ-0010 内容。PRD、能力、架构、路线、Backlog、EPIC-0003..0007、REQ/SPEC/RFC 与 active work 的改动均与路线重设计直接相关，未发现无关产品扩张。按审查任务限制，未读取或依赖归档 REQ-0010 分支、实现者结论或其他聊天历史；active `HANDOFF.md` 仅作为 exact diff 路径计入 scope，未作为实质审查输入。

Plan/TASKS 明确了 fresh independent Reviewer、Reviewer-only Review 文件、同一 Reviewer 复审 Blocker/Major、0/0 才接受以及不得进入 REQ-0010 实现；整改后 REQ-0034 与后续 Requirement 的 owner、命名证明和拓扑已可执行。

# Re-review evidence

- 审查链：initial candidate `cfdc65af64675b8066b9bc429fbf998d588231bc`，原 Review commit `72a0f8b597996688f31007bd2fc7f613528f5cdc`，remediation content `499116a8e93e00a737f0c112d0a0104eb9386840`，re-review candidate/evidence `660cfca9e230f1440505c8e3bfd9a07bf17529ab`；三者均为前者的 descendant，工作树在复审前 clean，原 Review 在设计者整改中 byte-identical。
- F-001：REQ-0034 收窄为 Procedure/TaskClass/Verified package/retained registry/pure admission 的 12 个 AC。独立解析得到 12 AC、12 mappings、0 missing/extra/duplicate；Backlog 35 rows、0 missing/backward prerequisite。`REDESIGN-TEST-PLAN` 为每个 AC 提供具体 filter/preflight 命令，后续五项 authority 各有唯一 owner。
- F-002：RFC 冻结 canonical template witness、node/branch cardinality、exact edge/schema/evidence/policy、Capability 子集、budget envelope、effectful repeatability 和 canonical parameter domain；超出 envelope 必须新 Procedure、Verified package 与审批。REQ-0018 的八类命名负测在 sequence 1/lease/Effect 前拒绝。
- F-003：第一版唯一 bootstrap 为用户或 pinned pure builder 生成的 content-addressed `PlanProposalArtifact`，Harness 内 pre-Run Provider/Tool/Workspace/Sandbox I/O 被禁止；sequence-1 Manifest 是首次 planning authority。外部用户输入明确标为 non-replayable intake boundary，且不得声称治理、Recorded replay 或模型费用核算。
- F-004：最低独立性按认证 `PrincipalRootId` 而非 Agent/Actor alias 判定；完整 subject、role assignment、四类 root separation/quorum、review verdict/open finding、freshness、revocation/invalidation 均由 REQ-0034 pure admission 重算，REQ-0036 不得放宽。
- F-005：ARCH-0003 已与 exact baseline 一致陈述 REQ-0009 的 Effect Schema、Kernel Runtime、Projection 和 Boundary Inventory V2 已实现，并保留真实 Provider/Tool/Sandbox 尚未实现的边界；baseline code/schema search 与该陈述相符。
- F-006：VALIDATION 固定 baseline、initial candidate、original Review 与 remediation content exact revisions；`660cfca` 仅追加 re-review evidence/Plan handoff，TASK-06/07 状态与提交事实一致，TASK-05/08/09/10 因 completion/freshness/re-review/acceptance 尚未完成而保持 unchecked，未提前宣告完成。
- 独立只读检查：`python -B -m unittest discover -s scripts/tests -p "test_*.py"` 27 passed；`cargo fmt --all -- --check`、baseline/candidate与initial/candidate code/schema scope diff、remediation `git diff --check` 均 exit 0。未重跑会写 build/schema 输出的 clippy、cargo test 或 schema generation；其结果只作为 VALIDATION 的实现者证据。
- `python -B scripts/check_docs.py` 在 freshness 更新前 exit 1，仅列出 REVIEW-0001..0007、REVIEW-0010..0013 stale；该结果未被描述为通过。
- Final closure `c2c6e32503e8a6e85c80126869c4b302374d4abd..6df161ff5d5fc150cfa09f48ae54b7501cababcb` 仅含 17 个正式文档/work路径、77 insertions/22 deletions；对 Runtime、Cargo、Schema、scripts、CI、AGENTS、skills/agents/templates 和原 Review bytes 均为零差异。
- ADR-0012 忠实记录 reviewed decision：REQ-0034 只交付纯 retained registry admission；REQ-0018/0035/0016/0014/0036 分别拥有 Plan/Manifest、Node、Evidence、executor 与 promotion；pre-Run 只允许用户或 pinned pure builder 的零 Harness 外部 I/O artifact；Memory 与 Provider/Tool/Workspace/Sandbox observation 均无 authority；四类 recovery/rollback 与三种 replay 继续分离。
- F-007：ARCH-0003 现分别将 Procedure/TaskClass/Verified package identity/registry、Plan/DAG/Manifest、Node lifecycle 归给 REQ-0034、REQ-0018、REQ-0035，和 approved Requirement/Spec/Backlog 一致。
- F-008：RFC-0013 compatibility 段落重复已删除，并把未批准候选撤回语句改为 accepted ADR 的 forward supersession 规则；未改变旧 Run、SchemaSet、reader 或 replay 合同。
- Closure 前独立复跑 `python -B -m unittest discover -s scripts/tests -p "test_*.py"` 为 27 passed；`cargo fmt --all -- --check`、exact diff whitespace/code-scope/Review-preservation checks均 exit 0。首次 `check_docs.py` 仅报告 11 份既有 approved Review freshness，未发现 ADR/link/status/finding 结构错误。
- Evidence-only candidate `1f3078c521118635ddc498f2767ab51daec709fd..1206ad9eb074763988609999e67962ec59a0c1b7` 恰好只修改 VALIDATION/TASKS/PLAN/HANDOFF 四个 work 文件（16 insertions/11 deletions）；完整 `e7a939c..1206ad9` 对 Cargo、Runtime、Schema、scripts、CI、AGENTS、skills/agents/templates 零差异。27 个治理测试、fmt 与 exact diff check 通过，且没有 REQ-0034/REQ-0010 实现声明。
- 独立重跑 VALIDATION 第 29 行原命令实际 exit 1；Handoff 第 3 行与第 15 行当前阶段相互矛盾，形成 F-009。`check_docs.py` 在未刷新旧 Review 时准确只报告 REVIEW-0001..0007、REVIEW-0010..0013 对这四个 work 文件 stale；因 F-009 open，本轮未前移这些 Review freshness，也未把 docs gate 描述为通过。

# Re-review history

- 2026-09-05：首次独立设计审查 exact `cfdc65af64675b8066b9bc429fbf998d588231bc`；结论 `changes-requested`，1 open Blocker，3 open Major。
- 2026-09-05：同一 Reviewer 复审 exact `660cfca9e230f1440505c8e3bfd9a07bf17529ab`；F-001 至 F-006 closed，新增 F-007/F-008 Minor，结论 `approved`，0 open Blocker，0 open Major。
- 2026-09-05：final same-reviewer freshness review exact `6df161ff5d5fc150cfa09f48ae54b7501cababcb` against approval commit `c2c6e32503e8a6e85c80126869c4b302374d4abd`；ADR/status/link closure忠实、无实现声明或authority回退，F-007/F-008 closed，无新finding，保持 `approved`、0 open Blocker、0 open Major。
- 2026-09-05：evidence-only freshness review exact `1206ad9eb074763988609999e67962ec59a0c1b7` against Reviewer commit `1f3078c521118635ddc498f2767ab51daec709fd`；四文件scope与无实现声明成立，但 Validation preservation命令不可按当前记录复算且Handoff current phase矛盾，新增F-009 open Major。evidence-only closure为`changes-requested`、0 Blocker/1 Major；exact `6df161f`设计批准不回退。
