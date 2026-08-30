---
id: SPEC-0008
title: Effect Intent/Receipt 与幂等效果规范
status: approved
owners: [runtime-kernel]
created: 2026-08-30
updated: 2026-08-30
links: [REQ-0009, EPIC-0002, REQ-0004, REQ-0007, REQ-0008, RFC-0009, RFC-0002, RFC-0003, RFC-0004, RFC-0005, RFC-0006, RFC-0008, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ADR-0007, ADR-0009, ADR-0010, ARCH-0002, ARCH-0003, REVIEW-0012]
---

# Behavioral contract

Effect 是 Kernel 治理的受保护外部边界，不是普通callback。每个 Effect 必须在外部调用前持久化 Intent；Runtime Control reservation与Intent是一个不可分割的atomic pair。Intent之后只有Kernel提交dispatch claim并持有单次opaque lease才能调用确定性Fake executor。request、client idempotency key、Receipt、Provider usage、同scope身份或Hook decision都不是authority。

系统提供at-most-once-or-reconcile语义而非虚构exactly-once。claim之后无法证明结果时，Runtime Control按verified/unknown规则保守settle，Effect明确进入reconciliation；后续受信对账只能在独立轴追加外部结论，不能改变原operation terminal、budget、lifecycle或已提交Event。Run/Task成功必须证明目标范围没有pending/unknown/partial/open-reconciliation Effect。

首片只执行进程内确定性Fake Effect，不访问真实文件、process、network、model、Provider、Tool或Sandbox。Recorded replay只读取source Effect历史、Projection和validated Boundary Inventory，无live执行类型。

# Inputs, outputs, states, and failure behavior

## Manifest and registry

- Run Manifest 3.0精确要求v2的全部roles/Hook pin，再增加`effect_registry` revision role与`effect_registry_config_digest`；v1/v2禁止Effect pin。
- `EffectRegistryRevisionV1`按effect kind排序唯一，registration固定effect/executor/adapter/producer/operation-contract revisions、内容地址executor descriptor digest、request/receipt Schema、external idempotency class、unknown/reconciliation/redaction policy和limits。
- `EffectExecutorDescriptorV1`固定`revision_kind=effect_executor`、executor/adapter/producer revisions、Schema、config digest、resource/meter/recovery contracts和reference implementation compatibility digest；registration revision/digest必须exact等于descriptor metadata/content digest，Fake binding再匹配descriptor内compatibility digest。compatibility digest不能替代executor revision/content identity。
- 首片external idempotency为`unsupported | keyed`，unknown policy固定`reconcile_only`；无自动redispatch、补偿或人工自证。
- Effect stream sequence 1绑定Manifest lifecycle cursor、registry/config、Boundary recording policy、exact SchemaSet/limits及Effect reducer/output/history identities。旧Run、缺pin、alternate/current registry或unknown major均拒绝初始化。

## Request and identity

`EffectRequestV1`只表达proposal：effect kind、subject、可选Task、Schema-validated request、client idempotency key digest、absolute deadline与correlation。Kernel从full scope、registry/effect revision、kind与key digest派生Effect ID；request digest再覆盖完整规范请求、subject/Task、contract和deadline policy。

Effect ID与request digest都覆盖executor revision/descriptor/config。同Effect ID/exact request retry返回原状态且不重复reserve/claim/dispatch/settle；same key异request/subject/Task/contract/deadline/executor为`idempotency_conflict`。跨scope/kind/revision不互相命中或泄漏。首片每Effect只有attempt 1，但Attempt ID/外部key token仍由Kernel稳定派生，为后续版本留下闭合身份而不授权重试。

## State and events

Effect stream事件为：initialized；intended；dispatch-claimed；receipt-admitted、attempt-concluded或reconciliation-required之一；reconciliation-observed/reconciled；late-receipt-observed；message-rejected。Projection分别保存dispatch、external conclusion与reconciliation轴：

