---
id: RFC-0006
title: Runtime Control 的 Capability、预算、取消与超时合同
status: accepted
owners: [maintainers]
created: 2026-08-25
updated: 2026-08-25
links: [REQ-0007, SPEC-0006, EPIC-0002, REQ-0003, REQ-0004, REQ-0005, REQ-0006, RFC-0002, RFC-0003, RFC-0004, RFC-0005, ADR-0001, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ADR-0007, ARCH-0002, ARCH-0003, REVIEW-0006]
---

# Summary

在可信内核内建立一个与 lifecycle stream 分离、同属一个 Run 的 append-only Runtime Control aggregate。sequence-1 初始化事件固定最小 Capability grant、与 Manifest `budget_revision`绑定的多维 budget plan、clock contract及Manifest-pinned control source contract；所有授权、委托、撤销、预算reserve/settle/refund、三级取消、deadline、operation terminal和late-result audit均为版本化control Event。Kernel在同一SQLite `BEGIN IMMEDIATE`中从persisted lifecycle/control历史建立authority、执行冻结的lifecycle准入、用retained trusted resource envelope原子预留并确定terminal winner；callback与ack绑定opaque operation lease，timeout可由显式Kernel recovery authority在无callback时权威化。状态与余额只由pure fold重建。活进程以monotonic deadline执行，持久化和重启以absolute UTC deadline为边界。Recorded control replay只读完整历史，永不调用Fake/真实Operation、timeout writer或重复核算。

# Motivation and requirements

REQ-0005 的owner-only lifecycle authority不是通用Capability，`RunManifest.budget_revision`也不是余额；REQ-0006的Recorded replay只证明Run/Task历史重建且刻意无Effect入口。若后续Hook、Provider、Tool、Sandbox、Loop或WASM各自做权限和预算，插件会成为自己的授权者，多个并发writer会超卖，取消与完成会双终态，Provider自报usage会成为伪造权威，replay会重复执行或扣费。

本RFC满足REQ-0007 AC-01至AC-20，并保持：Event Store唯一权威、Manifest/SchemaSet exact reader、lifecycle状态语义不变、Projection派生、插件只能request、Replay无Effect、quality/cost/latency分开观察。授权、资源、取消、deadline、并发和replay均为trusted-kernel职责。

# Proposed design

## 1. Aggregate and event families

每个Run在完整Manifest scope下将`run_<suffix>`确定性映射为`stream_runtime-control-<suffix>`。该stream sequence 1只能是：

- `runtime-control-initialized-v1`：完整`RuntimeControlInitializedPayloadV1 { lifecycle_cursor, budget_revision, initial_grants, budget_plan, clock_contract, source_contract }`；`source_contract`固定Manifest exact SchemaSet/limits、control reducer和output reader identity。

后续v1 typed events：

- `capability-issued`、`capability-revoked`；
- `protected-operation-denied`、`operation-reserved`、`operation-settled`、`budget-refunded`；
- `cancellation-requested`、`cancellation-acknowledged`；
- `late-result-observed`、`control-message-rejected`。

一个command只追加一个control Event。reserve event同时包含lifecycle admission、授权decision、trusted operation contract/envelope、全部account allocations、operation limit、deadline和soft warnings；因此状态准入、授权与多账户预留不会分裂。settlement event同时固定terminal outcome、producer/lease binding、usage authority、accounted/consumed/released向量和callback/recovery identity。Event Store继续提供sequence和event-id幂等；Runtime Control在同一事务中额外检查业务ID唯一、父链、余额与状态。

Control state不写独立表。SQLite v2 `events`已能存新stream，`user_version`、writer epoch、Snapshot表/trigger均不改。RunTask Projection只读lifecycle stream；RuntimeControl Projection只读control stream，避免一个reducer把两套状态语义耦合。

Event Store v2的validated reader要求`EventEnvelope.actor`与`IsolationScope.agent_id`一致。Control aggregate因此明确把envelope actor定义为persisted Manifest owner——即Kernel用来批准并追加事实的authority signer；实际authenticated requester、Capability subject、callback producer和cancellation requester使用各自闭合payload字段并在command admission时exact验证。这样delegated actor只能request而不能直接append，也无需为首片增加actor索引/DB migration；错误地把payload requester复制成envelope authority必须fail closed。未来若要多signer Event stream，需独立DB/协议RFC，不能重释v1 control历史。

