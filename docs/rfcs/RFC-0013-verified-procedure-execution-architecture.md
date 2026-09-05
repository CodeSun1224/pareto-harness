---
id: RFC-0013
title: 已验证流程执行与路线重排架构
status: proposed
owners: [maintainers, runtime-kernel]
created: 2026-09-05
updated: 2026-09-05
links: [REQ-0034, SPEC-0010, PRD-0001, CAP-0001, ARCH-0001, ARCH-0002, ARCH-0003, ROADMAP-0001, BACKLOG-0001, RFC-0001, ADR-0001]
---

# Summary

将 Pareto Harness 的首要产品闭环从“安全调用模型和工具的 Agent Runtime”收紧为“Kernel 强制执行的已验证流程复用系统”。新增 `ProcedureRevision` 与 `VerifiedProcedureRevision`，把基础 Plan Revision/Task DAG、Kernel 节点状态机和最小 Evidence Gate 前移到单 Agent 执行器之前；Memory 保持非权威，完整 Evidence Graph、自适应优化、Behavior Canary/Promote/Rollback 继续后置。

该设计承诺流程遵循和证据准入，不承诺现实结果百分百正确。Provider 是流程节点可使用的一种受治理外部能力，不是 Planner、流程记忆或完成判定。

# Motivation and requirements

当前 G1 已证明 Event、Revision、Manifest、Capability/Budget、Cancellation、Effect Intent/Receipt、恢复/对账和 fixed-horizon Recorded replay。实际代码只把 `plan_revision` 保存为可选 ID；没有 Plan/Task DAG、Procedure、Node lifecycle 或执行期 Evidence Gate。原路线却先安排 REQ-0014 自由 Planner/Agent Loop，再在 REQ-0016 加 Evidence Gate、REQ-0018 加 Plan/DAG、G5 才加 Behavior Promotion。这会固化一个由模型选择下一步并自报完成的普通 Agent Loop。

设计必须满足 REQ-0034，并回答：什么是权威流程版本；谁能推动节点和完成；成功轨迹如何成为候选而非自动成为事实；后续 Run 如何 exact 固定；异常如何恢复、对账或补偿；Recorded replay、reexecute、simulated 如何保持诚实。

# Proposed design

## Authority layers

```text
non-authoritative knowledge
  conversation memory | user preference | project guidance | retrieved experience
                         |
                         v proposal only
versioned strategy      Planner | Router | Memory | retry | evaluator
                         |
                         v Plan/Action proposal
trusted kernel          VerifiedProcedureRevision + RunManifest
                        PlanRevision/Task DAG + Node state machine
                        Capability/Budget + Evidence Gate + Effect boundary
                         |
                         v purpose-bound lease
external boundary       Provider | Tool | Workspace | Sandbox
                         |
                         v untrusted observation
trusted kernel          receipt/evidence admission -> node/run transition
```

任何从 non-authoritative/strategy/external 层直达 Event、Evidence、terminal、Promotion 或通用系统权限的路径都是架构违规。

## Revision model

`ProcedureRevision` 描述可执行、可检查的流程内容：

- node definitions、dependency edges 与合法状态转移；
- 输入输出 Schema、Capability 与预算类别；
- Evidence requirement、freshness 与 verifier class；
- checkpoint、retry、recovery、terminal 与 compensation policy refs；
- compatibility、limits、parent lineage 与 canonical content digest。

`VerifiedProcedureRevision` 是审批包，不是可变状态字段。它固定 exact Procedure 内容、Kernel-retained `TaskClassRevision`、验证 evidence set、独立 Review decision、批准策略/authority、限制与兼容范围。第一版 Task class 只使用闭合 Schema 与 canonical constraints，不执行模型或任意代码 classifier。成功 Run 只能成为候选输入；REQ-0036 负责从候选到已验证版本的受控晋升、默认选择与流程版本回退，但不能放宽 REQ-0034 的最低独立性。

最低独立性按认证 `PrincipalRootId` 而不是可变 Agent/Actor alias 比较。`PrincipalRoleAssignmentRevision` 固定 candidate creator/proposer/runner、mandatory evidence producer/verifier、reviewer 与 approver root sets。candidate creator/proposer/runner 与 mandatory evidence producer 不得担任 verifier、reviewer 或 approver；mandatory evidence producer、verifier、reviewer、approver 四类 principal root 两两不同，且至少各一名。review decision 必须绑定 Procedure、TaskClass、evidence set、role assignment、limitations、compatibility、approval policy/authority 的 exact revisions/digests、verdict、freshness horizon 与 reviewer root。任一内容/evidence/role/limitation/compatibility 变化、过期、撤销或 evidence invalidation 都使旧批准不可用于新 admission。