| Dispatch | External conclusion | Reconciliation | Meaning |
|---|---|---|---|
| intended | pending | not-required | 已reserve且有Intent，尚未越过外部claim |
| claimed | pending | not-required | 可能即将/已经发生；不能猜测未执行 |
| concluded | applied | not-required | deadline前可信Receipt完成 |
| concluded | not-applied | not-required | 可信证明执行前拒绝/取消 |
| concluded | partial/unknown | required | 原operation已保守settle，等待独立对账 |
| concluded | applied/not-applied/partial | resolved-* | 后续对账结论；不改原operation/lifecycle |

任何缺first event、gap、同Effect双Intent、claim-before-intent、双terminal pair、非法reconciliation、错误registry/pair/budget binding或unknown reader/reducer使aggregate fail closed。

## Reserve/Intent and terminal pairs

`OperationReservedPayloadV1`和`OperationSettledPayloadV1`在新SchemaSet增加可选`effect_pair`，与既有`hook_pair`互斥。Effect pair固定pair ID/kind/fingerprint、operation/reservation/effect/attempt、control/effect counterpart event IDs及双方prepared bytes digest。Intent同时持久化绑定scope/source/registry/executor/operation/reservation/meter/recovery/deadline/initial process epoch的`EffectRecoveryBaseKeyV1`。

reserve/Intent和terminal都使用Event Store通用transaction-local `append_atomic_pair`：zero-existing时两个expected cursors都命中才依次insert并commit；two-existing只接受完整exact cross-binding；one-existing为`corrupt_partial_pair`且永不补写。same pair ID异bytes优先冲突。Effect-bound operation的通用单streamcallback/timeout settlement稳定拒绝。

terminal映射：applied→Runtime succeeded；rejected-before-apply→failed/not-applied；partial或possible-effect→failed/reconciliation-required；deadline winner且未claim→timed_out/not-applied并verified zero release；deadline winner且已claim→timed_out/reconciliation-required；claim前有效取消→cancelled/not-applied。未claim history禁止进入unknown，已claim history禁止伪装not-applied。verified Kernel meter消费actual并释放差额；unknown全额消费reservation。Effect terminal和预算settlement同commit可见。

## Dispatch and Receipt

dispatch claim在独立writer transaction重新fold lifecycle/control/effect，验证live mode、registry/executor、reserved operation、取消/deadline和unclaimed attempt。claim扩展base recovery key，固定claim Event/digest、claim process epoch/Clock、executor/adapter/producer、external key与policy；commit成功才生成绑定完整key的crate-private `EffectDispatchLease`并由Kernel-owned orchestration直接调用Fake executor。already-claimed exact retry不再发lease或调用executor；crash-between-claim-and-call因此进入对账。

Fake executor返回`EffectReceiptObservationV1`或边界故障观察。Kernel先执行bounded decode，再验证executor/producer/adapter、lease/process epoch、scope/effect/attempt/external key/request digest、Receipt Schema、Clock与Kernel meter。错误executor/producer或跨域输入不写目标；已认证producer的invalid/oversized结果形成脱敏reject并按unknown terminal pair结清。Receipt仅保存safe digest、闭合outcome、bounded summary、usage observation与limitations，不保存敏感payload/秘密/路径/SQL/raw error。

## Explicit recovery

无background scanner。crate-private `EffectRecoveryCommandV1`只能由Kernel recovery authority从persisted Projection构造，固定base/full recovery key、`process_epoch_lost | deadline_due | cancellation_effective` cause、canonical Clock/current process epoch observation、verified meter或unknown evidence fingerprint、expected双stream cursors及terminal pair preimage。Kernel认证的process epoch observation才证明旧lease失效。

domain-separated command fingerprint确定pair/control/effect Event IDs。锁内优先级固定为integrity/source/schema/isolation/key → same-ID exact/mutation → existing terminal → eligibility/due/cancel/process loss → cursor/domain → atomic pair。not-eligible不append/消费identity；commit-response-loss exact pair retry幂等，新sample命中ExistingTerminal no-op。

