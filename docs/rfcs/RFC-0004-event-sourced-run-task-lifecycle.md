---
id: RFC-0004
title: Event-sourced Run/Task 生命周期与 Manifest 首事件合同
status: accepted
owners: [maintainers]
created: 2026-08-24
updated: 2026-08-24
links: [REQ-0005, SPEC-0004, EPIC-0002, REQ-0003, REQ-0004, RFC-0001, RFC-0002, RFC-0003, ADR-0001, ADR-0003, ADR-0004, ADR-0005]
---

# Summary

每个 Run 使用由 Run ID 确定性派生的专用 lifecycle stream。sequence 1 是 `RunCreated`，其闭合 payload 原子携带完整 `RunManifest`；后续 `TaskCreated`、`RunStateTransitioned` 和 `TaskStateTransitioned` 事实按 sequence 折叠为当前状态。Kernel 先由 persisted Manifest exact 建立 owner authority，再在一个 SQLite `BEGIN IMMEDIATE` 事务中重验并折叠 aggregate，随后完成 operation-event ID 幂等、前置条件判断与单事件 append，不维护第二份权威状态表。

本 RFC 冻结状态集合、合法迁移、父子约束、owner-only 首版权限、冲突优先级、Manifest bootstrap reader、恢复/兼容和 rollback。它不实现 Projection/Snapshot/Replay executor、通用 Capability/取消传播、Task dependency DAG、Provider 或 Agent Loop。

# Motivation and requirements

REQ-0003 已冻结闭合 Run Manifest 与 SchemaSet/limits identity，REQ-0004 已冻结 append-only Event Store、exact reader 和事务幂等；二者之间仍缺少 Run 生命周期语义。如果 Manifest 单独存表、状态在锁外读取、权限从 payload 派生或进程重启后选“当前 Schema”，将产生双写不一致、lost update、confused deputy 和历史重释。

这些选择会成为 REQ-0006 Projection/Replay、REQ-0007 Capability/取消以及 REQ-0018 Task DAG 的长期输入，发布后难以回退，故必须在实现前通过 RFC/ADR 冻结。设计满足 REQ-0005 AC-01 至 AC-13，并保持 ARCH-0002 的事件完整性、版本身份、状态合法性、可取消性、重放诚实与并发安全边界。

# Proposed design

## 1. Aggregate and lifecycle event contract

完整 `run_<suffix>` 在同一 IsolationScope 下唯一映射为 `stream_lifecycle-<suffix>`；Kernel 计算该值，命令/payload 不能选择 target stream。该 stream 只允许以下 v1 typed events：

- `run-created` → `RunCreatedPayload { manifest: RunManifest }`
- `task-created` → `TaskCreatedPayload { task_id, parent_task_id?, initial_state: created }`
- `run-state-transitioned` → `RunStateTransitionedPayload { from, to, reason_code }`
- `task-state-transitioned` → `TaskStateTransitionedPayload { task_id, from, to, reason_code }`

新增 `TaskId`、`RunState`、`TaskState` 和四个 payload 类型及 JSON Schema 1.0。每种 event type/version 在新 SchemaSet 的 EventTypeRegistry exact binding 到 payload Schema/typed decoder。既有 RunManifest 1.0 Schema 不原位修改；嵌套 Manifest 仍含自己的完整 `schema_ref`。

每个状态命令产生恰好一个 lifecycle event，operation identity 直接使用该 event 的 `event_id`。`occurred_at`、reason、expected sequence/state 和 target 均在命令创建时固定；重试必须复用相同 command/envelope，不重新取时钟或 event ID。expected sequence/state 是 admission 前置条件，不必复制为权威 payload；已提交 event sequence 与 `from` 值足以在 fold 时验证历史。

## 2. Manifest creation, pinning, and bootstrap reading

Create path 分离于 established-run path：

