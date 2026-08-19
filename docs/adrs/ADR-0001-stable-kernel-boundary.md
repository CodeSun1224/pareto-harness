---
id: ADR-0001
title: 采用稳定可信内核和版本化策略边界
status: accepted
owners: [maintainers]
created: 2026-08-20
updated: 2026-08-20
links: [REQ-0001, RFC-0001]
---

# Context

项目既要快速实验 Planner、Context 和 Router，又要保证事件、权限、版本、证据和晋升不可被实验代码绕过。

# Decision

采纳 RFC-0001。可信内核拥有事件完整性、版本身份、状态机、MVCC、能力、预算、取消、重放、证据准入及 Promote/Rollback。高频变化行为使用不可变 `BehaviorRevision` 组合版本化策略。

# Alternatives

- 全部运行时组件均为同权插件。
- 固定完整 Agent Loop，仅开放工具扩展。
- 只在外部实验平台优化 Prompt，不建立行为版本。

# Consequences

获得清晰安全边界、行为归因和回滚能力；代价是内核接口设计必须谨慎，实验能力不能绕过正式 Proposal 生命周期。

# Revisit triggers

只有当真实实现证明某项内核语义无法跨环境保持，或该语义不再承担安全/一致性责任时重新评审。