## 2. Capability model and trusted issuance

新增不可互换ID：Capability、BudgetAccount、Reservation、Operation、Callback、Cancellation。公开记录均闭合、显式Schema 1.0、拒绝unknown fields/null/default。

`CapabilityGrantV1`：

```text
grant_id, issuer_actor, subject_actor, scope,
task_id?, resource { kind, id? }, operations[],
constraints {
  not_before, expires_at,
  max_operation_usage[],
  allow_delegation, remaining_delegation_depth
},
parent_grant_id?, issued_at
```

Resource首版支持exact kind和可选exact logical ID，不支持glob、路径前缀、regex、network CIDR或arbitrary policy expression；后续Tool/Sandbox把具体资源规范化为版本化selector。operation列表和usage constraints按稳定key排序、唯一、非空。

root issuance authority来自persisted Run Manifest owner与认证principal，不来自grant字段。策略、插件、Hook、Provider、Tool和WASM guest只能提交`CapabilityRequestV1`，Kernel可deny或由受信command签发。`Validated<CapabilityGrantV1>`不等于authority，外部无法构造`ControlAuthority`或append。

delegation subset算法是Kernel不变量：

1. exact加载父grant chain；每个父在child issued-at有效、未撤销且Schema可读；禁止环、缺父或多父。
2. issuer必须是直接父subject；scope exact相同。
3. Task只能保持或从父无Task收窄为同Run具体Task。
4. resource kind必须相同，ID只能保持或从父None收窄为Some。
5. child operations是父非空子集；max usage每维存在于父且不增加。
6. child time窗包含于父；allow_delegation不得false→true；depth严格减一。

Kernel判定每次都重新验证chain、revocation和当前clock；不缓存“曾经有效”结果。父撤销/到期阻止descendant后续reserve，但不回滚已发生效果或隐式取消in-flight operation。

默认拒绝：Manifest owner也必须持有匹配grant才可执行受保护operation。owner身份仅允许初始化、root issuance/revocation、refund等明确Kernel管理命令，不提供通用operation bypass。

## 2.1 Lifecycle admission and cross-stream serialization

Lifecycle不是Capability的替代品，但它是每个control command的前置guard。Kernel只能在Run `created`初始化control；issue/delegate/revoke只允许Run `created | running | paused`，且Task-scoped grant要求Task `created | ready | running | paused`；新的reserve/dispatch只允许Run `running`及exact Task `running`。Run/Task cancellation request只允许相应target非终态；已经`reserved`的operation可在任何后续lifecycle state由受信callback、取消ack或timeout recovery结清，refund与read/replay也可在terminal后工作，但都不能产生新dispatch authority。

为避免lifecycle stream与control stream之间TOCTOU，reserve在同一`BEGIN IMMEDIATE`中exact fold两条history。对已初始化control的Run，Run/Task进入`paused | succeeded | failed | cancelled`的lifecycle transition也必须在相同writer transaction中fold control history；目标范围存在pending `reserved` operation时返回`operation_in_flight`。因此reserve先commit时pause/terminal失败，lifecycle先commit时reserve因state guard失败。旧SchemaSet且没有control aggregate的Run不改变REQ-0005行为。这个guard不把operation cancellation与lifecycle `cancelled`合并，只保证终态/暂停不会与新dispatch交错。

## 3. Budget model and ledger invariants

`BudgetDimensionV1`为闭合tag union：tokens、cost_microunits、elapsed_millis、tool_calls、other(name,unit_revision)。amount/limit使用canonical unsigned decimal string，Kernel checked解析为u64；wire无float、negative、NaN或隐式currency conversion。

`BudgetPlanV1`固定Run/Task/Actor accounts和resource-kind+operation per-operation limits。account唯一key为`(account_id, scope, dimension)`；scope subject必须属于same persisted Run。soft limit若存在不得超过hard limit。

