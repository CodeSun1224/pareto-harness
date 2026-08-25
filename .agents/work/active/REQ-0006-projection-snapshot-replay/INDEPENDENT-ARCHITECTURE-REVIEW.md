---
title: REQ-0006 Projection、Snapshot 与 Replay 独立架构评审
status: approved
owners: [independent-reviewer]
created: 2026-08-24
updated: 2026-08-24
links: [REQ-0006, SPEC-0005, RFC-0005, ADR-0006, REQ-0003, REQ-0004, REQ-0005]
independence: independent
reviewed_baseline: bb395ad78f762b53d5f486c742194dd8d551dc61
open_blockers: 0
open_majors: 0
---

# Reviewed subject

本评审由未参与 REQ-0006 文档编写的独立 reviewer 执行。首轮被审对象固定为 Git HEAD
`bb395ad78f762b53d5f486c742194dd8d551dc61` 加下列未跟踪文档的精确字节：

| Subject | SHA-256 |
|---|---|
| `docs/requirements/REQ-0006-projection-snapshot-replay.md` | `02c70ed80ed7781e442302fe4e23d0995044c341bfc3d4abe5c95a61c9022c47` |
| `docs/specs/SPEC-0005-projection-snapshot-replay.md` | `4b924a84ab412d1df86cd6fdfdd05d42cc5248b28c00c3377c92b8ad7f49ce36` |
| `docs/rfcs/RFC-0005-projection-snapshot-replay-contract.md` | `22cade24f6570f4459b15df695f30ac04c794d2a0d5f0cdd97c5ce516623f517` |
| `docs/adrs/ADR-0006-versioned-projection-snapshot-recorded-replay.md` | `b9bbbc7043f05348301a913db20afa0de6d0edd7a3ad753c8de30a66c52f3da3` |

评审实际对照了 REQ-0003/0004/0005 的 Requirement、Spec、RFC、ADR、独立 Review 与归档工作证据，读取了 `pareto-protocol` SchemaSet/ValidatedEvent 合同、SQLite Event Store v1 migration/trigger/reader 实现、Run/Task lifecycle pure fold、authority 与相应真实 SQLite 测试，而不是依据提案图批准。

Focused independent re-review 检查了 `ARCHITECTURE-REMEDIATION.md` 的逐 finding 声明，并以实际文档文字与 AC→test mapping 为准复核下列 exact bytes：

| Focused re-review subject | SHA-256 |
|---|---|
| `docs/requirements/REQ-0006-projection-snapshot-replay.md` | `4ffe8b85f215e577d1ae3391afe3df12404cfdc0e552312d419f141bd36494c4` |
| `docs/specs/SPEC-0005-projection-snapshot-replay.md` | `bd7577bd52b9bfa2c0253398be597b7165ad8185e53206a1e002d42dd47973d7` |
| `docs/rfcs/RFC-0005-projection-snapshot-replay-contract.md` | `b0022117eaac3fd7e51d172b67f3cafd7b7669b1d00d1510242e9f17952782ea` |
| `docs/adrs/ADR-0006-versioned-projection-snapshot-recorded-replay.md` | `33e8ee86886fd791e84db3f0edd1abd38bc8c73fdde9069b5913bbcd2b1fe092` |

# Verdict

**Focused design remediation approved.** 当前为 0 open Blocker、0 open Major。上述 focused re-review exact bytes 已充分关闭 IAR-F-001 至 IAR-F-007；RFC-0005 可转为 `accepted`，ADR-0006 的 decision 可最终确认，SPEC-0005 可转为 `approved`，REQ-0006 可转为 `planned`。只有这些 lifecycle/status 修改完成并由本 reviewer 对最终 bytes 作 freshness-only 确认后，才可把最终 status bytes 当作实施入口；该确认不得改变 finding disposition或设计合同。

既有基础实现本身仍健康：首轮独立复跑 `cargo test -p pareto-kernel event_store --offline` 为 33/33 通过，`cargo test -p pareto-kernel lifecycle:: --offline` 为 18/18 通过。绿灯证明 v1 Event Store/lifecycle 基线成立；focused approval证明修订后的设计合同可进入最终状态确认，不是尚未实现 Runtime 的测试证据，也不替代实施后的独立 code review。

# Findings

