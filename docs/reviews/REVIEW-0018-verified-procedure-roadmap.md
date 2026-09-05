---
id: REVIEW-0018
title: Verified Procedure 路线重设计独立审查
status: changes-requested
owners: [independent-reviewer]
created: 2026-09-05
updated: 2026-09-05
links: [REQ-0034, SPEC-0010, RFC-0013]
independence: independent
reviewed_revision: cfdc65af64675b8066b9bc429fbf998d588231bc
open_blockers: 1
open_majors: 3
---

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Blocker | `docs/requirements/REQ-0034-verified-procedure-revision.md:31-42`; `docs/specs/SPEC-0010-verified-procedure-revision.md:17,27,104-111`; `docs/roadmap/requirement-backlog.md:31-35,55-57` | **Violated invariant:** 每个 high-risk Requirement 必须能在其 prerequisite 已 verified 后独立满足全部 AC，且每个 AC 映射到该 Requirement 可执行的测试。REQ-0034 的 AC-05/06 需要尚由 REQ-0018 定义的 PlanRevision，AC-07/08/10/16 的证明又明确推迟到 REQ-0035、REQ-0016 或 REQ-0014；但这些 Requirement 都以 REQ-0034 `verified` 为 prerequisite。依照 activation rule，REQ-0034 无法验证，后继项也不能激活，路线虽表面无环却在验收语义上形成闭环。 | 重新划分 Requirement ownership：要么把 REQ-0034 收窄为其自身可实现、可测试的 identity/registry 合同，并将 Plan/Node/Evidence/executor AC 移至各自 Requirement；要么把满足这些 AC 的最小实现前移进 REQ-0034 并同步 prerequisite、scope、Plan/Tasks。提交逐 AC 的唯一 owner、具体命令和非零命中测试矩阵，并用拓扑检查证明每项只依赖已可 verified 的前置项。 | open |
| F-002 | Major | `docs/requirements/REQ-0034-verified-procedure-revision.md:28,32`; `docs/rfcs/RFC-0013-verified-procedure-execution-architecture.md:52,60,124`; `docs/specs/SPEC-0010-verified-procedure-revision.md:27,60,103` | **Violated invariant:** Procedure 是允许流程的唯一 authority，Plan 只能进行闭合、不可扩权的 Task-specific 实例化。候选同时让 Procedure 定义 nodes/dependencies/transitions，让 Plan “展开具体 DAG”，但没有冻结模板到实例的映射、可选/重复节点、分支、参数、预算、Capability、Evidence、retry/recovery/compensation 的单调约束；AC-06 的“偏离产生新 Plan 并重新准入”还可能被解释为只换 Plan 即可偏离已验证 Procedure。Kernel 因而没有可测试的 conformance 判定，Planner 仍可能通过 Plan 删除必需节点、弱化 Evidence 或扩大能力。 | 在 RFC/SPEC 中定义 closed instantiation relation：Plan 只能从 Procedure 声明的模板、分支和参数域实例化，不能新增/删除强制节点、放宽依赖/终止条件、弱化 Evidence 或扩大 Capability/预算/重试/补偿；超出 envelope 必须产生新的 ProcedureRevision、VerifiedProcedureRevision 和审批，而不只是新 Plan。负测至少覆盖删节点、改边、复制 effectful node、放宽 schema/evidence、扩大 capability/budget 及 cross-task binding，全部在 lease/effect 前 zero-write 拒绝。 | open |
| F-003 | Major | `docs/rfcs/RFC-0013-verified-procedure-execution-architecture.md:72-82`; `docs/specs/SPEC-0010-verified-procedure-revision.md:13,30-36` | **Violated invariant:** 每个行为影响输入与外部效果必须先由权威 Manifest、Capability/Budget、Event 与 replay boundary 约束，模型不得在流程外调用 Provider。RFC 的数据流先由 Planner 提出 Plan，随后才创建固定 Behavior/Context/Model/Tool/Budget/Boundary 的 RunManifest；若 Planner 使用模型、检索或工具，规划调用没有 Run/Node lineage、预算、效果记录或 replay 合同。若禁止这些调用，候选又没有明确 Plan proposal 的受支持 bootstrap 方式。 | 冻结唯一 bootstrap：例如以独立 planning Run/phase 先原子固定 Task、候选 Procedure、Behavior、Provider/Tool/Workspace/Environment/Schema/Budget/Boundary，再通过受治理 Effect 产出带 exact provenance 的 Plan proposal；或明确第一版只接受离线、已签名且无运行时外部调用的 Plan artifact。测试必须证明 sequence-1 权威固定之前的 Provider/Tool/Workspace/Sandbox 请求零 dispatch/零费用，并证明 execution Run 的 Plan 可追溯到 exact planning authority。 | open |
| F-004 | Major | `docs/requirements/REQ-0034-verified-procedure-revision.md:29-30,59`; `docs/specs/SPEC-0010-verified-procedure-revision.md:24,32,101,116`; `docs/rfcs/RFC-0013-verified-procedure-execution-architecture.md:58,131` | **Violated invariant:** VerifiedProcedure 的独立批准必须是 Kernel 可判定且运行者不可自签的 authority，而不是一个声称“independent”的引用。候选固定 review/approval revision 与 authority revision，却未定义不可削弱的角色分离、review subject 绑定、actor alias 判定、freshness/invalidation 或最低 quorum；这些最小规则被推迟到 REQ-0036，但 REQ-0034 已要求注册并准入 VerifiedProcedureRevision。一个 retained authority 仍可能批准自己创建、执行或验证的候选，Kernel 无从证明“独立”。 | 在 REQ-0034 admission 合同中冻结不可被 policy 放宽的最小独立性：review decision 绑定 exact Procedure digest、TaskClass、evidence set、limitations、compatibility 和 approval policy；定义 creator/proposer/runner/evidence producer/verifier/reviewer/approver 的禁止重合或明确 quorum；定义撤销和 evidence invalidation 的 admission 效果。用同 actor/别名、角色重叠、旧 review 复用、内容或 evidence 变更、跨 scope approval 负测证明全部在 Run/Event/Effect 前拒绝。REQ-0036 可编排 promotion，但不能延后 REQ-0034 准入所需的判定语义。 | open |
| F-005 | Minor | `docs/architecture/version-and-event-model.md:81` | 候选修改了该 accepted 权威文档，却保留“REQ-0009 尚未实现 Schema 或 Runtime”的旧事实。基线 exact `e7a939c` 已包含 `effect_runtime.rs`、Effect payload 类型/Schema 生成与 retained schema sets；该陈述也与 README、ARCH-0001、REDESIGN-IMPACT 和 VALIDATION 冲突，可能误导后续 Node/Effect 迁移设计。 | 将该行改为与 exact 基线一致的 implemented/deferred 边界，并增加或运行跨文档实现状态一致性检查。 | open |
| F-006 | Minor | `.agents/work/active/REQ-0034-verified-procedure-revision/VALIDATION.md:6,15,34-39`; `.agents/work/active/REQ-0034-verified-procedure-revision/TASKS.md:7-11` | 原始验证仍把 subject 写成 commit 前 working tree，`check_docs` 标为 skipped，TASK-05/06 也未完成；因此实现者证据未精确 pin 到本次 candidate。候选已诚实列出 remaining gates，且独立检查能确认 commit/scope，所以该问题不改变上述设计结论。 | closure revision 中把 validation subject、实际命令/exit code、TASK 状态和 reviewed revision 对齐；不得把本 Review 或旧结果当成整改后 exact revision 的 freshness 证明。 | open |

