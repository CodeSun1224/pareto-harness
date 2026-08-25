---
id: RFC-0005
title: Projection、Snapshot 与本地确定性 Replay 合同
status: accepted
owners: [maintainers]
created: 2026-08-24
updated: 2026-08-24
links: [REQ-0006, SPEC-0005, EPIC-0002, REQ-0003, REQ-0004, REQ-0005, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ARCH-0002, ARCH-0003]
---

# Summary

在可信内核内增加一个只读、版本化的Run/Task Projection reducer，以及同SQLite文件内的可忽略Snapshot cache。Projection只消费Event Store以persisted exact SchemaSet/limits读取并验证的lifecycle stream；Snapshot-assisted load仍重新读取、exact validate并用versioned rolling chain证明cursor前缀，只省reducer fold。Snapshot固定store/full scope/cursor、source及output reader、exact reducer和闭合摘要。Recorded replay始终忽略Snapshot、从完整权威历史重建，不调用任何外部Effect；Simulated在fixture resolver交付前fail closed。SQLite v2以writer-epoch trigger阻止迁移前已打开的v1 writer继续append。

# Motivation and requirements

REQ-0005 的 `fold_lifecycle` 已证明完整历史可确定性恢复状态，但它是 lifecycle 命令内部类型，没有版本化输出、Snapshot 或独立 Replay 入口。REQ-0006 必须降低重复 O(n) fold 的未来压力，同时保持事件完整性、exact historical reader、owner-only authority、状态机语义和 replay honesty。Snapshot 一旦持久化，DB version、格式、reducer identity 和 fallback 语义会被 REQ-0007、0012、0015、0024 消费，因此需要在实施前冻结。

# Proposed design

## 1. Trusted-kernel boundaries

- `pareto-protocol` 增加闭合的 `ProjectionReducerRef`、`EventCursor`、`RunTaskProjection` 和 `RunTaskProjectionSnapshot` 1.0 Schema，并发布新内容地址 SchemaSet；旧 SchemaSet 永久保留。
- `pareto-kernel` 拥有 reducer registry、Projection read、Snapshot create/load、Recorded replay 和 comparison。所有 authority-bearing 构造器、SQLite transaction 和 snapshot table 保持 crate-private；外部调用者不能提交 Event、Projection 或 Snapshot 冒充可信输入。
- Projection reader 复用 REQ-0005 的 sequence-1 persisted identity bootstrap 和 REQ-0004 `validate_row`。任何事件在进入 reducer 前必须成为绑定 exact SchemaSet/limits 的 `ValidatedEvent`。
- 策略、插件和未来消费者只能请求 Projection/Replay，不能选择替代 reader/reducer、写 snapshot bytes、跳过 fallback 或改变 authoritative Event Store。

## 2. Versioned reducer and projection

`ProjectionReducerDescriptorV1`是闭合、机器生成且纳入版本控制的contract record，包含reducer kind/major/minor、accepted lifecycle event type/version/variant/payload SchemaRef、RunManifest Schema与admission合同、完整Run/Task transition及parent guard table、Task ordering、history/projection/snapshot digest合同、exact output/snapshot SchemaSetRef/ProtocolLimitsRef及member SchemaRefs。`ProjectionReducerRef`固定descriptor SchemaRef与`contract_digest = digest_json("projection-reducer-contract", descriptor_schema_ref, descriptor)`；同major/minor但digest不同也不兼容，它不伪称可执行文件哈希。

Reducer API 在 Kernel 内等价于：

```text
reduce(exact reducer, optional validated seed, next ValidatedEvent)
  -> next RunTaskProjectionState | stable ProjectionError
```

它不读取时钟、随机数、环境变量、文件、网络、数据库或mutable global state；使用Task ID排序的确定性集合。sequence 1必须是完整`RunCreated`；其后只接受descriptor列出的lifecycle variants，并复用REQ-0005 Manifest admission、identity/state/parent guards。gap/reuse、未知event/version、错误SchemaSet/limits、scope/actor/stream混用或非法历史均fail closed。

