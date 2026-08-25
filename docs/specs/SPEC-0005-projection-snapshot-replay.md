---
id: SPEC-0005
title: Projection、Snapshot 与 Replay 规范
status: approved
owners: [maintainers]
created: 2026-08-24
updated: 2026-08-24
links: [REQ-0006, EPIC-0002, REQ-0003, REQ-0004, REQ-0005, RFC-0005, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ARCH-0002, ARCH-0003]
---

# Behavioral contract

Run/Task Projection 是由可信 Event Store 中一个 Run 的完整 lifecycle stream 经 exact versioned reducer 得到的派生值；Snapshot 只是该值在一个已验证事件游标处的可丢弃缓存；Recorded replay 是忽略 Snapshot 的完整历史只读重建。三者都不能写入或替代权威 Event、Manifest、lifecycle state，也不能取得真实 Effect 执行能力。

RFC-0005已接受，ADR-0006记录最终decision。独立architecture-review首轮提出6个Major；author remediation经同一独立reviewer focused re-review逐项关闭，最终0 open Blocker/Major。本Spec于2026-08-24最终批准进入Plan；Runtime实施尚未开始，design approval不能替代实施后的独立code review。

# Inputs, outputs, states, and failure behavior

## Inputs and authority

- `ProjectionTarget`：exact tenant、user presence/value、workspace、run 与 owner actor。构造保持 Kernel 私有；首版从 persisted sequence-1 Manifest 建立与 REQ-0005 相同的 owner-only authority。
- `ProjectionRequest`：选择 `full_history | snapshot_assisted` 和可选固定 horizon；外部不能选择 SchemaSet、Protocol Limits、reducer bytes、snapshot bytes或 SQL。
- `SnapshotCreateRequest`：只表达“为当前完整 horizon 创建 Snapshot”；首版不允许 caller 指定旧 cursor、阈值、路径或自报 Projection。
- `ReplayRequest`：`recorded` 固定 source target与exact reducer；`simulated` 必须带非空 fixture revisions 和合法 lineage。首版 recorded 可执行，simulated 在 fixture resolver缺失时稳定拒绝且 effect count为零，`reexecute` 不存在于本接口。

所有输入先通过 persisted identity bootstrap、owner authority和exact registry resolution；失败不能通过全局 event/snapshot ID 泄漏另一个 scope 的存在性。

## Projection and reducer contract

`RunTaskProjection` 1.0 输出包含：Kernel读取的`source_store_id`、完整scope、派生lifecycle stream、inclusive `EventCursor(sequence,event_id)`、source SchemaSetRef/ProtocolLimitsRef、exact `ProjectionReducerRef`、exact output `SchemaSetRef/ProtocolLimitsRef`、history-chain state、完整Run Manifest、RunState和按TaskId升序的`{task_id,parent_task_id,state}`。输出不含加载时间、DB path、map iteration order、snapshot provenance或运行时默认。所有provenance字段进入Projection digest；跨store复制相同Event bytes也产生不同digest且不可比较。

Reducer 1.0 复用REQ-0005已批准的完整Manifest admission、identity checks、Run/Task状态边与parent guards。它是纯函数：无I/O、clock、random、environment、process、network或mutable global；内部使用确定性顺序。seed若存在必须通过本Spec全部Snapshot与prefix验证。任何unknown event/version、decode mismatch、gap/reuse/non-positive/out-of-i64 sequence、identity drift或illegal history均返回稳定错误并停止。

`ProjectionReducerDescriptorV1`是纳入版本控制的闭合机器记录，至少包含：reducer kind/major/minor；accepted lifecycle event type/version/variant/payload SchemaRef集合；RunManifest SchemaRef与Manifest admission contract；完整Run/Task transition及parent guard table；Task ordering；history/projection/snapshot digest contract IDs与hash-view SchemaRefs；exact output/snapshot SchemaSetRef、ProtocolLimitsRef及Projection/Snapshot SchemaRefs。`ProjectionReducerRef`固定descriptor SchemaRef和`contract_digest = digest_json("projection-reducer-contract", descriptor_schema_ref, descriptor)`。

