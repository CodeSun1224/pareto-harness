---
id: REQ-0002
title: 建立 SDD、影响分析和独立评审门禁
status: done
owners: [maintainers]
created: 2026-08-22
updated: 2026-08-22
links: [EPIC-0001, SPEC-0001, REVIEW-0001]
risk: standard
work: .agents/work/archived/REQ-0002-sdd-governance
---

# Context and user

项目需要在 AI Coding 和新会话中稳定继承工程原则，而不能依赖历史聊天或某个 Agent 的临时记忆。

# Problem

现有仓库已有 Requirement、RFC、ADR 和若干 Skills，但没有强制的影响分析、测试追踪、实施任务和独立 Code Review。Roadmap 也尚未拆为可逐个验收的 Requirement。

# Desired outcome

建立由常驻规则、可复用 Skills、正式模板和自动检查共同约束的 SDD 流程，并以本需求完成一次自举演练。

# Acceptance criteria

- AC-01：`AGENTS.md` 定义 SDD 生命周期、风险分级、独立 Review 和分层测试门禁。
- AC-02：存在 Epic、Spec、Review、Plan 和 Tasks 模板，职责及链接方向明确。
- AC-03：Spec 强制分析直接/间接影响、调用权限、数据隔离、兼容、并发、持久化、性能和回滚。
- AC-04：每项验收条件可以映射到 Focused、Impacted、Core 或 Full 测试。
- AC-05：存在 SDD 编排、影响分析、测试计划、独立 Review 和 Requirement 拆解 Skills，并全部通过 Skill 校验。
- AC-06：存在独立 Reviewer Agent Profile；Review Finding 有 Blocker、Major、Minor、Note 等级和关闭规则。
- AC-07：文档检查能校验新增正式目录、合法状态、链接 ID、Spec/Review 追踪和完成态门禁。
- AC-08：PR 模板要求影响、隔离、兼容、权限、回归、无关修改和 Review 结论。
- AC-09：Roadmap 被拆为按依赖排序的 Epic 和 Requirement Backlog。
- AC-10：REQ-0002 留下 Plan、Tasks、测试证据和独立 Review，并通过所有仓库检查。

# Quality, cost, and latency guardrails

常驻 `AGENTS.md` 只保存不可跳过的原则；详细步骤放入 Skill 和模板。文档检查只使用 Python 标准库并在普通开发机数秒内完成。

# Non-goals

- 本需求不实现 Runtime。
- 不要求机械文档/拼写修改创建完整 Spec。
- 不为每个 Task 创建单独文件。
- 不引入外部项目管理系统。

# Risks and open questions

治理过重会拖慢小改动，因此设置 lightweight、standard、high 三档风险流程；所有 Runtime 行为变更最低为 standard。
