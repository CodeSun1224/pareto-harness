---
id: SPEC-0006
title: Capability、预算、取消与超时规范
status: approved
owners: [maintainers]
created: 2026-08-25
updated: 2026-08-25
links: [REQ-0007, EPIC-0002, REQ-0003, REQ-0004, REQ-0005, REQ-0006, RFC-0006, ADR-0001, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ADR-0007, ARCH-0002, ARCH-0003]
---

# Behavioral contract

Runtime Control 是可信内核拥有的第二个 event-sourced Run aggregate。每个 Run 使用由 Run ID 确定性派生的 control stream；sequence 1 是 `runtime-control-initialized`，原子固定最小 Capability、budget plan、clock/deadline contract 与 persisted lifecycle binding。后续 capability、budget reservation/settlement/refund、cancellation、operation outcome 和 late-result audit 事实只追加到该 stream。授权、余额、取消与操作状态只能由 exact validated control Event range 的版本化 pure fold 得出；不存在可被插件、策略或 callback 直接修改的第二权威状态。

受保护操作的调用链固定为：认证主体和 exact Run/Task target → persisted lifecycle/control bootstrap → Capability 判定 → cancellation/deadline 判定 → 同事务原子预算 reserve → Fake Operation boundary → 同事务 settlement 或取消/timeout → 版本化 Projection。Recorded replay 只读取和折叠，不能取得 Fake Operation 或未来 Effect executor。

# Inputs, outputs, states, and failure behavior

## Aggregate, identities, and inputs

- control stream 由 `run_<suffix>` 派生为 `stream_runtime-control-<suffix>`；caller、payload 和插件不能选择另一个 target。完整 Event Store isolation key仍为 tenant、user presence/value、workspace、run、owner agent 与 stream。
- 沿用Event Store v2既有actor合同，所有control `EventEnvelope.actor`都是persisted Manifest owner，即批准并追加事实的aggregate authority signer；实际发起者、Capability subject、callback producer和cancellation requester分别进入闭合payload字段并由Kernel对认证principal exact验证。不得把payload requester当成Event Store authority，也不得伪称delegate直接append了权威Event。
- 新强类型 ID：`CapabilityId(cap_)`、`BudgetAccountId(budget_)`、`ReservationId(reservation_)`、`OperationId(operation_)`、`CallbackId(callback_)`、`CancellationId(cancel_)`；均不可跨类型互换，ID 复用不能成为跨 scope capability。
- `RuntimeControlInitCommand` 只能由 Manifest owner authority 构造，固定 operation event ID、完整 Capability grant、budget revision/plan、clock contract和 expected empty stream；grant subject 可以是 owner 或同 scope 的受托 actor，但 signer authority 不能来自 grant payload。
- `ProtectedOperationRequest` 固定 command/event ID、operation/reservation ID、subject actor、Task、exact resource selector、operation name、requested resource vector、callback contract、interruptibility、absolute UTC deadline 与 timeout duration。所有字段在首次请求时固定，重试不得重新取时钟、预算或 ID。
- `CapabilityCommand` 支持 issue/delegate/revoke；`CancellationCommand` 支持 Run/Task/operation target；`SettlementCommand` 固定 callback ID、terminal outcome、verified/unknown usage、reason 与 clock sample；`RefundCommand` 固定 owner authority、原 settlement和修正向量。
- public protocol types只表达闭合数据；authority-bearing command constructors、Event Store transaction、fold seed、Fake Operation dispatch 与 replay executor均保持 crate-private。

## Capability contract

`CapabilityGrantV1` 固定：schema identity、grant ID、issuer、subject actor、完整 scope、可选 Task ID、exact `ResourceSelectorV1 { kind, id }`、排序去重的 operations、`CapabilityConstraintsV1 { not_before, expires_at, max_operation_usage, allow_delegation, remaining_depth }`、可选 parent grant与issued-at。时间均为 canonical UTC millisecond timestamp；`not_before < expires_at`。

root issuance 只由 Manifest owner的 trusted authority批准。delegation 必须证明：父 grant存在、当前有效、未撤销，issuer正是父 subject，父允许 delegation且remaining depth大于零；child scope exact相同，Task只能从无到具体或保持相同，resource selector只能 exact保持或从kind-only收窄到exact ID，operation 是父集合非空子集，时间窗包含于父，max usage逐维不增加，allow_delegation不能由false变true，depth严格下降。任何无法证明的约束关系 fail closed。

