---
id: REQ-0004
title: SQLite append-only Event Store
status: planned
owners: [maintainers]
created: 2026-08-23
updated: 2026-08-23
links: [EPIC-0002, REQ-0003, SPEC-0003, RFC-0003, ADR-0002, ADR-0003, ADR-0004]
risk: high
work: .agents/work/active/REQ-0004-sqlite-event-store
---

# Context and user

可信内核、后续 Run/Task 状态机、Projection 与 Replay 消费方需要一个本地、可恢复且不能绕过 `pareto-protocol` 的权威事件日志。REQ-0003 已交付闭合 `EventEnvelope`、SchemaSet 与可信验证边界，本需求激活 EPIC-0002 的首个持久化纵切。

# Problem

仓库尚无权威事件持久化。若序号、幂等、事务、隔离和兼容语义由后续消费者各自实现，竞争写入、崩溃或旧 Schema 数据可能造成顺序分叉、半写入、跨 Run 混用或不可重放历史。

# Desired outcome

提供可运行的 SQLite append-only Event Store：只接收协议层已验证的 `EventEnvelope`，原子追加后可按 Stream 或 Run 稳定读取；相同事件的重复提交幂等成功，冲突提交明确拒绝；进程重启后可继续读取和追加。

# Acceptance criteria

- AC-01：数据库初始化在单个受控入口执行版本化 migration；新库和已支持旧库均可打开，未知或更新的数据库版本 fail closed，失败 migration 不留下部分升级。
- AC-02：只接受 `pareto-protocol` 公共 API 产生的已验证 Event，且 append 只能由 trusted-kernel crate 内不可从公共 API 构造的 admission 调用；仅持有 `ValidatedEvent` 或自报 target 的外部消费者不能写权威日志。数据库无损保存 envelope、exact SchemaSet/limits 与显式索引身份，未知 Event/Schema 或存储字节与索引身份不一致时拒绝。
- AC-03：权威事件 append-only；公共 API 和 migration 不提供 update/delete，数据库约束或防护拒绝权威事件的原地修改与删除。
- AC-04：每个完整 `(tenant, user presence/value, workspace, run, agent, stream)` 的 sequence 从 1 开始严格连续单调递增；追加与 sequence 检查在同一事务中，多连接竞争同一下一序号时至多一个提交成功。
- AC-05：`event_id` 是幂等键；完全相同的已提交事件重复追加返回原提交结果且不增加行，复用相同 `event_id` 但内容、scope、run、stream 或 sequence 不同则返回稳定冲突错误。
- AC-06：提交是原子的；验证失败、序号冲突、SQLite 错误、取消或未提交事务均不留下半条权威事件，成功返回前数据已提交并可由新连接读取。
- AC-07：读取也只能由 trusted-kernel 内不可公开构造的 admission 调用；按完整隔离键与 sequence 提供稳定 Stream 读取，并按完整隔离键与确定性 `(stream_id, sequence, event_id)` 次序提供 Run 读取。分页使用绑定 scope、SchemaSet 和显式不可变 append ordinal horizon 的 opaque keyset cursor，不依赖 offset，新并发追加、重启和受支持 migration 不能造成漏读或混入。
- AC-08：Run、Workspace、tenant、user presence/value、Agent/actor 与 Stream 边界来自已验证 envelope 和显式查询作用域，payload 中同名字段不能覆盖；跨 Run/Stream 查询、causation 混用或索引/JSON 漂移 fail closed。
- AC-09：真实临时 SQLite 数据库证明初始化、追加、读取、幂等、同 Stream 并发冲突、失败原子性、重启恢复与继续追加；测试不得依赖空模块或 mock 数据库冒充持久化。
- AC-10：旧的受支持 Event Schema fixture 在重启后仍可读取并经固定 SchemaSet/协议 reader 验证；数据库 migration 与 Event Schema migration 分离，未知 Schema 不被静默升级或解释为当前版本。
- AC-11：REQ-0003 全部协议、Schema golden、兼容、隔离和 replay-manifest 测试保持通过；`pareto-protocol` 不反向依赖 Event Store、SQLite、Runtime 或网络。

# Quality, cost, and latency guardrails

- 质量：上述 AC 全部由确定性测试和真实 SQLite 文件证据覆盖；不得以 `INSERT OR IGNORE/REPLACE` 吞掉语义冲突。
- Token/费用：不调用模型或外部 Provider；不宣称 Token/费用优化。
- 延迟：记录单连接追加、顺序读取及竞争写入的可复现观察基线，不在首版虚构吞吐收益；SQLite busy 等待必须有有限上界。

# Non-goals

- 不实现 Run/Task 状态机、Projection、Snapshot、Replay executor、Capability 判定、Event Schema 自动迁移、网络服务或 CLI。
- 不提供修改/删除权威事件、跨租户管理查询或通用 SQL escape hatch。
- 不创建空 Runtime 模块；只交付 EventEnvelope 验证到持久化再到稳定读取的纵向切片。

# Risks and open questions

SQLite transaction mode、WAL durability、busy policy、数据库身份、migration 协议、幂等冲突优先级与权威 JSON 读取验证均会影响后续状态机和 Replay，属于跨需求且难逆决策；由 RFC-0003 冻结并接受独立架构评审后方可批准 SPEC-0003。