Intent未claim且旧epoch失效、due或cancel时可由history确定Kernel未交付executor lease，结清`not_applied`；存在claim后一律按verified partial或unknown保守结清并打开reconciliation。claim所属同一live epoch、deadline未到且无cancel时not-eligible。recovery不生成Receipt、refund或redispatch。

## Partial, timeout, cancellation, and reconciliation

partial必须保存confirmed与unknown component摘要、limitations和meter evidence；不得整体标not-applied或自动重试。claim后的cancel只表示pending，不能确认外部停止。callback/cancel/timeout竞争按same writer lock、deadline equality timeout wins与首个合法commit产生唯一terminal pair；loser只能读原winner或追加late audit。

普通late Receipt不能直接关闭reconciliation。`EffectReconciliationCommandV1`只能由owner request并由Manifest-pinned Fake query producer提供exact Effect/attempt/external key与evidence fingerprint。`confirmed_applied | confirmed_not_applied | confirmed_partial`关闭，`unresolved`保持open；exact retry幂等、mutation冲突。对账不refund、不重新dispatch、不修改control/lifecycle或已finalized inventory horizon。

Run/Task`succeeded` transition在同一transaction折叠Effect history并拒绝目标范围的reserved operation、intended/claimed未结论或open reconciliation。`failed | cancelled`在operation已settle后可保留对账；late事实不重开terminal。

## Projection, inventory, and replay

Effect Projection绑定store/full scope/owner/stream/显式inclusive cursor、Manifest source SchemaSet/limits、registry/config/policy、executor revision/descriptor/config、recovery identity、reducer/output/history digest，按Effect ID规范排序。reader只读取显式horizon；pure reducer无Clock、I/O、executor或mutable cache。

既有Boundary Inventory/Record 1.0的`Failed`只表示receipt前失败，禁止用于partial/unknown。Effect-capable Run发布`BoundaryInventoryRevisionV2`与`EffectBoundaryRecordV2`：固定exact Effect cursor/history digest；每个record固定Effect/request/attempt/external key/executor/operation identity，并闭合表达applied、not_applied、cancelled_before_claim、partial(confirmed/unknown component digests、receipt、limitations、reconciliation binding)或unknown(limitations、reconciliation binding)。v1 bytes/reader保留，空Effect使用显式空v2 inventory。

Run Manifest v3的Recorded replay只接受validated v2 inventory，读取其fixed cursor以内的source Effect stream、验证history digest并逐项核对Projection/inventory；horizon后追加late/reconciliation Event前后，同一pin结果byte-identical。alternate/current/unpinned inventory/cursor/digest拒绝。API不接收executor、writer、Clock、lease、reserve/settlement/recovery/reconciliation authority。Simulated/Reexecute Effect入口拒绝。

# Impact analysis

