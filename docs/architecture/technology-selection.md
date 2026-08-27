---
id: ARCH-0004
title: 技术选型基线
status: accepted
owners: [maintainers]
created: 2026-08-20
updated: 2026-08-27
links: [ADR-0001, ADR-0002, ROADMAP-0001]
---

# 技术选型基线

## 决策

| 领域 | 首选 | 理由 | 延后或拒绝 |
|---|---|---|---|
| 可信内核 | Rust stable + Tokio | 类型安全、并发控制、资源效率、可部署单二进制 | 不用 TypeScript 承担可信内核 |
| 序列化 | Serde + versioned JSON Schema | 人可读、便于事件导出和跨语言生成 | 首期不先引入 Protobuf/gRPC |
| 本地存储 | SQLite WAL + sqlx | 事务、迁移、单机可运维、快速纵切 | PostgreSQL 在多节点需求成立后增加 |
| Event API | Rust traits + JSONL export | 内部强类型，对外可检查 | 不暴露 Rust 动态库 ABI |
| 插件隔离 | Wasmtime/WASI | capability 和资源限制明确 | 强隔离实现延后到核心语义稳定后 |
| TypeScript | SDK、插件工具、Web 控制台 | 生态和 UI 效率 | 不在首期创建空 workspace |
| Python | 离线研究与评测；后续可选 Worker | 研究生态丰富 | G4 只消费导出 artifact；Runtime Worker 延后到 v0.2 并单独评审 |
| 部署 | 模块化单体 | 保持事务和调试简单 | 无真实部署边界前不拆微服务 |

## 分阶段语言边界

项目不以“全 Rust”为目标，而以“权威机制只能由可信内核提交”为边界。G1 至 G3 保持 Rust 主链和模块化单体，降低事务、恢复、权限与并发语义的实现风险；G4 允许离线 Python 分析工具消费版本化导出 artifact，但不引入 Runtime Worker；G5 再引入 Wasmtime/WASI 承载受限在线策略。正式 Python Worker、TypeScript SDK 和 Web 控制台不进入 G1 至 G5 的关键路径，保留到 v0.2 或出现明确且经过验证的需求时实施。

| 阶段 | 主语言与运行形态 | 功能侧重点 | 明确边界 |
|---|---|---|---|
| G1：可信内核骨架 | Rust 模块化单体 | 协议、事件、状态机、Manifest、Replay、Capability、Budget、Hook 与 Effect 合同 | 使用 Fake Model/Tool 隔离外部波动；不引入 Python/TypeScript Runtime |
| G2：稳定单 Agent CLI | Rust Runtime、CLI 与 adapter | 真实 OpenAI-compatible Provider、Coding Tools、Git Workspace、Sandbox、单 Agent Loop、Memory 与 Evidence Gate | Provider SDK 只存在于 adapter；模型和工具不能直接写权威状态 |
| G3：受控 Multi-Agent | Rust Scheduler 与 Workspace Coordinator | Task DAG、Lease、Heartbeat、结构化消息、worktree 合并和单/多 Agent 选择 | Agent 只提交 proposal、artifact 和 effect request；Lease、去重、合并与状态迁移由内核裁决 |
| G4：技术亮点增强 | Rust Runtime + 可选离线 Python 分析工具 | Context DAG/GC、Evidence Graph、成本感知 Router、消融、统计与 Pareto 分析 | Python 只消费导出的版本化 artifact，并产出非权威分析结果；不作为 Runtime Worker，不直接访问权威数据库 |
| G5：受控演化 | Rust Control Plane + Python 研究/评测 + Wasmtime/WASI | Behavior 谱系、Evolution Proposal、历史/隐藏集、MVCC、Canary、Promote/Rollback | Rust 掌握晋升和回滚；Python 负责候选生成与离线评测；在线不可信策略必须经过 WASI capability、资源限制和输出复验 |

### 职责按稳定性分配

- Rust 拥有 Event Store、Revision、Run/Task 状态机、Run Manifest、Capability、Budget、Cancellation、Effect Intent/Receipt、Replay、Lease、MVCC 以及 Promote/Rollback。
- Rust 内置策略作为 G1 至 G3 的默认实现，通过版本化 strategy trait 接入；planner、context、router、tool ranking、retry、evaluator 和 memory policy 不得成为内核不可替换逻辑。
- Python 从 G4 起可用于离线轨迹分析、统计检验、评测及 Router/Context 策略原型。输入是带 Schema、摘要和 provenance 的导出 artifact；输出是非权威 analysis、metric 或 candidate artifact，不能回调 Runtime 或提交权威状态。
- Wasmtime/WASI 从 G5 起承载需要在线运行的不可信策略；Guest 语言不限于 Rust，但默认无文件、网络、进程或秘密访问能力，且受 fuel、内存、deadline、预算和 capability 限制。
- TypeScript 用于后续 SDK、插件开发工具和 Web 控制台，不承担权威状态机、权限判定或晋升协议。

### 跨语言接入路径

1. G1 至 G3 使用进程内 Rust trait，优先证明稳定接口和可重放语义。
2. G4 只冻结语言无关的 artifact/metric Schema、内容摘要、provenance 和输出上限；离线 Python 工具通过显式导出/导入边界工作，不冻结常驻进程、双向 Worker 协议、队列、JSONL 或 gRPC 等传输机制。
3. 经离线评测证明有价值且需要在线执行的策略，在 G5 固化为可审查配置、Rust 内置实现或受限 WASI Component；Python 原型不得直接获得生产内核权限。
4. v0.2 以后，只有当 Rust 实现显著阻碍研究迭代、离线交换不足、需要独立资源/故障隔离，且存在可复现收益证据时，才为正式 Python Worker 建立 Requirement/RFC/ADR，并评审本地子进程、JSON/JSONL、队列或远程协议。远程执行、独立扩缩容或多服务部署仍需额外测量证据。

无论实现语言为何，非 Rust 扩展都不得直接写 Event Store、修改 Run Manifest、自授 Capability、增加 Budget、标记 Run 成功、原位覆盖 BehaviorRevision、绕过 Effect Intent/Receipt，或自行 Promote 候选。它们只能提出请求或候选，由可信内核验证、授权、记录并执行或拒绝。

## 依赖原则

- 公共协议包不依赖数据库、网络或具体模型 SDK。
- Provider SDK 只能存在于 adapter 边界。
- 离线 Python 工具、未来跨语言 Worker 和 WASI Guest 只能依赖版本化公共协议或 artifact，不依赖内核数据库布局。
- 数据库迁移与事件 Schema 迁移分别管理。
- 新依赖需记录维护状态、许可证、供应链风险、二进制体积和替代方案。
- 首个纵切使用 Fake Model/Tool，避免把 Provider 波动误认为内核问题。

## 可移植性

开发和文档工具支持 Windows。核心 Runtime 目标为 Windows、Linux 和 macOS；强 Sandbox 的安全声明优先以 Linux 为基准，其他平台必须明确能力差异。

## 触发重新评审的条件

- SQLite 写入竞争成为可测量瓶颈。
- 出现独立扩缩容、故障域或团队所有权需求。
- JSON Schema 无法满足兼容性或高吞吐协议需求。
- WASI 无法表达必需能力或隔离强度。
- 离线 artifact exchange 已无法满足有证据支持的研究吞吐、资源隔离或故障域需求。
