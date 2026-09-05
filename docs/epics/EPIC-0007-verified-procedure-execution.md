---
id: EPIC-0007
title: 已验证流程执行与复用
status: proposed
owners: [maintainers, runtime-kernel]
created: 2026-09-05
updated: 2026-09-05
links: [PRD-0001, REQ-0034, RFC-0013, BACKLOG-0001]
---

# Outcome

把已由证据和独立审批支持的成功任务路径表达为不可变、内容寻址的流程版本，并由 Kernel 在单 Agent 执行中强制 Plan/DAG、节点依赖、能力、Evidence、checkpoint、恢复和完成判定。

# Planned requirements

六个纵向 Requirement：REQ-0034 建立 Procedure/Verified Procedure identity 与 Manifest admission；REQ-0018 建立 Task-specific PlanRevision 与基础 DAG；REQ-0035 建立 Kernel Node 状态机与 checkpoint；REQ-0016 建立最小 Evidence Gate；REQ-0014 建立单 Agent 流程执行器；REQ-0036 建立成功流程候选提升、固定复用和流程版本回退。

# Exit criteria

- 确定性 Fake 任务可按 exact Verified Procedure 与 Plan 执行，模型不能跳步、自授能力或自报完成。
- 必需 Evidence 缺失、伪造、过期或跨域时 Node/Run 不能成功。
- 成功候选只有在验证与独立审批后才能进入 retained registry；后续 Run 固定 exact 版本。
- Run recovery、Workspace recovery、Procedure rollback 与 Effect reconciliation/compensation 使用不同合同且不改写历史。
- 单 Agent 流程执行在质量、成本和延迟上形成后续 Multi-Agent 的命名基线。