1. Kernel 从认证 principal、目标 workspace/run/owner agent、已 admission exact SchemaSet/limits 和可信 resolved inputs 构造 create-only `LifecycleAuthority`。
2. Manifest 的八个 role pins 必须恰为 `task/behavior/workspace/environment/context_graph/model_snapshot/tool_set/kernel`，并与 authority 的 resolved IDs exact match；SchemaSet、budget、limits、recording policy、scope、execution mode 同样 exact match。可选 plan 缺省只能表示未固定 Plan，不能由 default 补齐。
3. Kernel 验证嵌套 RunManifest 及 `RunCreatedPayload`，在 `BEGIN IMMEDIATE` 中确认 stream 为空并将 sequence-1 Event 原子追加。Manifest 没有单独权威表、mutable current row 或默认值 overlay。
4. 重启后 established reader 按已认证的 tenant/user/workspace/run/agent 与派生 stream 读取 sequence 1。reader 从持久 row 获取 SchemaSetRef/ProtocolLimitsRef，再让 retained registry exact resolve，随后重验 envelope、payload、Manifest 与行 identity。API 不接受调用者替代 SchemaSet/limits。

这解除“必须先知道 Manifest 才能读取 Manifest”的循环，并沿用 ADR-0004 persisted-identity-driven reader。`RunCreated` row 内的 Event Store SchemaSet/limits 与嵌套 Manifest 不一致视为 corruption，不选任一方作为默认。

## 3. State machine and hierarchy

Run states：`created, running, paused, succeeded, failed, cancelled`。Task states：`created, ready, running, paused, succeeded, failed, cancelled`。`succeeded/failed/cancelled` 为不可逆终态。

合法 state edges 与 guard 以 SPEC-0004 的两张表为规范。额外全局不变量：

1. Task 只在 Run=`created` 时创建；Task ID 在 Run 内唯一。
2. parent 若存在，必须是同一 stream 更早创建的 Task；parent link immutable，因此 ownership tree 无环。它不表达 DAG dependency。
3. Run 从 `created` 启动前至少有一个 Task且全部 ready；Run 运行时 Task 才能进入 running。
4. parent succeeded 要求直接 children 全 succeeded；parent failed/cancelled 要求 children 全 terminal。
5. Run succeeded 要求所有 Task succeeded；Run failed 要求全部 terminal 且至少一个 failed；Run cancelled 要求全部 terminal、无 failed 且至少一个 cancelled。结果优先级 `failed > cancelled > succeeded`。
6. Run pause 前没有 running/created Task；resume 后 Task 可显式从 paused/ready 继续。Kernel 不隐式级联状态。

每个历史 lifecycle event 都必须满足其 payload `from` 等于前缀 fold state及当时的 parent/Run guard。读取遇到非法历史不跳过、不修补，返回 aggregate corrupt/unsupported。

## 4. Request, approval, execution, and permissions

策略、Planner、Provider、Tool 或未来 Runtime 只能提交命令。请求者不能构造 `LifecycleAuthority`、validated lifecycle event 或 Event transaction。trusted Kernel：

- 从 create trusted inputs 或 persisted Manifest 派生 authority；
- 首版要求 actor exact 等于 Manifest `scope.agent_id`，tenant/user presence+value/workspace/run/agent/stream 全 exact；
- 执行 state edge、expected version、terminal、parent/Run guard；
- 只在全部批准后把 typed event 交给同事务 Event Store append。

Event Store 执行 append，但不决定业务合法边。`ValidatedEvent` 仍不是 capability。首片 owner-only 是安全的最小授权，不宣称支持用户管理员、agent delegation、scheduler service principal 或通用 capability；REQ-0007 必须在同 authority 构造点增加这些语义。

## 5. Optimistic concurrency and idempotency

Event Store 新增 crate-private lifecycle transaction primitive，不暴露 raw pool/SQL：

```text
BEGIN IMMEDIATE
  -> revalidate authorized aggregate binding
  -> load sequence-1 manifest and lifecycle rows under this transaction
  -> verify exact persisted readers and fold history
  -> same-aggregate exact committed event_id fingerprint => AlreadyApplied
  -> same ID / different fingerprint or other aggregate => generic IdempotencyConflict
  -> compare command expected_sequence and expected_state
  -> validate legal transition + hierarchy guards
  -> append exactly one next-sequence validated event
COMMIT
```

现有 `append` 重构为复用同一 transaction-local insert/check helper，保持 REQ-0004 的 continuous sequence、causation、fingerprint 和 rollback 合同。Lifecycle 的幂等优先级限定为：authority 与完整 aggregate validation/fold 通过后，先于 expected version、terminal 和业务 edge 判断；`AlreadyApplied` 还要求已提交 event 属于同一 authorized aggregate，跨 aggregate ID 复用只返回不含原结果的通用 conflict，exact retry也不得掩盖损坏历史。禁止 lifecycle service 在锁外 `read` 后再调用现有 `append`，也禁止 `INSERT OR IGNORE/REPLACE` 或 last-writer-wins。