untrusted request中的resource vector只是proposal。Kernel retained registry以exact `(Manifest-pinned source set, resource kind, operation, trusted adapter revision)`解析闭合`TrustedOperationContractV1`；其版本化envelope policy从validated parameters确定性产生覆盖执行的每维最大用量，列出全部required dimensions、Kernel meter与approved callback producer。proposal不能通过低报、零值或漏维度降低这个trusted vector；无法给出有限上界或无法由Kernel meter在越界前阻止执行时，`resource_envelope_unavailable`且不dispatch。首片Fake Operation全部资源访问由Kernel meter mediation，未来真实Provider/Tool必须在各自Requirement证明等价envelope/enforcement后才能注册。

Projection对每个account派生：

```text
reserved = Σ live reservation allocation - Σ settled/released allocation
gross_consumed = Σ settlement accounted usage
refunded = Σ accepted refund
net_consumed = gross_consumed - refunded
available = hard_limit - net_consumed - reserved
```

所有运算checked且必须保持`refunded <= gross_consumed`、`net+reserved <= hard`。per-operation ledger使用相同公式但不跨operation共享；它限制单次请求，Run/Task/Actor accounts限制总量。reserve同时覆盖每个requested dimension的全部applicable scopes；缺少所需account或operation limit为fail closed，而不是“不限额”。

reserve事务顺序：

```text
BEGIN IMMEDIATE
  -> exact lifecycle Manifest/Task read and validation
  -> exact control first event/read/fold
  -> command/event/business-ID idempotency
  -> frozen lifecycle admission
  -> capability chain and default-deny decision
  -> retained trusted operation contract and resource envelope
  -> effective cancellation and deadline decision
  -> checked resolve Run/Task/Actor accounts + operation limit
  -> all hard limits / grant max usage over trusted vector
  -> append one operation-reserved or safe denied audit event
COMMIT
```

不自动rebase或采纳较低requested amount。writer与lifecycle transition竞争由SQLite串行化，落后者基于新state/balance判定。软阈值只进入reserve event warning，不改变accounting。

reserve返回不可序列化的crate-private `OperationLeaseV1`，绑定full scope、operation/reservation、subject、trusted operation/meter revision、approved producer identity、callback namespace、process epoch和deadline。settlement command必须同时持有该opaque lease和注册producer handle；知道ID、拥有同scope actor或提供`Validated`payload都不足以settle。正确producer错误operation/reservation、stale epoch/revoked registration、伪造verified evidence和same-ID mutation均fail closed且不能触发unknown扣账。

settlement：首片只有Kernel trusted meter能给出`verified`；Fake producer仅报告outcome/observation。未来Receipt/evidence adapter必须由独立Requirement注册新的retained evidence class；Provider/Tool/plugin report永远是observation。verified actual不能超过reserved，consume actual并release差额。正确producer返回但Kernel meter不可用时，unknown/unverified对trusted reservation每个维度consume全部，避免低报。未经producer/lease admission的消息不能利用unknown policy扣账。若受信adapter违反meter合同报告actual大于reserved，记录`meter_contract_violation`、fail closed并全额consume reservation；由于首片Kernel meter已在越界前中止Fake执行，不会把事后错误当作hard-limit enforcement。

refund是新追加event：仅owner management authority，引用existing settlement及refund ID，checked不超过该settlement剩余可退gross usage；exact retry幂等，mutation冲突。refund只降低net余额，不删除gross、不改operation outcome、不重复执行。

## 4. Cancellation and terminal state machine

operation状态：`reserved | succeeded | failed | cancelled | timed_out`；四个terminal互斥不可逆。cancellation projection独立跟踪Run/Task/operation target的`requested`与`acknowledged`；Run/Task cancellation不自动更改RFC-0004 lifecycle状态。

v1取消authority冻结如下：Run和Task cancellation request仅Manifest owner可调用；operation request允许Manifest owner或reserved event中固定的exact subject。普通operation Capability不隐含management authority，issuer、同Task actor、Gate、Hook和plugin均只能提交proposal。`request_cancellation_v1`先做完整isolation，再从persisted owner/operation取requester class；未授权或跨域不写目标stream。未来若允许Capability-based management，必须新增显式resource/operation Schema与独立RFC，不能重释v1。

