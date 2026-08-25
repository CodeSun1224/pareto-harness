---
id: ADR-0006
title: 采用版本化 Projection、同库 Snapshot 与只读 Recorded Replay
status: accepted
owners: [maintainers]
created: 2026-08-24
updated: 2026-08-24
links: [REQ-0006, SPEC-0005, RFC-0005, ADR-0003, ADR-0004, ADR-0005]
---

# Context

REQ-0006 首次持久化 event-derived cache 并提供独立 Replay。Snapshot 的事实地位、reducer版本、游标和摘要绑定、数据库migration、fallback以及Recorded/Simulated语义会被权限、Workspace、Memory和Context Projection消费；若选择错误，会让缓存绕过protocol admission、形成第二权威状态或重复真实副作用。

# Decision

接受RFC-0005。Run/Task Projection只能由Kernel私有exact reader取得的已验证、连续lifecycle Event经版本化pure reducer构建。Kernel从persisted source contract exact resolve不可变reducer descriptor/implementation和output reader，并按历史保留；caller、Snapshot或current version不能替代。Projection输出和digest固定source store、完整scope/stream/cursor、source SchemaSet/limits、rolling history chain、reducer、exact output SchemaSet/limits、Manifest和确定排序状态；Event Store仍是唯一权威事实源。

Snapshot是同一SQLite store内的immutable、可忽略cache。Snapshot由`BEGIN IMMEDIATE`内的完整validated fold创建，绑定store、完整scope、stream、inclusive sequence/event ID、source identity、reducer、exact output/snapshot SchemaSet/limits和versioned history/projection/snapshot digest。加载时先验证candidate，再从Event Store重新读取、exact validate并用闭合rolling chain重算`[1..cursor]`；匹配后才以cached Projection跳过prefix reducer fold，并对suffix继续validation/hash/reduce。candidate损坏可full fallback，prefix/Event/DB损坏必须fail closed。

所有digest复用REQ-0003 `digest_json`和闭合1.0 seed/step/projection/snapshot/reducer hash-view Schema，固定domain、8-byte big-endian length-prefix、字段absence、顺序与golden vectors。Snapshot不能用自身携带的digest自证自身历史。

SQLite v2 migration不改历史Event JSON/sequence/SchemaSet/Manifest，但为`events`增加default epoch 1列和要求`NEW.writer_epoch=2`的checksum INSERT trigger；v2 writer显式写2。因此migration前已打开的v1 writer继续用旧SQL时也会fail closed，而不只依赖open时`user_version`检查。Snapshot UPDATE/DELETE均被checksum trigger拒绝；首片无GC，未来物理删除需新决策。

Recorded replay忽略Snapshot，对完整持久历史只读重建；其接口不接受Effect executor、不append、不覆盖source Run。Simulated replay只能使用固定fixtures且无live Effect capability；fixture resolver未交付前稳定拒绝。`reexecute`不属于本需求。只有store/full scope/stream/source set+limits/cursor/history chain/reducer/output set+limits exact相同的Kernel结果可比较；跨库clone只返回安全not-comparable。

首版沿用REQ-0005 owner-only authority、单进程SQLite和显式Snapshot创建；不建立通用Projection框架、自动调度、远程store或跨stream effect replay。

# Alternatives

- 每次完整fold：保留为oracle和fallback，但不能满足Snapshot/增量恢复，拒绝作为唯一路径。
- mutable Run/Task Projection表：形成双权威与事务/replay歧义，拒绝。
- 独立文件/远程Snapshot：缺少与Event Store的原子身份、权限与保留协议，延期。
- 允许caller导入Snapshot、选择current SchemaSet/reducer：可绕过history admission，拒绝。
- Recorded replay复用Snapshot或调用live executor：分别削弱oracle与可能重复副作用，拒绝。

# Consequences

获得可验证full Projection、prefix-proved cache-assisted恢复、corrupt-cache fallback、deterministic Recorded replay和完整provenance比较；代价是新protocol/hash-view Schema、reducer/output registry retention、SQLite v2 epoch migration、snapshot bytes、创建时O(n) fold/writer lock以及assisted load仍需O(n) prefix I/O/validation/hash。它只可能节省fold，不能宣称省去历史验证。

代码回滚必须继续支持SQLite v2、writer-epoch/snapshot triggers、retained source/output readers及reducers；不能降`user_version`、删除Snapshot row或历史。后续REQ-0007只能在authority前增加capability/budget/cancel并为新Event显式演进reducer mapping；REQ-0012/0015/0024只能消费带provenance的派生结果，不能获得snapshot写入口或把RunTask concrete schema当generic JSON escape hatch。

# Revisit triggers

- 可测的Snapshot创建writer-lock或full fold延迟超过项目目标，需要后台/CAS或增量digest设计。
- 需要跨stream Projection、Effect Receipt replay、fixture repository、reexecute或远程/分布式Snapshot。
- 需要Snapshot GC、签名/抗恶意raw DB writer、公共inspect API或多actor权限。
- 必须改变reducer descriptor/resolver/retention、rolling-chain或其他digest preimage、output reader、comparability、cursor binding、writer epoch、DB v2 schema或fallback语义。

触发后需新Requirement/RFC/ADR、forward migration、old-event/snapshot/reducer compatibility、effect-free replay、concurrency/isolation/rollback负测及独立架构评审；不得原位重释已持久化历史。
