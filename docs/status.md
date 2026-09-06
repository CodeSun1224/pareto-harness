# 项目状态

更新时间：2026-09-06

## Stable Kernel Baseline

REQ-0003 至 REQ-0009 已实现并通过验证，现冻结为 Stable Kernel Baseline：

- 版本化协议、Serde 类型与 JSON Schema。
- SQLite/sqlx append-only Event Store。
- Run/Task lifecycle 与 Run Manifest。
- Projection、Snapshot 与 recorded replay。
- Capability、Budget、Cancellation 与 Timeout。
- Kernel-governed Hook。
- Effect Intent/Receipt、幂等 claim、恢复与 reconciliation。

上层里程碑可使用这些能力，但不得为路线重排或治理瘦身改写其运行时语义。只有明确的内核合同变更或已证明的不变量缺陷可以重新打开该基线。

## 当前产品边界

可信内核已经能记录、约束和重放基础运行事实，但尚未形成“成功任务沉淀并复用”的完整产品闭环。Verified Procedure、Plan/Node 执行、最小 Evidence Gate、可用 Coding Agent、真实 Workspace/Sandbox、reexecute/simulated 模式和 Pareto 策略保留仍是上层能力。

## 下一里程碑

下一里程碑是 **Fake Verified Agent**：用确定性 Fake Provider/Tool/Workspace 跑通 Task → Verified Procedure → Plan → Node → Agent → Evidence → Success，并证明跳步、越权、缺证据完成和未结 Effect 均被拒绝。

路线和退出条件见[主路线图](roadmap/roadmap.md)。编号保留与候选能力见[Capability Backlog](roadmap/requirement-backlog.md)。本页只记录状态，不冻结新的架构合同。
