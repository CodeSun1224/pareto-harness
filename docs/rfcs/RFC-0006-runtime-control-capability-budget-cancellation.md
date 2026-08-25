---
id: RFC-0006
title: Runtime Control 的 Capability、预算、取消与超时合同
status: accepted
owners: [maintainers]
created: 2026-08-25
updated: 2026-08-25
links: [REQ-0007, SPEC-0006, EPIC-0002, REQ-0003, REQ-0004, REQ-0005, REQ-0006, RFC-0002, RFC-0003, RFC-0004, RFC-0005, ADR-0001, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ADR-0007, ARCH-0002, ARCH-0003]
---

# Summary

在可信内核内建立一个与 lifecycle stream 分离、同属一个 Run 的 append-only Runtime Control aggregate。sequence-1 初始化事件固定最小 Capability grant、与 Manifest `budget_revision`绑定的多维 budget plan及clock contract；所有授权、委托、撤销、预算reserve/settle/refund、三级取消、deadline、operation terminal和late-result audit均为版本化control Event。Kernel在同一SQLite `BEGIN IMMEDIATE`中从persisted lifecycle/control历史建立authority、默认拒绝、原子预留或确定terminal winner；状态与余额只由pure fold重建。活进程以monotonic deadline执行，持久化和重启以absolute UTC deadline为边界。Recorded control replay只读完整历史，永不调用Fake/真实Operation或重复核算。

# Motivation and requirements

REQ-0005 的owner-only lifecycle authority不是通用Capability，`RunManifest.budget_revision`也不是余额；REQ-0006的Recorded replay只证明Run/Task历史重建且刻意无Effect入口。若后续Hook、Provider、Tool、Sandbox、Loop或WASM各自做权限和预算，插件会成为自己的授权者，多个并发writer会超卖，取消与完成会双终态，Provider自报usage会成为伪造权威，replay会重复执行或扣费。

本RFC满足REQ-0007 AC-01至AC-20，并保持：Event Store唯一权威、Manifest/SchemaSet exact reader、lifecycle状态语义不变、Projection派生、插件只能request、Replay无Effect、quality/cost/latency分开观察。授权、资源、取消、deadline、并发和replay均为trusted-kernel职责。

# Proposed design

## 1. Aggregate and event families

每个Run在完整Manifest scope下将`run_<suffix>`确定性映射为`stream_runtime-control-<suffix>`。该stream sequence 1只能是：

- `runtime-control-initialized-v1`：完整`RuntimeControlInitializedPayloadV1 { lifecycle_cursor, budget_revision, initial_grants, budget_plan, clock_contract }`。

后续v1 typed events：

- `capability-issued`、`capability-revoked`；
- `protected-operation-denied`、`operation-reserved`、`operation-settled`、`budget-refunded`；
- `cancellation-requested`、`cancellation-acknowledged`；
- `late-result-observed`、`control-message-rejected`。

一个command只追加一个control Event。reserve event同时包含授权decision、全部account allocations、operation limit、deadline和soft warnings；因此授权与多账户预留不会分裂。settlement event同时固定terminal outcome、usage authority、accounted/consumed/released向量和callback identity。Event Store继续提供sequence和event-id幂等；Runtime Control在同一事务中额外检查业务ID唯一、父链、余额与状态。

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

## 3. Budget model and ledger invariants

`BudgetDimensionV1`为闭合tag union：tokens、cost_microunits、elapsed_millis、tool_calls、other(name,unit_revision)。amount/limit使用canonical unsigned decimal string，Kernel checked解析为u64；wire无float、negative、NaN或隐式currency conversion。

`BudgetPlanV1`固定Run/Task/Actor accounts和resource-kind+operation per-operation limits。account唯一key为`(account_id, scope, dimension)`；scope subject必须属于same persisted Run。soft limit若存在不得超过hard limit。

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
  -> capability chain and default-deny decision
  -> effective cancellation and deadline decision
  -> checked resolve Run/Task/Actor accounts + operation limit
  -> all hard limits / grant max usage
  -> append one operation-reserved or safe denied audit event
