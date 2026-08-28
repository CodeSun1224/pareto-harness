---
id: SPEC-0007
title: Observer、Gate 与 Transform Hook 骨架规范
status: draft
owners: [runtime-kernel]
created: 2026-08-28
updated: 2026-08-28
links: [REQ-0008, EPIC-0002, REQ-0004, REQ-0007, RFC-0007, RFC-0008, ADR-0008, ARCH-0002, ARCH-0003, ARCH-0004, REVIEW-0010]
---

# Behavioral contract

Hook 是可信 Kernel 编排的非权威扩展调用。Kernel 固定注册、顺序、输入视图、bounded invocation authority、预算、取消/deadline、输出重新验证、组合和最终 Event append；Hook handler只能observe、返回Gate判定、转换显式允许的非权威proposal，或提出请求。Hook不能直接取得Event transaction、Manifest mutable handle、Capability/lease constructor、budget state、timeout recovery authority、Effect/Evidence admission或lifecycle terminal authority。

一个Run的Hook配置必须在sequence-1 Manifest可验证地固定为不可变`HookRegistryRevisionV1`引用；Hook运行事实进入由Run ID派生的单一Hook stream。Recorded replay只折叠该stream，不持有handler或writer。REFERENCE实现为进程内Rust Fake handler，但公共合同不得要求Rust ABI或预选transport。

# Inputs, outputs, states, and failure behavior

## Hook types, points, and registration

`HookRegistrationV1`闭合固定：`hook_id`、`hook_revision`、`config_digest`、`kind`、允许的`hook_points`、`priority`、Gate的`required`、Observer专用failure policy、transform contract、trusted operation contract ref、input/output SchemaRef、limits、redaction policy及handler compatibility identity。Gate固定fail closed，Transform固定reject-whole-proposal，二者没有failure-policy字段。Registry按`(hook_point ordinal, phase ordinal, priority, hook_id, hook_revision)`升序规范排序；所有向量排序唯一，重复logical ID/revision、相同完整排序键、unknown kind/point/phase/major、kind不适用字段、缺handler或Schema均拒绝初始化。

最小纵切只冻结以下抽象point，不提前绑定Provider/Tool transport：

| Hook point | Observer | Gate | Transform | 输入事实地位 |
|---|---|---|---|---|
| `before_proposal_admission` | yes | yes | yes | 未提交proposal；Transform只可改transform mask允许字段 |
| `after_proposal_admission` | yes | no | no | 已准入决定，只读 |
| `before_authoritative_commit` | yes | yes | no | Kernel待提交决定的脱敏只读view；Gate不能改payload |
| `after_authoritative_commit` | yes | no | no | 已提交Event摘要与cursor，只读 |

后续Requirement新增point必须新minor/major、明确数据事实地位与每类允许矩阵。`after_*` Observer失败不能回滚已提交Event；其`fail-closed`只阻止后续阶段并记录结构化失败。

每个point的phase是持久合同而非注册者选择：`before_proposal_admission = Transform -> Gate -> Observer`，`before_authoritative_commit = Gate -> Observer`，两个`after_* = Observer`。所有phase串行；priority只在同一phase内排序，不能把Observer/Gate插到Transform pipeline中。point开始时固定`source_cursor`、初始proposal/view bytes与`initial_input_digest`；Transform n的`input_digest`必须等于point初始digest或Transform n-1的已验证`output_digest`。Transform phase成功后固定`final_input_digest`；所有Gate接收相同final input，Observer只在Gate/无Gate业务决定固定后接收final input及只读`HookPointDecisionV1`。Transform失败或Gate deny会规范skip所有后续phase，Kernel仍记录skip和point finalization，不调用Observer。Observer输出绝不回写已经固定的业务决定；Observer `fail-closed`只把分离的point execution status标为`observer_failed`并阻止尚未开始的下游阶段，不能把allow改成deny、deny改成allow或回滚已提交事实。`HookPointFinalizedV1`分别记录immutable business decision与execution status，禁止合并解释。

