---
id: EPIC-0003
title: 受治理 Coding 执行边界与 CLI
status: proposed
owners: [maintainers]
created: 2026-08-22
updated: 2026-09-05
links: [PRD-0001, EPIC-0007, REQ-0034, RFC-0013, ADR-0002, BACKLOG-0001]
---

# Outcome

为 EPIC-0007 提供受治理的 Provider、Coding Tools、Workspace 与 Sandbox 边界，并在已验证流程执行闭环后提供非权威 Memory 与 `run/resume/inspect/replay` CLI。

# Planned requirements

REQ-0010 至 REQ-0013 交付受治理 Provider/Tools/Workspace/Sandbox；REQ-0015 在 EPIC-0007 流程执行器之后交付非权威 Memory；REQ-0017 最后交付 CLI。已验证流程本身由 EPIC-0007 的六个 Requirement 交付。

REQ-0010 的定位是为流程执行器提供 Kernel-governed model invocation；REQ-0015 不承担流程保证；CLI 不承载 Procedure、Evidence、权限或终态业务规则。

# Exit criteria

四类真实执行边界都只能消费 Node-bound lease；Memory 不能推动权威状态；CLI 可操作 EPIC-0007 已交付的 exact Verified Procedure Run，且权限、费用、恢复和对账均可审计。