`PlanRevision` 由 REQ-0018 为 exact Task 实例化 verified procedure，并携带从 Procedure template 到每个 Plan node 的 canonical witness。闭合 instantiation relation 为：

- 每个 Plan node 引用 exact template ID 与唯一 instance ordinal；不得新增未声明模板，所有 required template 满足 min/max cardinality。
- branch 只能选择 Procedure 声明的闭合选项并满足 branch cardinality；实例化后的 dependency edges、terminal conditions 和 branch-derived structure 必须 exact，不允许任意增加、删除或改边。
- 第一版要求 I/O SchemaRef、Evidence requirement、freshness/verifier/quorum、retry/recovery/compensation policy 与 success/failure conditions exact；Plan 不能增加、删除、放宽或替换这些合同。
- Capability 是 Procedure allowance 的子集；预算位于声明的闭合 min/max envelope；retry 次数和 eligible failure class 不得扩大。
- effectful template 默认 max cardinality 1；只有 Procedure 显式声明 repeatable、idempotency class 与上限时才可复制。
- 参数值必须属于 canonical domain。第一版不做一般 Schema subsumption，所有 schema/policy 都用 exact ref 避免不可判定的“兼容”旁路。

超出上述 envelope 必须产生新的 `ProcedureRevision`、新的 `VerifiedProcedureRevision` 和新审批；只创建新 Plan 不足。基础阶段禁止运行中自由改变 Plan；后续 REQ-0023 的 adaptive replan 必须产生子 PlanRevision、记录触发证据并重新准入。

## Node and evidence authority

REQ-0035 在 Kernel 建立 Node stream/state machine。节点至少区分 pending、ready、claimed、running、evidence-pending、succeeded、failed、paused、recovery-required、compensation-required 与 terminal。只有 exact dependencies 已满足、当前状态合法、Capability/Budget 有效且输入匹配时才能签发一次性 Node execution lease。

REQ-0016 前移并提供最小 Evidence Gate：指定测试、构建、静态检查或人工批准的 evidence requirement 与 admitted record exact 对齐后，Kernel 才能提交 node/run success。REQ-0026 后续扩展完整 Evidence Graph、派生 provenance、invalidations 与复杂 evaluator，不替代第一版完成门禁。

模型只能提交 `ActionProposal`、`PlanProposal` 或 observation。adapter 成功、工具退出码、模型自然语言、Memory hit 和 Planner confidence 都不能直接转为 Evidence 或完成状态。

## Execution and capability flow

1. Intake 固定 `TaskRevision`，并接收一个内容寻址的 `PlanProposalArtifact`。第一版 artifact 只能由用户或 pinned pure builder 在 Harness 内无 Provider/Tool/Workspace/Sandbox I/O 地生成，携带 proposer root、builder revision、Task/Procedure identity 与 canonical digest；它在 admission 前不是 authority。用户在 Harness 外如何形成输入是显式 non-replayable intake boundary，系统不得把该过程称为受治理 planning、Recorded replay 或已核算模型费用。
2. Kernel 从 retained registry 解析 exact `VerifiedProcedureRevision`，按 closed instantiation relation 验证 Plan proposal、Task compatibility、proposer identity 与全部 digests。
3. Kernel 在 execution Run sequence 1 首次将 validated Plan 与 Procedure 变为 authority：`RunManifest` 原子固定 Plan artifact/provenance、Procedure、Task、Behavior、Workspace、Environment、Context、Model、Tool、Schema、Budget 与 Boundary identity。sequence 1 的 Kernel admission decision 是第一版唯一 planning authority；此前只有非权威输入 artifact。
4. Scheduler 只请求 ready Node；Kernel 重折叠 exact history并签发用途受限 lease。
5. 单 Agent 执行器请求 Provider/Tool/Workspace/Sandbox 子能力；每个子能力绑定 Run/Plan/Procedure/Node 与 Effect identity。
6. 外部结果仅是 observation；Kernel admission 形成 Receipt、Evidence 或失败事实。
7. Evidence Gate 决定 Node 与 Run 转移；模型不能自行完成。

REQ-0010 的新定位是“为已验证流程执行器提供受 Kernel 治理的模型调用能力”。其正式重设计必须按 Provider contract/Manifest/Secret/Network/Cost/Effect/Replay → 同路径 Fake → loopback Mock → HTTP/SSE adapter 的顺序，adapter 只能接收 Kernel-issued sealed session。