只有`before_proposal_admission`和`before_authoritative_commit`是Gate-bearing point，且每次执行都必须至少有一个`required=true` Gate；零Gate或仅optional Gate稳定deny。Registry不存在`gate_requirement=none`或其他绕过字段。两个`after_*` point从类型矩阵上禁止Gate，不进行Gate-required检查。

## Invocation and decision state machine

每个Hook调用固定`HookInvocationKeyV1 { scope, hook_point, phase, hook_id, hook_revision, subject_proposal_id, ordinal, source_cursor, input_digest, predecessor_output_digest?, attempt }`。Kernel从该闭合key、Manifest-pinned registry与command kind派生稳定invocation/command/event ID；retry复用完整bytes，不能重取顺序、Clock、config、lineage或identity。

状态为`reserved | succeeded | failed | cancelled | timed_out`，四个terminal互斥。Hook stream至少包含初始化、point-start/input-lineage、invocation-reserved、result/decision-terminal、skipped、point-finalized、late-result-observed和message-rejected事件。`HookPointFinalizedV1`绑定source cursor、初始/最终input digest、规范phase、有序invocation/terminal/skip identity、Gate component decision IDs和final outcome；它在全部需要的invocation terminal后由Hook-only command追加。budget reservation仍属于REQ-0007 control stream，Hook event必须引用exact operation/reservation/lease authority，不复制或替代budget事实。

## Kernel call path and authority

```text
authenticated request
  -> persisted Manifest + exact lifecycle/control/hook bootstrap
  -> reconstruct tenant/user/workspace/run/task/owner/subject
  -> resolve exact Hook registry/revision/config/input/output reader
  -> BEGIN IMMEDIATE
     -> fold lifecycle + Runtime Control + Hook histories
     -> lifecycle/cancellation/deadline/default-deny Capability checks
     -> retained Hook trusted operation contract
     -> HookReservePairCommandV1: atomic control reserve + Hook invocation-reserved
  -> opaque bounded HookInvocationLease + redacted input view
  -> Fake handler returns untrusted result
  -> BEGIN IMMEDIATE
     -> refold and verify lease/producer/attempt/deadline/terminal
     -> limits + Schema + scope + transform-mask/result semantics
     -> HookTerminalPairCommandV1: atomic control settlement + Hook result/decision-terminal
     -> only then Kernel may continue to authoritative commit
```

Hook lease只包含本次read-only input与提交结果所需的最小能力，不可序列化、不可委托，且不能访问raw Event、DB、filesystem、network、process或secret。Kernel不信任handler回传的scope、principal、usage、version或decision identity。

## Observer, Gate, and Transform semantics

- Observer输出只有`observed | warning | failure`及脱敏annotation摘要；永远不进入业务组合值。`warn-and-continue`记录告警后继续；`fail-closed`阻止后续未提交阶段。Observer返回deny、mutation、cancel或authority字段均为非法输出。
- Gate输出只允许`allow | deny {reason_code} | abstain`。组合按规范顺序执行；第一个有效deny终止剩余Gate及Observer phase，并为其记录规范skip reason。若无deny，则每个`required=true` Gate必须allow；required abstain、任何失败/timeout/invalid/unknown均形成安全deny。optional abstain不贡献allow。Gate-bearing point的空required集合（包括零Gate或仅optional Gate）稳定deny，没有显式none例外。
- Transform只接受并返回`TransformProposalV1`。每个point的闭合`TransformContractV1`固定allowed JSON-pointer-like field identifiers、字段Schema、最大输出和组合规则；首片按稳定顺序串行应用，每一步输出重新验证后才成为下一步非权威输入。任何失败固定拒绝整个proposal并skip后续Transform/Gate/Observer，不返回原proposal继续、不保留部分权威修改；中间结果只作审计摘要，不成为事实源。
- 保护字段集合是denylist之外的强制不可变hash-view：Event/Manifest/Schema/registry/identity/scope/principal/authority/Capability/grant/lease/budget/allocation/usage/deadline/cancellation/Receipt/Evidence/terminal及任何未知字段全部禁止。Kernel比较before/after保护hash-view，mask遗漏不会变成允许。

