---
id: EPIC-0001
title: 可持续的 SDD 工程基础
status: active
owners: [maintainers]
created: 2026-08-22
updated: 2026-08-22
links: [REQ-0002]
---

# Outcome

让人类和 Coding Agent 在跨会话开发时都遵循可审计的 Spec-driven delivery：编码前分析影响，编码后运行分层测试和独立 Review，最终以证据关闭需求。

# Requirements

- [REQ-0002：建立 SDD、影响分析和独立评审门禁](../requirements/REQ-0002-sdd-review-gates.md)

# Exit criteria

- 新会话仅依靠仓库文件即可执行完整 SDD 流程。
- 文档和 CI 能阻止缺少 Spec、测试追踪或 Review 的需求被标记为完成。
- 至少一个需求完成 Requirement → Spec → Plan → Tasks → Review → Verified 演练。