判定按以下顺序产生`AuthorizationDecisionV1`：aggregate/reader integrity → caller isolation/authority → cancellation/deadline → grant chain存在性与版本 → not-before/expiry/revocation → subject/Task/resource/operation exact match → grant usage constraints → allow。不存在匹配 grant 即 `capability_missing`，不是 implicit owner allow。revocation影响后续新 reserve；已提交 reservation 仍可 settlement，但若需停止必须另发 cancellation。

已认证同 aggregate 的业务拒绝追加`protected-operation-denied`，只保存decision ID、safe target IDs、resource kind、operation、reason、clock time与request digest；不保存秘密或任意 callback payload。无法通过完整 isolation/owner bootstrap 的探测只返回通用`unauthorized`且不写目标 stream。

## Budget contract

`BudgetPlanV1` 绑定 Manifest 的 `budget_revision`，包含排序唯一的：

- `BudgetAccountV1 { account_id, scope, dimension, hard_limit, soft_limit? }`，scope 为 Run、Task或Actor；
- `OperationBudgetLimitV1 { resource kind, operation, dimension, hard_limit, soft_limit? }`，每个 operation 独立建立账本上限；
- dimension 为 `tokens | cost_microunits | elapsed_millis | tool_calls | other { name, unit_revision }`。

所有 unit 使用canonical decimal string并在Kernel checked-convert为`u64`；相加、相减或转换溢出 fail closed。对每个account维护派生的`reserved`、`gross_consumed`、`refunded`和`net_consumed=gross-refunded`；可用量为`hard_limit-net_consumed-reserved`。soft limit仅在`net+reserved+request > soft`且未超过hard时产生warning，不拒绝。

`operation-reserved` 在一个`BEGIN IMMEDIATE`内读取并fold lifecycle和control history、完成Capability/cancellation/deadline检查，解析全部适用Run/Task/Actor accounts及per-operation limit，证明每个维度可用后一次追加完整allocation event。任何维度失败不留下部分reservation。两个writer竞争由SQLite写锁串行化，落后者在锁内看到新reserved后允许或拒绝，不能超卖。

settlement规则：

- `verified` usage必须由Kernel deterministic meter或后续受信Receipt/evidence adapter提供，且每维不超过reserved；accounted等于actual，差额release。
- `unknown`或只有provider/tool/plugin自报usage时，reported值只作observation；accounted等于该维全部reserved并记录`unverified_usage` limitation。
- outcome `succeeded | failed | cancelled | timed_out` 均可带已完成部分的verified usage；失败不等于零消耗，取消/timeout也不自动refund。
- settlement一次性把reserved转为gross consumed并release差额；operation终态后不可再次settle。
- refund只由owner authority对一个既有settlement追加，累计不得超过该settlement的gross accounted usage；它增加refunded、降低net，不删除历史、不更改operation outcome且不产生新执行资格。

## Cancellation, deadline, and race contract

Cancellation target是Run、Task或operation。`cancellation-requested`记录ID、requester、reason、requested-at；Run请求对该Run所有当前/后续Task和operation有效，Task请求对该Task全部operation有效，operation请求只影响自身。effective cancellation阻止新reservation；对已reserved cooperative operation设置pending，由执行边界probe并以`cancellation-acknowledged`或cancelled settlement确认；uninterruptible operation只记录pending，返回Kernel或timeout前不得声称已停止。

取消状态与lifecycle state分离：request/ack不自动追加Run/Task lifecycle transition，Task/Run `cancelled`仍按RFC-0004显式状态机命令完成。失败、取消、timeout、成功是不同operation terminal outcome；reason code与initiator保留。

`Clock` 是Kernel-private可注入接口，提供canonical UTC wall sample与进程内monotonic tick。持久Event只保存absolute UTC deadline、timeout policy和wall observation；活进程返回的`OperationLease`保存monotonic deadline并优先用于timeout。重启不持久化或比较旧monotonic tick，而以当前trusted wall sample判断absolute deadline；未过期时建立新monotonic deadline。Clock regression或无法建立可信sample fail closed，不延长已过期deadline。

胜负规则：