effective cancellation predicate：存在同Run的Run request，或同Task的Task request，或同operation request。它对未来operation也有效，因此Run/Task请求无需枚举或多事件cascade。新reserve先检查predicate并deny。existing cooperative operation持opaque lease得到只读Kernel probe；probe本身无writer。ack只能由reserve时批准的producer携带exact live lease，或由Kernel timeout/recovery authority形成，并绑定cancellation/operation/reservation/producer/process epoch。边界确认后settle cancelled并可追加ack；uninterruptible只保留pending，直到返回或timeout，不能提前宣称ack。

请求取消、确认取消和operation cancelled settlement分别是不同事实：请求表示intent；ack表示执行边界已观察；settlement表示Kernel已确定operation terminal和budget accounting。允许同一个atomic command生成settlement并携带ack reference，但不把三者合并为一个模糊状态。

## 5. Deadline and clock boundary

Kernel-private`Clock`返回`WallSample { canonical_utc, epoch_millis }`和只在当前process epoch内可比较的`monotonic_millis`。生产adapter尚不实现；首片只用FakeClock。Kernel验证wall sample自洽与非回退policy，event持久化canonical UTC，epoch数只用于当前进程计算，不作为跨进程事实。

reserve持久化absolute UTC deadline、requested timeout duration、clock contract revision和decision wall time；返回非持久`OperationLease { process_epoch, monotonic_deadline }`。活进程timeout判定只比较同epoch monotonic tick，同时要求当前wall不晚于absolute deadline。重启丢弃旧lease/tick；重新读取absolute deadline，若wall已到/超过则timeout，否则用剩余duration建立新lease。旧monotonic值不进Event/digest。

若wall sample回退、process epoch不匹配或无法证明remaining duration，Kernel fail closed为`clock_invalid`，不延长deadline。首片无background scanner。

reserve event同时固定`TimeoutKeyV1`：schema/recovery contract revision、完整scope/control stream、operation/reservation、absolute deadline、timeout/clock policy、source SchemaSet/limits digest与trusted operation/meter contract ref。Kernel在每次recovery逐字段对照persisted state，外部不能选择或替换。`TimeoutRecoveryCommandV1`再固定该key、canonical decision Clock sample和构造时冻结的Kernel-meter verified snapshot或`unknown` evidence fingerprint；domain-separated canonical JSON preimage `pareto.runtime-timeout-recovery.command.v1`的SHA-256既是`command_fingerprint`，其lowercase hex加`event_`也是command/event ID。recovery authority是不可序列化constructor权限，不进入wire fingerprint。

显式`reconcile_operation_timeout_v1`只能由Kernel recovery authority构造，可从live poll、callback admission或数据库reopen recovery调用；它在`BEGIN IMMEDIATE`中按以下优先级处理：integrity/isolation/source/expected key → existing same event ID exact fingerprint/bytes为`AlreadyApplied`、mutation为`idempotency_conflict` → different event ID但operation已terminal则返回既有winner no-op → pending时才判Clock。deadline前`not_due`不append也不消费ID；新sample可形成新ID。恰好/超过deadline则以该ID追加唯一`timed_out` settlement，有冻结的durable Kernel meter evidence时consume verified partial并release差额，否则unknown全额consume trusted reservation。提交响应丢失时调用方必须缓存并exact重试同command bytes；若进程丢失command，reopen以新sample/新ID命中terminal no-op。无callback的hung/uninterruptible operation因此可终结；reopen从Projection枚举pending逐个调用，不自动release/reexecute。

## 6. Completion, cancellation, and timeout race

所有terminal command在`BEGIN IMMEDIATE`内重新fold。

- completion只有在`mono_now < lease.deadline`、wall `< absolute deadline`且无effective cancellation/terminal时可commit success/failure。
- 任一clock恰好deadline或之后，callback admission或显式timeout recovery先commit`timed_out` settlement；callback随后为late。
- deadline前cancel与completion竞争由SQLite commit order决定：第一个合法terminal decision获胜。由于cancel request本身非operation terminal，cooperative policy规定“已提交effective cancel阻止后续success”；completion若先terminal，后续cancel不改变operation。
- 不允许一个event同时声明两个terminal outcome，不允许last-writer-wins。