Kernel从persisted source SchemaSet的Manifest及lifecycle bindings计算闭合`SourceReducerKeyV1`，只经纳入版本控制的`source contract -> exact ProjectionReducerRef` registry解析；caller、Snapshot或“当前版本”不能选择。所有被历史Run/Projection/Snapshot引用的descriptor、implementation和output reader按Event历史保留期保留；missing/wrong/current substitution返回`reducer_unavailable`。未来REQ-0007新增Event必须新增明确mapping/reducer revision，不能重释旧Run。

`RunTaskProjection`固定：Kernel读取的`source_store_id`、完整scope、lifecycle stream、source cursor、source SchemaSet/limits、history-chain state、exact reducer、exact output SchemaSet/limits、完整Manifest、Run state和按Task ID排序的records。digest preimage包含全部provenance但排除DB path、加载方式、时间和性能数据；不同store即使Event bytes相同也不相等/不可比较。

### 2.1 Digest preimages and incremental chain

四类摘要均复用REQ-0003 `digest_json`（RFC 8785 canonical JSON；每段8-byte unsigned big-endian长度前缀；SHA-256 lowercase wire form），并为以下闭合1.0 hash-view发布Schema与golden：

1. `H0 = digest_json("projection-history-chain-seed", ProjectionHistorySeedV1.schema_ref, {"algorithm":"run-task-history-chain-v1"})`。
2. 对第i个`ValidatedEvent`构造`ProjectionHistoryStepV1 { algorithm, previous_digest: H(i-1), sequence, envelope, source_schema_set_ref, source_protocol_limits_ref }`；envelope是完整canonical EventEnvelope且sequence必须一致；`Hi = digest_json("projection-history-chain-step", step_schema_ref, step)`。
3. `projection_digest = digest_json("run-task-projection", RunTaskProjectionHashViewV1.schema_ref, view)`；view含store/full scope/stream/cursor/source set+limits/Hi/reducer/output set+limits/Manifest与排序状态，不含自身digest。
4. `snapshot_digest = digest_json("run-task-projection-snapshot", RunTaskProjectionSnapshotHashViewV1.schema_ref, view)`；view含除自身digest外的完整Snapshot。

checked-in vectors必须覆盖empty、1、N、prefix+suffix与full逐字节等价、scope/source mutation及descriptor/projection/snapshot；absence只按Schema省略，不能以null/default替代。

## 3. Snapshot format and creation

`RunTaskProjectionSnapshot` 1.0 固定：

- snapshot/Projection Schema及其exact output/snapshot SchemaSetRef与ProtocolLimitsRef；
- exact tenant/user presence-value/workspace/run/owner actor scope；
- Event Store `store_id` 和派生 lifecycle stream；
- inclusive cursor `{ sequence, event_id }`；
- source SchemaSetRef 与 ProtocolLimitsRef；
- exact reducer ref；
- 按§2.1对已验证`[1..cursor]`计算的history-chain state `Hcursor`；
- canonical Projection、`projection_digest`；
- 对除自身摘要字段外完整 snapshot preimage 计算的 `snapshot_digest`。

首版 Snapshot 只由显式 Kernel 请求创建，不自动按条数或时间触发。创建使用 `BEGIN IMMEDIATE`：先从 persisted sequence 1 exact resolve reader，在锁内读取并验证当前连续 lifecycle stream、full fold、计算 history/projection/snapshot digest，再插入 immutable row并 commit。append 与 creation 因同一 SQLite writer lock 串行化；rollback 不留部分 row。相同完整 key/content 可幂等返回，identity 相同但 bytes 不同报 integrity conflict。

SQLite schema v2新增`projection_snapshots` cache table，保存canonical snapshot JSON/fingerprint、output SchemaSet/limits canonical identity和查询/隔离列；固定checksum triggers拒绝UPDATE与DELETE。首片不提供delete/import/raw SQL/GC，“可丢弃”表示reader可忽略并从Event重建，而不是当前可物理删除。未来GC需新Requirement/RFC/migration与Kernel authorization。Snapshot不改变Event、Manifest或lifecycle state。