在 sequence-1 execution Manifest 提交前，所有 Provider/Tool/Workspace/Sandbox proposal 必须零 dispatch、零费用、零外部读写。第一版明确不支持 model-assisted pre-Run planning；该能力必须由后续独立 Requirement 建立自己的 planning Run/Verified Procedure/Manifest/Node/Evidence 边界，不能复用无权威 bootstrap。由于 Provider/Tool/Workspace/Sandbox 先于 Node 状态机交付，G2A 只能暴露 Kernel-private proposal/orchestration 与确定性合同测试，不得提供 Agent 可直接调用的通用 dispatch API。REQ-0035 以 forward contract 将既有 operation/effect identity 绑定到 exact Procedure/Plan/Node，REQ-0014 才首次向 Agent executor 提供 Node-scoped 请求入口。任何 Task-only 或 ambient adapter 旁路都是 Blocker。

## Promotion and rollback boundaries

- Procedure promotion/rollback：REQ-0036 管理 candidate、evidence、independent approval、registry/default pointer 与审计事件；rollback 只影响后续 Run 选择。
- Behavior promotion/rollback：REQ-0028..0032 管理 Planner/Router/Memory 等策略及 Canary；与 Procedure identity 正交。
- Run recovery：在同一 Manifest/Plan/Procedure 下从 Event/checkpoint 恢复，不选择新 Procedure 或 Behavior。
- Workspace recovery：重建固定 WorkspaceRevision/checkpoint，或产生带 lineage 的新恢复 revision。
- Effect reconciliation/compensation：保存 applied/partial/unknown 事实；compensation 是新的受治理 Effect，不删除旧事实。

## Replay modes

- `recorded_replay`：固定 source Run 与 inclusive boundary/node/evidence horizon，只读 fold；零 Provider/Tool/Workspace 外部执行、零 reserve/settle、零权威写入。
- `reexecute`：固定 source lineage 和 comparison contract，创建新 Run 并重新执行；结果差异是事实，不称为确定性复现。
- `simulated`：固定非空 fixture revisions，创建 standalone 或 derived 新 Run；fixture 也必须通过版本和隔离准入。

## Roadmap order

保留已经发布的 Requirement ID，不重编号或复用。新的关键顺序为：

```text
REQ-0003..0009 trusted kernel foundation
-> REQ-0010 Provider
-> REQ-0011 Coding Tools
-> REQ-0012 Workspace
-> REQ-0013 Sandbox
-> REQ-0034 Procedure/Verified Procedure identity
-> REQ-0018 PlanRevision/basic Task DAG
-> REQ-0035 Kernel Node state machine/checkpoint
-> REQ-0016 minimal Evidence Gate
-> REQ-0014 single-Agent procedure executor
-> REQ-0036 candidate promotion/reuse/procedure rollback
-> REQ-0015 non-authoritative Memory
-> REQ-0017 CLI
-> REQ-0019..0022 Multi-Agent
-> REQ-0023..0033 adaptive optimization and controlled evolution
```

# Interfaces, data flow, and invariants

- Kernel owns Procedure/approval identity validation, Manifest pinning, Plan compatibility, Node states, Capability/Budget, Evidence admission, checkpoint/recovery, Effect admission and procedure promotion pointer changes.
- Planner、Agent、Memory 与 adapter 只持有 schema-bounded proposal/observation interfaces；不得持有 Event Store、mutable Manifest、Budget writer、Evidence writer、terminal writer 或 unrestricted OS/network/secret handle。
- Procedure、Plan 与 Behavior 是正交 Revision。Procedure 定义允许的流程；Plan 实例化一次 Task；Behavior 固定产生提议的策略。任一变化不得静默覆盖另两者。
- 每个外部 Effect 绑定 exact Node；Run/Node success 在同一 writer serialization 下检查 open operation、pending/unknown Effect 与 mandatory Evidence。
- 所有权威流都使用完整 scope 与 exact retained reader/reducer/schema；不存在“最新版本”隐式解析。
- 流程遵循保证仅覆盖 Kernel 可观察和控制的状态转移。领域正确性仍取决于 verifier、环境和现实边界，必须通过 limitations 明示。
- REQ-0018 必须用命名负测覆盖删 required node、改 required edge、复制非 repeatable Effect node、放宽 I/O Schema/Evidence/success、扩大 Capability/budget/retry/compensation 及 cross-task binding；全部在 lifecycle sequence 1、lease 与 Effect 前 zero-write 拒绝。
- pre-Manifest Provider/Tool/Workspace/Sandbox 负测必须同时断言 dispatcher、费用计数、外部读写、reservation 与 Event 均为零，并证明 execution Manifest 可追溯到 exact Plan artifact、pure builder、proposer root 与 admission policy。