| ID | Severity | Location | Finding、违反的不变量与影响 | 可测试修正 | Status |
|---|---|---|---|---|---|
| IAR-F-001 | Major | RFC-0005 §4 lines 63-71；SPEC-0005 lines 53-55；REQ-0006 AC-01/AC-06 | Snapshot-assisted load 明确“不重新验证 cursor 以前的每条 Event”，但它只重算 Snapshot 内自带的 fingerprint/digest，并把同一行自报的 `source_history_digest` 当作前缀证明。v1 `events` 没有事件哈希链、Merkle/accumulator root 或由 Snapshot 之外的权威 anchor；`store_id + Kernel-only creation + 两张表的 trigger` 证明创建路径，不证明加载时 `[1..cursor]` 仍与 seed 相同。这会让 Snapshot 遮蔽前缀 Event/Schema/sequence 损坏，违反 Event Store 唯一权威输入、每个权威输入经 exact reader 验证以及“Event history 损坏始终 fail closed”。 | 在 RFC/Spec 冻结一个可验证信任根：首片可选择每次 assisted load 重新读取、exact validate 并重算 `[1..cursor]`（只省 reducer fold），或另立受审的 append-transaction-bound authenticated prefix accumulator/anchor；不能由 Snapshot 自证 Snapshot。增加真实 SQLite 负测：创建 Snapshot 后逐项改变 prefix envelope、stored admission identity、sequence/cursor（含结构仍可读的受控 corruption fixture），assisted load 必须检测并 fail closed/fallback，绝不能直接返回 cached state。 | closed |
| IAR-F-002 | Major | RFC-0005 §2 lines 30-41、§3 lines 45-55；SPEC-0005 lines 30-34 | `source_history_digest` 只被描述为“按 sequence 覆盖完整 EventEnvelope 及 admission identity”，没有冻结 domain、元素边界、初始值、链式/平坦组合、SchemaSet/limits 的逐事件表示与 canonical preimage。若采用常见的整段 canonical-array SHA-256，Snapshot 只保存最终 digest 时无法仅凭该 digest 对后续 Event 增量计算 `[1..horizon]` digest；不同实现也可产生不同合法解释。projection/snapshot digest 与 `contract_digest` 同样缺少完整机器可生成 descriptor/vector。持久 Snapshot 和 comparability 因而没有跨版本确定身份。 | 在 Spec/RFC 给出逐字节算法或版本化闭合 hash-view Schema：明确 UTF-8 length-prefix/domain、所有字段、absence、顺序、初始值和增量步骤；若采用 rolling chain，固定 `H0`/`Hi`，若采用平坦摘要则 Snapshot 必须保存可安全恢复的版本化 accumulator state 或重读前缀。为 empty/1/N/prefix+suffix、scope/source identity mutation、projection/snapshot/reducer descriptor 固定 checked-in golden vectors，并证明 full 与 incremental 逐字节相同。 | closed |
| IAR-F-003 | Major | RFC-0005 §2 line 30、§4/§7；SPEC-0005 Inputs 与 Compatibility | `ProjectionReducerRef.contract_digest` 的“稳定 reducer contract preimage”未定义，full Projection 又没有冻结 reducer 选择规则。`ProjectionRequest` 禁止 caller 选择 reducer，但文档没有规定 Kernel 如何从 persisted source contract exact resolve reducer，也没有规定已引用 reducer descriptor/implementation 的保留与 current-substitution 拒绝。以后 REQ-0007 新增 lifecycle event 或修复 reducer 时，同一旧 Run 可被新二进制隐式选择“当前 reducer”，或旧 Snapshot 只能失效却无法用其原 reducer重建，违反版本身份和不得原位重释历史。 | 定义不可变 reducer descriptor（至少绑定 accepted event type/version/payload Schema、Manifest admission、transition/parent guards、ordering、projection Schema 与 digest contract），固定 descriptor 的 digest preimage；定义 `source SchemaSet/lifecycle contract -> allowed exact reducer ref` 的 fail-closed resolver和请求/结果 provenance，规定所有被 Event/Projection/Snapshot 引用的 reducer reader保留。测试旧 Run/旧 Snapshot在新 registry下 exact resolve原 reducer，missing/wrong/current reducer substitution均 fail closed/not comparable，而不是静默升级。 | closed |
| IAR-F-004 | Major | RFC-0005 §3/§4；SPEC-0005 lines 38、53、114-116；REQ-0006 AC-04/AC-12 | Snapshot 持久合同只固定 Snapshot/Projection 的 member `SchemaRef` 和 source Event 的 `SchemaSetRef/ProtocolLimitsRef`，没有固定“哪个 admitted SchemaSet + limits”验证 Snapshot/Projection 自身。现有 REQ-0003 实现以完整 admitted `SchemaSet` 建立 validator/member/compatibility trust，不把孤立 member SchemaRef 当完整 reader identity。未来多个 retained set 共存时，实现只能用当前 set、全局搜 member，或写下未规范的隐式 registry选择，都会绕过 exact historical reader合同。 | 在 Snapshot row/closed record或不可歧义的 store metadata中显式绑定 output/snapshot `SchemaSetRef` 与适用 `ProtocolLimitsRef`（与 source event set分轴），定义 bootstrap/retained registry exact resolution和兼容失效语义。增加两个包含相同/兼容 member外观但不同 manifest identity 的 retained set fixture，证明 exact old output reader成功、current/alternate/missing set拒绝或安全 fallback。 | closed |
| IAR-F-005 | Major | RFC-0005 §2 line 41、§5 line 81、§6 line 88；SPEC-0005 lines 30/62/75；REQ-0006 AC-08/AC-09 | Snapshot 绑定 `store_id`，但 `RunTaskProjection` 明确不含 store identity，comparison 只要求 cursor/history/reducer/projection Schema相同。两个不同 SQLite store 若复制相同 scope/Event bytes/cursor，可被判 `equal`；scope/stream/source SchemaSet/limits也不是 comparability gate的显式项。这与 AC-09“换库、跨完整隔离域不得比较”直接矛盾，也让 cursor/digest 充当跨 store capability。 | 定义不可伪造的 Kernel result provenance，并在 comparability gate exact 比较 store identity、tenant/user presence-value/workspace/run/owner actor/stream、source SchemaSet/limits、cursor、history digest、reducer和output schema；决定这些字段是否进入 projection digest必须一致写明。增加两库同字节clone、逐字段scope swap、same cursor/digest和payload shadow测试，全部返回安全 `not_comparable/unauthorized`，不得 `equal/divergent` 泄漏。 | closed |
| IAR-F-006 | Major | RFC-0005 §6 lines 85-87、§Compatibility lines 133-137；SPEC-0005 AC-11/migration；当前 `event_store.rs:296-400` | “`BEGIN EXCLUSIVE` migration + newer-version open拒绝”不足以兑现“旧 v1 binary 不得写 v2”。当前 v1 binary只在 open/migrate 时检查 `user_version`；一个迁移前已经打开的 v1 pool，在 v2 migration commit后仍可继续向未变的 `events` 表追加。SQLite WAL 下 `BEGIN EXCLUSIVE` 与 `BEGIN IMMEDIATE` 都只在事务期间阻止其他 writer，并不会永久撤销旧连接；本设计没有 process lease/schema epoch或每次append版本检查。即使首片单进程，rollback/兼容合同已作出了无法由当前机制保证的旧 writer声明。 | 明确单进程 open/migration coordination边界，并采用可执行防护：例如受控进程级独占生命周期，或每个 append/lifecycle write在同一事务中验证 schema epoch/user_version并由 v2 writer identity授权；不能只依赖迁移时锁。真实测试保留一个 migration 前已打开的 v1 writer，完成 v1→v2 后尝试 append，必须 fail closed；同时证明 v2 writer、Event triggers/bytes、rollback和reopen正常。若明确不支持并存旧进程，则收窄承诺并写出升级前关闭/锁定协议及可测试 operational gate。 | closed |
| IAR-F-007 | Minor | RFC-0005 §3 line 59、SPEC-0005 migration contract | `projection_snapshots` 只声明拒绝 UPDATE；“immutable row”、未来 GC 和当前无 public delete之间的 DELETE/trigger政策没有冻结。不同实现可能禁止所有 DELETE、允许 Kernel-private GC，或允许任意 crate-private raw deletion，影响 rollback、测试与未来保留合同。 | 明确 v2 对 DELETE 的 DDL/权限合同：首片是否用 trigger拒绝、仅受控 migration/未来 GC token可删，或允许任意完整row删除；为选定政策增加 trigger checksum/open drift及事务测试。 | closed |
| IAR-F-008 | Note | RFC-0005/SPEC-0005 status metadata与Approval段 | 文档在独立架构评审前已标为 `accepted/approved`，并以非独立自审“关闭”Major。非独立自审可作为作者检查，但不能预先代表本次独立批准。 | 作者修订 open Major 后更新 exact hashes；由独立 reviewer focused re-review。只有 re-review 0 open Blocker/Major 后再最终确认 RFC accepted、ADR accepted、Spec approved，并在记录中区分作者修订与 reviewer finding closure。 | closed |

