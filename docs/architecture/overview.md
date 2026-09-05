---
id: ARCH-0001
title: Pareto Harness 总体架构
status: accepted
owners: [maintainers]
created: 2026-08-20
updated: 2026-09-05
links: [PRD-0001, REQ-0034, RFC-0001, RFC-0007, RFC-0008, RFC-0009, RFC-0013, ADR-0001, ADR-0002, ADR-0008, ADR-0009, ADR-0010]
---

# 总体架构

## 架构目标

将一次 Agent 运行变成可版本化、可观测、可验证、可比较和可恢复的受控计算。架构使用模块化单体起步，但通过稳定的数据契约和 capability 边界避免未来拆分时重写语义。

## 四层模型

```text
┌──────────────────────────────────────────────────────┐
│ Evolution Control Plane                              │
│ propose → sandbox → replay → evaluate → canary       │
│                         → promote / rollback          │
├──────────────────────────────────────────────────────┤
│ Versioned Strategies                                 │
│ planner | context | router | tools | retry | eval     │
├──────────────────────────────────────────────────────┤
│ Runtime Services                                     │
│ context | workspace | projections | registries       │
│ scheduler | provider/tool adapters                    │
├──────────────────────────────────────────────────────┤
│ Trusted Kernel                                       │
│ event | procedure/plan/node state | version identity  │
│ capability | budget | evidence | replay | promotion   │
└──────────────────────────────────────────────────────┘
```

第三方工具、MCP、模型适配器和生成候选位于边界外，只能通过 capability 接口交互。图中的层是责任与信任边界，不是语言或进程边界：Rust 可信控制面提交权威裁决；经具体 Requirement 批准的外部或多语言组件只能提出请求、执行已授权操作或返回非权威结果。

## 运行数据流

1. Intake 将用户输入和验收条件固化为 `TaskRevision`。
2. Kernel 从 retained registry 准入 exact `VerifiedProcedureRevision`；Planner 只能提出绑定该流程和 Task 的 `PlanRevision`。
3. Kernel 校验 Plan/Task DAG 后创建固定 Procedure、Plan 与全部行为依赖的 `RunManifest`。
4. Kernel 节点状态机只把依赖已满足的节点置为 ready，并签发用途受限 Node lease。
5. Scheduler/Router/Agent 只能在当前节点内提出 Context、Provider 或 Tool 请求；执行只能通过 Node-bound Capability 与 Effect 边界。
6. Provider、Tool、Workspace 和 Sandbox 返回非权威 observation；Kernel admission 记录 Receipt、usage、artifact 和失败事实。
7. 最小 Evidence Gate 将测试、静态分析、构建或人工批准关联到 exact requirement/node/subject/verifier/freshness。
8. 只有必需 Evidence 满足且无未结 Effect/operation 时，Kernel 才提交 Node 或 Run success；否则失败、暂停、恢复或对账。
9. 投影器从 Event Log 构建 Procedure、Plan、Node、Context、Evidence、Cost 和 Timeline 视图。

## 演化数据流

流程版本和行为版本是正交演化轴。REQ-0036 先交付最小流程候选提升、复用和默认流程回退；REQ-0028 至 REQ-0032 后续交付 Planner、Router、Memory 等 `BehaviorRevision` 的历史评测、Canary 与 Promote/Rollback。

1. Observer 从失败簇、成本热点和轨迹差异形成带假设的 `EvolutionProposal`。
2. Proposal 以目标 `BehaviorRevision` 为 MVCC 基线生成候选，不能原位修改已发布行为。
3. 候选在隔离环境中运行固定历史集、隐藏集和针对性反例。
4. Evaluator 产出质量、成本、延迟向量和证据，不接受候选自报分数。
5. Candidate 必须满足质量底线且在至少一个目标维度产生有统计意义的改进，才可进入 Canary。
6. Kernel 按明确流量和停止条件执行 Canary，随后原子 Promote 或 Rollback。

## 模块边界

- `protocol`：版本化公共类型和 JSON Schema；不依赖运行时实现。
- `kernel`：事件、状态机、版本、能力、预算、重放和晋升。
- `runtime`：Context/Workspace 服务、调度与不具权威性的执行协调。
- `strategies`：内置策略实现；只能依赖公开接口。
- `adapters`：模型、工具、MCP、存储和 Sandbox 适配。
- `control-plane`：实验、评测、Canary 和版本管理。
- `cli`：首个操作入口，不承载业务规则。

这些是逻辑模块，不要求全部使用 Rust 或处于同一进程。Event、Procedure/Plan/Node identity 与 state、Capability、Budget、Cancellation、Effect/Evidence admission、Replay、Lease/MVCC 和 Promotion 保持在 Rust 权威控制面；Provider、Tool、Hook handler、Agent Worker、Memory 检索、评测、SDK 和受限 Guest 可按 Requirement 使用其他语言，但不得取得权威数据库或内核私有对象。设计基线阶段不创建空目录。

Hook 采用 ADR-0009 的 Kernel 治理合同，并已由REQ-0008交付最小Rust Fake纵切：Manifest固定registry/config和顺序，Observer只读，Gate默认拒绝，Transform只改明确允许的非权威proposal，预算准入与终结通过双stream原子pair提交，取消/timeout由Runtime Control裁决，Recorded replay只消费已记录决定。该实现不包含真实Hook Runtime、外部transport或Worker。

Effect采用ADR-0010合同，并已由REQ-0009交付最小Rust Fake纵切：Kernel以control/Effect atomic pair先提交Intent，再持久化dispatch claim并向Manifest-pinned sealed Fake executor交付用途受限lease；Receipt保持observation，partial/unknown经Kernel recovery authority与Manifest-pinned reconciliation producer追加事实；Projection与Boundary Inventory V2固定exact horizon，Recorded replay零执行/零写入/零核算。该实现明确提供at-most-once-or-reconcile而非虚构exactly-once，保持SQLite v2，且不包含真实文件、进程、网络、Provider、Tool或Sandbox效果。

## 一致性原则

- Event Log 是运行事实源；图和统计视图是可重建投影。
- Git 是工作区内容来源之一，不等同于 Agent Behavior 版本。
- 事件追加成功和外部效果成功不能假装成同一事务；采用 Effect Intent/Receipt 和幂等键处理边界。
- 外部模型输出、时钟、网络和工具环境是非确定性源，Replay 必须选择录制结果或重新执行并标记差异。
- 删除策略以墓碑和保留期实现；已被 Run Manifest 引用的版本不可物理删除。

## 当前实现与下一纵向路径

截至可信基线 `e7a939c`，REQ-0003 至 REQ-0009 已实现版本、Event Store、Run/Task lifecycle、Recorded replay、Runtime Control、Fake Hook 与 Fake Effect；Procedure、Plan/DAG、Node lifecycle 和执行期 Evidence Gate 尚未实现。

下一路径按 Provider → Coding Tools → Workspace → Sandbox → Verified Procedure identity → Plan/DAG → Node state machine → minimal Evidence Gate → single-Agent procedure executor → procedure promotion/reuse 推进。每个阶段必须产生可运行的确定性纵向切片，不能用路线目标冒充已实现事实。