冲突优先级固定为 SPEC-0004 所列九步。同 ID exact retry 即使在终态后仍返回原成功；不同 ID 请求相同 target 不视作幂等。两个连接从同 expected sequence 竞争，写锁持有者至多一个 commit；落后者获得锁后看到新 sequence 并稳定返回 optimistic conflict，不自动重算目标或 event sequence。

## 6. Terminal, cancellation, failure, and late results

本 RFC 只管理生命周期状态，不管理外部操作是否真正停止。Task/Run 进入 terminal 后，新 operation ID 的任何状态迁移均拒绝且不追加 lifecycle event；相同已提交 ID 的 exact retry仍返回原结果。取消或失败后的外部 late success 不能改变状态。

首版不做跨 Task 原子 cascade。Run terminal 前，调用方必须让 Task 逐一到达满足 guard 的 terminal states；每一步都是独立、可重试、权威事件。REQ-0007 后续增加 cancellation propagation/capability，REQ-0009 记录 Effect Intent/Receipt 与 late-result audit；它们可以追加新的事件族，但不能把 late receipt 解释成既有 Task/Run 状态迁移。

拒绝响应在本切片是明确文档化的 non-replayable command boundary：它无状态效果、无事件，也不作为审计证据。未来若安全或对账要求持久化拒绝，必须以独立 audit event Schema 追加，不能污染 lifecycle fold。

## 7. Recovery, Projection, and Replay boundary

`fold_lifecycle` 是纯确定函数：输入 exact validated events `[1..=horizon]`，输出 Manifest、Run state、Task map/parent tree 和 final sequence，或稳定 corruption error。它不读取时钟、网络、文件 defaults、current Schema 或 mutable registry head。

REQ-0006 的 Projection/Snapshot 必须用该 fold 作为 oracle：projection 可丢弃重建；snapshot 只能加速经过同样验证的前缀，不能成为幂等或权限事实。Recorded Replay 选择 source Run 固定的 SchemaSet/Manifest 和 event horizon；本 RFC只提供可供后续 executor 消费的合同，不声称实现 Replay。

# Interfaces, data flow, and invariants

```text
untrusted lifecycle command
  -> authenticated principal / trusted create inputs
  -> persisted Manifest-driven private LifecycleAuthority
  -> BEGIN IMMEDIATE
     -> revalidate authorized aggregate binding
     -> persisted SchemaSet/limits exact event validation and fold
     -> event-id idempotency
     -> lifecycle event validation + pure fold
     -> expected sequence/state + permission + hierarchy guard
     -> typed event construction and transaction-local append
  -> COMMIT
  -> Applied / AlreadyApplied / structured rejection
  -> future disposable Projection/Snapshot
```

责任边界：protocol 拥有 wire types/Schema/closed deserialization；lifecycle Kernel 拥有 state machine、authority、fold、parent/terminal guards；Event Store 拥有 transaction、sequence、idempotency和 durable append；未来 Runtime 只能发命令；策略/插件不能写权威状态。

关键不变量：Manifest first；完整版本 pin 无 default；event log 唯一权威；state check 与 append 同事务；exact retry 优先；terminal 不可逆；parent 先于 child；scope/actor/reader exact；Projection/Replay 不改变历史解释。

# Failure modes and security

| Failure | Required behavior and recovery |
|---|---|
| Manifest missing/extra/default or trusted pin mismatch | create 前 fail closed，无 stream/event；调用方修正完整显式输入后用新或相同未提交 operation ID 重试 |
| duplicate create | exact same event ID/bytes 返回 AlreadyApplied；不同 ID 即使 Manifest 相同也返回 aggregate/sequence conflict |
| stale state/sequence | optimistic conflict，无事件；调用方重新读取并显式决定新命令，不自动 rebase |
| same operation ID mutation | idempotency conflict，不泄漏原 Manifest/payload |
| illegal edge/parent guard | structured rejection，无事件、无隐式 child/run cascade |
| terminal late result | exact old retry幂等；新 ID terminal conflict；外部效果对账留给 REQ-0009 |
| busy/cancel before commit | transaction rollback；有限 busy；无 authoritative partial row |
| commit response lost | 只以同 ID、同完整 command 重试并取得 AlreadyApplied；禁止新 ID 猜测 |
| missing/wrong/alternate SchemaSet | persisted identity exact resolution失败；不得用 current/compatible set 代替 |
| illegal/corrupt historical event | aggregate fail closed；不跳过、不补默认、不从 projection 猜状态 |
| cross-scope/actor/Task confusion | exact authority、derived stream、Task-in-aggregate checks拒绝；payload同名字段无权限语义 |
| huge lifecycle stream | 首版 O(n) fold并记录基线；未有 approved Snapshot 前不得以不完整 prefix 决策 |