模型以command序列和可观察commit order为输入；不同调度可得到允许集合中的一个winner，但每条历史fold必须唯一、deterministic且至多一个terminal。

## 7. Idempotency, late results, and audit safety

command event ID沿用Event Store全库幂等；Capability/business IDs还在control aggregate内唯一。callback ID用于外部返回去重：

- exact同ID/同canonical bytes返回原`AlreadyApplied`，不append、不核算；
- 同ID异bytes为generic idempotency conflict；
- 只有通过原producer/opaque-lease binding的已terminal operation新callback ID可追加`late-result-observed`，只保存safe classification、payload digest、redaction policy revision和received-at；未授权producer不写目标；
- pre-reserve settle、unknown operation、重复异义ack等在authenticated aggregate内追加`control-message-rejected`；跨scope不写目标。

audit event进入control history chain但reducer只增加审计计数/索引，不修改grant、budget、operation、cancellation或lifecycle。未来REQ-0009可把late Effect Receipt关联到Boundary reconciliation，但不能改变本RFC既有terminal/budget事实。

## 8. Projection, recovery, and Recorded replay

`RuntimeControlReducerDescriptorV1`固定accepted control bindings、Capability subset、budget equations、cancellation predicate、terminal table、audit no-mutation规则、ordering、history/projection digest与exact output reader。Kernel用persisted control source contract exact resolveretained reducer；request/current/Snapshot不能选择。

`RuntimeControlProjectionV1`固定store/full scope/owner/control stream/cursor/Manifest-pinned source set+limits/reducer/history digest、initialized plan与operation-contract refs、sorted grants/revocations/accounts/operations/cancellations、audit counters和projection digest。sequence-1与每个row必须exact等于Manifest source；pure reducer只折叠Event中已决定的clock/authorization/usage事实，不读取current clock。

reopen从sequence1完整read/validate/fold；pending reservation保持pending，不能因crash自动release或reexecute。Recorded replay忽略cache、完整历史只读fold；API不接受Operation executor、Effect callback或append authority。测试在replay前后比较event count、Fake call count及projection每个budget字段逐字节相同。

首片不实现Control Snapshot。REQ-0006 existing lifecycle Snapshot不读取control stream，新增control bindings对RunTask source key是无关set evolution；旧digest保持。

## 9. Downstream interfaces

稳定Kernel-private抽象：

```text
request_capability(proposal) -> signed grant event | denied
authorize_and_reserve(protected_operation) -> OperationLease | decision
request_cancellation(target, proposal) -> requested | denied | terminal
cancellation_probe(operation) -> active | requested | terminal
acknowledge_cancellation(operation_lease, cancellation) -> acknowledged | denied
settle(operation_lease, callback, usage_observation) -> terminal accounting result
reconcile_operation_timeout(recovery_authority, operation, clock_sample, frozen_meter_snapshot) -> not_due | terminal
refund(settlement, correction) -> budget correction
project_control(target) / recorded_control_replay(target)
```

REQ-0008/0009/0010/0011/0013/0014/0018/0033只能消费这些request/result语义；不得拿到grant constructor、operation lease constructor、producer registry、recovery authority、budget mutable handle、Event transaction或replay dispatcher。接口仍为crate-private，后续Requirement可用versioned wrapper公开proposal/result，但要取得producer/resource-envelope authority必须另行受审，不能改变内核不变量。

# Interfaces, data flow, and invariants

```text
untrusted policy/plugin/provider/tool request
  -> authenticated principal + persisted Manifest/Task
  -> Manifest-pinned control first event + exact reader/reducer
  -> lifecycle admission
  -> default-deny Capability chain
  -> retained trusted operation contract/enforced resource envelope
  -> effective cancellation + injected Clock deadline
  -> atomic Run/Task/Actor/operation reserve
  -> opaque lease + approved Fake producer boundary
  -> authorized callback/cancel or Kernel timeout recovery/refund
  -> immutable control Event
  -> RuntimeControl Projection
  -> Recorded replay (read/fold only, no operation entry point)
```