| Dimension | Finding | Evidence / response |
|---|---|---|
| Direct | protocol新增Manifest v3、Effect registry/request/intent/receipt/pair/projection/Event Schema；Kernel新增`event_store/effect_runtime.rs`、Fake executor、pure fold、inventory/replay | `crates/pareto-protocol/src/{types,runtime_control,schema,validation}.rs`集中现有合同；`crates/pareto-kernel/src/event_store*.rs`是唯一authority；设计阶段不改runtime/schema |
| Direct | Runtime Control reservation/settlement当前只有`hook_pair: Option<HookPairBindingV1>`且fold调用`valid_hook_pair_binding` | `pareto-protocol/src/runtime_control.rs::{OperationReservedPayloadV1,OperationSettledPayloadV1}`；`event_store/runtime_control.rs::valid_hook_pair_binding`；新增mutually-exclusive effect binding和transaction-local Effect planning |
| Direct | lifecycle success当前只受pending Runtime Control/Hook边界保护，不能识别open Effect reconciliation | `event_store/runtime_control.rs::{ensure_no_pending_for_run,ensure_no_pending_for_task}`与Hook lifecycle guard；v3 transition同事务foldEffect并增加scope guard |
| Indirect | REQ-0010 Provider、0011 Tool、0013 Sandbox、0014 Loop将消费proposal/dispatch/receipt接口；Boundary Inventory/Replay消费者依赖结论语义 | BACKLOG-0001、RFC-0002、RFC-0007；只留versioned Kernel-private接口，不实现transport或下游行为 |
| Call/permission | Effect call path跨protocol validation→Manifest/lifecycle→Capability/reserve→Intent→claim→executor→Receipt admission→terminal pair→projection/recovery | RFC-0009 data flow；所有constructor/lease/producer/reconciler crate-private；Hook decision/Receipt/validated bytes不是authority |
| Data isolation | Event Store完整tenant/user presence-value/workspace/run/owner/stream；Effect需额外exact Task/subject/effect/attempt/operation/reservation/key/pair | `event_store.rs::{AdmittedAppend,AdmittedRead,validate_row}`；payload shadow/cross-scope/no-write与ID复用负例 |
| API/schema | Manifest semantic validation现以major 1/2选择闭合roles，schema hardener固定Hook role；新增v3、Effect/Executor和Inventory v2不能改变v1/v2 bytes/readers | `validation.rs::validate_run_manifest/validate_inventory`、`schema.rs::harden_schema`、retained `schemas/sets`; golden/old/current-substitution tests |
| Persistence/replay | 复用SQLite v2 events与generic `append_atomic_pair`，无outbox/status table；新增Effect stream/reducer/inventory；Recorded必须类型上无executor | `event_store.rs::{PreparedEvent,append_atomic_pair,insert_prepared}`；Hook pair precedent；DB checksum/schema scope tests |
| Concurrency | reserve/Intent、claim、callback/cancel/timeout、success transition和reconciliation可能竞态；锁外检查会TOCTOU | 所有authority/terminal/success决定在`BEGIN IMMEDIATE`内refold；pair zero/two/one；reverse-order bounded command model |
| Idempotency | Event ID幂等不足以覆盖client key/request mutation和claim执行；external keyed能力不能等于自动retry authority | Event Store RFC-0003冲突优先级；Effect ID/request digest/claim-once与Fake application counter tests |
| Failure/recovery | crash可落在Intent前后、claim前后、executor应用后、terminal pair前后；live lease丢失后只能稳定Kernel recovery，unknown必须对账而非release/reexecute | persisted recovery key/epoch/Clock/evidence；not-eligible、exact/mutation、ExistingTerminal、fault/reopen矩阵、Fake query reconciliation |
| Security | confused deputy、Receipt injection、oversize、secret/path leakage、key cross-domain probing、伪造reconciler是主要威胁 | bounded decode、opaque lease/producer、Kernel redaction、digest-only key/receipt、cross-scope no-write |
| Compatibility/migration | 新content-address SchemaSet、Manifest v3、Executor descriptor与Inventory v2；SQLite保持v2，old Manifest/Inventory/Run/reducers/SchemaSets byte-identical | schema generator byte diff、retained set hashes、DB `user_version=2`/DDL/checksum/snapshot regression；发现DB v3需求即停 |
| Snapshot | RunTask Snapshot仍只覆盖lifecycle；Effect首片完整fold，不增加Snapshot或把Snapshot当authority | REQ-0006 SPEC-0005；Effect规模观察后另立Requirement |
| Performance | 每Effect至少reserve pair、claim、terminal pair三次writer transaction并fold三stream；reconciliation另写 | 记录1/10/100 Effect与contention/fold/replay；无baseline前不并行、不background poll、不宣称优化 |
| Cost/token | Fake Effect无模型/付费Provider，budget仍核算Fake usage；unknown保守高估 | 分开报告reserved/verified/unknown usage与外部费用0，不声称Token/成本收益 |
| Dependency/operations | 现有serde/sha2/sqlx/Tokio足够；真实transport、queue、scheduler依赖超范围 | Cargo manifests/lock scope gate；network/process/sleep/new dependency触发停止和重审 |
| Documentation | index、EPIC、ARCH-0003/overview、Requirement backlog需同步设计；README只能在实现verified后声称交付 | design approval只记录合同，implementation closure才同步implemented facts |
| Rollback | 首个v3/Effect Event后必须保留reader/reducer/pair validator和对账解释；不能删Event或重执行 | 发布前Git revert；发布后stop writer+retained reader，forward schema/event修复 |

