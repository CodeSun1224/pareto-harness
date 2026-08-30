---
id: BACKLOG-0001
title: Pareto Harness Requirement Backlog
status: active
owners: [maintainers]
created: 2026-08-22
updated: 2026-08-30
links: [EPIC-0001, EPIC-0002, EPIC-0003, EPIC-0004, EPIC-0005, EPIC-0006, RFC-0007, ADR-0008, ARCH-0004]
---

# Requirement Backlog

Planned ID 表示编号已保留但尚未进入 SDD 实施窗口。进入窗口时必须创建正式 Requirement、影响分析和 Spec，不能直接按本表编码。

语言不是 Requirement 的默认信任边界。REQ-0008 至 REQ-0033 可在其 outcome 需要时批准多语言 handler、adapter、Worker 或 Guest，但必须保留 Rust 对 Event、identity、state、Capability、Budget、Cancellation、Effect/Evidence、Replay、Lease/MVCC 与 Promotion 的 authority，并在各自 Spec 中冻结协议、隔离、失败、兼容和回滚合同。RFC-0007/ADR-0008 不单独授权任何外部 Runtime，也不改变下列顺序或 prerequisite。

| Order | ID | Epic | Requirement outcome | Risk | Prerequisites | Planned evidence |
|---:|---|---|---|---|---|---|
| 1 | REQ-0002 | EPIC-0001 | SDD、影响分析、分层测试和独立 Review 门禁 | standard | REQ-0001 | 自举演练、Skill/文档检查 |
| 2 | REQ-0003 | EPIC-0002 | 版本化协议类型和 JSON Schema | high | REQ-0002 | Schema golden/compatibility tests |
| 3 | REQ-0004 | EPIC-0002 | SQLite append-only Event Store | high | REQ-0003 | 幂等、顺序、并发、崩溃测试 |
| 4 | REQ-0005 | EPIC-0002 | Run/Task 状态机与 Run Manifest | high | REQ-0003, REQ-0004 | 属性测试、非法迁移测试 |
| 5 | REQ-0006 | EPIC-0002 | Projection、Snapshot 与 Replay | high | REQ-0004, REQ-0005 | recorded/simulated replay |
| 6 | REQ-0007 | EPIC-0002 | Capability、预算、取消与超时 | high | REQ-0005 | 权限负例、预算和迟到结果 |
| 7 | REQ-0008 | EPIC-0002 | Observer/Gate/Transform Hook 骨架 | high | REQ-0004, REQ-0007 | 顺序、超时、不可改写事件 |
| 8 | REQ-0009 | EPIC-0002 | Effect Intent/Receipt 与幂等效果 | high | REQ-0004, REQ-0007, REQ-0008 | 部分成功、重复提交、对账 |
| 9 | REQ-0010 | EPIC-0003 | OpenAI 兼容 Provider 与 Fake Provider | standard | REQ-0005, REQ-0007 | contract、stream、usage、错误 |
| 10 | REQ-0011 | EPIC-0003 | Search/Read/Patch/Shell/Test Coding Tools | high | REQ-0007, REQ-0009 | 工具契约、路径和权限负例 |
| 11 | REQ-0012 | EPIC-0003 | Git Workspace Revision 与 Artifact | high | REQ-0006, REQ-0011 | dirty patch、恢复、隔离 |
| 12 | REQ-0013 | EPIC-0003 | Linux Container 与 Windows 开发 Sandbox | high | REQ-0007, REQ-0009, REQ-0011 | escape、网络、秘密、资源限制 |
| 13 | REQ-0014 | EPIC-0003 | 基础 Planner 和单 Agent Loop | standard | REQ-0010..REQ-0013 | Fake E2E、失败恢复 |
| 14 | REQ-0015 | EPIC-0003 | Working/Run/Project Memory 基线 | high | REQ-0006, REQ-0014 | 来源、隔离、失效、注入负例 |
| 15 | REQ-0016 | EPIC-0003 | 测试/构建/静态检查 Evidence Gate | high | REQ-0011, REQ-0014 | 缺失/伪造/过期证据拒绝 |
| 16 | REQ-0017 | EPIC-0003 | run/resume/inspect/replay CLI | standard | REQ-0014..REQ-0016 | 真实仓库端到端基线 |
| 17 | REQ-0018 | EPIC-0004 | Plan Revision 和基础 Task DAG | high | REQ-0005, REQ-0014 | DAG、失败、取消、重试 |
| 18 | REQ-0019 | EPIC-0004 | Agent Profile、Task Lease 和心跳 | high | REQ-0018 | 重领、重复领取、迟到结果 |
| 19 | REQ-0020 | EPIC-0004 | 结构化 Agent Message 和 Artifact Reference | high | REQ-0019 | Schema、隔离、Token accounting |
| 20 | REQ-0021 | EPIC-0004 | Git worktree 隔离、Patch 合并和验证节点 | high | REQ-0012, REQ-0019 | 写冲突、失败合并、回滚 |
| 21 | REQ-0022 | EPIC-0004 | 单/Multi-Agent 选择策略 | standard | REQ-0017..REQ-0021 | 配对成本/质量/延迟基准 |
| 22 | REQ-0023 | EPIC-0005 | 自适应 Task DAG 和重规划 | standard | REQ-0018 | 计划谱系、关键路径消融 |
| 23 | REQ-0024 | EPIC-0005 | Context DAG 与 Projection | high | REQ-0015 | 来源、派生、隔离、Token 追踪 |
| 24 | REQ-0025 | EPIC-0005 | Context Cache、压缩、失效和 GC | high | REQ-0024 | 失效正确性和质量消融 |
| 25 | REQ-0026 | EPIC-0005 | 完整 Evidence Graph | high | REQ-0016 | Requirement/Verifier/Verdict 谱系 |
| 26 | REQ-0027 | EPIC-0005 | 成本感知 Model Router | standard | REQ-0010, REQ-0024, REQ-0026 | 质量底线下 Pareto 对照 |
| 27 | REQ-0028 | EPIC-0006 | Behavior Revision 与策略谱系 | high | REQ-0003, REQ-0027 | 不可变版本和兼容测试 |
| 28 | REQ-0029 | EPIC-0006 | Evolution Proposal 生命周期 | high | REQ-0028 | 候选隔离和状态机测试 |
| 29 | REQ-0030 | EPIC-0006 | 历史/隐藏/安全集与 Pareto Archive | high | REQ-0026, REQ-0029 | 泄漏、过拟合、统计检查 |
| 30 | REQ-0031 | EPIC-0006 | Candidate MVCC 和冲突处理 | high | REQ-0028, REQ-0029 | 并发 rebase/merge/拒绝 |
| 31 | REQ-0032 | EPIC-0006 | Canary、Promote 和 Rollback | high | REQ-0030, REQ-0031 | 自动停止、原子切换、恢复 |
| 32 | REQ-0033 | EPIC-0006 | WASM/WASI 不可信策略隔离 | high | REQ-0007, REQ-0028 | Capability、资源、逃逸负例 |

## Activation rule

同一时间最多一个 Runtime Requirement 处于 `implementing`，另一个可处于 `specified/planned`。只有 prerequisite 全部 `verified` 后才能激活依赖项。每个 Epic 必须产生可运行纵向切片，禁止按 crate 横向完成后统一集成。