## 4. Point-in-time load and fallback

Projection load 在一个 SQLite read transaction 内：

1. 用 exact target/scope 和派生 stream 读取 sequence 1，固定 persisted SchemaSet/limits、owner authority及当前 maximum sequence/event cursor；该 transaction 建立 point-in-time horizon。
2. 查找不晚于horizon且exact store/scope/stream/source identity/reducer/output set+limits匹配的最新Snapshot；先由row中独立canonical output identity exact resolve retained reader，再验证record。
3. 比对extract columns、canonical bytes/fingerprints、全部digests及cursor Event ID/sequence。candidate自身无效时记录`missing | rejected_integrity | rejected_cursor | rejected_incompatible`并full fallback。
4. candidate自身有效后，仍从Event Store逐条读取并exact validate`[1..cursor]`的envelope、persisted SchemaSet/limits、identity和sequence，按§2.1重算`Hcursor`。只有外部权威前缀得到的值与Snapshot相同才可用cached Projection作为seed；prefix Event无需再执行reducer。
5. 从已验证`Hcursor`继续对`(cursor,horizon]`逐条做row/protocol/sequence/history-chain/reducer validation。prefix corruption或chain mismatch是Event history failure，必须fail closed，不得降级成cache miss或直接返回cached state。

因此assisted path只节省prefix reducer fold，不省Event读取或protocol validation。SQLite/Event Store identity/trigger/checksum/row损坏或full fold失败都整体fail closed；candidate fallback不修复/删除数据、不产生Event。

## 5. Replay modes and digest comparison

本 RFC 区分“Replay 操作”与 `RunManifest.execution_mode`：后者描述 Run 当初如何执行；前者是对既有 source Run 的只读 Projection 请求，不创建或覆盖 Run。

- `recorded`：从 source lifecycle sequence 1 到固定末端 cursor 读取全部持久化 Event，忽略 Snapshot，用 exact retained reader 和 reducer重建 Projection。它读取已经记录的事实，不调用 Provider、Tool、文件、网络、进程或 Effect dispatcher，不追加 Event。
- `simulated`：未来只可从固定、非空 fixture revisions 取得边界结果，且 capability 集合中不存在 live Effect。REQ-0006 首片不实现 fixture repository；请求在解析并验证 lineage 后以 `simulation_unavailable` 在任何 Effect dispatch 前拒绝。测试使用可观测 fake Effect counter 证明调用数恒为零。
- `reexecute`：需要真实外部调用和新的 Run，明确不在本需求范围。

comparison先验证Kernel结果provenance：source store ID、tenant/user presence-value/workspace/run/owner actor/stream、source SchemaSet/limits、cursor、history-chain state、reducer及output SchemaSet/limits必须全部exact。任一不同只返回安全`not_comparable/unauthorized`；全部相同时才按projection digest返回`equal | divergent`。它不写Event、不改变source，也不让cross-store/scope差异泄漏为divergent。

## 6. Concurrency, crash and isolation

- 普通 Projection load 使用 SQLite read snapshot，返回 point-in-time view；并发 append 只能出现在 horizon 之后。
- Snapshot creation 使用 `BEGIN IMMEDIATE`，所以其 cursor 和内容不与同 Run append 撕裂。首版接受 SQLite 单 writer 成本，不引入后台 worker/lease。
- Snapshot commit 前崩溃由事务回滚；commit 后 reopen 重新校验 row。Snapshot 损坏只触发完整 fold；Event history 损坏始终 fail closed。
- 每个查询和 Snapshot row 绑定 tenant、user presence/value、workspace、run、owner agent/actor、stream 和 store ID。Run ID、Task ID、event ID、cursor 或 digest 不能充当跨 scope capability。错误不回显其他 aggregate 的存在、payload 或状态。
- v2 migration为`events`增加`writer_epoch INTEGER NOT NULL DEFAULT 1 CHECK(writer_epoch IN (1,2))`和checksum `events_writer_epoch_v2` BEFORE INSERT trigger；trigger在`NEW.writer_epoch != 2`时拒绝。既有rows保持逻辑epoch 1，v2 writer显式insert epoch 2；迁移前已打开的v1 pool继续使用不含该列的旧SQL时获得default 1并被拒绝。该机制而非migration lock本身保证old live writer fail closed。

