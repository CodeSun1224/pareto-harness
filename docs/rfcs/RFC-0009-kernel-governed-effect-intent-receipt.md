---
id: RFC-0009
title: Kernel 治理的 Effect Intent、Receipt 与对账合同
status: accepted
owners: [runtime-kernel]
created: 2026-08-30
updated: 2026-08-30
links: [REQ-0009, SPEC-0008, EPIC-0002, REQ-0004, REQ-0007, REQ-0008, RFC-0002, RFC-0003, RFC-0004, RFC-0005, RFC-0006, RFC-0008, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ADR-0007, ADR-0009, ADR-0010, ARCH-0002, ARCH-0003, REVIEW-0012]
---

# Summary

在可信 Kernel 内建立一个与 lifecycle、Runtime Control 和 Hook stream 分离的单 Run Effect aggregate。每个受保护 Effect 先在同一 SQLite `BEGIN IMMEDIATE` 中原子提交 control `operation-reserved` 与 Effect `effect-intended`；Kernel 随后提交 `effect-dispatch-claimed`，才向进程内确定性 Fake executor 交付不可伪造的单次 lease。外部结果永远先是 Receipt observation；只有 Manifest 固定的 producer/adapter、exact lease 与 Kernel meter 通过 admission 后，才能以 control settlement + Effect conclusion 原子 pair 形成权威结果。

本合同不承诺外部 exactly-once。dispatch claim 后的进程崩溃、响应丢失、timeout 或部分成功进入 `reconciliation-required`，默认不重新执行；受信对账只能追加结论，不能改写原 Intent、operation terminal、预算 gross history 或生命周期。Recorded replay 只有 reader/reducer/inventory 依赖，不持有 executor、lease constructor、writer 或 Runtime Control settlement authority。

# Motivation and requirements

REQ-0004 的 SQLite 事务只能保证 Event 原子性，不能与文件、进程、网络或模型调用形成分布式事务。REQ-0007 能在 Effect 前授权和 reserve，但其公开 outcome 不区分“确定未执行”和“可能已执行”，且现有 pair 字段只认识 Hook。REQ-0008 的双 stream pair 证明了 reserve/terminal 不能先单流提交再补扩展事实，但 Hook 明确禁止创建 Receipt 或执行 Effect。

REQ-0009 要求 Intent-before-dispatch、同键异请求冲突、部分成功不丢失、unknown 不盲重试、Receipt 非权威、Effect/operation 原子终结、对账、成功准入、隔离、恢复和 replay 零执行。这些语义会被 Provider、Tool、Sandbox 与 Agent Loop 消费，必须在首个真实边界实现前冻结。

# Proposed design

## 1. Version, Manifest, and registry

发布 Run Manifest 3.0。v3 保留 v2 的全部字段与闭合 revision role，并新增：

- `revisions["effect_registry"]`：内容地址 `EffectRegistryRevisionV1`；
- `effect_registry_config_digest`：完整有序 registry 配置摘要。

v1 禁止 Hook/Effect pins；v2 要求 Hook、禁止 Effect；v3 同时要求 Hook 与 Effect pins。每个 major 使用自己的 retained JSON Schema 和 semantic reader，不按 current manifest 解释旧 Run。REQ-0009 因此把已完成 REQ-0008 列为实际前置，不重释 backlog 中原有并行意图。

`EffectRegistryRevisionV1` 由 `RevisionMetadata(revision_kind=effect_registry)`、`config_digest` 与按 `effect_kind` 排序唯一的 registrations 组成。每个 `EffectRegistrationV1` 固定：

```text
effect_kind, effect_revision, executor_revision, executor_descriptor_digest,
adapter_revision, producer_revision,
operation_contract_revision, request_schema_ref, receipt_schema_ref,
idempotency_policy, unknown_outcome_policy, reconciliation_policy_revision,
redaction_policy_revision, limits
```

首片策略闭合为：external idempotency `unsupported | keyed`；unknown outcome 一律 `reconcile_only`。`keyed` 只证明外部系统按同一 key 去重，不自动授权 Kernel 重试；首片不实现 redispatch。真实 adapter/producer/transport 必须由后续 Requirement 注册。

