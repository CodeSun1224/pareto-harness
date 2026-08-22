---
id: EPIC-0004
title: 受控 Multi-Agent 执行
status: proposed
owners: [maintainers]
created: 2026-08-22
updated: 2026-08-22
links: [PRD-0001, BACKLOG-0001]
---

# Outcome

通过 Task DAG、Lease、结构化消息和隔离工作区安全并行任务，并以单 Agent 基线判断是否值得启用。

# Planned requirements

REQ-0018 至 REQ-0022：Plan/Task DAG、Agent Lease、消息与 Artifact、Worktree 合并、单/多 Agent Router。

# Exit criteria

Agent 不共享可写工作区、不重复提交效果；崩溃任务可重领；无收益任务自动选择单 Agent。