## 7. Downstream contracts

- REQ-0007 可在现有 owner-only authority 前增加 capability、budget、cancellation checks；不得让授权 payload 或 Snapshot 决定权限。新增 lifecycle events 需要 reducer/version演进，旧 Snapshot exact-invalidate。
- REQ-0012 的 Workspace Revision 仍由 Manifest 固定；Projection 只暴露 identity，不读取或嵌入 workspace bytes，Snapshot scope 必须 exact workspace-bound。
- REQ-0015 Memory 不得把 Snapshot 当记忆事实源；只能消费带 source cursor/digest 的派生 Projection，并自行定义 provenance/失效/隔离。
- REQ-0024 Context DAG 使用独立 Projection/Reducer/Schema，不复用 RunTask payload 或把本 RFC 的 concrete type 伪装为通用 JSON escape hatch。可复用 exact-version、cursor、digest、fallback 原则，但公共类型演进另立 Requirement。

# Interfaces, data flow, and invariants

```text
authenticated owner target
  -> persisted sequence-1 identity bootstrap
  -> exact retained source SchemaSet/limits + Event Store validated range
  -> source-contract registry -> exact retained RunTask reducer/output reader
       -> full Projection + history/projection digests
       -> optional BEGIN IMMEDIATE snapshot commit
       -> optional candidate validation
            -> exact re-read/validate/hash prefix
            -> trusted snapshot seed + validated/hash/reduced suffix
  -> read-only Recorded replay (always full range)
  -> full-provenance comparability -> equal/divergent/not_comparable
```

Kernel invariants：Event Store是唯一权威输入；Snapshot只缓存reducer输出且不能自证prefix；prefix与suffix Event均exact验证并进入同一rolling chain；unknown/corrupt history不跳过；source reducer/output reader exact且retained；cursor inclusive并绑定event ID；comparison绑定store/full provenance；Snapshot/Replay无Effect capability；任何模式不覆盖source Run；隔离键不从payload或cache派生。

# Failure modes and security

- Snapshot bytes/fingerprint/digest/schema/cursor drift：拒绝 candidate并 full fallback；fallback 成功前不返回状态。
- Snapshot compatible-looking substitution：store/scope/stream/source set/limits/reducer/output schema exact mismatch即拒绝。
- Snapshot创建后prefix envelope、stored admission identity、sequence或cursor drift：prefix exact validation/history chain失败并fail closed，不能用cached state遮蔽。
- unknown old reader/event/reducer：Snapshot invalid；full fold缺 reader或事件 variant则 `schema_unavailable/unsupported_event`，禁止 current reader替换或跳过。
- sequence gap/reuse/out-of-range：`invalid_sequence/aggregate_corrupt`；不继续增量、不从后续事件“修复”。
- writer competition/busy：有限等待后稳定 `busy`；不自动改变 cursor或创建旧 horizon snapshot。
- already-open v1 writer after v2：旧INSERT触发`writer_epoch_conflict`并rollback；不得归类为成功、busy或可自动retry的新ID。
- commit response uncertainty：以完整 Snapshot identity/content重试；不得生成不同 bytes覆盖。
- malicious caller：没有 public snapshot import、SQL、effect callback或 authority constructor；cross-scope只返回通用 unauthorized/not-comparable。
- raw DB writer：与 REQ-0004 相同，不声称抵抗拥有数据库文件写权限且可同步重算所有摘要的攻击者；open/read能检测定义内的 schema/trigger/fingerprint/identity drift，Snapshot 可疑即回退，Event Store 可疑则 fail closed。