# Failure modes and security

- 自签 approval 或把成功 Run 当 verified：registry/admission 在 Run 创建前拒绝，零 Effect。
- Plan 偷换 Procedure/Task/Node：exact lineage mismatch，零节点 lease。
- Memory/prompt 注入要求跳步：proposal 被状态机拒绝并记录有界安全事件。
- Provider/Tool 直接访问 OS/network：Sandbox 和 capability boundary 拒绝；发现旁路是 Blocker。
- Evidence 过期、伪造或跨 scope：Evidence Gate fail closed，Node 保持 evidence-pending/failed。
- crash 位于 Node/Effect 边界：从 exact Event/checkpoint 恢复；unknown external outcome 进入 reconciliation，不自动 redispatch。
- procedure default pointer 并发变化：已创建 Run 仍使用 Manifest pin；新选择使用 MVCC compare-and-swap，不修改历史。
- verifier 缺陷导致错误批准：产生新 Procedure/approval revision并回退默认选择；不改写历史，发布 limitation/invalidated evidence。

# Alternatives considered

1. 保持原路线，先实现自由 Agent Loop：短期可演示，但会把模型控制流固化为事实，后补 Evidence/DAG 无法证明无旁路；拒绝。
2. 用 Memory/Markdown 保存成功流程：实现便宜，但非权威文本不能强制依赖、权限、证据、恢复或补偿；拒绝。
3. 只前移 PlanRevision/Task DAG：能表达一次计划，却没有跨 Run 的已验证流程身份、审批和复用合同；不足。
4. 一次实现完整工作流语言、Evidence Graph 与自动演化：能力完整但范围、迁移与安全面过大，难以形成可评审纵切；拒绝。
5. 采用本 RFC 的分层纵切：先固定 Verified Procedure 身份，再依次交付 Plan、Node、最小 Evidence、单 Agent 与早期流程晋升；接受为候选。

# Compatibility, migration, and rollback

路线重排只修改规划和正式设计，不修改当前 Rust、SQLite v2、SchemaSet 或已完成 Requirement 事实。既有 REQ-0001..0033 ID 不变；新增 REQ-0034..0036，不把归档 REQ-0010 设计或实现带入新基线。

RFC-0007/ADR-0008 的 Rust authority 与多语言隔离决策保持有效；其中把 REQ-0018 与 Multi-Agent 同组的路线表仅在顺序和 Epic ownership 上由本 RFC 细化，Plan/DAG 的 Rust authority 不变。已完成 Requirement 中“后续由 REQ-0018/REQ-0014 交付”的历史边界声明继续成立，不把旧文档改写为已实现事实。

RFC-0007/ADR-0008 的 Rust authority 与多语言隔离决策保持有效；其中把 REQ-0018 与 Multi-Agent 同组的路线表仅在顺序和 Epic ownership 上由本 RFC 细化，Plan/DAG 的 Rust authority 不变。已完成 Requirement 中“后续由 REQ-0018/REQ-0014 交付”的历史边界声明继续成立，不把旧文档改写为已实现事实。

未来实现采用 forward-only Schema/Manifest/Event major。旧 Run 不补 Procedure 身份，不宣称符合已验证流程；旧 recorded replay 保持原合同。新 writer 可停用，已写入的新事实保留。若架构评审不批准，本 RFC、SPEC 与路线候选整体撤回，`origin/main@e7a939c` 仍是准确基线。

# Evaluation and acceptance

- Quality：用确定性 Fake Procedure/Plan/Node/Evidence/Effect 证明跳步、伪造证据、跨域、未审批和旁路均 fail closed；清楚区分流程遵循与领域正确性。
- Cost：每次 Run/Node 记录 manifest/event/storage overhead；真实 Provider 费用由 REQ-0010 单独评估，不用综合分掩盖质量回退。
- Latency：分别测量 registry/Run admission、Node transition、Evidence gate 与 replay；不以未验证的百分比作为门禁。
- 设计接受：fresh Reviewer 使用 architecture-review 与 code-review 检查 exact candidate；0 Blocker、0 Major 且 approved 后才能接受 RFC/Spec/路线。
- 实现接受：每个 Requirement 独立完成 impact/spec/plan/tasks、分层测试、schema compatibility、completion gates 与 fresh implementation review，不允许路线批准替代实现批准。
