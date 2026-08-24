---
id: ADR-0005
title: 采用 Manifest 首事件与单流 event-sourced Run/Task 生命周期
status: accepted
owners: [maintainers]
created: 2026-08-24
updated: 2026-08-24
links: [REQ-0005, SPEC-0004, RFC-0004, ADR-0001, ADR-0003, ADR-0004]
---

# Context

REQ-0005 首次让 Kernel 管理 Run 生命周期。Manifest 保存位置、状态事实源、transaction boundary、状态/冲突语义、父子约束和 bootstrap reader 一旦被 Projection、Replay、Capability 和 Task DAG 消费就难以回退。专项审查发现 Manifest/状态双写、persisted reader 循环、锁外 TOCTOU、隐式取消 cascade 和权限过度声明会破坏可信内核不变量。

# Decision

接受 RFC-0004。每个 Run 使用确定性派生的单一 lifecycle stream；`RunCreated` 必须是 sequence 1，并以闭合 payload 原子持久化完整 Run Manifest。Run/Task 当前状态只由该 stream 的 validated event range 折叠，不建立第二权威 state/Manifest 表；未来 Projection/Snapshot 是可丢弃缓存。

Kernel 先由 persisted SchemaSet/limits 和完整 Manifest exact 建立 owner authority；Kernel-private lifecycle transaction 再在同一 SQLite `BEGIN IMMEDIATE` 中重验并折叠完整 aggregate，然后执行 event-ID 幂等、expected sequence/state、合法 edge与parent/terminal guard，最后追加一个事件并提交。全局 event ID 不得在 authority/aggregate validation 前查询；authorized exact retry只先于终态/版本判断，不能掩盖损坏历史。same-ID mutation、stale writer、非法边和terminal late result明确拒绝。首版不隐式 cascade，Run终结前逐一终结Tasks。

Run 状态为 `created/running/paused/succeeded/failed/cancelled`；Task另含 `ready`，三个terminal不可逆。Task immutable parent只表达同Run ownership且必须引用更早Task；完整dependency DAG留给REQ-0018。首版只有Manifest owner actor可请求，通用Capability/delegation和外部取消传播留给REQ-0007。

# Alternatives

- 独立Manifest/state权威表加审计事件：产生双写和Replay歧义，拒绝。
- 锁外fold后复用单append：存在TOCTOU和lost approval，拒绝。
- 每Task独立stream加原子cascade：提前引入跨stream batch/DAG复杂度，拒绝。
- 任意认证actor或相同target即幂等：分别形成越权与stale-command吞噬，拒绝。
- 只存Manifest digest、内容放外部artifact：依赖尚不存在的原子存储和保留合同，拒绝。

# Consequences

获得单一事实源、Manifest/Event原子性、确定性恢复、明确并发/幂等和可供REQ-0006验证的pure fold；不需要SQLite schema migration或新第三方依赖。代价是首版每次命令O(n)折叠单Run lifecycle stream、SQLite单writer，以及取消/失败需要显式逐Task收敛。

新 lifecycle Schema 发布到新的内容地址SchemaSet，旧set保留。持久Run出现后，即使停止writer也必须保留对应SchemaSet、Manifest/lifecycle reader和fold；不得删除/更新历史或由进程默认值补齐。此ADR不代表Projection、Replay executor、Capability、Provider、Agent Loop或Task DAG已实现。

# Revisit triggers

- 可测的long-run fold延迟需要snapshot-assisted validation。
- 需要原子跨Task/跨stream取消、动态replan或dependency DAG。
- 需要service principal、delegation或多actor共同拥有Run。
- lifecycle state/edge、Manifest首事件、conflict priority、derived stream或persisted reader语义必须改变。

触发后需新Requirement/RFC/ADR、旧event/Manifest compatibility、migration/rollback、concurrency/isolation/late-result负测和独立架构评审；不得原位重释已持久化历史。
