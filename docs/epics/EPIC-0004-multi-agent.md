---
id: EPIC-0004
title: 受控 Multi-Agent 执行
status: proposed
owners: [maintainers]
created: 2026-08-22
updated: 2026-09-05
links: [PRD-0001, REQ-0034, RFC-0013, BACKLOG-0001]
---

# Outcome

在复用 G2 已交付的 Plan/Task DAG、Node 状态机和 Evidence Gate 前提下，通过 Agent Lease、结构化消息和隔离工作区安全并行节点，并以单 Agent 基线判断是否值得启用。

# Planned requirements

REQ-0019 至 REQ-0022：Agent Lease、消息与 Artifact、Worktree 合并、单/多 Agent Router。REQ-0018 已前移至 EPIC-0007，Multi-Agent 不再拥有基础 Plan/DAG authority。

# Exit criteria

Agent 不共享可写工作区、不重复提交效果；崩溃任务可重领；无收益任务自动选择单 Agent。
