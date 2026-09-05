---
id: SPEC-0010
title: 不可变已验证流程版本规范
status: draft
owners: [runtime-kernel]
created: 2026-09-05
updated: 2026-09-05
links: [REQ-0034, RFC-0013, REQ-0003, REQ-0004, REQ-0005, REQ-0007, REQ-0009]
---

# Behavioral contract

`ProcedureRevision` 是可执行流程内容；`TaskClassRevision` 是闭合适用范围；`VerifiedProcedureRevision` 是引用 exact 流程内容、Task class、验证证据和独立审批的不可变批准包。REQ-0034 只交付这些协议、retained registry 与纯 admission，不创建 Run 或执行状态。只有 Kernel 保留注册表可解析、验证并返回不可序列化的 `AdmittedVerifiedProcedure`。

模型、Planner、Memory、Provider、Tool、Workspace adapter、Sandbox 与外部 Worker 都是不可信提议者或执行边界。它们不能创建审批包、写权威节点/Evidence/终态、扩大 Capability/预算、替换 Manifest pin 或直接选择“当前最新”流程。

本规范冻结身份、最低独立性、信任边界、兼容与后续 Requirement 的责任划分。REQ-0034 的实现纵切只发布公共合同、保留注册表和确定性纯 admission；procedure-capable Manifest/Plan、节点状态机、最小 Evidence Gate、执行器及流程晋升分别由 REQ-0018、REQ-0035、REQ-0016、REQ-0014 与 REQ-0036 实现。

# Inputs, outputs, states, and failure behavior

## Immutable identities

- `ProcedureRevision` 固定 metadata、hash-view Schema、节点与依赖、合法转移、节点 I/O Schema、Capability/Evidence/checkpoint/recovery/retry/terminal/compensation refs、限制和兼容声明。
- `VerifiedProcedureRevision` 固定 procedure revision/digest、Task class revision、evaluation evidence-set revision/digest、`PrincipalRoleAssignmentRevision` revision/digest、independent review decision revision/digest、approval policy/authority revision、limitations、compatibility range 与批准主体。
- 第一版 `TaskClassRevision` 只允许 Kernel-retained 的闭合 Task Schema 与 canonical constraint predicates；不执行模型分类器、任意代码或 caller 自报标签。
- `PrincipalRootId` 来自认证根身份，Agent/Actor alias 只作为显示或委托身份，不能制造独立性。mandatory evidence producer、verifier、reviewer、approver 四类 root 两两不同，且都不能等于 candidate creator/proposer/runner。
- `ProcedureRegistryRevision` 以 canonical order 固定可用 verified revisions、Task classes、evidence/verifier roots、approval authorities/policies、撤销/失效事实引用和 registry config digest。注册表只解析 exact retained content；不接受 caller object、current pointer 或同 ID 替换。
- `AdmittedVerifiedProcedure` 是 crate-private opaque value，绑定 scope、Task revision/digest、verified procedure/registry/policy revisions/digests、admission policy 与一次性 nonce/seal；公共 JSON 值不能构造或 reseal。

## Admission and authority

1. Kernel bootstrap 从构建/部署允许列表加载 retained SchemaSet、Procedure registry 和 approval authority roots；runtime principal 没有 registry writer。
2. caller 只能提出 exact scope、TaskRevision 和 requested VerifiedProcedureRevision；这些值都不是 authority。
3. admission 验证 Revision metadata/content digest、完整 review subject、evidence set、registry membership、Task class、principal-root 角色分离/quorum、freshness、invalidation/revocation、compatibility 与 scope。
4. 成功返回 opaque `AdmittedVerifiedProcedure`，失败返回有界分类；两者均不写 Event、registry、budget 或 Effect。
5. REQ-0018 必须重新验证 opaque binding，并在 sequence-1 execution Manifest 中首次固定 Procedure/Plan 与全部行为输入；REQ-0034 的 token 本身不能启动 Run。

## Closed failure classes

