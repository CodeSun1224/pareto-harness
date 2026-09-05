---
id: ROADMAP-0001
title: Pareto Harness Verified Procedure 实施路线图
status: active
owners: [maintainers]
created: 2026-08-20
updated: 2026-09-05
links: [PRD-0001, REQ-0001, REQ-0034, RFC-0007, RFC-0013, ADR-0008, ARCH-0004, BACKLOG-0001, BENCH-0001, EPIC-0007]
---

# Verified Procedure 实施路线图

核心产品路径是：成功任务路径经过证据验证和独立批准后，提升为不可变、内容寻址的已验证流程版本；后续 Run 固定该版本，Kernel 强制节点、依赖、能力、Evidence、checkpoint、恢复和补偿规则。模型只能提出动作。阶段只以验收证据而非日期宣告完成；原 16–20 周估算未覆盖本次重排，不能继续作为承诺，待各 Requirement 独立规划后重新估算。

## G0：工程、设计与 SDD 基线

交付：独立 Git 仓库、治理文件、Agent/Skill 体系、模板、文档检查、PRD、研究、架构、ADR/RFC、Benchmark，以及 SDD、影响分析、分层测试和独立 Review 门禁。

退出条件：REQ-0001 和 REQ-0002 满足；新 Agent 无需原始聊天即可完成 Requirement → Spec → Plan → Tasks → Review → Verified；无运行时代码空壳。

## G1：可信内核骨架

交付：REQ-0003 至 REQ-0009，包括协议/Revision、Event Store、Run Manifest、Snapshot/Replay、Capability/Budget、Hook 骨架和 Effect Intent/Receipt。

截至2026-09-01，REQ-0003至REQ-0009均已交付并通过各自独立评审。G1 作为可信内核基础已完成，但不等于产品闭环；Provider、Procedure、Plan/Node、Evidence Gate 与 CLI 均未实现。

退出条件：

- 幂等追加、顺序、状态迁移和崩溃恢复测试通过。
- 同一录制 Run 重放得到相同投影摘要。
- Schema、数据库迁移和兼容失败均有测试。

## G2A：受治理 Coding 执行边界

交付：REQ-0010 至 REQ-0013，包括为后续流程节点服务的 OpenAI-compatible Provider、Coding Tools、Workspace 与 Sandbox。Provider 不是最终产品闭环，也不拥有 Planner、Evidence 或完成 authority。

REQ-0010 按 authority-first 顺序：Provider contract/Manifest identity/Secret/Network/Cost/Effect/Replay → 同治理路径 Fake → loopback Mock Server → OpenAI-compatible HTTP/SSE adapter。adapter 只接收 Kernel-issued sealed session，不读取环境变量、Manifest、Event Store、Budget 或系统 DNS；返回值只是 observation。

退出条件：四类边界都默认拒绝且只能消费 Kernel 用途受限 lease；身份、权限、秘密、网络、费用、Workspace 与 Effect 可审计；Fake/Mock 路径确定性通过。本阶段不提供 Agent 可调用的通用 dispatch 入口；REQ-0035/REQ-0014 完成 Node binding 前不进入 Agent 执行路径，也不允许自由 Agent Loop 或自报完成。

## G2B：已验证流程执行与复用

交付：EPIC-0007。顺序为 REQ-0034 Procedure/TaskClass/Verified Procedure identity、最低独立审批与纯 registry admission → REQ-0018 无外部调用 Plan proposal、closed instantiation、procedure-capable Manifest/基础 Task DAG → REQ-0035 Kernel Node 状态机/checkpoint → REQ-0016 最小 Evidence Gate → REQ-0014 单 Agent 流程执行器 → REQ-0036 成功流程候选提升、复用与流程版本回退。

设计边界：Procedure 定义可复用允许路径；Plan 为 exact Task 实例化 DAG；Behavior 固定提出计划/动作的策略。Memory、模型、Planner 和 adapter 都不能推动权威节点、Evidence、完成或 Promotion。

退出条件：确定性任务按 exact Verified Procedure 执行；跳步、越权和缺证据完成被拒绝；成功候选只有经验证与独立审批才能提升；后续 Run 固定 exact 版本；恢复/回退/对账/补偿边界清楚。

## G2C：Memory 与操作 CLI

交付：REQ-0015 非权威 Working/Run/Project Memory，随后 REQ-0017 `run/resume/inspect/replay` CLI。Memory 可提供用户偏好、项目经验和操作说明，但不能成为流程或 Evidence authority；CLI 只调用 Kernel API。

退出条件：能在真实 Git 仓库按已验证流程完成缺陷修复和小型功能；失败可恢复；必需证据缺失不能成功；权限、隔离、费用和流程版本均可审计。

## G3：受控 Multi-Agent

交付：REQ-0019 至 REQ-0022，包括 Agent Lease、结构化消息、Worktree 合并和单/多 Agent Router。基础 Plan/DAG 与 Node authority 已由 G2B 交付，不在 Multi-Agent 阶段首次出现。

语言边界：Rust 保持 Plan/Node admission、Lease、去重、取消/预算传播、Workspace ownership、合并和最终验证；Agent Worker 可多语言或来自不同产品，但不得共享权威数据库、共享可写 Workspace 或自行判定完成。

退出条件：Agent 不共享可写工作区、不重复提交效果；崩溃可重领；无收益任务默认单 Agent。

## G4：技术亮点增强

交付：REQ-0023 至 REQ-0027，包括自适应 Task DAG、Context DAG、Context Cache/GC、完整 Evidence Graph 和成本感知 Router。完整 Evidence Graph 扩展 provenance、失效与复杂 evaluator，不替代 G2B 最小 Evidence Gate。

语言边界：Rust 保持 Context/Evidence provenance、Router admission、版本和质量底线；Python 可承担离线分析，经 accepted Requirement 和可复现收益证明后也可成为受控 retrieval/evaluation Worker，外部索引和自报指标不具权威性。

退出条件：每项优化分别完成消融；只在质量底线成立且至少改善一个目标维度时进入默认 Behavior。

## G5：受控演化

交付：REQ-0028 至 REQ-0033，包括 Behavior 谱系、Evolution Proposal、历史/隐藏评测、MVCC、Canary/Promote/Rollback 和 WASM 隔离。Behavior 演化与 REQ-0036 的 Procedure 晋升正交；两者都不能冒充 Run/Workspace 恢复或 Effect 补偿。

语言边界：Rust 保持 Proposal/MVCC/Canary/Promote/Rollback；Python 可用于研究与评测，多语言 Guest 可经 WASI 隔离运行，但不能自授权、自报晋升或绕过 Evidence gate。

退出条件：越权和资源超限被拒绝；Canary 触发停止后恢复到指定 Behavior；历史 Run 仍可由原版本解释；发布首份可复现实验报告。

## v0.2 以后

- TypeScript SDK 和 Web 控制台。
- 更多经独立 Requirement 批准的 Python/TypeScript/其他语言 Worker 与 SDK。
- MCP 和更多模型 Provider 适配。
- PostgreSQL、多节点执行和远程 Sandbox。
- 团队权限、签名策略、数据保留和合规能力。

## 项目治理节奏

- 每周：风险、证据缺口、架构债务和指标回顾。
- 每个里程碑：Requirement 验收、RFC/ADR 同步、Benchmark 报告和回滚演练。
- 每月：竞品/论文证据账本重新核验，过期结论标记而非静默保留。
- 每次重大失败：Fix；若已逃逸或暴露系统性缺口，则追加 Postmortem。
