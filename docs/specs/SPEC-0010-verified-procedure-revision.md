---
id: SPEC-0010
title: 不可变已验证流程版本规范
status: draft
owners: [runtime-kernel]
created: 2026-09-05
updated: 2026-09-05
links: [REQ-0034, RFC-0013, REQ-0003, REQ-0005, REQ-0006, REQ-0007, REQ-0009]
---

# Behavioral contract

`ProcedureRevision` 是可执行流程内容；`VerifiedProcedureRevision` 是引用 exact 流程内容、适用范围、验证证据和独立审批的不可变批准包。procedure-capable Run 必须在 sequence 1 的 `RunManifest` 中固定 exact 已验证流程与 Task-specific `PlanRevision`。只有 Kernel 保留注册表可解析、验证并准入这些身份。

模型、Planner、Memory、Provider、Tool、Workspace adapter、Sandbox 与外部 Worker 都是不可信提议者或执行边界。它们不能创建审批包、写权威节点/Evidence/终态、扩大 Capability/预算、替换 Manifest pin 或直接选择“当前最新”流程。

本规范冻结身份、信任边界、兼容与后续 Requirement 的责任划分。REQ-0034 的实现纵切只发布公共合同、保留注册表、procedure-capable Manifest major 和确定性 Fake admission；节点状态机、最小 Evidence Gate、执行器及流程晋升分别由 REQ-0035、REQ-0016、REQ-0014 与 REQ-0036 实现。

# Inputs, outputs, states, and failure behavior

## Immutable identities

- `ProcedureRevision` 固定 metadata、hash-view Schema、节点与依赖、合法转移、节点 I/O Schema、Capability/Evidence/checkpoint/recovery/retry/terminal/compensation refs、限制和兼容声明。
- `VerifiedProcedureRevision` 固定 procedure revision/digest、Task class revision、evaluation evidence-set revision/digest、independent review decision revision/digest、approval policy/authority revision、limitations、compatibility range 与批准主体。
- 第一版 `TaskClassRevision` 只允许 Kernel-retained 的闭合 Task Schema 与 canonical constraint predicates；不执行模型分类器、任意代码或 caller 自报标签。
- `ProcedureRegistryRevision` 以 canonical order 固定可用于某环境的 verified revisions、撤销/禁用事实引用和 registry config digest。注册表只解析 exact retained content；不接受 caller object、current pointer 或同 ID 替换。
- `PlanRevision` 由 REQ-0018 定义，必须绑定 exact Task、Verified Procedure 与实例化 Node DAG；procedure-compatible 不等于可以省略 exact equality 与适用范围检查。

## Admission and authority

1. Kernel 从认证 scope、retained SchemaSet、Manifest writer policy 与 retained Procedure registry 构造 Run admission。
2. admission 验证所有 Revision metadata/content digest、approval evidence/review decision、registry membership、compatibility、scope 与 Task classification。
3. `RunManifest` 原子固定 verified procedure、Plan 与既有版本角色；任何失败发生在 lifecycle sequence 1 与外部 Effect 前。
4. 后续节点请求必须携带 Run/Plan/Procedure/Node identity；REQ-0035 重新折叠 exact history 后才可签发节点用途能力。
5. Evidence 与终态只有 Kernel admission 可提交；模型文本、Memory hit、adapter success 或退出码 observation 均不是 authority。

## Closed failure classes

- `procedure_unknown_or_unretained`
- `procedure_identity_mismatch`
- `procedure_not_verified`
- `procedure_revoked_or_disabled`
- `procedure_task_incompatible`
- `plan_procedure_mismatch`
- `manifest_procedure_incomplete`
- `procedure_scope_mismatch`
- `procedure_schema_unsupported`
- `procedure_history_corrupt`

这些失败只返回有界安全分类，不泄漏另一隔离域的 procedure、evidence 或 approval 是否存在。任何失败均为零 Run Event、零 Capability、零 reservation、零 Effect。

## Authority separation