核心不变量：插件只能request；owner无operation bypass；child不扩权；lifecycle非运行态无新dispatch；event log唯一权威；trusted envelope在effect前覆盖并约束执行；余额checked且不超卖；provider report非权威；cancel/ack/callback/recovery各有独立authority；request cancel不等于ack；monotonic不跨重启；terminal唯一不可逆；late audit无状态/预算副作用；replay零执行/零append；Manifest、所有scope和版本exact。

# Failure modes and security

| Failure | Required behavior and recovery |
|---|---|
| missing/invalid control first event | 只有Manifest固定control-capable set且Run `created`时owner可在空stream初始化；旧set为`control_schema_not_pinned`；非法first event aggregate corrupt |
| self-signed/widened/cyclic grant | fail closed并在已认证aggregate追加safe denied/rejected audit；无grant state变化 |
| lifecycle state不准入或transition/reserve竞争 | 同一writer transaction exact fold两条stream；非running reserve无append/dispatch；pending operation阻止pause/terminal |
| revoked/expired parent | descendant后续reserve拒绝；既有reservation仍可settle，停止需cancel |
| proposal低报/漏维度或无trusted envelope | Kernel使用retained contract的完整上界；不能证明/enforce上界则effect前拒绝 |
| budget overflow/insufficient account | checked failure；同一transaction无部分reserve；不自动缩小request |
| concurrent reserve | SQLite有限等待/序列化；至多余额允许者commit；不得oversell |
| provider usage missing/spoofed | observation不成权威；只有approved producer+opaque lease能触发settlement；unknown按trusted reserved全额consume |
| callback/ack producer伪造或错误binding | 同scope身份/业务ID不足以授权；producer、operation、reservation、lease、epoch、namespace任一不匹配均无settlement/目标audit |
| crash after reserve | pending保持；reopen Projection显示；显式Kernel recovery逐项处理已到deadline，禁止自动release/reexecute |
| 无callback operation到期 | timeout recovery锁内追加唯一timed-out settlement；无verified meter时unknown全额consume；late callback不能翻转 |
| timeout recovery响应丢失/identity重用 | TimeoutKey+Clock sample+冻结evidence确定性派生event ID/fingerprint；exact bytes重试AlreadyApplied；same-ID mutation冲突；not_due不消费；different-ID terminal no-op |
| cancel/complete/timeout race | 锁内重fold，按deadline与commit-order规则唯一terminal；loser变terminal conflict/late audit |
| 未授权cancel request/ack | Run/Task仅owner、operation仅owner/subject、ack仅approved lease/recovery；跨域和未授权不写目标 |
| wall/monotonic mismatch | fail closed，不沿用旧process tick或延长deadline；运维修复clock后显式重试 |
| late/duplicate/out-of-order callback | exact retry幂等；mutation conflict；新late只写digest audit且不改budget/outcome/effect |
| Manifest/control set mismatch、unknown Schema/reducer/current substitution | fail closed；旧set Run不后加control；部署retained support后重试；不靠compatible/current解释历史 |
| cross-scope ID reuse | exact authority与SQL key拒绝，不泄漏目标；payload字段无权限语义 |
| replay invocation | 只有read/reducer依赖；Fake/Effect count、event count和budget全不变 |
| malicious raw DB writer | 沿用REQ-0004边界：不声称抗拥有文件写权限且可重算所有摘要者；open/read检测定义内row/trigger/fingerprint drift |

# Alternatives considered

