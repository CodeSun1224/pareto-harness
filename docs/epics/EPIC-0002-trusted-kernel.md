---
id: EPIC-0002
title: 可信内核骨架
status: active
owners: [maintainers]
created: 2026-08-22
updated: 2026-08-24
links: [PRD-0001, RFC-0001, RFC-0002, RFC-0003, ADR-0001, ADR-0003, ADR-0004, BACKLOG-0001, REQ-0003, REQ-0004, SPEC-0002, SPEC-0003]
---

# Outcome

建立可审计、可恢复、可重放且不能被扩展绕过的最小运行内核。

# Planned requirements

REQ-0003 至 REQ-0009：协议与 Revision、Event Store、状态机与 Run Manifest、Snapshot/Replay、Capability/Budget、Hook 骨架、Effect Intent/Receipt。详细顺序见 Requirement Backlog。

已交付 REQ-0003 协议基础与 REQ-0004 SQLite append-only Event Store；REQ-0005 起的状态机、Projection/Replay 和权限能力仍未实现。

# Exit criteria

Fake Model/Tool 可以完成确定性 Run；崩溃后恢复；Recorded Replay 产生相同投影；越权效果被拒绝并留下事件。