- `procedure_unknown_or_unretained`
- `procedure_identity_mismatch`
- `procedure_not_verified`
- `procedure_revoked_or_disabled`
- `procedure_task_incompatible`
- `procedure_scope_mismatch`
- `procedure_schema_unsupported`
- `procedure_history_corrupt`
- `procedure_independence_violation`
- `procedure_review_stale_or_invalidated`
- `procedure_approval_quorum_missing`

这些失败只返回有界安全分类，不泄漏另一隔离域的 procedure、evidence 或 approval 是否存在。任何失败均为零 Run Event、零 Capability、零 reservation、零 Effect。

## Authority separation

| Object | Purpose | Authority |
|---|---|---|
| Conversation/user preference | 帮助交互与默认建议 | 非权威上下文 |
| Project knowledge/instructions | 提供来源可追踪的操作知识 | 非权威 artifact/context |
| ProcedureRevision | 描述候选流程内容 | 不因存在而可用于 verified execution |
| VerifiedProcedureRevision | 固定流程与外部验证/独立审批 | Kernel retained identity |
| PlanRevision/Task DAG | 为 exact Task 实例化已验证流程 | REQ-0018 Kernel admission；不属于 REQ-0034 |
| Evidence requirement/record | 声明与证明节点/Run 条件 | Kernel gate；producer observation 非权威 |
| BehaviorRevision | 固定 Planner/Router/Memory 等策略 | 与 Procedure 正交；后期独立晋升 |

## Recovery and replay taxonomy

- Procedure/Behavior rollback：只切换后续 Run 的默认选择指针并记录审计事实；历史 Manifest 不变。
- Run recovery：以同一 Manifest、Plan、Procedure 与 Event history 从合法 checkpoint 恢复；不选择新版本。
- Workspace recovery：从固定 WorkspaceRevision/checkpoint 重建或生成新的恢复 revision；不篡改 Effect history。
- External reconciliation/compensation：先确认已发生事实，再由新受治理 Effect 执行允许的补偿；不声称时间倒流。
- Recorded replay：固定 source horizon，只读复用记录边界；reexecute 和 simulated 都是有明确 lineage 的新 Run。

# Impact analysis

| Dimension | Finding | Evidence / response |
|---|---|---|
| Direct | 修改 PRD、能力地图、架构、路线图、Backlog 与 EPIC-0003..0006；新增 EPIC-0007、REQ-0034/SPEC-0010/RFC-0013 和工作记录 | 文档 diff 必须保持无产品代码、Cargo、schema 改动 |
| Indirect | REQ-0014、REQ-0016、REQ-0018 的顺序和职责改变；新增 REQ-0035/REQ-0036 计划项 | Backlog 显式列出新 prerequisite，保持既有 ID 不复用 |
| Call/permission | 当前 lifecycle 只校验可选 `plan_revision`，没有 procedure/node authority | `crates/pareto-kernel/src/event_store/lifecycle.rs`; `rg` 仅发现 `plan_revision` equality |
| Data isolation | 现有 `IsolationScope` 覆盖 tenant/user/workspace/run/agent，但没有 Procedure/approval identity | REQ-0034 绑定 scope/Task/Procedure/Principal/Evidence/Registry；Plan/Node 负向矩阵分别由 REQ-0018/REQ-0035 拥有 |
| API/schema | 基线没有 Procedure/TaskClass/Verified package/registry Schema | REQ-0034 只前向发布这些类型；Manifest/Plan major 由 REQ-0018 发布并保留旧 bytes |
| Persistence/replay | Event Store、Effect 与 fixed-horizon Recorded replay 已实现；procedure/node/evidence execution stream 未实现 | 复用 append-only/fold/horizon原则；不声称当前已有流程 replay |
| Concurrency | 未来 Plan revision、node claims、recovery 与 promotion pointer 存在 TOCTOU/MVCC 风险 | REQ-0035 固定 writer-lock revalidation；REQ-0036 固定 promotion MVCC |
| Security | Memory/Planner/model/adapter 可能成为 confused deputy；外部能力可能绕过节点 | Kernel-only admission、用途受限 Node lease、zero-effect rejection；architecture-review 必须检查 |
| Performance | Manifest/registry与每节点Event增加存储和准入延迟 | 先记录基线；不声明优化，完整 DAG/Graph 投影后再设阈值 |
| Rollback | “回滚”混用会导致错误恢复或虚假撤销 | 正式拆分 procedure/behavior rollback、run/workspace recovery、effect reconciliation/compensation |