1. **继续owner-only并让各Tool自行判断**：实现少，但默认允许、无法委托/撤销且形成多个绕过点；拒绝。
2. **Capability作为可传递bearer JSON/token**：跨模块方便，但拿到bytes即可冒充authority、撤销和scope binding困难；拒绝。公开grant只是事实，Kernel persisted admission才是authority。
3. **可编程ABAC/Rego policy首发**：表达力强但引入解释器、策略版本、termination和WASM安全面；没有当前证据，拒绝。选择闭合exact selector和subset规则。
4. **每个预算scope独立stream/account row**：并行性较高，但一次operation需要跨stream原子reserve、恢复与batch idempotency；首片拒绝，单Run control stream串行化全部适用账户。
5. **mutable balance/operation表作authority，Event仅审计**：读取快但产生双权威和replay歧义；拒绝。只fold Event。
6. **先执行Operation，再按Provider usage扣费**：减少reservation失败，但会超卖且Provider自报可伪造；拒绝。先reserve，unknown保守consume。
7. **取消请求立即把Run/Task/operation标cancelled**：混淆intent与实际停止，对不可中断边界作虚假承诺；拒绝。request、ack、settlement分离。
8. **只用wall clock**：可持久但受时钟跳变影响live timeout；拒绝。live monotonic+persisted absolute UTC边界。
9. **持久化monotonic tick跨重启**：不同process/boot不可比较，会延长或提前deadline；拒绝。
10. **终态后丢弃所有late callback**：状态安全但丢失审计/对账；拒绝。保存脱敏digest audit且隔离副作用。
11. **Recorded replay调用相同live service但传dry-run flag**：flag可漏传或未来分支执行效果；拒绝。独立read/reducer API无executor类型。
12. **状态 quo、等REQ-0008/0009一起实现**：会让Hook/Effect先形成未授权入口，之后难以收紧；拒绝。

# Compatibility, migration, and rollback

新增协议Schema/Event binding发布为新control-capable内容地址SchemaSet，旧set只读保留。新set保留既有lifecycle 1.0 binding；RunTask reducer只抽取这四个binding，因此descriptor/output digest不得改变。只有Manifest已固定该新set/limits的Run能在`created`初始化control；sequence-1及每个后续row必须exact等于Manifest，known alternate/current substitution、stream内set或limits漂移均fail closed。四个旧set Run不得后加control，但其既有lifecycle/Projection/Snapshot保持兼容。Runtime Control使用独立source key/reducer/output Schema，unknown major和missing retained implementation fail closed。

SQLite保持v2，不增加table/column/index/trigger，不更改v2 ledger checksum、writer epoch或snapshot bytes。control stream通过events表写入，完整scope/Manifest SchemaSet/limits同样参与fingerprint/reader validation。旧数据库可原样open；旧Run没有control aggregate时不自动创建、升级SchemaSet或默认授权。

实现发布前可Git revert。首个control Event写入后，rollback writer只能停止新control command，同时保留新SchemaSet、control reader/reducer和DB v2 open；不能删除event、重写grant/balance、把pending reservation静默release、把unknown usage改成零或重释deadline。修复使用新event/schema/reducer或forward DB migration，并保留旧历史解释。

# Evaluation and acceptance

- 质量：Capability table/default deny/delegation widening/revocation/expiry、lifecycle全状态准入及transition/reserve竞争、全scope isolation、多维budget equations、trusted envelope低报/漏维度、two-writer no-oversell、callback/cancel authority、无callback timeout recovery、TimeoutKey/ID/fingerprint golden、not_due/commit-loss/same-ID mutation/different-ID terminal优先级、FakeClock deadline、terminal race model、late/duplicate、Manifest/control set binding、crash/reopen、exact reader/reducer和replay zero-effect必须通过；每个test filter须先证明非零命中。
- Token/费用：无模型/Provider/真实Tool；整数usage仅为测试事实。分别记录unknown conservative accounting和本地test cost，不声明成本优化。
- 延迟：记录grant evaluation、reserve/settlement、contention、不同control event/account规模fold和Recorded replay；无baseline前不设收益阈值或新增Snapshot/background worker。
- 设计批准：REVIEW-0006首轮fresh independent review对`05dd7ca`给出0 Blocker、6 Major和`changes-requested`，Runtime实施暂停。本修订逐项补齐F-001至F-006所需的durable contract；必须由同一independent reviewer对固定修订commit复审、重做AC trace并确认0 open Blocker/Major后才可实施。实施后的fresh independent code review使用新的Review ID，仍是另一道门禁。