## Budget, cancellation, timeout, and recovery

每个invocation是REQ-0007受保护operation。trusted contract根据Hook kind、point、input size与版本产生完整finite envelope；reserve在handler调用前原子覆盖Run/Task/Actor/per-operation accounts。Handler只提交observation，Kernel meter为首片唯一verified usage。失败/取消/timeout也settle实际verified usage并release差额；meter未知保守consume全部reservation。

Run/Task/operation cancellation ancestor predicate直接适用。cooperative Fake handler持只读probe；uninterruptible/hung handler在返回或absolute deadline前不宣称停止。Hook operation的返回、取消和timeout recovery必须进入同一个Hook专用terminal pair入口；它在同一transaction调用REQ-0007 transaction-local settlement admission并追加Hook terminal，不能先调用会自行commit的单stream Runtime Control API。重启时pending invocation不得自动release/reexecute，必须从Projection枚举并由持有persisted TimeoutKey、Fake Clock和recovery authority的Kernel显式提交`HookTerminalPairCommandV1`；exact retry幂等，handler不重跑。

## Atomic control/Hook pair commands

`HookReservePairCommandV1`和`HookTerminalPairCommandV1`是crate-private值，不是protocol API或handler能力。每个命令闭合固定`pair_id`、`pair_kind`、canonical full-command fingerprint、完整scope/owner、control/hook stream ID与expected inclusive cursor/next sequence、两个确定性Event ID/sequence及完整prepared Event bytes；两个Event payload互相绑定pair ID、counterpart Event ID、operation/invocation/reservation identity。reserve pair的control Event是预算reservation，Hook Event是invocation-reserved；terminal pair的control Event是唯一operation settlement，Hook Event同时保存已验证result/component decision与invocation terminal。

在单个`BEGIN IMMEDIATE`内，Kernel按以下不可交换优先级处理pair：Event完整性/Schema/隔离 → pair ID exact/mutation → 两stream现有状态 → expected cursor与领域准入 → 依次insert control、Hook → commit。zero-existing时两个expected cursor都必须命中，任一validation/insert/commit失败rollback且两边均不可见；two-existing只在两个Event bytes、fingerprint和交叉引用全部exact时返回原pair结果；同pair ID不同bytes为mutation conflict；one-existing即使单边历史连续、摘要和签名均有效也视为`corrupt_partial_pair`，绝不补写、调用handler或继续point。different pair ID竞争由writer序列化和expected cursor决定唯一winner。commit response loss必须用相同pair bytes重试，不可重建Clock/identity。

Hook受保护operation的所有terminal只允许terminal pair入口，通用单streamcompletion/cancellation/timeout入口在看到Hook operation binding时fail closed。这样control terminal与Hook terminal没有合法单边catch-up状态；reopen发现one-existing只报告损坏并阻止恢复。`HookPointFinalizedV1`不结算budget，在所有terminal pair已验证后单独、幂等追加到Hook stream。

## Ordering, idempotency, failure, and audit

单stream point/late/audit command的完整优先级：integrity/source/version → isolation/authority → same command ID exact/mutation → existing terminal → lifecycle/cancellation/deadline → handler result limits/Schema → kind-specific semantics → combination → append；pair command使用上一节更严格优先级。same-ID exact返回原结果；mutation冲突；different-ID late result只允许原producer/lease绑定后追加安全摘要audit。response loss复用exact persisted/cached command bytes。

Gate拒绝结果固定`HookRejectionV1 { decision_id, point, hook_id/revision?, reason_code, safe_subject_id, input_digest, registry_revision, source_cursor, redaction_policy }`，不含payload。Observer warning和Transform拒绝使用相同安全原则。oversized/deep/unknown-field在decode前拒绝；日志只允许Kernel构造的安全字段。

## Projection and replay