| Object | Purpose | Authority |
|---|---|---|
| Conversation/user preference | 帮助交互与默认建议 | 非权威上下文 |
| Project knowledge/instructions | 提供来源可追踪的操作知识 | 非权威 artifact/context |
| ProcedureRevision | 描述候选流程内容 | 不因存在而可用于 verified execution |
| VerifiedProcedureRevision | 固定流程与外部验证/独立审批 | Kernel retained identity |
| PlanRevision/Task DAG | 为 exact Task 实例化已验证流程 | Kernel admission；Planner 仅 proposal |
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
| Data isolation | 现有 `IsolationScope` 覆盖 tenant/user/workspace/run/agent，但未覆盖 Plan/Procedure/Node lineage | 新合同要求在命令、Event 与投影中绑定 exact identity；负向矩阵进入 REQ-0034/0035 |
| API/schema | 基线最高 Run Manifest v3；没有 Procedure/Plan/Node Schema | 实现必须前向发布新 major，并保留全部旧 SchemaSet/reader bytes |
| Persistence/replay | Event Store、Effect 与 fixed-horizon Recorded replay 已实现；procedure/node/evidence execution stream 未实现 | 复用 append-only/fold/horizon原则；不声称当前已有流程 replay |
| Concurrency | 未来 Plan revision、node claims、recovery 与 promotion pointer 存在 TOCTOU/MVCC 风险 | REQ-0035 固定 writer-lock revalidation；REQ-0036 固定 promotion MVCC |
| Security | Memory/Planner/model/adapter 可能成为 confused deputy；外部能力可能绕过节点 | Kernel-only admission、用途受限 Node lease、zero-effect rejection；architecture-review 必须检查 |
| Performance | Manifest/registry与每节点Event增加存储和准入延迟 | 先记录基线；不声明优化，完整 DAG/Graph 投影后再设阈值 |
| Rollback | “回滚”混用会导致错误恢复或虚假撤销 | 正式拆分 procedure/behavior rollback、run/workspace recovery、effect reconciliation/compensation |

# Compatibility and migration

- 只新增 procedure-capable Manifest/Event/Schema major；旧 v1-v3 writer/reader、SchemaSet、Snapshot、Boundary Inventory 与 SQLite v2 保持 byte-identical。
- 旧 Run 不补写 Procedure pin，也不被描述为已验证流程执行；可以按原语义 inspect/recorded replay。
- 新 writer 可通过停用 procedure-capable Run admission 回滚；已持久化的新 Event 与 Revision 只保留、不可删除或降格解释。
- 路线图接受不授权 schema 或 runtime 实现；REQ-0034 实施前仍需独立 Plan/Tasks review。
- 归档 REQ-0010 分支不得整体 cherry-pick；未来只按新设计选择性复用无权威纯函数与边界测试。
- G2A 的 Provider/Tool/Workspace/Sandbox 在 Node contract 交付前不得暴露 Agent-callable dispatch；REQ-0035 以前向 identity/binding 接入，REQ-0014 才开放 Node-scoped executor proposal。

# Test traceability

| Acceptance | Scope/layer | Scenario | Planned evidence |
|---|---|---|---|
| AC-01, AC-02 | Focused protocol | canonical identity、字段变化、未知字段、DAG/transition closure | exact protocol contract tests + schema golden |
| AC-03, AC-04 | Focused kernel | 缺失/自签/替换/撤销 approval 与 registry entry 全部 zero-write | `verified_procedure_registry_default_deny` filter |
| AC-05 | Protocol/compatibility | 新 Manifest exact pin；v1-v3 与 retained sets byte-identical | generator twice + retained digest assertions |
| AC-06 | Contract/integration | Plan 绑定错误 Task/Procedure/参数或偏离流程 | `plan_procedure_binding_is_exact` filter |
| AC-07, AC-08 | Impacted lifecycle | 模型/producer不能直接推进节点或 Run success | REQ-0035/REQ-0016 named negative tests |
| AC-09 | Security/isolation | Memory、Markdown、自然语言 completion 不满足依赖/Evidence | `memory_and_model_output_are_non_authoritative` filter |
| AC-10 | Security/E2E | Provider/Tool/Workspace/Sandbox 缺 Node lease 时 zero effect | procedure executor capability-negative matrix |
| AC-11 | Security/isolation | scope、Task、Plan、Procedure、Node、Evidence 混用 | cross-axis matrix + no existence leak assertions |
| AC-12 | Contract/recovery | 四类恢复/回退产生不同事件与允许动作 | recovery taxonomy contract tests |
| AC-13 | Replay/E2E | recorded 零执行；reexecute/simulated 新 Run 与 lineage | fixed-horizon counters and manifest assertions |
| AC-14 | Scope/static | REQ-0034 不引入 loop/multi-agent/auto-promotion/live provider | repository scope checker |
| AC-15, AC-16 | Core/full | 旧历史可读、未知 major fail closed、全部 completion gates | workspace offline suite + schema identity report |

# Deferred design scope

- REQ-0034 的第一版 Task matching 固定为 retained `TaskClassRevision` 的闭合 Schema/constraint predicate，不引入模型或任意代码 classifier；扩展 classifier 需要独立 Requirement。
- REQ-0034 只接受 Kernel-retained approval authority 和 exact review/evidence references。REQ-0036 冻结 promotion command、独立批准最小集合与默认指针 MVCC；普通运行者不能注册 approval authority。
- 第一版 compensation 只允许引用已注册 Effect policy，默认不自动执行；通用 compensation DSL 不在当前路线范围。