1. completion只在Kernel接收时`monotonic_now < deadline`、persisted wall未过absolute deadline且没有更早提交的effective cancellation/terminal event时可成功；恰好deadline为timeout。
2. deadline前 cancellation 与 completion并发时，`BEGIN IMMEDIATE`序列化后第一个合法commit决定；completion先commit则后续cancel返回terminal no-op，cancel先commit则后续成功不能成为authoritative outcome。
3. timeout、cancel acknowledgement或completion都在锁内先重新fold；至多一个terminal event。exact command/callback retry返回原结果，same ID异内容冲突。

## Late, duplicate, and out-of-order results

对已terminal operation：完全相同callback ID/bytes返回`AlreadyApplied`；same callback ID异bytes返回`idempotency_conflict`；新callback ID只追加`late-result-observed`，内容为operation/callback ID、terminal outcome、safe classification、payload digest、received-at和redaction policy ID，不保存任意敏感payload。该事件不触发Fake Operation、不改变gross/net/reserved、operation outcome、cancellation或lifecycle。

对尚不存在的operation、未reserve即settle、重复ack或乱序消息，只有caller已通过目标aggregate authority时才追加相同脱敏的`control-message-rejected`审计事实；exact retry幂等，异义复用冲突。跨scope消息不写目标aggregate。

## Runtime Control Projection and replay

`RuntimeControlProjectionV1` 绑定source store ID、完整scope/owner/control stream、inclusive cursor、source SchemaSet/limits、exact reducer ref、history-chain state、initial capability/budget plan、sorted grants/revocations/accounts/operations/cancellations、late/rejected audit counters与projection digest。Reducer无clock/I/O/random/global mutable state；time decisions作为Event事实折叠，Projection不重新读取当前Clock。

Projection完整历史fold是正确性oracle；首片不创建Control Snapshot。`recorded_control_replay`总是完整读取exact source control stream并构建Projection，不调用Fake Operation、不append Event、不reserve/settle/refund。普通与Recorded结果只有完整provenance相同才可比较。REQ-0006现有RunTask Projection/Snapshot只读取lifecycle stream，必须保持字节和行为兼容。

## Stable errors

稳定类别至少包含：`unauthorized | aggregate_not_found | aggregate_corrupt | schema_unavailable | reducer_unavailable | capability_missing | capability_inactive | capability_revoked | delegation_widening | budget_exhausted | budget_conflict | cancellation_pending | deadline_exceeded | terminal_conflict | idempotency_conflict | usage_unverified | invalid_usage | clock_invalid | late_result_recorded | busy | io`。错误不回显Capability详情、budget余额、Manifest、payload、SQL、DB path、其他scope identity或秘密。

# Impact analysis

