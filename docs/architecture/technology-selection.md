---
id: ARCH-0004
title: 技术选型基线
status: accepted
owners: [maintainers]
created: 2026-08-20
updated: 2026-09-05
links: [RFC-0007, RFC-0013, REQ-0034, ADR-0001, ADR-0002, ADR-0008, ROADMAP-0001, REVIEW-0009]
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
| Python | 研究、评测及经 Requirement 批准的受控 Worker | 研究与模型生态丰富 | 不得形成第二 Runtime authority；无收益证据时保持离线或 Rust reference path |
| 部署 | 模块化单体 | 保持事务和调试简单 | 无真实部署边界前不拆微服务 |

## 分阶段语言边界

项目不以“全 Rust”为目标，而以“权威机制只能由 Rust 可信控制面提交”为稳定边界。G1 使用 Rust 模块化单体证明事务、恢复、权限和并发语义；G2 至 G5 的 reference path 可以继续用 Rust，但 Provider、Tool、Hook handler、Agent Worker、Memory 检索、评测、SDK 和 Guest 可在具体 Requirement 证明收益与安全合同后采用其他语言。阶段名称不构成外部 Runtime 授权，正式传输和进程模型也不在本基线预选。

| 阶段 | Rust 权威控制面 | 扩展与计算面 | 明确边界 |
|---|---|---|---|
| G1：可信内核骨架 | 协议、事件、状态机、Manifest、Replay、Capability、Budget、Hook 与 Effect 合同 | Fake Model/Tool 和确定性夹具 | 不引入真实外部 adapter、通用 Worker 或插件 Runtime |
| G2：已验证单 Agent 流程 | Procedure/Plan/Node lifecycle、Effect/Evidence gate、Workspace/Sandbox policy、流程晋升与 CLI admission | 默认 Rust reference Provider/Tool/Agent；具体 Requirement 可批准受控外部 adapter | 外部组件不能构造 Event、Procedure approval、Capability、Evidence、Budget 或终态；Memory 非权威 |
| G3：受控 Multi-Agent | Agent Lease/Heartbeat、取消与预算传播、Workspace ownership、合并和最终验证 | 隔离的本地或远程 Agent Worker，语言和产品不限 | 复用 G2 的 Plan/Node authority；Worker 不共享权威数据库或可写 Workspace，不自行判定 Task 完成 |
| G4：技术亮点增强 | Context/Evidence provenance、Router admission、版本与质量底线 | Python 可做离线分析；经批准并有可复现收益时可做受控 retrieval/evaluation Worker | 外部索引不是事实源；输入输出必须版本化、有上限且可审计 |
| G5：受控演化 | Behavior/Proposal/MVCC/Canary/Promote/Rollback | Python research/evaluation 与多语言 WASI Guest | Guest 不得自授权、自报晋升或绕过历史/隐藏集；输出由 Rust 复验 |

### 职责按稳定性分配

- Rust 拥有 Event Store、Revision、Run/Task/Plan/Node 状态机、Run Manifest、Verified Procedure admission、Capability、Budget、Cancellation、Effect Intent/Receipt、Evidence admission、Replay、Lease、MVCC 以及 Promote/Rollback。
- Rust 内置策略是默认 reference implementation，通过版本化 strategy interface 接入；planner、context、router、tool ranking、retry、evaluator 和 memory policy 不得成为内核不可替换逻辑。
- Python 可用于离线轨迹分析、统计检验、评测及 Router/Context 策略原型；若具体 Requirement 证明在线收益，也可成为隔离 Worker。其输入输出必须版本化、可取消、可核算并有 provenance，结果默认非权威。
- Wasmtime/WASI 从 G5 起承载需要在线运行的不可信策略；Guest 语言不限于 Rust，但默认无文件、网络、进程或秘密访问能力，且受 fuel、内存、deadline、预算和 capability 限制。
- TypeScript 用于后续 SDK、插件开发工具和 Web 控制台，不承担权威状态机、权限判定或晋升协议。

### 跨语言接入路径

1. G1 使用进程内 Rust trait，先证明稳定接口和可重放语义。
2. G2 至 G5 的每个真实跨语言边界由首次需要它的 Requirement 冻结协议版本、identity、Capability、Budget、Cancellation、late result、Effect、Replay、隔离、升级与回滚；本基线不预选子进程、JSONL、MCP、WASI、队列或 RPC。
3. 离线工具通过显式 artifact 导出/导入工作；在线 Worker 只有在生态、故障隔离或独立扩缩容收益可复现时才进入关键路径，且不能依赖 SQLite 布局或 Rust ABI。
4. 需要在线运行的不可信策略优先评估受限 WASI Component；Python/TypeScript 原型和远程服务不得直接获得生产内核权限。

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