# Verdict

`changes-requested`。候选已把核心产品目标收紧为 Kernel 强制的 Verified Procedure，明确 Memory 非权威、流程遵循不等于现实正确性，并区分了版本回退、Run/Workspace recovery、Effect reconciliation/compensation 以及 recorded replay/reexecute/simulated；质量、Token/费用和延迟也仍为独立维度。

但当前 Requirement 分解存在无法按 activation rule 完成的验收依赖闭环，Procedure/Plan 约束、Run 创建前的 Planner authority 和独立审批判定也尚未形成可执行、不可绕过的合同。当前 1 个 Blocker、3 个 Major 均为 open，不能批准 RFC、Spec 或路线。

# Acceptance trace

| Acceptance | Review result |
|---|---|
| AC-01/02 | identity、canonical digest 与闭合 Procedure 字段方向成立；Plan 实例化边界仍受 F-002 阻塞。 |
| AC-03/04 | exact retained registry、default deny 与 zero-effect admission 方向成立；独立审批判定受 F-004 阻塞。 |
| AC-05/06 | exact Manifest pin 的目标成立，但 REQ-0034 对未交付 PlanRevision 的依赖受 F-001 阻塞。 |
| AC-07/08/10/16 | Kernel-only Node/Evidence/Capability 目标成立；测试与交付 owner 被推迟给以 REQ-0034 为 prerequisite 的后继项，受 F-001 阻塞。 |
| AC-09/11 | Memory/自然语言非权威与完整 identity lineage/无存在性泄漏合同明确；后续仍需矩阵负测。 |
| AC-12/13 | Procedure/Behavior rollback、Run recovery、Workspace recovery、Effect reconciliation/compensation 与三种 replay 模式已清楚区分，未承诺虚假确定性或撤销外部事实。 |
| AC-14/15 | exact diff 为文档/工作记录，无 Rust、Cargo、Schema 或 dependency 改动；forward-major、旧 reader/SchemaSet/Run byte preservation 与 writer disable 策略明确。 |

# Compatibility, permission, and isolation review

- 候选正确保留旧 Requirement ID，并把 Procedure、Plan、Behavior 建模为正交 Revision；旧 Run 不补写 Procedure、不被重新解释为 verified execution。
- Provider/Tool/Workspace/Sandbox 被定义为 observation/execution boundary，不能取得 Event Store、mutable Manifest、Budget/Evidence/terminal writer 或 unrestricted OS/network/secret handle；Node-bound activation 的最终方向正确。
- admission failure 的 zero Run Event/Capability/reservation/Effect 和有界错误分类能避免跨域存在性泄漏。
- F-002 至 F-004 关闭前，Plan conformance、pre-Run planning 和 approval independence 仍是可绕过 trusted-kernel 的路径。

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

Plan/TASKS 明确了 fresh independent Reviewer、Reviewer-only Review 文件、同一 Reviewer 复审 Blocker/Major、0/0 才接受以及不得进入 REQ-0010 实现，独立门禁设计正确；但 F-001 使后续 runtime Requirement 的逐项验收计划本身尚不可执行。

# Re-review history

- 2026-09-05：首次独立设计审查 exact `cfdc65af64675b8066b9bc429fbf998d588231bc`；结论 `changes-requested`，1 open Blocker，3 open Major。