| Dimension | Finding | Evidence / response |
|---|---|---|
| Direct | `pareto-protocol`需增加6类强ID、Capability/Budget/clock/operation/control projection闭合类型、约14类control payload/EventTypeBinding与新内容地址SchemaSet；`pareto-kernel`需新增crate-private runtime-control module、pure fold、transactional commands、FakeClock/FakeOperation测试与Recorded control replay | `crates/pareto-protocol/src/types.rs:1-1093`现有ID/lifecycle/projection类型；`schema.rs:38-151`集中生成set/bindings；`validation.rs:202-226`仅有4个builtin lifecycle decoder；Kernel当前只有event_store/lifecycle/projection modules |
| Indirect | REQ-0008 Gate/Transform必须只能请求控制决策；REQ-0009 Effect必须在reserve后产生intent并以receipt settlement；REQ-0010 Provider usage非权威；REQ-0011 Tool资源映射到selector；REQ-0013 Sandbox enforcement消费已批准capability；REQ-0014 Loop消费取消probe；REQ-0018 DAG映射Task预算/取消；REQ-0033 WASM host calls只能走相同request API | `requirement-backlog.md`依赖顺序；`kernel-constitution.md`权限/资源/取消不变量；本Spec冻结request/decision而不实现下游执行器 |
| Call/permission | 当前`KernelAuthority::authenticated`和`LifecycleTarget`均可在crate内由scope/actor构造，适合tests但不能成为通用Capability。Runtime Control必须从persisted sequence-1 Manifest和control first event建立，不允许调用方自报grant/balance | `event_store.rs:179-224`; `lifecycle.rs:421-589`; REVIEW-0003 F-001/F-005与REVIEW-0004 AC-04已把self-reported authority列为Major风险 |
| Data isolation | Event Store sequence key含tenant/user/workspace/run/owner-agent/stream且现有reader要求envelope actor=scope owner；新增control stream保持owner signer，delegated requester/subject作为closed payload并由Kernel对认证principal检查。所有grant/account/operation ID只能在aggregate内解析 | `event_store.rs:31-51,621-657,700-747`; lifecycle/projection逐字段负测；新增owner-signer/requester、scope、subject、Task、ID复用矩阵 |
| API/schema | 新事件、Capability/Budget/Projection和摘要是持久合同；enum、units、subset规则、conflict priority、deadline边界和redaction语义难逆 | 使用1.0 closed Schema、排序唯一向量、canonical decimal和explicit reducer descriptor/golden；breaking变化新major并保留old reader/reducer |
| Persistence/replay | 不新增权威表；control state/balance只fold Event。Recorded replay必须走只读reader/reducer，不能复用live reserve或Fake dispatcher。现有Snapshot表只缓存RunTask Projection且不得注入control state | ADR-0004 append-only、ADR-0005 event-sourced state、ADR-0006 replay无Effect；新增event-count/budget-before-after/fake-call-counter断言 |
| Concurrency | reserve、settle、cancel/complete/timeout与refund若锁外检查会TOCTOU或双终态；全部需同一`BEGIN IMMEDIATE`中读取lifecycle+control、fold、判定并append一个event | `lifecycle.rs:178-378`已有同事务模式；two-pool barrier、防超卖与race model tests |
| Security | 默认允许、自签grant、delegation widening、revocation bypass、confused deputy、resource wildcard、budget integer overflow、provider usage spoofing、cross-scope refund、late payload泄密是主要威胁 | private authority、exact persisted scope、checked decimal、parent subset proof、safe audit digest/redaction、negative matrix和architecture review |
| Failure/crash | reserve后进程崩溃会留下pending reservation和unknown operation；不能自动reexecute或假装release。reopen projection显示pending，受信reconciliation再settle/owner refund | real SQLite close/reopen tests；同callback retry；unknown usage保守consume；no automatic timeout worker/effect retry |
| Compatibility/migration | Event Store DB v2足以保存新stream，无DDL。新SchemaSet添加control bindings；现有RunTask source key只抽取四个lifecycle bindings，因此新增无关binding不得改变existing reducer identity | `projection.rs:25-45,309-377`显式只取4类lifecycle binding；schema retention/generation diff和old Run/Snapshot tests |
| Time | wall clock可持久但受调整影响，monotonic稳定但不能跨重启；混用会导致deadline延长或误timeout | 注入Clock；live lease仅保存monotonic；Event保存absolute UTC；restart重新采样且不复用旧tick；全部测试无sleep |
| Performance | 每个control command完整fold两条单Run stream且SQLite单writer；多维account检查和canonical digest增加CPU/bytes | 首片正确性优先，记录1/10/100/1000 events、account数量和two-writer观察；无证据前不加Snapshot/background worker |
| Cost/token | 无真实模型/Provider；测试使用整数token/cost和Fake meter。未知usage保守计入会高估净成本但避免低报 | 分开记录accounted/observation/limitation；不宣称优化，后续Effect/Provider reconciliation独立Requirement |
| Dependency/operations | 现有serde/sha2/sqlx/Tokio test依赖足够；预期不新增第三方依赖或DB migration。Clock解析/production scheduler均延期 | Cargo diff由review检查；若需要chrono/background service/remote coordinator，先退回impact/RFC |
| Documentation | index、EPIC、ARCH-0003、README/status和后续Requirement contract需同步planned/implemented事实 | 设计批准时链接REQ/SPEC/RFC/ADR；完成后才把README/EPIC/ARCH标记implemented |
| Rollback | 发布前可revert；首个control Event后必须保留新SchemaSet、control reader/reducer和Event Store v2。可停止新operations但不能删/改Event、把pending reservation静默release或降DB | rollback只停writer并保持read/replay；修复用新event/schema/forward rule，不原位重释历史 |

## Direct and indirect call path