Kernel从persisted source SchemaSet中提取exact Manifest与四类lifecycle binding，形成`SourceReducerKeyV1`，只通过纳入版本控制的`source contract -> exact ProjectionReducerRef` registry解析；request或Snapshot不能选择“当前reducer”。所有已被历史Run、Projection或Snapshot引用的descriptor、implementation及output reader必须保留；missing/wrong/current substitution返回`reducer_unavailable`，不能静默升级。

## Digest and hash-view contract

所有摘要复用REQ-0003 `digest_json`：RFC 8785 canonical JSON；每段使用8-byte unsigned big-endian长度前缀；SHA-256 wire form为`sha256:<64 lowercase hex>`。以下闭合1.0 hash-view Schema与golden vectors纳入版本控制，字段absence只能按Schema规定省略，不能用`null`或默认值替代：

- `H0 = digest_json("projection-history-chain-seed", ProjectionHistorySeedV1.schema_ref, {"algorithm":"run-task-history-chain-v1"})`。
- 第i个已验证Event构造`ProjectionHistoryStepV1 { algorithm, previous_digest: H(i-1), sequence, envelope, source_schema_set_ref, source_protocol_limits_ref }`；其中`envelope`是完整canonical EventEnvelope，sequence必须与envelope一致；`Hi = digest_json("projection-history-chain-step", step_schema_ref, step)`。Snapshot保存`Hcursor`，suffix从经prefix重算验证的`Hcursor`继续，因此full与incremental算法逐字节相同。
- `projection_digest = digest_json("run-task-projection", RunTaskProjectionHashViewV1.schema_ref, view)`；view包含source store/full scope/stream/cursor/source set+limits/history chain/reducer/output set+limits/Manifest和排序状态，不包含digest字段自身。
- `snapshot_digest = digest_json("run-task-projection-snapshot", RunTaskProjectionSnapshotHashViewV1.schema_ref, view)`；view包含除`snapshot_digest`自身外的完整Snapshot，包括Projection及其digest。

必须固定empty seed、1 event、N events、prefix+suffix等价、scope/source identity mutation、projection、snapshot与reducer descriptor的checked-in golden vectors；不同平台和重复生成逐字节一致。

## Snapshot contract

`RunTaskProjectionSnapshot` 1.0 是闭合协议记录，包含snapshot/projection Schema、exact output/snapshot SchemaSetRef与ProtocolLimitsRef、scope、store ID、stream、cursor、source SchemaSet/limits、reducer ref、history-chain state、Projection与projection/snapshot digest。数据库另存canonical JSON fingerprint、output set/limits canonical JSON及查询所需extract columns，读取双向比对。output/snapshot registry与source Event registry是两条独立轴；不能用孤立member SchemaRef或current/alternate set验证。

创建流程固定为：

```text
owner target -> BEGIN IMMEDIATE
  -> sequence-1 persisted identity + exact reader
  -> validated complete current lifecycle range
  -> reducer full fold + history/projection/snapshot digest
  -> insert immutable snapshot row
  -> COMMIT
```

首版只支持显式按当前 horizon创建；不自动调度。相同identity/content重试幂等，same key different bytes为integrity conflict。Snapshot不写Event、不改变Manifest/state，transaction rollback不留部分row。

读取流程在一个SQLite read transaction内先固定current horizon，再选择`cursor <= horizon`的最新candidate。先从row的独立canonical columns取得exact output/snapshot SchemaSetRef与limits并由retained registry解析，再验证candidate bytes；candidate必须exact匹配store/scope/stream/source set/limits/reducer/output set+limits/projection schema，canonical bytes、fingerprints、全部digests和cursor对应Event ID/sequence均重验。

candidate自身通过后，reader仍须从Event Store重新读取并逐条exact validate`[1..candidate cursor]`，按上述rolling算法重算`Hcursor`；只有它与Snapshot保存值相同，Snapshot Projection才可作为reducer seed。prefix Event无需再次执行reducer，所以优化仅是“省prefix fold”，不是省Event I/O或protocol validation。然后对`(cursor,horizon]`逐条Event执行row check、exact validation、history-chain step和reducer。

candidate级missing、format/schema invalid、digest/cursor/store/scope/source/reducer/output identity mismatch触发full-history fallback并返回disposition。prefix Event或admission identity损坏、history-chain mismatch、SQLite/Event Store identity/migration/trigger/row corruption不是cache miss，必须fail closed；不得直接返回cached state。fallback不删除/修复Snapshot，不产生Event。

