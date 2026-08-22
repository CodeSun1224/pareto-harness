---
id: EPIC-0006
title: Behavior 版本与受控演化
status: proposed
owners: [maintainers]
created: 2026-08-22
updated: 2026-08-22
links: [PRD-0001, RFC-0001, BENCH-0001, BACKLOG-0001]
---

# Outcome

让策略候选经过历史回放、隐藏集、安全门禁和 Canary 后受控晋升，并可以精确回滚。

# Planned requirements

REQ-0028 至 REQ-0033：Behavior 谱系、Evolution Proposal、评测集与 Pareto Archive、MVCC、Canary/Promotion、WASM 策略隔离。

# Exit criteria

候选不能原位修改生产行为；回退、越权、过拟合和成本失控候选被拒绝；历史 Run 始终可解释。