# Alternatives considered

1. 保持现状、每次完整 fold：最简单且仍作为 correctness oracle，但不交付 Snapshot/增量恢复和独立 replay 接口，拒绝作为需求结果。
2. mutable `runs/tasks` Projection 表作为当前状态事实源：读取快，但与 Event Log 双写、崩溃后产生权威歧义，拒绝。
3. Snapshot 放独立文件或远程对象存储：需要额外原子提交、路径权限、保留和换库协议，超出单机最小切片，拒绝。
4. caller 提供任意 snapshot/reducer 或 current SchemaSet：允许绕过历史 admission 与 confused deputy，拒绝。
5. 自动每 N 条/每 T 秒创建：在无性能证据时引入后台调度、时钟与写竞争，首片选择显式请求。
6. Recorded replay 复用 Snapshot：更快但不能作为完整历史确定性 oracle，拒绝；普通 Projection load才允许 Snapshot-assisted。
7. Snapshot自带history digest作为前缀证明：由cache自证cache，无法检测prefix Event损坏，拒绝。首片选择重新读取/validate/hash prefix，仅省reducer fold；authenticated accumulator需另立高风险RFC。
8. 只靠`BEGIN EXCLUSIVE`和`user_version=2`阻止旧writer：已打开pool在migration后仍可写，拒绝；采用writer-epoch列+insert trigger。

# Compatibility, migration, and rollback

协议发布新SchemaSet但不修改旧set。source Event与Projection/Snapshot output是两条exact reader轴，record/row显式固定各自SchemaSetRef/limits；孤立member SchemaRef、current/alternate set不能替代。`SourceReducerKeyV1 -> ProjectionReducerRef` registry、descriptor、implementation和output reader随历史保留。Snapshot不兼容可fallback；missing source reader/reducer或unknown Event必须fail closed，不能靠cache掩盖或静默升级。

SQLite v2 migration在exclusive transaction内校验v1 identity/checksum/既有triggers，然后原子增加writer_epoch列、v2 INSERT epoch trigger、snapshot table/index、Snapshot UPDATE/DELETE triggers及全部checksum，最后设置`user_version=2`。不修改历史Event JSON、append ordinal、sequence、SchemaSet、Manifest或store ID；v2 writer显式epoch 2。失败rollback到完整v1。真实fixture必须保持一个migration前open的v1 writer并证明migration后旧INSERT被trigger拒绝，同时v2 writer正常。

旧binary重新open按newer-version规则拒绝；已打开binary由epoch trigger拒绝写。代码rollback必须保留v2 open、epoch/snapshot triggers、old source/output readers和reducers，可停用Snapshot功能并忽略rows，但不能降`user_version`、drop tables/triggers、DELETE Snapshot、删除旧Schema/reducer或改写历史。

# Evaluation and acceptance

- 质量：full fold、prefix-validated snapshot incremental/fallback、rolling-chain golden、exact reducer/output reader、cross-store comparison与Recorded replay equality；prefix corruption、unknown、隔离、并发、crash、already-open-v1-writer migration和Effect-free负测全部通过，REQ-0003/4/5全回归。
- Token/费用：不调用模型/Provider/Tool，报告不适用；记录新增 Schema/DB bytes与测试成本，不声明Token优化。
- 延迟：在真实本地SQLite分别记录1/10/100/1000 Event的读取+validation、reducer fold、rolling hash、Snapshot create writer-lock、1/10-event suffix和Recorded replay；assisted仍O(n)验证prefix，只观察fold节省，不宣称总延迟收益。
- 设计批准：独立architecture-review首轮提出6 Major；author remediation由同一独立reviewer以exact hashes focused re-review并逐项关闭，最终0 open Blocker/Major。RFC-0005于2026-08-24接受，ADR-0006最终确认，SPEC-0005批准。实施后仍需fresh independent code-review检查exact代码diff、Schema/DDL和验证证据。
