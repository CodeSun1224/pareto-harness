---
id: ADR-0010
title: 采用 Kernel 治理的 Effect Intent、Receipt 与对账边界
status: accepted
owners: [runtime-kernel]
created: 2026-08-30
updated: 2026-08-30
links: [REQ-0009, SPEC-0008, RFC-0009, REVIEW-0012, REQ-0004, REQ-0007, REQ-0008, ADR-0003, ADR-0004, ADR-0006, ADR-0007, ADR-0009, ARCH-0002, ARCH-0003]
---

# Context

SQLite Event transaction不能与文件、process、network、model或其他外部系统形成同一原子提交。REQ-0007已经提供默认拒绝Capability、执行前预算reserve、opaque operation lease、取消/deadline、保守unknown settlement和replay零执行；REQ-0008又证明扩展事实与control reservation/terminal必须使用双stream atomic pair，不能先提交一个stream再补另一个。但二者都明确不执行Effect、不构造Intent/Receipt或关闭外部对账。

REQ-0009需要冻结Intent-before-dispatch、业务幂等、executor identity、partial/unknown、crash recovery、Receipt trust、成功准入、Boundary Inventory和Recorded replay。REVIEW-0012对初始设计提出4个Major，并经多轮同Reviewer复审关闭：source replay horizon、partial/unknown inventory语义、immutable executor identity、stable recovery command及未claim/已claim timeout唯一结论。fixed `b7acbd82824d8410d432117c89be1bd56c8ce05c`达到independent approved、0 Blocker、0 Major。

# Decision

接受RFC-0009与SPEC-0008。Effect是可信Kernel治理的受保护边界，不是普通callback；request、client key、Hook decision、Receipt或Provider usage都不是authority。每个Effect先用control reserve + `effect-intended` atomic pair持久化Intent，再提交dispatch claim，只有绑定exact Manifest/registry/executor/request/operation/reservation/process epoch的opaque lease能进入确定性Fake executor。

不承诺外部exactly-once；采用at-most-once-or-reconcile。Intent未claim时，history证明Kernel未交付executor lease，取消/deadline/process-loss recovery只能`not_applied + verified zero + full release`。claim后可能已越过外部边界，crash/cancel/timeout只能verified partial或unknown保守settle并进入reconciliation，不能伪装未执行或自动redispatch。Effect-bound operation的reserve与terminal都使用control/Effect atomic pair；单边pair是损坏且不补写。

Run Manifest 3.0精确固定Effect registry；内容地址`EffectExecutorDescriptorV1`贯穿Effect ID/request digest、Intent、claim、recovery key、lease、Receipt admission、Projection与reopen。Receipt只是不可信observation，只有exact executor/producer/adapter/lease/Schema/Clock/Kernel-meter admission能形成terminal pair。reconciliation由独立pinned producer追加结论，不改原operation terminal、budget、lifecycle或inventory。

Effect stream是唯一权威Effect状态；Projection由explicit inclusive cursor pure fold。Run/Task成功在同一writer transaction拒绝pending/claimed/partial/unknown/open-reconciliation Effect；failed/cancelled可在operation settle后保留对账。SQLite保持v2，不增加mutable outbox/status表或background scanner。

发布Boundary Inventory/Effect Record 2.0，无损表达applied、not-applied、cancelled-before-claim、partial和unknown，并固定exact Effect cursor/history digest。Recorded replay只读inventory horizon内事实；horizon后late/reconciliation不改变同一pin。旧Manifest、Inventory、SchemaSet、reader/reducer和SQLite bytes全部保留。

# Alternatives

- 先执行再记录、mutable outbox为authority或声称exactly-once：分别无法证明crash结果、形成双权威或虚构跨边界原子性，拒绝。
- 复用Runtime Control单stream reserve/settle再补Effect Event：允许单边事实，拒绝。
- Receipt/Provider usage直接权威化或cancel后宣称未执行：允许伪造/低报或对不可中断边界作虚假承诺，拒绝。
- keyed Effect自动redispatch、自动补偿/saga或background poller首发：需要真实adapter与额外权限/恢复证据，拒绝并留给后续Requirement。
- Recorded replay走live dry-run或读取source最新stream：分别保留Effect入口或让相同Manifest随late facts漂移，拒绝。

# Consequences

获得Intent-before-dispatch、同键异请求拒绝、executor不可替换、双stream原子budget/Effect、partial/unknown诚实表达、显式幂等recovery、成功准入和fixed-horizon replay。代价是每Effect至少三次SQLite writer transaction、多stream fold、更多版本化Event/Schema和保守unknown成本；claim后crash可能长期需要显式reconciliation，首片没有自动redispatch、background worker、Effect Snapshot或真实外部I/O。

实施必须保持Kernel-private authority、SQLite v2、历史reader与zero-execution replay。首个v3/Effect事实写入后，rollback只能停止新writer并保留Manifest/Schema/pair validator/reader/reducer/recovery/reconciliation解释；不能删Event、补单边pair、静默release、把unknown改零或重执行pending Effect。

# Revisit triggers

- 真实Provider/Tool需要证明keyed redispatch、query reconciliation、compensation或billing evidence。
- Effect fold/SQLite writer达到可测瓶颈，需要Snapshot、非权威索引、background dispatcher或远程协调。
- 需要跨Run去重、distributed transaction、multi-signer Event、service principal或外部queue。
- Boundary Inventory v2不足以表达新的partial component/evidence或派生Run需要pin post-inventory reconciliation。

触发后必须有新Requirement/RFC/ADR、forward-compatible Schema、old reader/replay/permission/isolation/race/rollback负测和独立架构评审；不得原位重释本决策历史。