`EffectExecutorDescriptorV1` 是独立内容地址revision：metadata的`revision_kind=effect_executor`，content包含executor/adapter/producer revisions、request/receipt Schema、config digest、resource/meter/recovery contracts和reference implementation compatibility digest。registration的`executor_revision`必须等于descriptor metadata revision，`executor_descriptor_digest`必须等于descriptor canonical content digest；Fake binding再同时匹配descriptor内implementation compatibility digest。compatibility digest只证明某个进程内实现符合该exact descriptor，不能替代revision或content identity。

executor revision/digest/config是Effect ID与request digest的一部分，并逐事件绑定到Intent、dispatch claim、recovery key、opaque lease、Receipt admission、Projection和reopen resolver。registry不变但executor bytes/descriptor替换必须fail closed；same client key切换executor为idempotency mutation。

## 2. Aggregate, events, and state

Effect stream 从完整 scope 的 Run ID 确定性派生为 `stream_effect-<run suffix>`，Event envelope actor仍是 persisted Manifest owner/Kernal authority signer；真实 requester、subject、producer 与 reconciler 写入闭合 payload并在 admission 时验证。

sequence 1 固定 `effect-stream-initialized-v1`：Manifest lifecycle cursor、Effect registry revision/config digest、Boundary recording policy、source SchemaSet/limits、Effect reducer/output reader/history digest revisions。后续 v1 事件：

- `effect-intended`：与 control reserve 原子成 pair；
- `effect-dispatch-claimed`：外部边界前的最后持久事实；
- `effect-receipt-admitted`：有可信 Receipt 时与 control settlement 原子成 pair；
- `effect-attempt-concluded`：确定执行前失败、取消或无 Receipt 的已知结论，与 control settlement原子成 pair；
- `effect-reconciliation-required`：部分、可能已执行、响应丢失或 timeout，与 control settlement原子成 pair；
- `effect-reconciliation-observed`、`effect-reconciled`；
- `effect-late-receipt-observed`、`effect-message-rejected`。

Effect projection 分离两个不可混淆的轴：

```text
dispatch: intended | claimed | concluded
external conclusion:
  pending | applied | not_applied | partial | unknown
reconciliation:
  not_required | required | resolved_applied | resolved_not_applied |
  resolved_partial
```

Runtime Control operation仍只有 `succeeded | failed | cancelled | timed_out` 且不可逆。若 timeout 后对账证明外部实际 applied，Effect reconciliation 可如实记录 `resolved_applied`，但不能把 Runtime Control `timed_out`、预算 settlement 或 Run/Task terminal 改成成功。这样“执行路径失败”和“外部世界最终观察”不会互相覆盖。

## 3. Identity and idempotency

公开请求使用闭合 `EffectRequestV1`，包含 effect kind、subject、可选 Task、规范化 request value、Kernel 接纳的 client idempotency key digest、deadline 与 correlation。明文客户 key 不进入权威 Event；Kernel 在注册策略域中规范化并摘要。

Kernel 从以下 preimage 派生 `EffectIdV1`：

```text
domain, full isolation scope, effect registry revision, effect kind,
effect revision, executor revision/descriptor/config digests,
idempotency-key digest
```

`request_digest` 覆盖 exact request Schema/value、subject/Task、operation contract、deadline policy和registry identity。相同 Effect ID 与 exact digest 返回原 Intent/Projection；相同 key 但任一语义字段不同为稳定 `idempotency_conflict`。跨 scope/kind/revision 的相同客户 key 既不去重也不泄漏另一 Effect。

每个 `AttemptId` 从 Effect ID 与从 1 开始的 ordinal 派生；首片只创建 attempt 1。Kernel 同时派生不含客户明文的 external key token/digest。`unsupported` registration不向 executor承诺去重；`keyed` registration必须通过 Fake contract tests证明相同 external token不重复应用，但首片 Kernel仍不因 crash自动重调。

