---
id: ROADMAP-0001
title: Pareto Harness Evidence-gated 实施路线图
status: active
owners: [maintainers]
created: 2026-08-20
updated: 2026-08-20
links: [PRD-0001, REQ-0001, BENCH-0001]
---

# Evidence-gated 实施路线图

以一名主要开发者配合 AI Coding 估算为 16–20 周。时间是规划基线，阶段只以验收证据而非日期宣告完成。逐项顺序和稳定编号见 [Requirement Backlog](requirement-backlog.md)。

## G0：工程、设计与 SDD 基线

交付：独立 Git 仓库、治理文件、Agent/Skill 体系、模板、文档检查、PRD、研究、架构、ADR/RFC、Benchmark，以及 SDD、影响分析、分层测试和独立 Review 门禁。

退出条件：REQ-0001 和 REQ-0002 满足；新 Agent 无需原始聊天即可完成 Requirement → Spec → Plan → Tasks → Review → Verified；无运行时代码空壳。

## G1：可信内核骨架

交付：REQ-0003 至 REQ-0009，包括协议/Revision、Event Store、Run Manifest、Snapshot/Replay、Capability/Budget、Hook 骨架和 Effect Intent/Receipt。

退出条件：

- 幂等追加、顺序、状态迁移和崩溃恢复测试通过。
- 同一录制 Run 重放得到相同投影摘要。
- CLI 可以创建、运行、检查和重放确定性示例。
- Schema、数据库迁移和兼容失败均有测试。

## G2：稳定单 Agent Coding CLI

交付：REQ-0010 至 REQ-0017，包括 OpenAI 兼容 Provider、Coding Tools、Workspace/Sandbox、单 Agent Loop、Memory 基线、Evidence Gate 和 CLI。

退出条件：能在真实仓库完成缺陷修复和小型功能；失败可恢复；必需证据缺失不能成功；权限、隔离和费用可审计。

## G3：受控 Multi-Agent

交付：REQ-0018 至 REQ-0022，包括基础 Task DAG、Agent Lease、结构化消息、Worktree 合并和单/多 Agent Router。

退出条件：Agent 不共享可写工作区、不重复提交效果；崩溃可重领；无收益任务默认单 Agent。

## G4：技术亮点增强

交付：REQ-0023 至 REQ-0027，包括自适应 Task DAG、Context DAG、Context Cache/GC、完整 Evidence Graph 和成本感知 Router。

退出条件：每项优化分别完成消融；只在质量底线成立且至少改善一个目标维度时进入默认 Behavior。

## G5：受控演化

交付：REQ-0028 至 REQ-0033，包括 Behavior 谱系、Evolution Proposal、历史/隐藏评测、MVCC、Canary/Promote/Rollback 和 WASM 隔离。

退出条件：越权和资源超限被拒绝；Canary 触发停止后恢复到指定 Behavior；历史 Run 仍可由原版本解释；发布首份可复现实验报告。

## v0.2 以后

- TypeScript SDK 和 Web 控制台。
- Python 研究/评测 Worker。
- MCP 和更多模型 Provider 适配。
- PostgreSQL、多节点执行和远程 Sandbox。
- 团队权限、签名策略、数据保留和合规能力。

## 项目治理节奏

- 每周：风险、证据缺口、架构债务和指标回顾。
- 每个里程碑：Requirement 验收、RFC/ADR 同步、Benchmark 报告和回滚演练。
- 每月：竞品/论文证据账本重新核验，过期结论标记而非静默保留。
- 每次重大失败：Fix；若已逃逸或暴露系统性缺口，则追加 Postmortem。