# Compatibility and migration

- REQ-0034 只新增 Procedure/TaskClass/Verified package/registry Schema；旧 Manifest writer/reader、SchemaSet、Snapshot、Boundary Inventory 与 SQLite v2 保持 byte-identical。
- 旧 Run 不补写 Procedure pin，也不被描述为已验证流程执行；可以按原语义 inspect/recorded replay。
- 新 writer 可通过停用 procedure-capable Run admission 回滚；已持久化的新 Event 与 Revision 只保留、不可删除或降格解释。
- 路线图接受不授权 schema 或 runtime 实现；REQ-0034 实施前仍需独立 Plan/Tasks review。
- 归档 REQ-0010 分支不得整体 cherry-pick；未来只按新设计选择性复用无权威纯函数与边界测试。
- G2A 的 Provider/Tool/Workspace/Sandbox 在 Node contract 交付前不得暴露 Agent-callable dispatch；REQ-0035 以前向 identity/binding 接入，REQ-0014 才开放 Node-scoped executor proposal。

# Test traceability

| Acceptance | Scope/layer | Scenario | Planned evidence |
|---|---|---|---|
| AC-01, AC-02 | Focused protocol | canonical Procedure identity、字段变化、未知字段、template/transition closure | `procedure_revision_contract_is_closed` + schema golden |
| AC-03 | Focused protocol | closed Task schema/predicates；模型、代码和 caller label 拒绝 | `task_class_revision_is_closed_and_pure` |
| AC-04 | Focused protocol/kernel | approval subject 任一 digest/role assignment/limitation/compatibility 变化使旧 decision 无效 | `verified_package_binds_complete_review_subject` |
| AC-05 | Security/isolation | 每一对角色重叠及 Agent alias 同 root 均拒绝 | `verified_package_independence_matrix` |
| AC-06 | Focused kernel | non-approved/open Major、过期 review、错误 subject/actor 拒绝 | `verified_review_decision_default_deny` |
| AC-07 | Focused kernel | caller/current/substituted registry 与 runtime authority registration 拒绝 | `verified_registry_is_retained_and_kernel_owned` |
| AC-08 | Focused kernel | exact admission positive path与 opaque binding mutation | `verified_procedure_admission_is_exact_and_opaque` |
| AC-09 | Security/isolation | revocation/invalidation/cross-scope/unknown major 的有界无泄漏拒绝 | `verified_procedure_admission_negative_matrix` |
| AC-10 | Impacted kernel | 所有 admission 路径计数 Run/Event/Capability/reservation/Effect/registry mutation 为零 | `verified_procedure_admission_has_zero_effects` |
| AC-11 | Compatibility | 新类型生成两次；所有旧 Manifest/SchemaSet/SQLite bytes 不变 | retained digest assertions + scope checker |
| AC-12 | Core/full | 每个 exact filter 非零命中，workspace completion gates | filter preflight + offline suite + validation report |

# Downstream requirement ownership

- REQ-0018：closed Plan instantiation relation、无外部调用的 pre-Run Plan proposal、procedure-capable Manifest 与 Plan provenance。
- REQ-0035：Node Event/state/lease/checkpoint、run recovery 和 Node-bound Effect identity。
- REQ-0016：最小 Evidence admission/coverage 与 Node/Run completion gate。
- REQ-0014：单 Agent executor 只能通过 Node-scoped Provider/Tool/Workspace/Sandbox 请求。
- REQ-0036：promotion command、候选生命周期、registry/default pointer MVCC 与 procedure rollback；不得放宽 REQ-0034 独立性。
- REQ-0026/REQ-0023：完整 Evidence Graph 与受治理 adaptive replan；不作为前述 Requirement 的完成条件。