业务幂等优先级为：integrity/source/isolation → same command/event ID exact/mutation → Effect ID exact/mutation → terminal/reconciliation state → lifecycle/cancel/deadline →领域准入。exact retry不能重新采样 Clock、生成新 attempt或再次调用 executor。

## 4. Authority and call path

```text
untrusted Effect proposal
  -> authenticated principal + persisted v3 Manifest/Task
  -> exact Effect registry + lifecycle/control/effect histories
  -> default-deny Capability + trusted resource envelope + cancellation/deadline
  -> atomic reserve/Intent pair
  -> atomic Effect dispatch claim + persisted recovery key
  -> opaque single-attempt EffectDispatchLease + redacted Fake request view
  -> Fake executor returns untrusted observation
  -> receipt/producer/lease/schema/limits/meter admission
  -> atomic control settlement/Effect conclusion pair
  -> Effect Projection + optional reconciliation
  -> Recorded replay/inventory (read/fold only)
```

公开协议对象、validated request、Effect ID、Receipt bytes或同scope Actor都不构成 authority。`EffectAuthority`、`EffectDispatchLease`、producer handle、recovery authority与reconciliation authority均为 crate-private、不可序列化、用途受限的值。Fake executor无 Event Store、raw SQLite、Capability constructor、budget state、Hook authority、filesystem、network、process或secret访问。

Hook decision既不是 Effect authority也不是 Receipt evidence。后续 Loop可在提交 Effect proposal前执行REQ-0008 Gate；本切片不把 Hook handler变成executor、producer或reconciler。

## 5. Atomic reserve/Intent pair

Runtime Control `OperationReservedPayloadV1` 新增可选 `effect_pair: EffectPairBindingV1`，与既有 `hook_pair` 互斥。retained旧Schema reader仍读取旧payload；新Effect-capable set使用新增optional binding并保持非Effect操作两者皆空。

`EffectReservePairCommandV1` 固定 pair ID/kind/fingerprint、完整scope/owner、control/effect stream及expected cursor、两个确定性Event ID/sequence和完整prepared bytes。control payload绑定operation/reservation/trusted envelope；Effect payload绑定Effect/attempt/idempotency/request/registry/executor/policy、initial process epoch、deadline及`EffectRecoveryBaseKeyV1`。base key同时固定source SchemaSet/limits、operation/reservation/meter/recovery contract，供Intent尚未claim时的确定未执行恢复；counterpart Event交叉绑定。

在单一 `BEGIN IMMEDIATE` 内按 integrity → pair exact/mutation → zero/one/two presence → expected cursor/domain admission → insert control → insert Effect → commit。zero 才写两个；two 只允许双方bytes/fingerprint/cross-reference全等的 exact retry；one 一律 `corrupt_partial_pair`，不得补写、dispatch或自动settle。第二insert和commit fault injection必须证明rollback后两边均无事实。

Intent提交才表示已获准尝试，不表示已执行。pair成功返回后仍需下一节的dispatch claim；Intent commit响应丢失的 exact retry返回既有状态且不自动claim/execute。

## 6. Dispatch claim and Fake executor

Kernel-owned orchestration在调用executor前以单stream `effect-dispatch-claimed` 命令重新fold lifecycle/control/effect，验证 live mode、operation仍reserved、无effective cancellation、deadline未到、attempt未claim且registry/lease source exact。claim commit失败绝不调用 executor。

claim event扩展base key为完整`EffectRecoveryKeyV1`，增加claim Event ID/digest、claim process epoch/Clock sample、executor revision/descriptor/config、external key digest和claim policy。claim成功后 Kernel在同一进程调用 Fake executor一次。lease绑定scope/effect/attempt/operation/reservation/external key digest、executor/producer/adapter、process epoch、deadline、request digest、claim Event与完整recovery key；executor不能保存或委托它。对已经claim的 command，exact retry只返回 `already_claimed/recovery_required`，不再次发放可执行 lease。因此 crash 发生在claim与调用之间会保守地产生unknown，而不是猜测未执行。