```text
authenticated request / plugin proposal
  -> persisted lifecycle Manifest + Task exact admission
  -> persisted runtime-control first event + exact reader/reducer
  -> Kernel capability decision (default deny)
  -> cancellation + wall/monotonic deadline decision
  -> BEGIN IMMEDIATE atomic multi-scope budget reserve
  -> Fake Operation only
  -> BEGIN IMMEDIATE settle/cancel/timeout/refund
  -> authoritative control Event range
  -> RuntimeControlProjection / read-only Recorded replay
```

不存在`plugin/provider/tool -> grant`、`Projection/Snapshot -> budget mutation`、`Recorded replay -> Fake Operation/append`或`late result -> lifecycle/operation overwrite`路径。实施若需要任一路径，必须停止并退回RFC。

## Downstream compatibility contract

- REQ-0008：Observer只读结构化decision；Gate可提出deny/cancel请求但Kernel重新判定；Transform不能改变grant、allocation、deadline或authoritative Event。
- REQ-0009：Effect Intent必须引用已批准operation/reservation；Receipt只作为settlement evidence，late receipt进入安全audit/reconciliation，不能重开operation。
- REQ-0010：Provider adapter提供usage observation，不能直接写accounted usage；stream中断按partial/unknown policy处理。
- REQ-0011/0013：Tool和Sandbox把file/network/process/secret资源映射到exact selector并执行Kernel decision；不得把Sandbox本身当授权源。
- REQ-0014：Agent Loop传播`OperationLease`/cancellation probe并尊重terminal结果；retry复用command identity。
- REQ-0018：DAG node映射既有Task ID并继承Run cancellation/budget；调度依赖边不能扩大Task Capability。
- REQ-0033：WASM host functions只能构造request，fuel/memory映射other budget维度；guest不能拿到authority constructor。

# Compatibility and migration

发布新内容地址SchemaSet并保留`68535b...`、`7adfe3...`、`dae028...`、`4ce387...`所有tracked bytes。新set继续包含既有4个lifecycle binding并新增control bindings；RunTask reducer的source key与descriptor/digest不得变化。Runtime Control reducer通过显式control source key→exact implementation/output reader allowlist解析，historical source/output reader随Event保留。

SQLite保持`user_version=2`、writer epoch和Snapshot DDL/trigger完全不变；control Event复用events表、自己的derived stream sequence和exact SchemaSet/limits。旧Run没有control sequence-1 event时返回`aggregate_not_found`，不能猜默认grant/budget。unknown Capability/Budget/control event major、missing reader/reducer或current substitution整体fail closed；compatible-looking旧grant不能自动升级。代码rollback必须保留新control reader/reducer；可拒绝新写入但不能把pending reservation当release、删除audit或重释旧decision。

# Test traceability