## Downstream and boundary decisions

- Provider/Tool只能实现registration批准的adapter/producer/executor，不能直接reserve、append Intent、提交Receipt为authority或关闭对账。
- Sandbox负责真实资源enforcement，但不能绕过Intent/claim；Effect lease不是通用filesystem/network/process capability。
- Agent Loop可调用Hook Gate后提出Effect request，但Hook allow不代替Capability、budget或Effect admission。
- Boundary Inventory v2 finalizer只派生fixed-horizon事实；horizon后reconciliation不改变同一Recorded pin。Evidence Gate、artifact persistence、CLI和reexecute/simulated仍属后续Requirement。
- 真实exactly-once、automatic keyed redispatch、compensation/saga、background outbox、distributed queue/DB、Effect Snapshot均需新Requirement/RFC。

# Compatibility and migration

实现发布一个新SchemaSet，不修改历史目录。Manifest v3基于v2添加Effect pin；Executor descriptor 1.0与Boundary Inventory/Effect Record 2.0使用独立Schema；protocol runtime dispatch必须按exact `schema_ref.major`选择roles/types，禁止用current struct语义接受旧bytes。新增Effect event binding不能改变RunTask/RuntimeControl/Hook retained source keys、output digests或Snapshot bytes。

SQLite保持`user_version=2`及accepted DDL/triggers/checksums。Effect/Control pair使用现有events表和generic transaction-local inserts。若需要新索引，只能证明为非权威且不改变accepted DDL；首片默认不增加。首个Effect事实后旧binary回滚仅允许只读保留，不得用旧writer忽略v3；部署rollback需停止新Effect writer并保留兼容reader。

# Test traceability