Fake executor由 Manifest-pinned compatibility identity解析，确定性模拟：applied receipt、rejected-before-apply、partial、accepted-with-response-loss、timeout、malformed/oversized receipt及return-before-terminal crash。调用计数与按external key的应用计数用于证明Kernel重试和Recorded replay不重复效果，但计数不是权威状态。

## 7. Receipt admission and atomic terminal pair

`EffectReceiptObservationV1` 固定Effect/attempt/external key digest、producer/adapter revision、outcome class、observed-at、safe receipt digest、bounded result摘要、observed usage与limitations。允许的外部 outcome为 `applied | rejected_before_apply | partial | unknown`。它始终是不可信输入。

只有持有exact live dispatch lease与registered producer handle的返回路径可触发terminal admission。Kernel在decode前执行bytes/depth/count上限，再验证Schema、scope、request/external key digest、executor/producer/adapter、Clock/deadline与Kernel meter。错误executor/producer或跨域消息不写目标；正确producer的非法输出形成安全rejected observation并按unknown规则结清。

`OperationSettledPayloadV1` 同样新增与 Hook 互斥的 `effect_pair`。terminal pair根据结论固定：

| Observation/boundary | Runtime outcome/accounting | Effect-side event/conclusion |
|---|---|---|
| applied且deadline前 | succeeded；verified meter consume/release | receipt-admitted / applied |
| rejected-before-apply | failed；verified zero/partial meter | attempt-concluded / not_applied |
| partial | failed；verified partial或unknown全额 | reconciliation-required / partial |
| response loss/possible effect | failed；unknown全额 | reconciliation-required / unknown |
| deadline winner且尚未claim | timed_out；verified zero并release | attempt-concluded / not_applied |
| deadline winner且已经claim | timed_out；verified partial或unknown全额 | reconciliation-required / unknown或partial |
| effective cancellation before claim | cancelled；verified zero | attempt-concluded / not_applied |

`EffectTerminalPairCommandV1`复用reserve pair的zero/two/one规则与相同writer serialization。Effect-bound operation禁止调用通用单stream Runtime Control settlement/timeout入口；callback和下节的recovery都必须生成Effect terminal pair。这样没有合法的control-only catch-up状态，budget gross/释放与Effect conclusion原子一致。

## 7.1 Crash and timeout recovery command

Kernel提供显式、无background scanner的`EffectRecoveryCommandV1`。它只能由crate-private recovery authority从persisted Effect projection构造；临时caller、owner、executor或Receipt不能创建。command固定：base/full recovery key、recovery cause、canonical Clock sample、current process epoch observation、Kernel meter snapshot或`unknown` evidence fingerprint、expected control/effect cursors、terminal pair kind及双方prepared Event preimage。

recovery cause闭合为：`process_epoch_lost | deadline_due | cancellation_effective`。Kernel认证的process epoch observation证明Intent/claim所属旧epoch不再拥有live executor lease；普通字符串不构成证明。domain-separated canonical preimage `pareto.effect-recovery.command.v1` 的SHA-256形成command fingerprint，并确定性派生pair ID、control settlement Event ID和Effect conclusion Event ID。

锁内顺序不可交换：

```text
integrity/source/schema/isolation/recovery-key exact
  -> same command/pair/Event ID exact bytes = AlreadyApplied
  -> same ID mutation = idempotency_conflict
  -> different ID but operation already terminal = ExistingTerminal no-op
  -> eligibility/due/cancellation/process-epoch proof
  -> control/effect expected cursors + domain state
  -> append terminal pair + commit
```

`not_eligible`不append也不消费identity；新Clock/process observation可形成新ID。Intent尚未claim且旧epoch失效、deadline due或取消生效时，history证明Kernel从未交付executor lease，recovery必须以`not_applied`和verified zero usage结清并释放全部reservation，禁止partial/unknown分支。存在claim时任何process loss/cancel/timeout都不能证明外部未执行，必须以`unknown`或persisted verified partial结清并打开reconciliation。claim所属同一live epoch且deadline未到、无cancel时not-eligible。