## Replay and comparison contract

- Recorded replay总是忽略Snapshot，从sequence 1到固定末端cursor读取并重新验证完整source lifecycle history，使用exact reducer构建Projection。
- Recorded replay只有reader/reducer依赖，不接受 Effect executor/callback，不append Event，不访问Provider/Tool/network/file/process，也不创建/覆盖Run。未来其他stream的Effect Receipt只能作为数据读取，不能被dispatch。
- Simulated replay与Recorded明确分离：输入来自fixed fixture revisions而非source Event Store结果，且没有live Effect capability。fixture repository不在最小切片，故首版在任何Effect dispatch前返回`simulation_unavailable`；Fake Effect counter必须保持零。
- compare先验证Kernel结果provenance，只有source store ID、tenant/user presence-value/workspace/run/owner actor/stream、source SchemaSet/limits、cursor、history-chain state、reducer及output SchemaSet/limits全部exact相同才比较projection digest并返回`equal | divergent`；否则只返回安全`not_comparable/unauthorized`，不得以`divergent`泄漏另一个scope/store的存在。结果和错误不修改任何持久状态。

## Errors and fallback semantics

稳定类别至少包括：`unauthorized | aggregate_not_found | aggregate_corrupt | schema_unavailable | reducer_unavailable | unsupported_event | invalid_sequence | history_mismatch | snapshot_integrity | snapshot_incompatible | writer_epoch_conflict | optimistic_horizon_conflict | simulation_unavailable | not_comparable | busy | io`。Snapshot candidate错误可降级为带disposition的full fallback；authority、prefix/full Event validation、Event Store或full fold错误不可降级。错误不包含Manifest、payload、SQL、数据库路径、其他scope identity或秘密。

# Impact analysis