## Focused re-review closure evidence

- **IAR-F-001 closed**：REQ AC-01/06、Spec Snapshot contract与RFC §4现在要求 candidate通过后仍从Event Store逐条exact reread/validate/hash `[1..cursor]`。candidate自身错误才fallback；prefix envelope/admission/sequence/history mismatch明确是Event failure并fail closed。`snapshot::prefix_validation`与`snapshot::prefix_corruption`映射覆盖该区分。
- **IAR-F-002 closed**：RFC §2.1与Spec Digest contract逐字冻结`digest_json`、domains、闭合seed/step/hash-view、`H0`、`Hi`、previous digest、envelope、source set/limits、absence与full/prefix+suffix golden。Snapshot保存并从经权威prefix重算的`Hcursor`继续，增量算法可计算且与full同构。
- **IAR-F-003 closed**：`ProjectionReducerDescriptorV1`机器记录绑定event/Manifest/state/parent/order/digest/output合同；`SourceReducerKeyV1 -> exact ProjectionReducerRef`只由persisted source contract解析。历史descriptor、implementation和output reader要求保留，missing/wrong/current substitution稳定拒绝；`projection::reducer_resolution`与compatibility矩阵可验证。
- **IAR-F-004 closed**：Projection/Snapshot record及独立row columns分别固定source与exact output/snapshot SchemaSetRef/ProtocolLimitsRef；读取先exact resolve retained output set再验证record，不允许孤立member/current/alternate set替代。`snapshot::output_reader`覆盖retained/alternate/current/missing矩阵。
- **IAR-F-005 closed**：Projection与digest纳入Kernel-read store ID和full provenance；comparison在digest前exact gate store/scope/stream/source/cursor/chain/reducer/output identities，cross-store clone与逐字段swap只返回安全not-comparable/unauthorized。AC-08/09及`replay::cross_store_not_comparable`形成闭环。
- **IAR-F-006 closed**：修订没有把SQLite `BEGIN EXCLUSIVE`误当永久writer lease。v2在新RFC授权下保留既有Event triggers/bytes并增加`writer_epoch DEFAULT 1`与BEFORE INSERT epoch-2 trigger；v2 SQL显式写2，already-open v1旧SQL省略列而取1，因trigger稳定rollback。migration/checksum/open、`writer_epoch_conflict`和held-open v1 writer测试均明确；这满足RFC-0003“物理变化需新RFC且不得drop/bypass既有trigger”的演进条件。
- **IAR-F-007 closed**：Snapshot UPDATE/DELETE均由checksum trigger拒绝，首片无delete/GC API；“discardable”只表示reader忽略并从Event重建，未来GC需新Requirement/RFC/migration/authorization。
- **IAR-F-008 closed**：REQ/SPEC/RFC已分别退回`specified/draft/proposed`，正文明确独立门禁前不得实施。本次focused re-review由独立reviewer而非作者关闭finding。ADR类型在当前治理Schema只允许`accepted/superseded`；本次review对其exact decision bytes作实质确认。最终RFC/SPEC/REQ status更新仍需下述freshness-only确认。