| Acceptance | Scope/layer | Scenario | Planned evidence |
|---|---|---|---|
| AC-01 | Focused protocol/table | Capability闭合字段、排序、时间/parent/constraints；完整主体-资源-operation判定表 | `cargo test -p pareto-protocol capability_budget_contract --offline`; `cargo test -p pareto-kernel runtime_control::capability_table --offline` |
| AC-02 | Core security/API | 无grant默认拒绝；plugin/payload/裸grant/self-issued grant无authority/append/dispatch入口 | `cargo test -p pareto-kernel runtime_control::default_deny --offline`; `cargo test -p pareto-kernel --doc --offline` |
| AC-03 | Focused/Core negative | root、合法child收窄；subject/Task/resource/operation/time/usage/depth逐项widen拒绝；revocation cascade和expiry | `cargo test -p pareto-kernel runtime_control::delegation --offline`; `cargo test -p pareto-kernel runtime_control::revocation_and_expiry --offline` |
| AC-04 | Core audit/security | 同aggregate denial追加safe event；跨scope probe无append/无存在性泄漏；错误不含敏感payload | `cargo test -p pareto-kernel runtime_control::denial_audit --offline`; `cargo test -p pareto-kernel runtime_control::isolation --offline` |
| AC-05 | Focused protocol/model | 5类dimension、Run/Task/Actor/operation scope、soft warning/hard deny、decimal/overflow负例 | `cargo test -p pareto-protocol capability_budget_contract --offline`; `cargo test -p pareto-kernel runtime_control::budget_model --offline` |
| AC-06 | Focused/Core concurrency | reserve正例、任一account不足全rollback、两个pool同余额竞争不超卖 | `cargo test -p pareto-kernel runtime_control::reserve --offline`; `cargo test -p pareto-kernel runtime_control::budget_concurrency --offline` |
| AC-07 | Focused/model | success/failure/partial/cancel/timeout consume+release；unknown全额保守；owner refund上限/幂等/不重开 | `cargo test -p pareto-kernel runtime_control::settlement --offline`; `cargo test -p pareto-kernel runtime_control::refund --offline` |
| AC-08 | Core idempotency/security | provider report不权威；command/callback/refund exact retry与same-ID mutation；无重复核算 | `cargo test -p pareto-kernel runtime_control::usage_authority --offline`; `cargo test -p pareto-kernel runtime_control::idempotency --offline` |
| AC-09 | Focused integration | Run/Task/operation request传播到当前和后续operation；request与ack分离；lifecycle state不隐式改变 | `cargo test -p pareto-kernel runtime_control::cancellation_propagation --offline` |
| AC-10 | Focused boundary | cooperative probe确认；uninterruptible仅pending直到返回；failure/cancel/timeout/success互斥 | `cargo test -p pareto-kernel runtime_control::interruptibility --offline` |
| AC-11 | Focused deterministic time | FakeClock推进、deadline前/恰好/后、重启新monotonic epoch、wall regression fail closed；无sleep | `cargo test -p pareto-kernel runtime_control::deadline --offline`; source inspection rejects `sleep` |
| AC-12 | Core concurrency/model | completion/cancel两pool竞争、deadline equality timeout、bounded order model至多一terminal | `cargo test -p pareto-kernel runtime_control::terminal_race --offline`; `cargo test -p pareto-kernel runtime_control::model_sequences --offline` |
| AC-13 | Focused/Core negative | terminal后exact duplicate、mutation、新late callback、pre-reserve/out-of-order、重复ack；budget/effect/state不变 | `cargo test -p pareto-kernel runtime_control::late_and_duplicate --offline` |
| AC-14 | Core recovery/compatibility | close/reopen恢复；missing first/gap/illegal grant/budget/cancel/unknown event/row drift/current reader替代fail closed | `cargo test -p pareto-kernel runtime_control::recovery --offline`; `cargo test -p pareto-kernel runtime_control::compatibility --offline` |
| AC-15 | Focused/Core replay | projection完整provenance/digest；normal=recorded；replay前后event count、budget和Fake调用数相同 | `cargo test -p pareto-kernel runtime_control::projection --offline`; `cargo test -p pareto-kernel runtime_control::recorded_replay --offline` |
| AC-16 | Core isolation/security | tenant/user presence-value/workspace/run/owner/subject/Task/stream和所有ID逐字段swap/payload shadow | `cargo test -p pareto-kernel runtime_control::isolation --offline` |
| AC-17 | Impacted compatibility/golden | 新set重复生成一致；四个旧setbyte-identical；old lifecycle/RunTask Snapshot仍读；unknown control major拒绝；DB v2不变 | protocol full suite；`cargo test -p pareto-kernel event_store --offline`; schema generation diff |
| AC-18 | Core static/scope | 无Hook/Effect/Provider/Tool/Sandbox/Loop/Memory/DAG/WASM实现或依赖，稳定private interface可供后续消费 | exact diff/API/dependency review；`git diff --check` |
| AC-19 | Focused/Impacted/Core | 全部命名测试、默认并行、model/property、real SQLite、FakeClock/FakeOperation证据命中非零 | `cargo test -p pareto-kernel runtime_control:: --offline`; `cargo test -p pareto-kernel --all-targets --all-features --offline` |
| AC-20 | Impacted/Core regression | REQ-0003..0006与全仓治理/static/schema完整门禁 | `cargo test --workspace --all-targets --all-features --offline`; AGENTS.md completion gates |

# Open questions

无阻塞实施的问题。架构/安全专项自审提出的owner operation bypass、delegation widening/TOCTOU、delegate envelope actor、预算超卖与usage spoof、cancel/complete race、双时钟恢复、late-result污染和replay effect reachability风险均已转化为本文可测试合同，0 open Blocker/Major；该self-review不替代实施后的fresh independent code review。生产Clock adapter、background timeout scanner、跨进程cancel transport、真实Effect usage evidence、auto refund、control Snapshot、global admin与distributed budget均明确延期；任何实施中出现的需求必须先更新REQ/SPEC/RFC，不得以内部helper偷偷扩张。