terminal pair commit响应丢失必须exact重试同command bytes；若command bytes随进程丢失，reopen以新sample/epoch产生新ID并命中ExistingTerminal no-op。claim-before-call、external-applied-before-response和return-before-terminal-pair在history上都落入同一保守unknown语义；recovery不能redispatch、refund或制造Receipt。

## 8. Partial, unknown, late Receipt, and reconciliation

partial记录已确认components/result digest、未知components摘要、limitations与Kernel-meter usage；不得降格为not-applied或整体自动重试。unknown/partial一律打开reconciliation，保留原operation terminal和保守预算。

普通迟到Receipt即使producer正确，也只追加`effect-late-receipt-observed`安全摘要，不能直接关闭reconciliation。关闭必须经`EffectReconciliationCommandV1`：owner只能request；实际结论必须来自registry固定的Fake query adapter/reconciliation producer、绑定原Effect/attempt/external key、包含source observation IDs和canonical evidence fingerprint。首片无人工自证、自动refund、补偿或redispatch。

对账结果为 `confirmed_applied | confirmed_not_applied | confirmed_partial | unresolved`。前三者追加`effect-reconciled`并关闭；unresolved追加observation但保持open。exact command retry幂等，same ID mutation冲突。任何结果都不修改control settlement、budget、lifecycle或已finalized Boundary Inventory；Run terminal后的新事实进入 `BoundaryReconciliationRevision` lineage，但不会改变已finalized inventory固定的source horizon，也不会被只pin该inventory的Recorded replay读取。

## 9. Cancellation, concurrency, and lifecycle success

新的Intent只允许Run `running`且exact Task（若有）`running`。取消/deadline在reserve pair和dispatch claim都重查。Intent已提交但claim之前，取消或deadline recovery由history唯一证明Kernel未交付executor lease，必须以not-applied terminal pair结清并释放；不得走unknown/reconciliation分支。claim之后取消/deadline只能partial/unknown，直到返回、Effect recovery或reconciliation，不虚假ack外部停止。

callback、cancel、timeout、lifecycle transition与terminal pair都在writer transaction内重新fold。deadline equality由timeout获胜；deadline前有效取消与Receipt completion按首个合法commit决定。loser只能返回既有winner或追加late audit，不能双terminal。

对v3 Run，Run/Task进入`succeeded`的transition必须同事务foldEffect stream；目标范围有reserved operation、Intent未claim/结论未定、claim未terminal或open reconciliation时返回`effect_unresolved`。`failed | cancelled`仍要求REQ-0007 operation已settle，但允许保留open reconciliation；后续对账不重开lifecycle。pause阻止新Intent/claim，但不伪装已claim Effect停止。

## 10. Projection, inventory, and replay

`EffectProjectionV1`绑定store ID、完整scope/owner/effect stream/cursor、Manifest source SchemaSet/limits、registry/config/policy、executor revision/descriptor/config、exact reducer/output reader/history digest，按Effect ID排序保存request digest、idempotency digest、attempt/claim/recovery/receipt、operation/reservation/pair、dispatch/external/reconciliation状态与audit counters。projection API必须接受显式inclusive cursor并只读取该horizon；pure fold无Clock、executor、I/O或global mutable state。

既有`BoundaryInventoryRevision`/`BoundaryRecord` 1.0明确把`Failed`定义为receipt前失败，不能承载partial/unknown。REQ-0009因此发布`BoundaryInventoryRevisionV2`和`EffectBoundaryRecordV2`，不原位改变V1：inventory固定source Run、SchemaSet/policy、exact Effect stream inclusive cursor、该horizon的history digest及规范排序records。每个record固定Effect/request/attempt/external key/executor/operation identity，结论闭合为：

```text
applied { receipt_digest, result_digest, limitations }
not_applied { reason_code, limitations }
partial { receipt_digest?, confirmed_components_digest,
          unknown_components_digest, limitations, reconciliation_binding }
unknown { limitations, reconciliation_binding }
cancelled_before_claim { reason_code }
```