# Alternatives considered

1. **Manifest 独立可变表 + RunCreated event**：查询方便，但需要跨表示双写、回填和一致性修复；即使同 DB 事务也会形成第二权威源和 Replay 歧义。拒绝，Manifest 作为首事件 payload。
2. **单独 current-state 表是权威，事件作审计**：读快，但状态表与事件可分叉，Projection/Snapshot 无法证明可重建。拒绝，event log 唯一权威。
3. **锁外读状态，再调用 REQ-0004 append**：复用代码最少，但两个 writer 可同时批准同一状态，TOCTOU 后只能看到 sequence 冲突且可能已产生其他状态副作用。拒绝，同 `BEGIN IMMEDIATE` fold-and-append。
4. **每个 Run/Task 独立 stream，Run terminal 原子批量更新 children**：并行性较高，但首片需要跨 stream transaction、batch idempotency 和 cascade recovery；也提前绑定 Task DAG。当前拒绝，单 lifecycle stream + 显式逐 Task 迁移。
5. **任何 authenticated actor 均可迁移，等待 REQ-0007 再收紧**：易用但会制造已发布越权路径。拒绝，首版 owner-only fail closed。
6. **新命令到相同 target state视为成功**：表面幂等但会吞掉 stale caller 和不同原因的重复效果。拒绝，只有同 event ID/同字节是幂等。
7. **只把 Manifest digest 放事件，内容另存 artifact**：减小 event，但恢复依赖另一个存储、原子性和保留协议尚不存在。拒绝，首片直接持久完整闭合 Manifest。

# Compatibility, migration, and rollback

新增 protocol types/schema/event bindings 形成新的内容地址 SchemaSet；旧集合不改字节、不删除。RunManifest 1.0 本身保持现状，因此现有 fixtures继续读；新 `RunCreatedPayload` 引用完整 Manifest。unknown lifecycle event major、enum variant或非法 minor变更 fail closed；同 major 演进必须由 old-writer → new-reader fixture和保守 checker证明。

SQLite v1 `events` 已能保存所有新事实，不增加表/列，不改变 `user_version`。若实现发现必须新建 Manifest/state 表或公开 transaction escape hatch，必须停止并退回 RFC。旧普通 Event stream 不自动升级为 lifecycle aggregate；没有合法 first event 就返回 not-found/unsupported。

实现发布前可 Git revert。首个 Run 写入后，rollback writer 只能停止新创建/迁移，必须保留新 SchemaSet、v1 lifecycle reader/fold 和 SQLite reader；不能删除事件、重写 Manifest或以默认值补字段。未来修复使用新 event/schema/forward migration并保留来源。

# Evaluation and acceptance

- 质量：全部状态对、模型化 bounded command sequences、hierarchy、Manifest完整性、transaction rollback、same/different ID、两连接竞争、terminal/late、isolation、reopen/corrupt history与pure fold determinism 必须通过。
- Token/费用：无模型/Provider调用，记录不适用，不宣称优化。
- 延迟：在 Windows 本地真实 SQLite记录 create、不同 lifecycle 长度 fold、single transition和two-writer contention观察；无 baseline 前不设收益阈值。Linux/macOS由全仓 CI 验证兼容，不把单机观察当跨平台性能结论。
- Replay/compatibility：同一 exact event range多次 fold产生相同 canonical state digest；old SchemaSet仍 byte-identical/readable，alternate current set不能替代persisted reader；不声称已实现 Projection/Snapshot/Replay executor。
- 批准：架构专项检查 Manifest原子性、authority、transaction TOCTOU、终态/迟到、父子约束、old reader和后续 Replay。所有 Blocker/Major关闭后接受本 RFC、创建 ADR-0005并批准 SPEC-0004；Runtime代码只能在此后按 Plan实施。