# Architecture trace

## Authority、effect 与 recovery

- 现有可信路径是 persisted sequence-1 identity → exact retained SchemaSet/limits → `validate_row` → `fold_lifecycle`；代码确认 `ValidatedEvent` 不是 read/write capability，lifecycle target同时绑定 `IsolationScope` 与 actor。
- Proposed Recorded replay 的 reader/reducer-only依赖、无 Effect callback/append以及 Simulated 在 fixture resolver前拒绝，方向正确；本轮未发现 Effect 可达路径。后续代码评审仍必须以依赖/API检查和 fake counter负测证明，而不能以文字声明替代。
- `BEGIN IMMEDIATE` 对 Snapshot create 在单 SQLite文件内能先获得 writer reservation，因而可与 append串行；单个显式 read transaction在第一次 SELECT 后能固定 WAL read snapshot。prefix exact reread/hash和writer-epoch trigger分别关闭了该事务语义本身不能解决的历史trust与old-live-writer问题。
- crash前rollback/commit后完整row的SQLite原子边界方向正确；prefix corruption现在被归为Event failure而非Snapshot fallback，缓存不能遮蔽恢复失败。

## Compatibility、isolation 与 evolution

- v2通过本新RFC增加Event writer-epoch列/trigger及Snapshot结构，保留既有UPDATE/DELETE triggers、Event JSON/ordinal/sequence/source identity；DDL/trigger checksum、atomic rollback与already-open writer负测使其满足RFC-0003要求的新RFC/forward migration门禁。
- owner-only authority、完整tenant/user presence-value/workspace/run/agent/actor/stream/store绑定已进入Projection result、digest、Snapshot query与comparison；cross-scope/store不以divergent泄漏。
- REQ-0012/0015/0024使用独立事实/Projection、不得把concrete RunTask Snapshot当通用JSON escape hatch的边界正确。REQ-0007新增lifecycle Event必须新增source mapping/reducer revision，不能重释旧Run。

