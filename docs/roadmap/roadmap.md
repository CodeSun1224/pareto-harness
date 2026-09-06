---
id: ROADMAP-0001
title: Pareto Harness 产品实施路线图
status: active
owners: [maintainers]
created: 2026-08-20
updated: 2026-09-06
links: [PRD-0001, REQ-0001, REQ-0034, RFC-0013, ADR-0012, ARCH-0004, BACKLOG-0001, BENCH-0001]
---

# 产品实施路线图

路线围绕三个结果组织：把成功执行沉淀为可复用的不可变 Verified Procedure；为同类任务探索并保留质量、Token/费用、延迟上的 Pareto 最优策略；提供可审计的 replay、reexecute、simulation、恢复、回退与对账。里程碑按可运行纵向能力验收，不按 crate、语言或日期验收。

## M0：Stable Kernel Baseline

状态：完成并冻结。

范围：REQ-0003 至 REQ-0009。协议、Event Store、Run/Task lifecycle、Manifest、projection/snapshot/recorded replay、runtime control、hooks 与 Effect Intent/Receipt 构成上层产品的可信基础。

退出条件：既有独立评审与回归证据继续成立；上层开发不绕过或重写基线 authority。

## M1：Fake Verified Agent

目标：先用完全确定性的 Fake 边界跑通首个可复用成功闭环。

范围：Verified Procedure identity/admission、closed Plan、Node lifecycle、最小 Evidence Gate，以及受治理的 Fake Provider、Tool、Workspace 和单 Agent executor。成功 Run 可生成候选流程，但 distillation 不是复制日志或聊天轨迹：必须移除秘密、临时 ID 和偶然步骤，区分必要节点、可选分支与失败/恢复分支，完成参数化、适用范围定义和反例验证，再经外部验证与独立批准成为不可变版本。

退出条件：Task → Verified Procedure → Plan → Node → Agent → Evidence → Success 端到端通过；候选流程不包含秘密或运行偶然量，并能用正例/反例证明适用边界；跳步、越权、缺证据完成、版本替换和未结 Effect 被拒绝；旧流程版本仍可选择。

## M2：Real Coding Agent

目标：在真实 Git 仓库完成小型修复和功能任务。

范围：OpenAI-compatible Provider、Search/Read/Patch/Shell/Test 工具、Workspace Revision、Sandbox、非权威 Memory 与 `run/resume/inspect` CLI。所有外部操作继续走 Capability、Budget 和 Effect 边界。

退出条件：真实任务结果有可核验证据；秘密、网络、路径、费用和 Workspace 隔离可审计；模型或工具不能自行推动节点或 Run 成功。

## M3：Reliable Runtime

目标：让失败可诊断、可恢复、可重放，而不混淆不同语义。

范围：crash resume、checkpoint、late result、retry、recorded replay、reexecute、simulated fixtures、workspace recovery、Effect reconciliation/compensation，以及 procedure/behavior selection rollback。

退出条件：每种机制有独立命令、事件谱系和测试：Run recovery 延续同一 Manifest；reexecute 创建新 Run；simulation 固定 fixture；Workspace recovery 产生明确 revision；Effect reconciliation/compensation 不冒充回滚；Procedure/Behavior rollback 只影响后续选择。

## M4：Multi-Agent / Context

目标：在不削弱单 Agent 正确性的前提下扩展并行执行与上下文效率。

范围：Agent lease/heartbeat、结构化消息、worktree 隔离与合并、单/多 Agent Router、Context DAG、cache/GC 与完整 Evidence provenance。

退出条件：无共享可写 Workspace、重复效果或迟到结果越权；只有基准证明质量底线不下降且成本或延迟改善时才启用多 Agent/Context 优化。

## M5：Self-Evolution / Pareto

目标：对同一 TaskClass 系统探索替代 Procedure/Behavior 策略并保留 Pareto frontier。

范围：Behavior lineage、Evolution Proposal、候选隔离、历史/隐藏/安全评测、Pareto Archive、MVCC、Canary、Promote 与 Rollback。

退出条件：质量、Token/费用、延迟分别报告；被支配策略可淘汰但历史 Run 仍可解释；候选不能自评分或自晋升；Canary 可自动停止并恢复到指定版本。

## 使用规则

- [Capability Backlog](requirement-backlog.md) 保留既有 REQ ID，作为候选能力清单而非 35 项全局硬 DAG。
- 激活一个里程碑时，只冻结支撑下一可运行纵切所需的 Requirement、依赖和测试。
- Python、TypeScript、WASI、PostgreSQL、远程 Worker 和 RPC 均由证据触发，不是路线承诺。
- 当前完成度只更新[状态页](../status.md)，架构文档保持长期稳定。
