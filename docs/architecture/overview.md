---
id: ARCH-0001
title: Pareto Harness 总体架构
status: accepted
owners: [maintainers]
created: 2026-08-20
updated: 2026-08-20
links: [PRD-0001, RFC-0001, ADR-0001, ADR-0002]
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
│ Task DAG | Context DAG | Evidence | workspace        │
│ projections | model/tool registry | scheduler        │
├──────────────────────────────────────────────────────┤
│ Trusted Kernel                                       │
│ event | revisions | state machine | MVCC | capability │
│ budget | cancellation | snapshot/replay | promotion  │
└──────────────────────────────────────────────────────┘
```

第三方工具、MCP、模型适配器和生成候选位于边界外，只能通过 capability 接口交互。

## 运行数据流

1. Intake 将用户输入和验收条件固化为 `TaskRevision`。
2. Kernel 创建固定所有行为依赖的 `RunManifest` 并追加 `RunStarted`。
3. Planner 产生 `PlanRevision`；Task Graph 校验无环、依赖和预算。
4. Scheduler 请求 Context Projector 为下一节点生成 `ContextProjectionRevision`。
5. Router 根据节点、证据缺口、预算、失败史和模型能力选择模型。
6. 执行只能通过 Tool Capability 提交效果；每次请求、授权、结果和费用均记录事件。
7. Evidence Service 将测试、静态分析、构建、人工确认或 Judge 结果关联到 Requirement。
8. Completion Gate 仅在必需证据满足时产生 `RunSucceeded`，否则失败、暂停或请求补充。
9. 投影器从 Event Log 构建 Task、Context、Evidence、Cost 和 Timeline 视图。

## 演化数据流

1. Observer 从失败簇、成本热点和轨迹差异形成带假设的 `EvolutionProposal`。
2. Proposal 以目标 `BehaviorRevision` 为 MVCC 基线生成候选，不能原位修改已发布行为。
3. 候选在隔离环境中运行固定历史集、隐藏集和针对性反例。
4. Evaluator 产出质量、成本、延迟向量和证据，不接受候选自报分数。
5. Candidate 必须满足质量底线且在至少一个目标维度产生有统计意义的改进，才可进入 Canary。
6. Kernel 按明确流量和停止条件执行 Canary，随后原子 Promote 或 Rollback。

## 模块边界

- `protocol`：版本化公共类型和 JSON Schema；不依赖运行时实现。
- `kernel`：事件、状态机、版本、能力、预算、重放和晋升。
- `runtime`：Task/Context/Evidence 服务和调度。
- `strategies`：内置策略实现；只能依赖公开接口。
- `adapters`：模型、工具、MCP、存储和 Sandbox 适配。
- `control-plane`：实验、评测、Canary 和版本管理。
- `cli`：首个操作入口，不承载业务规则。

这些是未来代码模块，不在设计基线阶段创建空目录。

## 一致性原则

- Event Log 是运行事实源；图和统计视图是可重建投影。
- Git 是工作区内容来源之一，不等同于 Agent Behavior 版本。
- 事件追加成功和外部效果成功不能假装成同一事务；采用 Effect Intent/Receipt 和幂等键处理边界。
- 外部模型输出、时钟、网络和工具环境是非确定性源，Replay 必须选择录制结果或重新执行并标记差异。
- 删除策略以墓碑和保留期实现；已被 Run Manifest 引用的版本不可物理删除。

## 首个纵向切片

设计基线完成后实现：CLI 创建 Task/Behavior/Workspace Revision → 生成 Run Manifest → SQLite 追加事件 → 构建投影 → Snapshot → Replay → 输出证据摘要。首个切片使用确定性 Fake Model/Tool，不接真实模型，以先证明内核语义。