## Quality、cost 与 latency

- 质量 oracle（full fold/Recorded replay）、无模型费用以及 1/10/100/1000 event本地观察被分别列出，没有把目标写成已证优化。
- Snapshot create在writer lock内O(n)验证；assisted load也明确保持O(n) prefix I/O/validation/hash，只观察prefix reducer fold节省。评测按读取验证、fold、rolling hash、writer lock、suffix与Recorded replay分别记录，未虚构总延迟收益。
- 本评审不批准任何质量、成本或延迟优化声明；实现后必须给 named baseline、环境、事件规模和原始观察。

# Final freshness confirmation

**Freshness approved.** 最终正式设计字节固定为：

| Final subject | SHA-256 |
|---|---|
| `docs/requirements/REQ-0006-projection-snapshot-replay.md` | `4631646938831e27f96fc2d3e8e63e7958c58ccdcab411654d643aef766d37a7` |
| `docs/specs/SPEC-0005-projection-snapshot-replay.md` | `a3e4231fdbcd5cc0a71ed4aa2eb32716d203cb1c50dd28258c834dca8ba9e180` |
| `docs/rfcs/RFC-0005-projection-snapshot-replay-contract.md` | `e80d1914f535683da21a6ec1a5e84968414f06e17d8105213322595242a2ef05` |
| `docs/adrs/ADR-0006-versioned-projection-snapshot-recorded-replay.md` | `33e8ee86886fd791e84db3f0edd1abd38bc8c73fdde9069b5913bbcd2b1fe092` |

相对focused-approved subjects，REQ仅将`specified -> planned`；SPEC仅将`draft -> approved`并把开头/Approval改为如实记录本独立focused approval；RFC仅将`proposed -> accepted`并同步Evaluation的design-approval事实；ADR bytes完全不变。独立逆向hash检查确认REQ还原status后精确得到已批准`4ffe8b85...`，RFC还原status与原design-approval bullet后精确得到已批准`b0022117...`。实际抽查确认prefix/Event-failure区分、rolling H0/Hi、exact reducer/output reader retention、full-provenance comparison、writer epoch、Snapshot DELETE政策以及AC-01至AC-14 test mapping均未改变；IAR-F-001至F-008 closure保持有效。

REQ-0006现在可按approved Spec/RFC/ADR与Plan进入Runtime实施。实施完成后仍须由fresh independent code reviewer检查exact代码、Schema、DDL、migration、测试与VALIDATION证据；当前design/freshness approval不能替代该门禁。

# Re-review history

- 2026-08-24：首轮独立架构评审固定baseline `bb395ad78f762b53d5f486c742194dd8d551dc61`及四份初始SHA-256；0 Blocker、6 Major、1 Minor、1 Note，Verdict为changes requested。
- 2026-08-24：focused independent re-review检查`ARCHITECTURE-REMEDIATION.md`和四份新exact bytes（REQ `4ffe8b85...`、SPEC `bd7577bd...`、RFC `b0022117...`、ADR `33e8ee86...`），逐项核对实际文字与AC→test mapping。IAR-F-001至F-007 closed，F-008 process Note closed；最终0 open Blocker、0 open Major，focused design remediation approved，等待final status bytes freshness-only确认。
- 2026-08-24：final freshness-only review检查最终四份SHA-256与实际status/approval文字；REQ/RFC逆向hash精确回到focused-approved bytes，SPEC变更限定为status和独立批准事实，ADR byte-identical。设计合同、AC→test mapping和finding closure未变；freshness approved，0 open Blocker/Major，允许按Plan开始实施但不替代后续独立code review。
