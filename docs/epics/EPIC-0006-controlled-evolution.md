---
id: EPIC-0006
title: Behavior 版本与受控演化
status: proposed
owners: [maintainers]
created: 2026-08-22
updated: 2026-09-05
links: [PRD-0001, REQ-0034, RFC-0001, RFC-0013, BENCH-0001, BACKLOG-0001]
---

# Outcome

让 Planner、Router、Memory 等 Behavior 策略候选经过历史回放、隐藏集、安全门禁和 Canary 后受控晋升，并可以精确回退后续 Run 的默认 Behavior。该轴不替代 REQ-0036 的流程版本晋升，也不承担 Run/Workspace 恢复或外部 Effect 补偿。

# Planned requirements

REQ-0028 至 REQ-0033：Behavior 谱系、Evolution Proposal、评测集与 Pareto Archive、MVCC、Canary/Promotion、WASM 策略隔离。

# Exit criteria

候选不能原位修改生产行为；回退、越权、过拟合和成本失控候选被拒绝；历史 Run 始终可解释。
