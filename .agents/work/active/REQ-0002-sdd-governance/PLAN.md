---
title: REQ-0002 实施计划
status: active
owner: primary-agent
updated: 2026-08-22
links: [REQ-0002, SPEC-0001]
---

# Goal and acceptance

完成 REQ-0002 全部验收项，并用本需求自举验证新的 SDD 流程。

# Implementation order

1. 建立 Epic、Requirement 和 Spec。
2. 更新 AGENTS、贡献说明、模板和 Reviewer Profile。
3. 建立五个 Skills 并校验。
4. 扩展文档检查和 CI/PR 门禁。
5. 将 Roadmap 拆成 Epic 和 Requirement Backlog。
6. 运行测试，生成独立 Review，关闭 finding 后归档工作目录。

# Regression selection

- Focused：新增状态、模板、Skill 和检查器自测。
- Impacted：现有 38 个 Markdown 文档继续通过检查。
- Core：所有项目 Skills、断链、ID 和 whitespace 检查。
- Full：不适用；仓库尚无 Runtime。

# Rollback

本需求作为单一治理基线提交；可通过 Git revert 回退，不影响 Runtime 数据。
