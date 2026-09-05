---
id: CAP-0001
title: Pareto Harness 核心能力地图
status: proposed
owners: [maintainers]
created: 2026-08-20
updated: 2026-09-05
links: [PRD-0001, REQ-0034, RFC-0001, RFC-0013, ADR-0012]
---

# 核心能力地图

## P0：可信执行基座

| 能力 | 用户价值 | 最小证明 |
|---|---|---|
| Append-only Event Log | 可审计、可投影、可恢复 | 篡改检测、幂等追加、顺序测试 |
| Run Manifest | 精确回答“运行了什么” | 所有行为依赖均被固定或声明外部边界 |
| Revision Registry | Task/Behavior/Workspace 等可追踪 | 父子谱系、内容摘要、兼容校验 |
| Snapshot/Replay | 复现问题和历史评测 | 固定夹具投影一致、非确定性被标记 |
| Capability/Budget | 插件不能越权或无限消耗 | 拒绝路径、超时、取消和预算耗尽测试 |
| Verified Procedure Registry | 只运行证据验证且独立批准的流程版本 | 内容身份、审批包、撤销、替换和 Manifest pin 负测 |

## P1：完成质量

| 能力 | 用户价值 | 最小证明 |
|---|---|---|
| Plan Revision / Task DAG | 为 exact Task 实例化已验证流程 | DAG 校验、Procedure binding、取消、重试和失败传播 |
| Kernel Node Lifecycle | 模型不能跳步或自报节点完成 | 前置依赖、lease、checkpoint、恢复和非法转移测试 |
| Minimal Evidence Gate | 第一版执行器即由证据决定转移 | 缺失、伪造、过期和跨域证据拒绝 |
| Evaluator | 结果可比较 | 版本化 rubric、测试输出和盲评记录 |
| Workspace Revision | 代码变化与行为变化解耦 | Git revision、dirty patch、环境摘要齐全 |

## P1：成本与速度

| 能力 | 用户价值 | 最小证明 |
|---|---|---|
| Context DAG | 只投影当前决策所需上下文 | 来源、依赖、选择理由和 Token 成本可查 |
| Context cache/GC | 避免重复读取和无界增长 | 命中率、失效正确性、质量无回退 |
| Model Router | 简单步骤用便宜模型，难点升级 | 固定任务集上的成本/延迟/质量前沿 |
| Speculative/parallel work | 缩短关键路径 | 取消浪费计入成本，结果保持一致 |

## P2：受控演化

| 能力 | 用户价值 | 最小证明 |
|---|---|---|
| Behavior Revision | Agent 行为可以像代码一样比较 | 配置、策略、Prompt、Skill 和父版本完整 |
| Evolution Proposal | 改进具有假设、风险和验收 | 候选不能直接进入生产 |
| Historical replay | 发现跨任务回归 | 与固定基线同任务、同环境对照 |
| Canary and rollback | 控制上线风险 | 自动停止条件、快速恢复、审计事件 |
| MVCC experiments | 多候选并发而不互相覆盖 | 基线冲突检测、显式 rebase/merge |

## Authority classification

| Information class | Role | May advance authoritative state? |
|---|---|---|
| Conversation memory and user preference | 改善交互与默认建议 | No |
| Project experience and operating guidance | 提供有来源的候选步骤和说明 | No |
| ProcedureRevision | 描述候选流程内容 | No；须被批准包引用 |
| VerifiedProcedureRevision | 固定流程、验证证据和独立审批 | Yes，且只能由 Kernel 准入 |
| PlanRevision / Task DAG | 为 exact Task 实例化流程 | Yes，且必须符合已验证流程 |
| Evidence Record / Gate | 证明节点或 Run 条件 | 只有 Kernel-admitted evidence 可推动状态 |

## Agent/Task 版本控制可挖掘点

传统 Git 只描述工作区文件，Session ID 只描述一次对话。本项目将版本拆为正交维度：

- `TaskRevision`：目标、约束、验收标准和任务输入。
- `ProcedureRevision`：可复用的节点、依赖、能力、证据、恢复与补偿合同。
- `VerifiedProcedureRevision`：exact Procedure 与验证证据、独立审批的不可变批准包。
- `PlanRevision`：为 exact Task 实例化 Verified Procedure 的 Task DAG、参数与预算。
- `BehaviorRevision`：策略、Prompt、Skill、路由和重试配置。
- `ContextProjectionRevision`：本次具体可见上下文，而不只是 Context DAG 本体。
- `WorkspaceRevision`：Git commit、未提交补丁、依赖锁和构建环境。
- `EnvironmentRevision`：操作系统、Sandbox、工具和权限能力。
- `RunManifest`：上述版本与模型/provider 快照的不可歧义组合。

该拆分支持差异归因：代码没变但行为版本变了，或行为没变但上下文投影变了，都能被单独比较。
