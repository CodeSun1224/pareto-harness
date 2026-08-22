---
id: EPIC-0003
title: 稳定单 Agent Coding CLI
status: proposed
owners: [maintainers]
created: 2026-08-22
updated: 2026-08-22
links: [PRD-0001, ADR-0002, BACKLOG-0001]
---

# Outcome

在真实 Git 仓库中完成可验证的缺陷修复和小型功能实现，并具有权限、隔离、恢复和成本观测。

# Planned requirements

REQ-0010 至 REQ-0017：OpenAI 兼容 Provider、Coding Tools、Workspace、Sandbox、单 Agent Loop、Memory 基线、Evidence Gate、CLI 恢复与检查。

# Exit criteria

真实仓库任务可通过 `run/resume/inspect/replay` 完成；没有必需证据不能成功结束；权限和费用均可审计。