`reconciliation_binding`固定inventory finalization时`open | resolved`状态、source reconciliation Event ID（若有）和evidence digest；它不能把partial/unknown编码为V1 Failed。空Effect生成显式空V2 inventory。V2 finalizer只读取指定Effect cursor并验证history digest，不写回source Event、不覆盖inventory。horizon后的late/reconciliation facts只能产生独立`BoundaryReconciliationRevision`，不扩展或替换V2 inventory。

Run Manifest v3的Recorded Effect replay只接受validated V2 inventory；`ExecutionMode`仍pin其immutable revision ID，Kernel按v3 SchemaSet解析V2 reader。replay只读取V2固定inclusive cursor内的source Effect stream，验证history digest、重建Projection并逐项核对Effect records。horizon后追加late/reconciliation Event前后，相同Manifest/inventory pin的projection digest与records必须byte-identical；alternate/current/unpinned inventory或cursor/digest拒绝。API类型图中不存在executor、dispatch lease、Clock、writer、reserve/settle/recovery或reconciliation producer；source/derived event count、budget与Fake counters前后不变。Simulated/Reexecute入口稳定拒绝。

# Interfaces, data flow, and invariants

协议层新增 Effect registry/request/intent/receipt/pair/projection Schema，但不暴露Kernel commands或leases。Kernel新增独立`effect_runtime`模块；只通过`EventStore` crate-private方法访问Event、lifecycle与transaction-local Runtime Control planning。`pareto-protocol`继续不依赖Kernel/SQLite。

核心不变量：Intent先于claim、claim先于executor；proposal/Receipt不是authority；Effect ID exact幂等；同键异请求冲突；pair零或二、单边即损坏；可能已执行不盲重试；partial不丢失；operation terminal与budget不因对账改写；success无未决Effect；Recorded replay无executor；所有scope、SchemaSet、registry与reader exact。

# Failure modes and security

| Failure/threat | Required behavior |
|---|---|
| 无v3/effect registry或current substitution | 初始化前fail closed；v1/v2 Run不后加Effect |
| 自报scope/key/Receipt/usage或confused deputy | persisted scope、registry、Capability、lease、producer和Kernel meter为准；未授权探测不写目标 |
| reserve/Intent第二insert或commit失败 | 整个transaction rollback；无dispatch |
| pair响应丢失 | exact bytes重试返回two-existing；不生成新ID/Clock/attempt |
| 单边pair/resealed counterpart | aggregate corrupt；不补写、不执行、不settle |
| claim前crash | Intent保持；显式恢复可判定未claim，不自动执行 |
| Intent后claim前crash | persisted base recovery key + old epoch proof；terminal pair确定not-applied，不调用executor |
| claim后、调用前或返回前crash | persisted full recovery key；process-loss command保守unknown/reconciliation-required；默认不redispatch |
| external accepted但无响应 | operation保守settle，Effect unknown；对账 |
| partial | 保留confirmed/unknown摘要和partial usage；不整体重试 |
| malformed/oversized/injected Receipt | predecode limits；安全拒绝摘要；unknown保守结算 |
| wrong producer/lease/epoch | 无settlement/目标audit；不能借unknown policy消耗他人预算 |
| cancel/timeout/Receipt race | writer内refold；唯一operation terminal/Effect pair；late只audit |
| reconciliation伪造 | owner request不是evidence；仅pinned producer可关闭；不改budget/lifecycle |
| Run success与unknown Effect竞争 | 同writer fold；至多一个合法顺序，unknown阻止success |
| recovery exact/mutation/response loss | stable key+Clock/epoch/evidence fingerprint；not-eligible不消费；exact pair retry/ExistingTerminal no-op |
| Recorded replay调用live路径或读取horizon后facts | API无executor/lease/writer类型且cursor固定；counter/event/budget/digest断言不变 |
| sensitive request/receipt/error | Event只存safe ID、digest、reason、bounded摘要与redaction revision |
| raw DB owner篡改 | 沿用REQ-0004威胁边界；open/read/fold检测定义内fingerprint/row/pair drift |

# Alternatives considered

