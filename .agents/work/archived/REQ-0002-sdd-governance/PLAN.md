---
title: REQ-0002 实施计划
status: completed
owner: primary-agent
updated: 2026-08-22
links: [REQ-0002, SPEC-0001, REVIEW-0001]
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
- Impacted：既有正式文档继续通过检查。
- Core：所有项目 Skills、断链、ID、Review、Validation、Task 和 whitespace 检查。
- Full：不适用；仓库尚无 Runtime。

# Validation commands

```text
python -m py_compile scripts/check_docs.py scripts/tests/test_check_docs.py
python -m unittest discover -s scripts/tests -p "test_*.py"
python scripts/check_docs.py
python C:/Users/codes/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/sdd-delivery
python C:/Users/codes/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/impact-analysis
python C:/Users/codes/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/test-planning
python C:/Users/codes/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/code-review
python C:/Users/codes/.codex/skills/.system/skill-creator/scripts/quick_validate.py .agents/skills/requirement-decomposition
git diff --check
git status --short
```

# Rollback

治理变更由连续、可定位的 Git 提交组成；可按逆序 revert，不影响 Runtime 数据。