| Acceptance | Scope/layer | Scenario | Planned evidence |
|---|---|---|---|
| AC-01 | Focused protocol/contract | Effect request/registry/Intent/executor descriptor闭合字段、content identity、排序、unknown field/version/key digest | `cargo test -p pareto-protocol --test protocol_contract effect_contract --offline -- --exact` |
| AC-02 | Focused security/integration | no Capability、裸Validated、Hook/Provider自报scope与cross-domain探测默认拒绝且no-write | `cargo test -p pareto-kernel event_store::effect_runtime::default_deny --offline -- --exact` |
| AC-03 | Focused integration/fault | reserve+Intent atomic；Intent存在且claim提交后executor counter才增加；pair fault全rollback | `cargo test -p pareto-kernel event_store::effect_runtime::intent_before_dispatch --offline -- --exact` |
| AC-04 | Focused contract/integration | exact retry、same-key request/operation/scope/revision mutation、cross-kind key | `cargo test -p pareto-kernel event_store::effect_runtime::idempotency --offline -- --exact` |
| AC-05 | Focused security | lease跨executor/attempt/Run/Task/Actor/epoch复用、wrong executor substitution、already-claimed retry不发lease | `cargo test -p pareto-kernel event_store::effect_runtime::dispatch_lease --offline -- --exact` |
| AC-06 | Focused Fake component | applied/rejected/partial/response-loss/timeout/crash/malformed完整outcome矩阵且无真实I/O | `cargo test -p pareto-kernel event_store::effect_runtime::fake_outcomes --offline -- --exact` |
| AC-07 | Focused security/contract | Receipt observation非authority；producer/adapter/schema/limits/meter绑定 | `cargo test -p pareto-kernel event_store::effect_runtime::receipt_admission --offline -- --exact` |
| AC-08 | Focused unit/model | dispatch/external/reconciliation product state合法序列、唯一conclusion | `cargo test -p pareto-kernel event_store::effect_runtime::state_model --offline -- --exact` |
| AC-09 | Focused integration | partial confirmed/unknown摘要与usage保留，不整体retry/补偿 | `cargo test -p pareto-kernel event_store::effect_runtime::partial_success --offline -- --exact` |
| AC-10 | Focused recovery/model | Intent/claim/external/terminal各crash点；recovery key、not-eligible、exact/mutation、response-loss与new-sample ExistingTerminal | `cargo test -p pareto-kernel event_store::effect_runtime::crash_recovery --offline -- --exact` |
| AC-11 | Focused security/integration | owner request非evidence、pinned query producer、exact/mutation/unresolved/closed对账 | `cargo test -p pareto-kernel event_store::effect_runtime::reconciliation --offline -- --exact` |
| AC-12 | Focused integration/concurrency | Effect/control terminal pair、verified/unknown accounting、single-sided corruption、duplicate settlement | `cargo test -p pareto-kernel event_store::effect_runtime::atomic_settlement --offline -- --exact` |
| AC-13 | Focused FakeClock/model | 未claim cancel/deadline唯一not-applied、已claim唯一partial/unknown、deadline equality与timeout/callback reverse races、无sleep | `cargo test -p pareto-kernel event_store::effect_runtime::cancellation_timeout --offline -- --exact` |
| AC-14 | Focused security/integration | exact duplicate、late/out-of-order/conflicting Receipt、redaction/injection | `cargo test -p pareto-kernel event_store::effect_runtime::late_receipts --offline -- --exact` |
| AC-15 | Focused fold/persistence | continuous stream pure fold、无mutable authority table、gap/illegal/double terminal fail closed | `cargo test -p pareto-kernel event_store::effect_runtime::fold_contract --offline -- --exact` |
| AC-16 | Focused security/isolation | tenant/user presence/workspace/run/task/actor/effect/key/pair全矩阵 | `cargo test -p pareto-kernel event_store::effect_runtime::isolation --offline -- --exact` |
| AC-17 | Focused recovery/projection | reopen Projection executor/recovery identity与explicit horizon/digest；missing first/gap/row drift/alternate reducer拒绝 | `cargo test -p pareto-kernel event_store::effect_runtime::projection_recovery --offline -- --exact` |
| AC-18 | Focused replay/E2E | Inventory v2无损partial/unknown、fixed cursor/digest；horizon后facts不变；Recorded零executor/append/reserve/settle | `cargo test -p pareto-kernel event_store::effect_runtime::recorded_replay --offline -- --exact` |
| AC-19 | Impacted compatibility/static | Manifest v1/v2/v3、Inventory v1/v2、executor old/current substitution与retained sets；SQLite v2/old reducer/snapshot bytes不变 | `cargo test -p pareto-kernel event_store::effect_runtime::compatibility --offline -- --exact`; schema generator byte check |
| AC-20 | Core layered/model | named filters非零、pair/crash/race bounded model、Event Store/Lifecycle/Control/Hook/Projection全回归 | 全部Focused命令；`cargo test --workspace --all-targets --all-features --offline` |
| AC-21 | Focused API/static | Kernel-private接口无raw DB/authority/transport，Cargo无新增依赖/真实I/O | Rust visibility tests、scope script、`cargo tree --workspace --offline`与manifest diff |
| AC-22 | Focused integration/concurrency | success被pending/unknown/partial/open reconciliation阻断；failed/cancelled保留对账；transition race | `cargo test -p pareto-kernel event_store::effect_runtime::lifecycle_success_guard --offline -- --exact` |
| AC-01..22 regression | Core governance/protocol/kernel | docs、format、lint、schemas、全部protocol/kernel tests、diff hygiene | AGENTS.md全部completion gates；Plan逐项记录命令与non-zero proof |

# Open questions

初次独立REVIEW-0012对fixed `9f8bf23`提出4个Major；修订冻结fixed-horizon Recorded replay、Boundary Inventory/Effect Record v2、内容地址executor identity与stable Effect recovery command，并唯一化未claim/已claim recovery结论。原Reviewer在fixed `b7acbd82824d8410d432117c89be1bd56c8ce05c`关闭F-001至F-004，最终independent approved、0 Blocker、0 Major；ADR-0010接受合同。没有允许实施阶段自行决定的开放合同。