1. **先执行再记录Receipt**：少一次写入，但crash后无法知道是否发生且盲重试会重复效果；拒绝。
2. **mutable SQL outbox/status表为authority**：易轮询，但产生第二权威、UPDATE语义和replay歧义；拒绝。首片只用append-only Event stream，且无background poller。
3. **把Intent与外部调用宣称为exactly-once**：SQLite无法覆盖外部事务；拒绝。采用at-most-once-or-reconcile并诚实表达unknown。
4. **复用REQ-0007单流reserve/settle再补Effect Event**：会留下合法control-only事实并使恢复无法判断；拒绝。两个边界都用atomic pair。
5. **把Receipt或Provider usage直接当权威**：允许伪造成功/低报；拒绝。pinned producer只是来源，仍需lease/Schema/meter admission。
6. **claim后自动用相同key重试所有Effect**：某些Provider支持，但`keyed`声明本身不足以证明完整语义；首片拒绝自动redispatch，只对账。
7. **cancel立即标记外部未执行**：不可中断边界上不诚实；拒绝。claim后只表示pending/unknown。
8. **让late Receipt直接翻转timeout/failure**：破坏终态与预算一致性；拒绝。独立reconciliation轴追加事实。
9. **Recorded replay走live API的dry-run flag**：flag漏传会重复效果；拒绝。独立reader/reducer接口。
10. **等待Provider/Tool再设计**：会让各adapter先形成不同幂等、权限和对账语义；拒绝。

# Compatibility, migration, and rollback

发布新的content-addressed Effect-capable SchemaSet、Run Manifest 3.0、Executor Descriptor 1.0与Boundary Inventory/Effect Record 2.0，保留全部历史sets、Run Manifest 1.0/2.0、Boundary Inventory/Record 1.0、RunTask/RuntimeControl/Hook reducer和output readers。新Runtime Control payload set允许mutually-exclusive `hook_pair | effect_pair`；旧set bytes与reader不变。新增Effect bindings对既有reducers是无关成员，任何旧reducer identity变化均为实现阻断项。

SQLite维持v2，不增加table/column/index/trigger，不改变migration checksum、snapshot bytes或Event Store actor规则。若实现发现需要DB v3、多signer actor、mutable outbox或current reader substitution，必须退回Requirement/RFC和独立评审，不得在实施中顺带加入。

首个v3/Effect Event写入前可revert全部设计实现。写入后rollback writer只能停止新Intent/dispatch，同时保留v3 Manifest、Effect/control Schema、pair validator、Effect reader/reducer/inventory解释与对账读取；不能删Event、补单边pair、静默release reservation、把unknown改零或重新执行pending Effect。修复只能用forward Schema/Event/reducer与保留历史解释。

# Evaluation and acceptance

- 质量：协议golden/compatibility、v1/v2/v3 Manifest、Inventory v1/v2、executor descriptor pin、default deny、Intent-before-dispatch、pair fault、幂等mutation、scope矩阵、lease/producer、Fake outcome矩阵、partial/response-loss、Intent/claim/external/pair各crash点、recovery exact/mutation/not-eligible/ExistingTerminal、cancel/deadline/timeout race、unknown/reconciliation、success guard、reopen/fold、fixed-horizon Recorded零执行全部通过；bounded model验证至多一个 terminal pair。
- Token/费用：不调用真实模型/Provider/付费系统；分别记录Fake reserved/accounted usage和全套本地测试成本，不宣称优化。
- 延迟：记录Intent pair、claim、terminal pair、reconciliation、1/10/100 Effect fold、writer竞争、inventory和Recorded replay；busy等待保持有限，无证据前不设收益阈值或新增Snapshot/background worker。
- 设计批准：REVIEW-0012对初始fixed `9f8bf23`提出4个Major；经多轮同Reviewerfocused复审，fixed `b7acbd82824d8410d432117c89be1bd56c8ce05c`关闭F-001至F-004，最终independent approved、0 Blocker、0 Major。ADR-0010接受本RFC；实现后仍需fresh独立代码评审。