COMMIT
```

不自动rebase requested amount。writer竞争由SQLite串行化，落后者基于新balance判定。软阈值只进入reserve event warning，不改变accounting。

settlement：Kernel trusted meter给出`verified`，未来Receipt/evidence adapter也必须由独立Requirement授权；Provider/Tool/plugin report永远是observation。verified actual不能超过reserved，consume actual并release差额。unknown/unverified对每个维度consume全部reserved，避免低报。失败、取消、timeout可有partial verified usage；terminal原因不推导用量。

refund是新追加event：仅owner management authority，引用existing settlement及refund ID，checked不超过该settlement剩余可退gross usage；exact retry幂等，mutation冲突。refund只降低net余额，不删除gross、不改operation outcome、不重复执行。

## 4. Cancellation and terminal state machine

operation状态：`reserved | succeeded | failed | cancelled | timed_out`；四个terminal互斥不可逆。cancellation projection独立跟踪Run/Task/operation target的`requested`与`acknowledged`；Run/Task cancellation不自动更改RFC-0004 lifecycle状态。

effective cancellation predicate：存在同Run的Run request，或同Task的Task request，或同operation request。它对未来operation也有效，因此Run/Task请求无需枚举或多事件cascade。新reserve先检查predicate并deny。existing cooperative operation得到只读Kernel probe，边界确认后settle cancelled并可追加ack；uninterruptible只保留pending，直到返回或timeout，不能提前宣称ack。

请求取消、确认取消和operation cancelled settlement分别是不同事实：请求表示intent；ack表示执行边界已观察；settlement表示Kernel已确定operation terminal和budget accounting。允许同一个atomic command生成settlement并携带ack reference，但不把三者合并为一个模糊状态。

## 5. Deadline and clock boundary

Kernel-private`Clock`返回`WallSample { canonical_utc, epoch_millis }`和只在当前process epoch内可比较的`monotonic_millis`。生产adapter尚不实现；首片只用FakeClock。Kernel验证wall sample自洽与非回退policy，event持久化canonical UTC，epoch数只用于当前进程计算，不作为跨进程事实。

reserve持久化absolute UTC deadline、requested timeout duration、clock contract revision和decision wall time；返回非持久`OperationLease { process_epoch, monotonic_deadline }`。活进程timeout判定只比较同epoch monotonic tick，同时要求当前wall不晚于absolute deadline。重启丢弃旧lease/tick；重新读取absolute deadline，若wall已到/超过则timeout，否则用剩余duration建立新lease。旧monotonic值不进Event/digest。

若wall sample回退、process epoch不匹配或无法证明remaining duration，Kernel fail closed为`clock_invalid`，不延长deadline。首片无background scanner；timeout在下一次Kernel poll/callback/recovery命令时权威化。

## 6. Completion, cancellation, and timeout race

所有terminal command在`BEGIN IMMEDIATE`内重新fold。

- completion只有在`mono_now < lease.deadline`、wall `< absolute deadline`且无effective cancellation/terminal时可commit success/failure。
- 任一clock恰好deadline或之后，Kernel先commit`timed_out` settlement；callback随后为late。
- deadline前cancel与completion竞争由SQLite commit order决定：第一个合法terminal decision获胜。由于cancel request本身非operation terminal，cooperative policy规定“已提交effective cancel阻止后续success”；completion若先terminal，后续cancel不改变operation。
- 不允许一个event同时声明两个terminal outcome，不允许last-writer-wins。

模型以command序列和可观察commit order为输入；不同调度可得到允许集合中的一个winner，但每条历史fold必须唯一、deterministic且至多一个terminal。

## 7. Idempotency, late results, and audit safety

command event ID沿用Event Store全库幂等；Capability/business IDs还在control aggregate内唯一。callback ID用于外部返回去重：

- exact同ID/同canonical bytes返回原`AlreadyApplied`，不append、不核算；
- 同ID异bytes为generic idempotency conflict；
- 已terminal operation的新callback ID追加`late-result-observed`，只保存safe classification、payload digest、redaction policy revision和received-at；
- pre-reserve settle、unknown operation、重复异义ack等在authenticated aggregate内追加`control-message-rejected`；跨scope不写目标。

audit event进入control history chain但reducer只增加审计计数/索引，不修改grant、budget、operation、cancellation或lifecycle。未来REQ-0009可把late Effect Receipt关联到Boundary reconciliation，但不能改变本RFC既有terminal/budget事实。

## 8. Projection, recovery, and Recorded replay

`RuntimeControlReducerDescriptorV1`固定accepted control bindings、Capability subset、budget equations、cancellation predicate、terminal table、audit no-mutation规则、ordering、history/projection digest与exact output reader。Kernel用persisted control source contract exact resolveretained reducer；request/current/Snapshot不能选择。

`RuntimeControlProjectionV1`固定store/full scope/owner/control stream/cursor/source set+limits/reducer/history digest、initialized plan、sorted grants/revocations/accounts/operations/cancellations、audit counters和projection digest。pure reducer只折叠Event中已决定的clock/authorization/usage事实，不读取current clock。

reopen从sequence1完整read/validate/fold；pending reservation保持pending，不能因crash自动release或reexecute。Recorded replay忽略cache、完整历史只读fold；API不接受Operation executor、Effect callback或append authority。测试在replay前后比较event count、Fake call count及projection每个budget字段逐字节相同。

首片不实现Control Snapshot。REQ-0006 existing lifecycle Snapshot不读取control stream，新增control bindings对RunTask source key是无关set evolution；旧digest保持。

## 9. Downstream interfaces

稳定Kernel-private抽象：

```text
request_capability(proposal) -> signed grant event | denied
authorize_and_reserve(protected_operation) -> OperationLease | decision
cancellation_probe(operation) -> active | requested | terminal
settle(callback, usage_evidence) -> terminal accounting result
refund(settlement, correction) -> budget correction
project_control(target) / recorded_control_replay(target)
```

REQ-0008/0009/0010/0011/0013/0014/0018/0033只能消费这些request/result语义；不得拿到grant constructor、budget mutable handle、Event transaction或replay dispatcher。接口仍为crate-private，后续Requirement可用versioned wrapper公开而不改变内核不变量。

# Interfaces, data flow, and invariants

```text
untrusted policy/plugin/provider/tool request
  -> authenticated principal + persisted Manifest/Task
  -> persisted control first event + exact reader/reducer
  -> default-deny Capability chain
  -> effective cancellation + injected Clock deadline
  -> atomic Run/Task/Actor/operation reserve
  -> Fake Operation boundary
  -> atomic settlement/cancel/timeout/refund
  -> immutable control Event
  -> RuntimeControl Projection
  -> Recorded replay (read/fold only, no operation entry point)
