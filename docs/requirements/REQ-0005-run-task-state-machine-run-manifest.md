---
id: REQ-0005
title: Run/Task 状态机与 Run Manifest
status: reviewing
owners: [maintainers]
created: 2026-08-24
updated: 2026-08-24
links: [EPIC-0002, REQ-0003, REQ-0004, SPEC-0004, RFC-0004, ADR-0001, ADR-0003, ADR-0004, ADR-0005]
risk: high
work: .agents/work/active/REQ-0005-run-task-state-machine
---

# Context and user

可信内核、后续 Projection/Replay、Capability、Provider、Agent Loop 和 Task DAG 需要一个可恢复、并发安全且不能被策略绕过的 Run 生命周期事实源。REQ-0003 已交付闭合协议与 Run Manifest 合同，REQ-0004 已交付 Kernel 私有 SQLite append-only Event Store；本需求把二者连接成首个可运行的生命周期纵向切片。

# Problem

仓库目前能验证 Run Manifest、保存和读取 Event，但没有权威 Run/Task 状态、合法迁移、命令幂等、Manifest 持久化或从事件恢复当前状态的 Kernel 路径。若调用方自行维护状态或用进程默认值补齐历史 Manifest，并发、崩溃、取消和迟到结果会造成状态分叉、跨边界混用或不可重放历史。

# Desired outcome

提供 Kernel 私有的最小 Run/Task 生命周期服务：创建时完整固定 Run Manifest 并以权威事件原子持久化；创建 Run 与层级 Task；只接受授权、前置状态和乐观并发版本均合法的迁移；由 Event Store 中固定范围的事件恢复当前状态；对非法、冲突、重复异义和终态后的迟到迁移 fail closed。

# Acceptance criteria

- AC-01：创建 Run 前完整验证并固定 Task、Behavior、Workspace、Environment、Context Graph、Model Snapshot、Tool Set、Kernel、SchemaSet、Budget、Protocol Limits、Boundary Recording Policy 与执行模式身份；缺失、额外、错误作用域或不匹配的 trusted input 均拒绝，已持久化 Manifest 禁止由进程默认值补齐或重释。
- AC-02：Manifest 是专用 Run lifecycle stream 的首个 `RunCreated` 事件 payload；Manifest、Run 初始状态和事件在同一 SQLite 事务中提交，失败或取消不得留下孤立 Manifest、Run 或半条事件，成功后可由新连接读取。
- AC-03：Run 状态固定为 `created | running | paused | succeeded | failed | cancelled`；Task 状态固定为 `created | ready | running | paused | succeeded | failed | cancelled`。实现只允许 SPEC-0004/RFC-0004 声明的迁移，所有终态不可逆。
- AC-04：状态迁移只能由 exact tenant、user presence/value、workspace、run、owner agent/actor 和 lifecycle stream 派生的 Kernel 私有 authority 请求；策略、插件、payload、裸 `ValidatedEvent` 或自报 scope 不能批准或执行迁移。首版只允许 Manifest owner actor，不宣称 REQ-0007 的通用 Capability/delegation 已实现。
- AC-05：每个命令固定 operation event ID、期望 lifecycle sequence、期望前置状态和完整参数；同一 authorized aggregate 内同 ID、同字节的已提交重试返回原结果，同 ID 异内容或跨 aggregate 复用返回不泄漏原结果的幂等冲突，不同 ID 争用同一期望版本时至多一个成功并对落后者返回乐观并发冲突。
- AC-06：相同命令的幂等命中先于终态/版本检查；除此之外，终态后的新迁移、重复目标、失败后的迟到成功或取消后的迟到结果均拒绝且不追加状态事件，不改变已终结 Run/Task。
- AC-07：Task 必须属于同一 Run，`parent_task_id` 只表达不可变的包含关系且只能引用同一 lifecycle stream 中更早创建的 Task，因此不能形成环；Run 启动、暂停和终结以及父 Task 终结必须满足 SPEC-0004 的子状态约束。依赖边、调度、重试和完整 Task DAG 留给 REQ-0018。
- AC-08：Run/Task 当前状态不保存为第二份权威可变事实；Kernel 在 `BEGIN IMMEDIATE` 持有期间从 lifecycle event range 折叠状态、检查命令并追加事件，Projection/Snapshot 以后只能作为可丢弃缓存，事务失败不得出现状态与事件不一致。
- AC-09：关闭并重新打开真实 SQLite 文件后，Kernel 只根据持久化首事件中的 exact SchemaSet/limits 和完整 Manifest 恢复 authority 与 Run/Task 状态；缺失首事件、未知 Schema、损坏序列、非法历史、索引/JSON 漂移或替代 reader 均 fail closed。
- AC-10：tenant、user presence/value、workspace、run、owner agent/actor、Task 和 stream 隔离均有负例；另一个 Run/Workspace/Actor 即使复用 operation ID、Task ID、payload 字段或兼容 SchemaSet，也不能读取、批准或改变目标生命周期。
- AC-11：发布新增 lifecycle event/payload Schema 和新的不可变 SchemaSet，不原位修改或删除 REQ-0003 已发布 SchemaSet；旧 Event/Manifest 仍由其 exact reader 读取，未知 major 或缺失 reader 不得静默迁移。DB migration 与 Event/Manifest Schema 演进保持分离。
- AC-12：测试至少覆盖状态机单元与全部状态对、非法负例、模型化状态序列、父子约束、重复/异义命令、并发竞争、终态与迟到结果、Manifest 完整/不可变、Manifest/Event 原子持久化、崩溃重开和 Projection/Replay fold 合同。
- AC-13：REQ-0003 协议与 Schema golden/compatibility/isolation/replay-manifest、REQ-0004 Event Store migration/append/read/idempotency/concurrency/recovery 全部回归通过；`pareto-protocol` 不反向依赖 Kernel/SQLite，且本切片不引入 Provider、网络、CLI 或真实模型调用。

