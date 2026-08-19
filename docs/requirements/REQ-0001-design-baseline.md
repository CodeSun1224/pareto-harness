---
id: REQ-0001
title: 建立可实施的 Pareto Harness 设计基线
status: accepted
owners: [maintainers]
created: 2026-08-20
updated: 2026-08-20
links: [PRD-0001, RFC-0001, ADR-0001, ADR-0002]
---

# Context and user

基础设施工程师和 Coding Agent 必须能在没有原始聊天上下文的情况下理解项目定位并开始实现。

# Problem

讨论中形成了 Task DAG、Context DAG、Model Router、Evidence Loop、Event Log、Agent/Task 版本控制和 Evolution Engine 等方向，但尚未形成边界一致、证据可追溯、可直接实施的工程基线。

# Desired outcome

建立独立 Git 仓库，沉淀产品、研究、架构、评测、路线图和 AI Coding 工作流，并明确机制内核与策略扩展的边界。

# Acceptance criteria

- 新 Agent 能从 `AGENTS.md` 和 `docs/index.md` 找到权威事实及完成门禁。
- 每个关键竞品或论文判断包含直接来源、证据等级、核验日期和成熟度。
- 架构定义可信内核、运行时服务、版本化策略、扩展边界和演化控制面。
- Task、Behavior、Context Projection、Workspace、Environment 和 Run 均有明确版本身份。
- 评测协议分别报告质量、成本和延迟，并定义配对对照与晋升规则。
- Requirement、RFC、ADR、Fix、Postmortem 和临时 Agent Work 的职责不重叠。
- 自动检查能发现缺少元数据、重复正式 ID、断裂的本地 Markdown 链接和遗留占位符。
- 首期不包含无实现、无测试的运行时代码空壳。

# Quality, cost, and latency guardrails

文档深度优先于文件数量；Agent 首次定位任务所需入口不超过三个。文档检查应在普通开发机上数秒内完成且无需第三方 Python 包。

# Non-goals

首期不实现 Runtime，不发布远端仓库，不报告尚未测量的性能收益。

# Risks and open questions

研究领域变化快，证据账本需持续核验。公开发布前仍需完成名称、商标、域名和包名检查。