```

核心不变量：插件只能request；owner无operation bypass；child不扩权；event log唯一权威；余额checked且不超卖；provider report非权威；request cancel不等于ack；monotonic不跨重启；terminal唯一不可逆；late audit无状态/预算副作用；replay零执行/零append；所有scope和版本exact。

# Failure modes and security

| Failure | Required behavior and recovery |
|---|---|
| missing/invalid control first event | aggregate unavailable/corrupt；不猜默认grant、budget或clock；owner可在空stream合法初始化 |
| self-signed/widened/cyclic grant | fail closed并在已认证aggregate追加safe denied/rejected audit；无grant state变化 |
| revoked/expired parent | descendant后续reserve拒绝；既有reservation仍可settle，停止需cancel |
| budget overflow/insufficient account | checked failure；同一transaction无部分reserve；不自动缩小request |
| concurrent reserve | SQLite有限等待/序列化；至多余额允许者commit；不得oversell |
| provider usage missing/spoofed | observation不成权威；unknown按reserved全额consume并标limitation |
| crash after reserve | pending保持；reopen Projection显示；禁止自动release/reexecute，受信callback/owner correction后处理 |
| cancel/complete/timeout race | 锁内重fold，按deadline与commit-order规则唯一terminal；loser变terminal conflict/late audit |
| wall/monotonic mismatch | fail closed，不沿用旧process tick或延长deadline；运维修复clock后显式重试 |
| late/duplicate/out-of-order callback | exact retry幂等；mutation conflict；新late只写digest audit且不改budget/outcome/effect |
| unknown Schema/reducer/current substitution | fail closed；部署retained support后重试；不靠compatible/current解释历史 |
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

新增协议Schema/Event binding发布为新内容地址SchemaSet，旧set只读保留。新set保留既有lifecycle 1.0 binding；RunTask reducer只抽取这四个binding，因此descriptor/output digest不得改变。Runtime Control使用独立source key/reducer/output Schema，unknown major和missing retained implementation fail closed。

SQLite保持v2，不增加table/column/index/trigger，不更改v2 ledger checksum、writer epoch或snapshot bytes。control stream通过events表写入，完整scope/SchemaSet/limits同样参与fingerprint/reader validation。旧数据库可原样open；旧Run没有control aggregate时不自动创建或默认授权。

实现发布前可Git revert。首个control Event写入后，rollback writer只能停止新control command，同时保留新SchemaSet、control reader/reducer和DB v2 open；不能删除event、重写grant/balance、把pending reservation静默release、把unknown usage改成零或重释deadline。修复使用新event/schema/reducer或forward DB migration，并保留旧历史解释。

# Evaluation and acceptance

- 质量：Capability table/default deny/delegation widening/revocation/expiry、全scope isolation、多维budget equations、two-writer no-oversell、usage authority、三级cancel、FakeClock deadline、terminal race model、late/duplicate、crash/reopen、exact reader/reducer和replay zero-effect必须通过。
- Token/费用：无模型/Provider/真实Tool；整数usage仅为测试事实。分别记录unknown conservative accounting和本地test cost，不声明成本优化。
- 延迟：记录grant evaluation、reserve/settlement、contention、不同control event/account规模fold和Recorded replay；无baseline前不设收益阈值或新增Snapshot/background worker。
- 设计批准：architecture-review已逐项追踪request→capability→budget→operation boundary→event→projection→recovery，并检查插件绕过、time/replay、isolation、race、compatibility与rollback。专项self-review为0 open Blocker/Major，ADR-0007已接受，SPEC-0006已批准；实施后的fresh independent code review仍是独立门禁。
