---
id: ROADMAP-0001
title: Pareto Harness 16 周实施路线图
status: active
owners: [maintainers]
created: 2026-08-20
updated: 2026-08-20
links: [PRD-0001, REQ-0001, BENCH-0001]
---

# 16 周实施路线图

时间是规划基线，里程碑以验收证据而非日期宣告完成。

## M0：工程与设计基线（第 1 周）

交付：独立 Git 仓库、治理文件、Agent/Skill 体系、模板、文档检查、PRD、研究、架构、ADR/RFC、Benchmark 和 Roadmap。

退出条件：REQ-0001 全部满足；新 Agent 无需原始聊天即可说明目标、边界和下一步；无运行时代码空壳。

## M1：事件—版本—重放纵切（第 2–4 周）

交付：Rust workspace；protocol/kernel/sqlite/cli/testkit 的最小实现；Task、Behavior、Workspace Revision；Run Manifest；append-only Event Log；Snapshot/Replay；Fake Model/Tool。

退出条件：

- 幂等追加、顺序、状态迁移和崩溃恢复测试通过。
- 同一录制 Run 重放得到相同投影摘要。
- CLI 可以创建、运行、检查和重放确定性示例。
- Schema、数据库迁移和兼容失败均有测试。

## M2：Task DAG 与 Evidence Loop（第 5–7 周）

交付：Plan Revision、DAG 校验和调度；预算/取消/失败传播；Requirement—Evidence—Verifier—Verdict；测试/构建验证器。

退出条件：循环依赖、节点失败、重试、取消、迟到结果和证据缺失场景通过；Agent 不能在必需证据缺失时成功结束。

## M3：Context DAG 与成本感知路由（第 8–10 周）

交付：Context 来源/派生图、Projection Revision、缓存和失效、Token 预算、模型 Provider 接口、Router、升级/降级策略。

退出条件：每个上下文片段可追溯来源和选择理由；缓存失效正确；在固定质量底线下形成至少一个成本或延迟不劣的候选，而不是预设收益。

## M4：Behavior Revision 与离线演化（第 11–13 周）

交付：策略注册、Behavior 谱系、Evolution Proposal、历史/隐藏集执行、Pareto Archive、MVCC 冲突处理。

退出条件：候选不能原位修改基线；并发 Proposal 冲突显式；过拟合和质量回退候选被门禁拒绝；评测成本完整核算。

## M5：Canary、Rollback 与隔离（第 14–16 周）

交付：Canary 分配、自动停止、原子 Promote/Rollback、审计视图、WASM/进程隔离最小边界、安全负例集。

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