`HookProjectionV1`绑定store ID、完整scope/owner/hook stream、inclusive cursor、Manifest-pinned source SchemaSet/limits、Hook registry revision/config digest、exact reducer/output reader、history digest、sorted invocations/decisions/skip/audit counters与projection digest。Reducer只折叠Event事实，无Clock/I/O/random/global mutable state。

Recorded replay完整读取Hook stream并重建Projection，不加载handler registry implementation、不reserve/settle、不调用Fake Hook、不append。Recorded决策作为source事实消费，不能重新运行Gate/Transform。Simulated/Reexecute没有本切片入口；任何请求在dispatch前稳定拒绝，并且source Run的event、budget和handler counter不变。

# Impact analysis

| Dimension | Finding | Evidence / response |
|---|---|---|
| Direct | 实施阶段将扩展protocol Hook ID/registry/invocation/decision/projection/Event Schema；Kernel新增crate-private Hook orchestration、Fake handlers、pure fold与Recorded replay | 当前`pareto-protocol::{types,runtime_control,schema,validation}`集中公共合同；`pareto-kernel::event_store::{lifecycle,projection,runtime_control}`是唯一可信入口；本设计阶段不改代码/Schema |
| Indirect | REQ-0009 Effect、0010 Provider、0011 Tool、0013 Sandbox、0014 Loop、0015 Memory、0018 DAG、0026 Evidence、0033 WASI都可能消费Hook proposal/result | BACKLOG-0001与RFC-0007；只留versioned/default-deny接口，不实现transport或下游行为 |
| Call/permission | Hook必须通过persisted Manifest/lifecycle/control重建authority；不能复用测试用`KernelAuthority::authenticated`或让registration/result自授权 | REQ-0007 REVIEW-0007关于opaque lease/producer/recovery authority；实现需独立Hook target与lease constructor，全部crate-private |
| Data isolation | 现有Event Store key绑定tenant/user/workspace/run/owner/stream；Hook subject/Task/handler作为闭合payload并由Kernel验证，不能改变Event envelope signer | `event_store.rs` authority/row validation；RFC-0006 owner signer合同；全scope与business-ID负例 |
| API/schema | 新Hook kind/point、排序、mask、failure reason和Projection会成为持久合同；Rust trait只能是reference implementation | RFC-0007/ADR-0008禁止Rust ABI强制；1.0闭合Schema、retained exact readers、breaking change major bump |
| Persistence/replay | Hook事实复用Event Store v2且无第二表；budget仍在control stream。reserve/terminal以private atomic pair cross-bind；单边pair是损坏；Recorded不调用handler | Event Store transaction-local insert、Runtime Control Projection/Recorded replay；双insert fault/rollback、reopen、event-count/budget/handler-counter前后断言 |
| Concurrency | lifecycle terminal、cancel/timeout、budget pair、Gate short-circuit与多Hook phase可能竞态；所有authority/terminal决策需同writer lock重fold | `BEGIN IMMEDIATE`、stable phase/ordinal、bounded command graph、pair exact/mutation与two-writer reverse race；不得锁外组合后blind append |
| Security | self-escalation、confused deputy、Transform保护字段绕过、output injection、oversize与敏感日志是主要风险 | bounded authority、保护hash-view+allow mask、pre-decode limits、Kernel-only redaction和cross-scope no-write |
| Failure/recovery | crash after reserve、第二append失败、hung handler、response loss、late result不能自动release/reexecute、补写单边pair或重开decision | transaction-local REQ-0007 admission；Hook Projection枚举pending；pair exact retry、commit-response-loss、one-existing corruption及late audit |
| Compatibility/migration | 预期不需DB v3；新增内容地址SchemaSet且保留历史。RunTask/RuntimeControl source mapping需把新增Hook binding视为无关演进或显式保留旧mapping | 现有SQLite v2与retained reducer测试；实现若需DDL、actor变化或current substitution必须退回RFC |
| Performance | 串行Hook与每命令多stream fold增加latency/bytes，SQLite仍单writer；无数据支持并行执行 | 首片稳定串行顺序；记录1/N Hook与争用观察，不声明优化；并行Hook另立Requirement |
| Cost/token | Fake Hook无模型/Provider；usage仍占Run/Task/Actor预算 | 分别报告fake usage、测试成本和零外部费用，不声称优化 |
| Dependency/operations | 现有serde/sha2/sqlx/Tokio应足够；真实runtime/transport/SDK依赖不允许进入 | Cargo diff/scope checker；新依赖或进程/network访问触发停止 |
| Documentation | index、EPIC、ARCH-0003与Requirement backlog需同步设计/implemented事实 | 设计批准只标approved合同；实现完成后才改README为implemented |
| Rollback | 写入首个Hook Event后必须保留Schema、registry reader/reducer和decision解释；可停新writer，不能删/改历史或重新执行Recorded Hook | 发布前可revert；发布后forward schema/event修复，source Run不覆盖 |