# Quality, cost, and latency guardrails

- 质量：每个合法状态边和所有非法状态对均有确定性证据；任何状态结论都可由权威 event range 重算，不以缓存或内存值冒充事实。
- Token/费用：本需求不调用模型或外部 Provider，不宣称 Token/费用优化。
- 延迟：记录真实 SQLite 下创建、单次迁移、固定范围 fold 和竞争写入的可复现观察基线；首版不虚构吞吐收益，busy 等待沿用有限上界。

# Non-goals

- 不实现通用 Capability、delegation、预算扣减、超时传播或外部操作取消；这些属于 REQ-0007。首版只提供 owner-only fail-closed authority 和生命周期取消状态。
- 不实现 Projection 表、Snapshot、Replay executor 或 Boundary Inventory finalizer；这些属于 REQ-0006。
- 不实现 Provider、Tool、Agent Loop、Task dependency DAG、lease、scheduler、retry、dynamic replan、CLI 或跨进程服务。
- 不建立完整 Revision repository，也不声称 opaque Revision ID 的内容已由本需求持久化；创建 authority 只接受 Kernel 已解析并与 Manifest exact-match 的固定输入集合。
- 不追加安全审计型 late-result 事件或 Effect Receipt；本切片对无效状态命令返回结构化拒绝且不改变权威状态，REQ-0007/0009 扩展审计和效果对账。

# Risks and open questions

状态集合、父子终结规则、Manifest bootstrap reader、命令冲突优先级、同事务 fold-and-append 与 lifecycle Event Schema 会成为 REQ-0006/0007/0018 的长期合同，属于跨模块且难以回退的高风险决策。RFC-0004 已冻结这些选择，架构专项自审关闭全部 Blocker/Major，ADR-0005 记录接受决定；实现若需要改变状态边、权限主体、event ordering、Manifest 首事件或事务边界，必须退回 Spec/RFC 门禁。
