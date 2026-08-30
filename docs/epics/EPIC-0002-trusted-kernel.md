---
id: EPIC-0002
title: 可信内核骨架
status: active
owners: [maintainers]
created: 2026-08-22
updated: 2026-08-30
links: [PRD-0001, RFC-0001, RFC-0002, RFC-0003, RFC-0004, RFC-0005, RFC-0006, RFC-0008, RFC-0009, ADR-0001, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ADR-0007, ADR-0009, ADR-0010, BACKLOG-0001, REQ-0003, REQ-0004, REQ-0005, REQ-0006, REQ-0007, REQ-0008, REQ-0009, SPEC-0002, SPEC-0003, SPEC-0004, SPEC-0005, SPEC-0006, SPEC-0007, SPEC-0008, REVIEW-0004, REVIEW-0005, REVIEW-0006, REVIEW-0007, REVIEW-0010, REVIEW-0011, REVIEW-0012]
---

# Outcome

建立可审计、可恢复、可重放且不能被扩展绕过的最小运行内核。

# Planned requirements

REQ-0003 至 REQ-0009：协议与 Revision、Event Store、状态机与 Run Manifest、Snapshot/Replay、Capability/Budget、Hook 骨架、Effect Intent/Receipt。详细顺序见 Requirement Backlog。

已交付 REQ-0003 协议基础、REQ-0004 SQLite append-only Event Store、REQ-0005 Run/Task 状态机及完整 Run Manifest、REQ-0006 版本化 Projection/同库 Snapshot/Recorded replay、REQ-0007 默认拒绝 Runtime Control，以及 REQ-0008 Kernel治理的Observer/Gate/Transform Fake Hook纵切。REQ-0009 Effect Intent/Receipt 已完成影响分析、Spec/RFC与独立设计评审并获批准，尚未规划或实现；真实外部Hook Runtime、Provider/Agent Loop、Memory和DAG仍未实现。

# Exit criteria

Fake Model/Tool 可以完成确定性 Run；崩溃后恢复；Recorded Replay 产生相同投影；越权效果被拒绝并留下事件。