## Downstream boundaries

- REQ-0009：Gate可在Effect intent前拒绝proposal，但不能构造Intent/Receipt、settlement evidence或执行Effect；late receipt不回写Hook terminal。
- REQ-0010/0011/0013：Provider/Tool/Sandbox handler只能在各自Requirement批准transport、resource envelope和isolation后接入；Hook决定不是Capability或Effect authority。
- REQ-0014：Loop消费结构化Hook decision并尊重cancel/terminal；不能跳过required Gate或重跑Recorded Hook。
- REQ-0015/0026：Memory/Evidence输入仍需provenance和独立admission；Observer annotation不自动成为Memory或Evidence。
- REQ-0018：DAG point需独立新增并保持Task budget/cancel scope；Hook不能创建/迁移Task。
- REQ-0033：WASI只可实现handler计算；fuel/memory映射budget，Guest无authority constructor、DB或host unrestricted I/O。

# Compatibility and migration

实施发布新的Hook-capable内容地址SchemaSet并保留当前全部sets，包括最终Runtime Control set`sha256-a95c824d3a47dbc891f884921811859dc2d132e1e39f6f781e833ea9b306a217`。新Run Manifest必须exact固定新set、Hook registry revision/config digest；旧Run不升级。Hook stream每行使用Manifest-pinned set/limits；current/alternate/compatible-looking set拒绝。

SQLite保持`user_version=2`、writer epoch、events/snapshot DDL与trigger字节不变；Hook事实只进入events表的derived stream。RunTask reducer继续只抽取四类lifecycle binding；Runtime Control reducer继续只抽取control bindings。新增Hook binding不得改变既有source key/digest，除非显式retained mapping证明。代码rollback保留Hook reader/reducer和全部historical registry/config；不能把pending invocation释放、把abstain重释allow或在Recorded replay重跑handler。

# Test traceability

