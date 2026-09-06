---
id: ARCH-0004
title: 技术选型基线
status: accepted
owners: [maintainers]
created: 2026-08-20
updated: 2026-09-06
links: [RFC-0007, RFC-0013, REQ-0034, ADR-0001, ADR-0002, ADR-0008, ADR-0012, ROADMAP-0001, REVIEW-0009, REVIEW-0018]
---

# 技术选型基线

## 已冻结的当前选择

| 领域 | 当前选择 | 稳定边界 |
|---|---|---|
| 可信控制面 | Rust stable + Tokio | 权威状态、并发与恢复语义留在 Rust |
| 序列化 | Serde + versioned JSON Schema | 公共协议版本化、可校验、可生成 |
| 本地持久化 | SQLite WAL + sqlx | 单机事务与迁移；数据库布局保持 Kernel 私有 |
| 架构形态 | 模块化单体 | 先保留事务与调试简单性，通过协议/capability 解耦 |
| Event API | Rust traits + versioned artifact export | 不暴露 Rust 动态库 ABI，不让外部组件直写 Event Store |

这些选择支撑 Stable Kernel Baseline。上层能力默认继续使用当前 reference path，除非可复现实验证明新增边界值得承担兼容、故障和运维成本。

## 未来假设，而非合同

| 假设 | 何时评估 | 采用前必须证明 |
|---|---|---|
| Python worker | Rust reference path 无法满足有价值的研究、评测或检索生态 | 可复现质量/成本/速度收益；版本化 I/O、隔离、取消、预算与 provenance |
| TypeScript SDK/UI | 出现明确的外部 SDK 或 Web 控制台用户需求 | 不进入可信控制面；兼容与维护成本可接受 |
| WASI guest | 需要在线运行第三方或不可信策略 | capability、资源、逃逸和故障隔离优于进程内方案 |
| PostgreSQL | SQLite 写竞争或多节点一致性成为可测瓶颈 | 迁移、回放、运维和事务语义有完整证据 |
| Remote worker | 独立扩缩容、故障域或硬件调度收益明确 | 身份、秘密、网络、late result、重试和数据隔离合同闭合 |
| RPC/queue/gRPC | 真实进程或网络边界已经存在 | 版本兼容、背压、幂等、可观测和回滚成本优于本地调用 |
| Protobuf 或其他 Schema | JSON Schema 无法满足兼容或吞吐目标 | 双栈迁移收益大于协议复杂度 |

“可能使用”不授权创建空 workspace、第二权威 Runtime、远程服务或提前冻结传输协议。首次需要某项假设的 Requirement 负责给出基线、测量、失败语义、迁移和退出方案。

## 不变量

- Event、Revision、Run/Task/Plan/Node 状态、Run Manifest、Verified Procedure admission、Capability、Budget、Cancellation、Effect/Evidence admission、Replay、Lease/MVCC 和 Promote/Rollback 由 Rust 可信控制面裁决。
- Planner、Context、Router、Tool ranking、Retry、Evaluator 与 Memory policy 是可替换的版本化策略，不固化进 Kernel。
- 外部组件只能接收用途受限输入并返回 proposal/observation；不得直接写 Event Store、修改 Manifest、自授 Capability、增加 Budget、标记成功或 Promote 候选。
- 离线工具和未来 Worker 依赖公共协议或显式 artifact，不依赖 SQLite 布局或 Rust ABI。
- Provider SDK 只存在于 adapter 边界；新依赖必须说明维护、许可证、供应链、体积和替代方案。

## 可移植性

开发和文档工具支持 Windows。核心 Runtime 目标为 Windows、Linux 和 macOS；强 Sandbox 的安全声明优先以 Linux 为基准，其他平台必须明确能力差异。

实现进度与当前采用范围见[项目状态](../status.md)。