| Dimension | Finding | Evidence / response |
|---|---|---|
| Direct | `pareto-protocol`需增加Reducer/Cursor/Projection/Snapshot闭合类型、Schema与新SchemaSet；`pareto-kernel`需把REQ-0005 pure fold提取为可版本化reducer，增加projection/replay路径、snapshot store和SQLite v2 migration；增加真实SQLite与协议测试 | `crates/pareto-protocol/src/{types,schema,validation}.rs`当前无Projection/Snapshot；`event_store/lifecycle.rs:603`已有crate-private pure fold；`event_store.rs`当前`DB_VERSION=1`且只有events/store metadata |
| Indirect | REQ-0007在authority和新增事件处扩展；REQ-0012消费Manifest的Workspace identity；REQ-0015消费带provenance的Projection；REQ-0024需独立Context reducer/projection；REQ-0017未来CLI消费inspect/replay结果 | backlog依赖：REQ-0012/0015直接依赖REQ-0006，REQ-0024依赖Memory；不得把concrete RunTask snapshot提升为通用可写JSON接口 |
| Call/permission | `ValidatedEvent`不是read capability；ProjectionTarget不能由payload/snapshot构造。沿用persisted Manifest owner-only authority，REQ-0007前不支持admin/delegation | REQ-0004 REVIEW-0003 F-001/F-005及REQ-0005 REVIEW-0004证明self-reported authority/reader substitution是Major风险；compile-fail与cross-scope负测 |
| Data isolation | snapshot query/index/JSON/digest必须同时绑定tenant、user presence/value、workspace、run、owner agent/actor、stream和store ID；Task只在aggregate内解析 | 当前events UNIQUE/query使用完整non-NULL user key；增加逐字段swap、换库、same cursor/digest/Task ID payload shadow负测 |
| API/schema | 新Projection/Snapshot JSON、reducer descriptor和四类hash-view会成为持久化合同；task排序、rolling chain、digest preimage、resolver及unknown-event/error semantics难以回退 | 新1.0闭合Schema、empty/1/N/prefix+suffix golden、contract vectors；breaking变化新major并保留old reader/reducer；Kernel authority/SQL仍private |
| Persistence/replay | Snapshot是同DB派生cache而非第二authority；assisted load必须从Event Store exact revalidate prefix并重算独立history chain，只省fold。Recorded replay忽略cache成为oracle | ARCH-0002事件完整性/重放诚实；ADR-0004 exact reader；ADR-0005 state仅由Event fold；prefix corruption和snapshot/full/replay digest equality测试 |
| Concurrency | 锁外build后blind insert会让cursor与state撕裂；普通read也需固定horizon | creation用`BEGIN IMMEDIATE`内完整read/fold/insert；load用单read transaction固定horizon；two-pool barrier测试append/create和append/read |
| Failure/crash | partial Snapshot可误导增量恢复；corrupt cache应fallback，但prefix Event/admission corruption不能被cache遮蔽 | SQLite atomic commit；drop/reopen；candidate corruption fallback；prefix envelope/set/limits/sequence逐项corruption必须fail closed；full replay强制重验历史 |
| Security/effects | replay若调用command/effect路径可能重复Provider/Tool/文件/网络副作用；generic JSON snapshot import可注入state | replay模块只依赖read/reducer，API不接受Effect callback；simulated unavailable before dispatch；dependency/API inspection与fake counter负测 |
| Compatibility/migration | DB v1→v2、source Event Schema、output Schema、Reducer是独立演进轴；已打开v1 writer也不能越过v2 | migration新增snapshot structures及events writer_epoch/default+insert trigger；v2 writer显式epoch 2，old SQL默认1被拒绝；Event JSON/identity保留；old/current reader/reducer substitution和rollback fixtures |
| Performance | full fold与Snapshot create为O(n)；assisted load仍O(n)读取/validate/hash prefix，只省prefix reducer fold，再对suffix O(k) fold；canonical digest与完整JSON增加CPU/bytes | 分别记录Event I/O/validation、reducer fold、writer-lock、rolling-hash和DB bytes的1/10/100/1000观察；不宣称省去history validation，若成本不利再RFC |
| Cost/token | 无模型、Provider或外部Tool调用；Snapshot占本地存储，测试耗时增加 | 分别记录storage/test cost；不宣称Token/费用收益 |
| Dependency/operations | 现有serde/sha2/sqlx足够，预期无需新增第三方依赖；v2 backup仍遵循SQLite WAL机制 | 实施发现新依赖或remote store立即退回影响分析；Cargo/dependency diff由review检查 |
| Documentation | index、EPIC-0002、ARCH-0003需记录approved pending合同；完成时README/status再改为implemented | 本设计阶段只声明planned/approved，不能把Projection写成已实现；实现完成同步durable facts |
| Rollback | 发布前可revert；v2后不能降user_version或去掉writer-epoch trigger，必须保留v2 open、old readers/reducers/output sets。Snapshot功能可停用并忽略，v2首片DELETE trigger禁止删除 | migration/reopen/already-open-v1-writer/trigger drift/历史保留测试；writer rollback策略见RFC-0005/ADR-0006 |

## Direct and indirect call path

```text
Projection/Replay request
  -> owner target admission
  -> persisted lifecycle sequence-1 bootstrap
  -> exact SchemaSet/limits registry
  -> Event Store row consistency + protocol validation
  -> exact reducer
       -> full Projection
       -> optional trusted Snapshot seed + incremental events
       -> Recorded replay full-history oracle
  -> exact digest comparison
```

不存在 `Projection/Snapshot -> Event append/state transition/Effect dispatch` 路径。若实施需要该边，属于设计偏离并阻塞编码。

## Regression scope derived from impact

- Focused：reducer、projection canonical digest、snapshot format/store/create/load/incremental/fallback、recorded replay/comparison、simulated no-effect、migration v2。
- Impacted：protocol Schema/golden/compatibility、Event Store exact reader/migration/append/read/concurrency/reopen、lifecycle Manifest/state/fold/authority。
- Core：trusted-kernel event integrity、isolation、unknown schema/event、crash、replay honesty、API/dependency direction和全部仓库门禁。
- Full：真实Provider、Effect reexecution、remote/distributed Snapshot、CLI与跨平台性能留给后续milestone；本需求不能据此声称端到端Agent replay。

# Compatibility and migration

新增Schema以新内容地址SchemaSet发布并保留REQ-0003/0005全部set；source Event、Projection/Snapshot output分别按persisted exact SchemaSet/limits读取。Projection/Snapshot writer只产生descriptor固定的1.0 output；unknown output set/limits使candidate fallback，missing source reader/reducer或unknown Event使完整Projection/Replay fail closed。所有被历史引用的source/output set、limits、descriptor和implementation按Event历史保留期保留。