| Acceptance | Scope/layer | Scenario | Planned evidence |
|---|---|---|---|
| AC-01 | Focused protocol/golden | 闭合registration/registry/config/Schema、Manifest exact pin、Observer-only policy、Gate/Transform非法策略字段拒绝 | `cargo test -p pareto-protocol hook_contract --offline`; schema generation byte diff |
| AC-02 | Focused table | 全Hook kind × point允许矩阵；未列组合默认拒绝 | non-zero filter `hook_runtime::kind_point_table` |
| AC-03 | Focused property/model | Transform/Gate/Observer priority交错仍按phase；逐步input/predecessor/final digest、point finalization、重启与Recorded等价 | `hook_runtime::phase_order_lineage`；`hook_runtime::ordering`；bounded permutation/property cases |
| AC-04 | Focused/Core decision | allow/deny/abstain、required/optional、零Gate/仅optional、failure/timeout/invalid/unknown与短路表；不存在none policy | `hook_runtime::gate_composition`；`hook_runtime::default_deny` |
| AC-05 | Focused failure | Observer两策略只改变execution status/下游推进且业务决定字节不变；Gate固定fail closed；Transform chain中间失败固定reject-whole且后续phase skip、零partial authority | `hook_runtime::failure_policy`; `hook_runtime::observer_non_authority`; `hook_runtime::transform_chain_failure` |
| AC-06 | Core security | Transform每个保护字段mutation、unknown字段、mask遗漏、chain中间输出注入均拒绝 | `hook_runtime::transform_protected_fields` |
| AC-07 | Core authority/API | self-signed authority、grant/lease constructor、scope claim、delegation widening负例；外部无Event/SQL API | `hook_runtime::authority`; Kernel compile-fail doctest |
| AC-08 | Core isolation | tenant/user/workspace/run/task/owner/subject/hook/invocation/attempt/decision逐字段swap与payload shadow no-write | `hook_runtime::isolation` |
| AC-09 | Core SQLite/budget/concurrency | reserve pair zero/two exact、mutation、one-existing重封损坏、第一/第二insert与commit fault全rollback、response loss、two-writer reverse winner防超卖 | `hook_runtime::reserve_pair_atomicity`; `hook_runtime::pair_fault_injection`; `hook_runtime::budget_concurrency` |
| AC-10 | Focused accounting/idempotency | success/fail/cancel/timeout verified/unknown consume-release；exact retry零重复核算 | `hook_runtime::settlement`; `hook_runtime::idempotency` |
| AC-11 | Focused deterministic time | FakeClock probe、deadline前/等于/后、hung/uninterruptible recovery，无sleep | `hook_runtime::cancellation_deadline`; static scope checker |
| AC-12 | Core concurrency/model | complete/cancel/timeout terminal pair、通用单stream入口拒绝、response-loss/late/duplicate/retry/out-of-order唯一terminal和budget守恒 | `hook_runtime::terminal_pair_atomicity`; `hook_runtime::terminal_race`; `hook_runtime::model_sequences` |
| AC-13 | Core security/limits | output injection、oversize/depth/collection、unknown字段、敏感日志/错误不回显 | protocol limits tests；`hook_runtime::output_security` |
| AC-14 | Core persistence | point-start/lineage/finalization pure fold、非法phase/digest/双terminal/顺序/authority/budget/pair引用fail closed、无第二表 | `hook_runtime::fold_contract`; `hook_runtime::phase_order_lineage`; DB/static inspection |
| AC-15 | Core recovery | close/reopen pending/terminal恢复、pair response-loss exact retry、单边validly-resealed/unknown reader/current substitution拒绝 | `hook_runtime::recovery`; `hook_runtime::pair_corruption`; `hook_runtime::compatibility` |
| AC-16 | Core replay | Recorded前后Fake调用、pair/event数、全部budget逐字段不变且phase/finalization等价；Simulated/Reexecute dispatch前拒绝 | `hook_runtime::recorded_replay`; `hook_runtime::unsupported_modes` |
| AC-17 | Impacted compatibility | 新set复算、全部旧setbyte-identical、old Run无Hook、DB v2与既有reducers不变 | protocol retained-set suite；Event Store/Projection/Runtime Control regressions |
| AC-18 | Core static/scope | 只有Rust Fake，公共协议无Rust ABI/SQLite/transport，Cargo无runtime依赖 | API/dependency inspection；scope checker |
| AC-19 | Impacted contract | 下游只见proposal/result，不可获得Effect/Evidence/terminal authority；无后续模块 | exact diff/API review |
| AC-20 | Focused/Impacted/Core | 每个filter先list并断言非零；全矩阵、real SQLite双append fault/commit-loss/重封、FakeClock、bounded pair/phase/concurrency model和REQ-0003..0007回归 | helper + full completion gates recorded in VALIDATION |

# Open questions

无可由实现者自行决定的开放项。独立设计Reviewer若发现Hook point、Event顺序、Gate empty-set、Transform mask、budget/control双流原子性、late audit或replay路径仍含歧义，REQ/SPEC/RFC保持未批准且禁止创建active work或Runtime代码。真实transport、并行Hook、background recovery、Hook Snapshot与外部handler全部延期。
