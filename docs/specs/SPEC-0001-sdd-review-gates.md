---
id: SPEC-0001
title: SDD、影响分析和独立评审规范
status: approved
owners: [maintainers]
created: 2026-08-22
updated: 2026-08-22
links: [REQ-0002, EPIC-0001]
---

# Behavioral contract

非轻量变更必须先有 Requirement 和 Spec，再进入实现。实现完成后必须经过分层测试和独立 Review；只有验收项全部有证据且无未关闭 Blocker/Major 时，Requirement 才能进入 `verified` 或 `done`。

# Risk classification

- `lightweight`：只修改拼写、注释、链接或无行为配置；需要影响说明和基础检查，不强制独立 Review 文档。
- `standard`：任何 Runtime 行为、测试、工具、公共文档结构或自动化变化；需要完整 SDD 和独立 Review。
- `high`：权限、Sandbox、数据隔离、Event/Schema/持久化、并发、Replay、秘密或晋升；额外触发专项 Review 和负向测试。

# Impact analysis

| Dimension | Finding | Evidence / response |
|---|---|---|
| Direct | AGENTS、模板、Skills、Roadmap、PR 和文档校验脚本 | 变更均在仓库治理层，无 Runtime 代码 |
| Indirect | 新会话工作流、未来 Requirement 状态及 CI | 保留现有 ID 和固定路径，新增规则向后兼容现有文档 |
| Call/permission | 文档检查读取仓库 Markdown，不需要网络或写权限 | 使用 Python 标准库，只读扫描工作树 |
| Data isolation | 不处理用户/租户数据；Reviewer 只接收 Spec、Diff 和测试证据 | Agent Profile 禁止读取实现者主观结论作为评审依据 |
| API/schema | 扩展文档 frontmatter 状态和记录类型 | 现有文档状态继续合法；模板占位符不参与正式文档校验 |
| Persistence/replay | 不修改 Runtime Event 或数据库 | 无迁移；Git 历史保留旧治理基线 |
| Concurrency | 多 Agent 可能同时更新同一工作记录 | 一个 Requirement 工作目录只允许一个 active owner；Review 只读 |
| Security | Review 必须覆盖权限、隔离和无关修改 | 高风险变更触发专项检查 |
| Performance | 增加扫描和链接追踪 | 只扫描文本文件，目标少于 5 秒 |
| Rollback | 单一 Git commit 回退 | 不修改既有正式 ID 和历史文件路径 |

# Document relationships

`EPIC → REQ → SPEC → optional RFC/ADR → PLAN/TASKS → REVIEW → verified`。正式记录使用稳定 ID；执行记录位于 `.agents/work/active/REQ-####-*`，完成后整体归档。

# State model

Requirement 状态：`proposed → impact-analyzed → specified → approved → planned → implementing → reviewing → verified → done`，另有 `rejected` 和 `blocked`。允许在评审或验证失败时退回 `implementing`。

Spec 状态：`draft → approved → superseded`。Review 状态：`open → changes-requested → approved`。

# Test traceability

| Acceptance | Layer | Scenario | Evidence |
|---|---|---|---|
| AC-01..AC-04 | Focused | 人工检查常驻规则和模板职责 | Reviewer 对照 Requirement/Spec |
| AC-05..AC-06 | Focused | 对五个 Skill 运行官方 validator，检查 Agent Profile | 命令输出和 Review |
| AC-07 | Unit/component | 对真实仓库运行检查；临时夹具覆盖非法状态、重复 ID、断链和缺失 Review | `scripts/check_docs.py` 自测试模式 |
| AC-08..AC-09 | Impacted | 检查 PR 模板、Roadmap、Epic 和 Backlog 链接 | 文档检查和 Review |
| AC-10 | Core | 全部 Skill 校验、文档检查、`git diff --check`、独立 Review | 工作目录中的验证记录 |

# Failure and rollback

检查器必须输出可定位的文件和原因，不自动修改文件。误报时先修正规则或显式豁免，不允许删除门禁绕过。整个需求可以通过 Git revert 回退。