SQLite v2 migration在exclusive transaction中保留既有UPDATE/DELETE triggers，执行：为`events`增加`writer_epoch INTEGER NOT NULL DEFAULT 1 CHECK(writer_epoch IN (1,2))`；创建`events_writer_epoch_v2` BEFORE INSERT trigger，在`NEW.writer_epoch != 2`时拒绝；创建`projection_snapshots`、index及固定UPDATE/DELETE拒绝triggers；写全部DDL/trigger checksum，最后设置`user_version=2`。既有rows逻辑epoch为1且Event JSON/append ordinal/sequence/store identity不变；v2 writer insert显式绑定2。迁移前已打开的v1 pool继续使用旧INSERT时得到default 1并被trigger拒绝，不能越过migration。open校验column、epoch和全部trigger SQL/checksum；失败完整rollback。

v2首片不提供Snapshot DELETE或GC；UPDATE/DELETE均由checksum trigger拒绝，“可丢弃”只表示reader可忽略并从Event重建。未来GC需新Requirement/RFC/migration和Kernel-private authorization。代码rollback必须保留v2 open、writer-epoch/snapshot triggers、old source/output readers与reducers；可以停用Snapshot writer/reader但不能降库、删除cache/history或重释Event。

# Test traceability

| Acceptance | Scope/layer | Scenario | Planned evidence |
|---|---|---|---|
| AC-01 | Core contract/security | 只接受Event Store exact validated range；assisted load逐条重验prefix set/limits/envelope/sequence并重算chain；裸Event/Projection/Snapshot、替代reader与public authority无入口 | `cargo test -p pareto-kernel projection::authority --offline`; `cargo test -p pareto-kernel snapshot::prefix_validation --offline`; `cargo test -p pareto-kernel --doc --offline` |
| AC-02 | Focused unit/golden | reducer descriptor/resolver、empty/1/N/prefix+suffix chain及projection/snapshot digest跨顺序/线程/平台一致；old/current reducer substitution拒绝 | `cargo test -p pareto-protocol projection_digest_golden --offline`; `cargo test -p pareto-kernel projection::reducer --offline`; `cargo test -p pareto-kernel projection::reducer_resolution --offline` |
| AC-03 | Focused component/negative | 完整Manifest、Run/Task/parent/state重建；unknown event、wrong major/schema、gap/reuse/0/MAX和illegal history拒绝 | `cargo test -p pareto-kernel projection::full_history --offline`; `cargo test -p pareto-kernel projection::invalid_history --offline`; `cargo test -p pareto-kernel projection::invalid_sequence_schema_and_lifecycle_matrix --offline` |
| AC-04 | Focused protocol/integration | Snapshot闭合格式固定store/scope/cursor/source/reducer/output set+limits/history chain/projection/snapshot digest；孤立member/current set/external import拒绝 | `cargo test -p pareto-protocol projection_snapshot_contract --offline`; `cargo test -p pareto-kernel snapshot::creation --offline`; `cargo test -p pareto-kernel snapshot::output_reader --offline` |
| AC-05 | Core transaction/crash | `BEGIN IMMEDIATE`内create，append竞争无撕裂；transaction drop无row，commit后fresh connection可见；events不变 | `cargo test -p pareto-kernel snapshot::atomicity --offline`; `cargo test -p pareto-kernel snapshot::concurrency --offline` |
| AC-06 | Focused/Core fallback | candidate format/schema/version/digest/cursor逐项fallback；source/output/reducer substitution归入incompatible；prefix envelope/admission identity/sequence受控损坏不得被cached seed遮蔽且必须fail closed；verified prefix+suffix chain等于full | `cargo test -p pareto-kernel snapshot::incremental --offline`; `cargo test -p pareto-kernel snapshot::candidate_failure_matrix --offline`; `cargo test -p pareto-kernel snapshot::prefix_corruption --offline`; `cargo test -p pareto-kernel projection::no_snapshot --offline` |
| AC-07 | Core replay/security | Recorded忽略Snapshot且无append/effect；Simulated fixture resolver缺失在dispatch前拒绝，Fake Effect计数0；无reexecute API | `cargo test -p pareto-kernel replay::recorded_read_only --offline`; `cargo test -p pareto-kernel replay::simulated_no_effect --offline`; API/dependency inspection |
| AC-08 | Focused replay/golden | same store/full provenance normal与recorded相同digest；两库同bytes clone及scope/source/output/cursor/history/reducer任一差异not-comparable/unauthorized，只有provenance相同digest差异才divergent | `cargo test -p pareto-kernel replay::recorded_determinism --offline`; `cargo test -p pareto-kernel replay::digest_equivalence --offline`; `cargo test -p pareto-kernel replay::cross_store_not_comparable --offline`; `cargo test -p pareto-kernel replay::comparison_matrix --offline` |
| AC-09 | Core security/isolation | tenant/user presence-value/workspace/run/actor/stream/store/source/output逐字段swap，cross-run Snapshot/cursor/digest/Task ID和payload shadow全部拒绝且不泄漏 | `cargo test -p pareto-kernel projection::isolation --offline`; `cargo test -p pareto-kernel snapshot::snapshot_lookup_isolation_matrix --offline`; `cargo test -p pareto-kernel replay::comparison_matrix --offline` |
| AC-10 | Core concurrency/recovery | read与append得到固定horizon；snapshot-create与append单一胜序；close/reopen后snapshot增量；中断后full source恢复 | `cargo test -p pareto-kernel projection::concurrency --offline`; `cargo test -p pareto-kernel snapshot::recovery --offline` |
| AC-11 | Impacted migration/rollback | new v2、含两条exact lifecycle历史的真实v1→v2、每个v2 DDL阶段失败rollback、actual table CHECK/UNIQUE/type与index-order drift、newer version拒绝；迁移前保持open的v1 writer在v2后append被epoch trigger拒绝；v2 writer成功，Event/Manifest bytes与old triggers保留 | `cargo test -p pareto-kernel snapshot::migration --offline`; `cargo test -p pareto-kernel snapshot::migration_rolls_back_each_v2_ddl_stage_with_history_intact --offline`; `cargo test -p pareto-kernel snapshot::snapshot_actual_ddl_drift_is_rejected --offline`; `cargo test -p pareto-kernel snapshot::already_open_v1_writer --offline` |
| AC-12 | Impacted golden/compatibility | 新set重复生成一致且旧set无diff；显式source-key→reducer/output/implementation allowlist保留old contract；registry顺序不影响选择；unrelated-set evolution沿用相同source key，changed/current/alternate/missing substitution拒绝或candidate fallback | `cargo test -p pareto-protocol --all-targets --all-features --offline`; `cargo test -p pareto-kernel projection::reducer_resolution --offline`; `cargo test -p pareto-kernel projection::compatibility --offline`; `cargo test -p pareto-kernel snapshot::candidate_failure_matrix --offline`; schema generation diff |
| AC-13 | Impacted/Core regression | REQ-0003、REQ-0004、REQ-0005所有现有测试与全仓静态/治理门禁通过 | `cargo test -p pareto-kernel event_store --offline`; `cargo test -p pareto-kernel lifecycle:: --offline`; `cargo test --workspace --all-targets --all-features --offline`; completion gates |
| AC-14 | Core static/scope | 无Capability/Hook/Effect executor/Provider/Agent Loop/Memory/DAG/distributed/remote实现或新依赖；只有最小纵切 | exact diff/API/dependency review；`git diff --check`; independent REVIEW-0005 |

# Approval

RFC-0005因跨协议、数据库、reducer与replay边界且难以回退而必须存在。独立架构评审首轮提出IAR-F-001至IAR-F-006 Major；作者按可测试合同修订prefix trust、digest algorithm、reducer resolver/retention、output reader、comparability和writer epoch，全部finding仅由同一独立reviewer在focused re-review中关闭。最终记录为approved、0 open Blocker/Major，RFC/ADR/Spec可进入accepted/approved与实施Plan。实施后仍必须使用`code-review`由fresh Agent/session检查exact代码diff和VALIDATION；两轮评审不能互相替代。

# Open questions

无阻塞实施的设计问题。自动Snapshot策略、fixture repository、Snapshot GC、cross-stream/effect replay、public inspect API、remote store与generic projection framework均明确延期；若最小切片实施证明现有concrete contract不足，必须先更新本Spec/RFC而不是在代码中隐式扩张。
