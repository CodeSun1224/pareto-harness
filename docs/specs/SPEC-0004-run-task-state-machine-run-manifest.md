---
id: SPEC-0004
title: Run/Task 状态机与 Run Manifest 规范
status: approved
owners: [maintainers]
created: 2026-08-24
updated: 2026-08-24
links: [REQ-0005, EPIC-0002, REQ-0003, REQ-0004, RFC-0004, ADR-0001, ADR-0003, ADR-0004, ADR-0005, ARCH-0001, ARCH-0002, ARCH-0003]
---

# Behavioral contract

Run lifecycle 是可信内核拥有的 event-sourced aggregate。一个 Run 使用由 Run ID 确定性派生的专用 lifecycle stream；sequence 1 必须是 `RunCreated`，其闭合 payload 包含完整 Run Manifest。其后 `TaskCreated`、`RunStateTransitioned`、`TaskStateTransitioned` 事件按 stream sequence 折叠为当前状态。数据库中不得另存第二份权威可变状态；未来 Projection/Snapshot 只能由相同事件范围重建。

RFC-0004 已接受，ADR-0005 冻结 durable decision。架构专项自审关闭了 Manifest 双写、bootstrap reader 循环、锁外状态检查、取消级联部分提交和权限过度声明风险。本 Spec 于 2026-08-24 批准；本轮仅建立 Plan/Tasks/Handoff，不在本设计步骤编写 Runtime 功能代码。

# Inputs, outputs, states, and failure behavior

## Inputs and outputs

- `CreateRunCommand`：由 Kernel intake 固定 operation event ID、occurred-at、完整 expected scope、已 admission SchemaSet、Protocol Limits、已解析的八个 required revision pins、Budget/recording policy、执行模式和 root lifecycle target。外部字段不能直接构造 authority。
- `CreateTaskCommand`：固定 operation event ID、expected sequence、Task ID、可选 immutable parent Task ID；首版只允许 Run 仍为 `created`。
- `TransitionCommand`：固定 operation event ID、expected lifecycle sequence、实体 ID、expected state、target state、稳定 reason code 和显式 occurred-at。重试必须复用相同完整命令。
- 成功输出 `Applied { event_id, sequence, state }`；exact retry 输出 `AlreadyApplied` 并返回原序号/状态；失败输出不含 Manifest/payload 的稳定类别：`manifest_invalid | unauthorized | aggregate_not_found | aggregate_corrupt | invalid_transition | parent_state_conflict | terminal_state_conflict | optimistic_concurrency_conflict | idempotency_conflict | schema_unavailable | busy | io`。
- 拒绝命令不产生状态事件。本切片把结构化拒绝响应明确记录为非持久化 command-response boundary；REQ-0007/0009 增加权限/late-effect 审计前，不得将拒绝响应描述为 replayable audit fact。

## Manifest and lifecycle bootstrap

1. `RunManifest.revisions` 必须恰好含 `task`、`behavior`、`workspace`、`environment`、`context_graph`、`model_snapshot`、`tool_set`、`kernel`；`schema_set_ref`、`budget_revision`、`protocol_limits_ref`、`boundary_recording_policy_ref`、`execution_mode` 和可选 `plan_revision` 继续遵循 REQ-0003 闭合合同。
2. Kernel create authority 以可信解析的输入集合逐字段 exact-match Manifest；不从环境变量、当前模型/工具、默认预算、当前 SchemaSet 或 registry head 填充缺失字段。本切片只证明 ID 集合和 trusted input exact match，不声称实现完整 Revision repository。
3. lifecycle stream ID 由 `run_<suffix>` 规范派生为 `stream_lifecycle-<suffix>`；禁止调用方选择另一个 stream。Run ID 已在完整 isolation scope 内，因此映射在该 scope 中唯一。
4. `RunCreated` 是 sequence 1，payload 为 `RunCreatedPayload { manifest }`，初始 Run state 为 `created`。Event row 持久化的 SchemaSet/limits 必须与嵌套 Manifest exact match。
5. 重启读取先按 exact tenant/user/workspace/run/agent 和派生 stream 找到 sequence 1，从该已持久化 row 取得 exact SchemaSet/limits，再由 retained registry 解析并重验 envelope、payload 与 Manifest。调用者不能提供替代/current reader；缺失或歧义首事件 fail closed。

## State sets and legal transitions

Run 合法边：

| From | To | Additional guard |
|---|---|---|
| `created` | `running` | 至少一个 Task；全部 Task 为 `ready` |
| `created` | `failed` | 全部 Task terminal，且至少一个 `failed` |
| `created` | `cancelled` | 全部 Task terminal、无 `failed`，且至少一个 `cancelled`；无 Task 时也允许取消未启动 Run |
| `running` | `paused` | 无 Task 为 `running` 或 `created`；非终态 Task 只能为 `ready/paused` |
| `running` | `succeeded` | 至少一个 Task，且全部为 `succeeded` |
| `running` | `failed` | 全部 Task terminal，且至少一个 `failed` |
| `running` | `cancelled` | 全部 Task terminal、无 `failed`，且至少一个 `cancelled` |
| `paused` | `running` | 至少一个非终态 Task；非终态 Task 只能为 `ready/paused` |
| `paused` | `failed` | 全部 Task terminal，且至少一个 `failed` |
| `paused` | `cancelled` | 全部 Task terminal、无 `failed`，且至少一个 `cancelled` |

Task 合法边：

| From | To | Additional guard |
|---|---|---|
| `created` | `ready` | Run 为 `created`；parent 若存在则非 terminal |
| `created` | `failed/cancelled` | 所有直接 children 已 terminal；Run 非 terminal |
| `ready` | `running` | Run 为 `running`；parent 若存在则为 `running` |
| `ready` | `failed/cancelled` | 所有直接 children 已 terminal；Run 非 terminal |
| `running` | `paused` | Run 为 `running`；所有直接 children 不为 `running` |
| `running` | `succeeded` | 所有直接 children 为 `succeeded` |
| `running` | `failed/cancelled` | 所有直接 children已 terminal；Run 非 terminal |
| `paused` | `running` | Run 为 `running`；parent 若存在则为 `running` |
| `paused` | `failed/cancelled` | 所有直接 children 已 terminal；Run 非 terminal |

未列出的边全部非法。`succeeded/failed/cancelled` 均为终态，无 outgoing edge。Task 只能在 Run=`created` 时创建；parent 必须在同一 stream 更早创建且 Task ID 在 Run 内唯一，故 ownership tree 无环。这里的 parent 不是依赖边；REQ-0018 可增加 DAG dependency，但不得改变已发布 lifecycle 事件的含义。

Run 终态由 child outcome 决定：失败优先于取消，取消优先于成功。Kernel 不做隐式级联；调用方先把非终态 Task 逐个迁移到合法终态，再迁移 Run。这样每个命令只追加一个事件，避免取消/失败时跨实体部分提交。REQ-0007 可另行设计原子批量取消，但不得重释本版本历史。

## Authority, command ordering, and transaction

```text
authenticated principal + trusted resolved run inputs
  -> create-only LifecycleAuthority
  -> validate complete RunManifest against admitted exact SchemaSet
  -> BEGIN IMMEDIATE
     -> event_id exact-idempotency check
     -> assert lifecycle stream empty
     -> append validated RunCreated(sequence=1, manifest payload)
  -> COMMIT

later command + persisted RunCreated
  -> persisted-row-driven exact SchemaSet/limits resolution
  -> owner-only LifecycleAuthority (scope.agent_id == authenticated actor)
  -> BEGIN IMMEDIATE
     -> revalidate aggregate binding and read/fold exact lifecycle stream
     -> event_id exact-idempotency check under the validated aggregate
     -> expected sequence/state + permission + parent/terminal guard
     -> append one validated lifecycle event at next sequence
  -> COMMIT
```

批准者是 trusted Kernel state machine，不是请求 payload；执行者是同一事务内的 Kernel-private Event Store path。策略/插件只能请求。首版权限矩阵只有 Manifest owner actor；tenant、user presence/value、workspace、run、agent/actor 和派生 stream 全部 exact match，不支持 wildcard、subset、delegation 或 cross-run administration。

冲突判定优先级固定为：

1. aggregate/Manifest 损坏或 exact reader 缺失：fail closed。
2. authority/scope/actor/target mismatch：返回不泄漏 aggregate/event 存在性的通用 `unauthorized`，不得先查询全局 event ID。
3. 同一 authorized aggregate 内同 event ID 且完整 canonical event/SchemaSet/limits 相同：`AlreadyApplied`；即使实体已终态或 expected sequence 已过期，也返回原结果。
4. 同 event ID 但任一内容不同，或该 ID 已属于另一个 aggregate：返回不泄漏原 scope/状态/结果的 `idempotency_conflict`。
5. expected sequence 不等于当前：`optimistic_concurrency_conflict`。
6. expected state 不等于 folded current state：`optimistic_concurrency_conflict`。
7. 当前 state terminal：`terminal_state_conflict`。
8. 非法 edge 或 parent/Run guard 失败：对应结构化冲突。
9. append/commit 失败：rollback；commit response 不确定时只能以同 event ID 和完全相同命令重试。

# Impact analysis

| Dimension | Finding | Evidence / response |
|---|---|---|
| Direct | `pareto-protocol` 后续增加 Task ID、Run/Task state 和四类 lifecycle payload Schema/event bindings；`pareto-kernel` 后续增加 lifecycle aggregate、state fold、Manifest bootstrap read 与同事务 fold-and-append；真实 SQLite 测试扩展 | 当前 `types.rs` 已有 RunManifest/EventEnvelope 但无 Task/状态类型；`event_store.rs` 只有单事件私有 append/read，`lib.rs` 不暴露 Runtime API |
| Indirect | REQ-0006 Projection/Snapshot/Replay 消费纯 fold 与固定 horizon；REQ-0007 在 authority 点增加 capability/cancel；REQ-0010/0014 消费 Run/Task 生命周期；REQ-0018 在 immutable parent ownership 之上增加 dependency DAG | `requirement-backlog.md` 依赖顺序；ARCH-0001/0003 声明 Event Log 为事实源、Projection 为派生视图 |
| Call/permission | 当前 `KernelAuthority::authenticated` 仅在 Event Store tests 中由 scope/actor 自建，不能直接充当 lifecycle 授权。create 与 established-run 两条入口必须分离，后者只由 persisted Manifest 派生，首版 owner-only | RFC-0002 create-run/established-run 分离；REVIEW-0003 F-001/F-005 已证明自报 authority 和 reader 替换是 Major 风险 |
| Data isolation | lifecycle stream 继续落在完整 `(tenant,user presence/value,workspace,run,agent,stream)` sequence scope；Task ID 只在该 aggregate 内解析。operation ID 虽是全库唯一，`AlreadyApplied` 只允许同一 validated authorized aggregate 的 exact retry；跨 scope/aggregate 复用只能得到不含原 scope/状态/结果的通用冲突 | REQ-0004 DDL/reader 已覆盖完整 scope；新增负例逐字段 swap、payload shadow、Task ID/operation ID 跨 Run 复用 |
| API/schema | 新公开协议类型和 event bindings 是持久化合同；状态 enum 增值、合法边或 payload 字段变化可能破坏 exhaustive consumers。Kernel authority/transaction API 保持 crate-private，不泄漏 sqlx | 新类型发布为 1.0 闭合 Schema；breaking 变化 major bump；protocol 依赖方向保持无 DB/Runtime |
| Persistence/replay | Manifest 作为首事件 payload 避免单独表双写；current state 只 fold Event Log。读取必须由 persisted row 驱动 exact SchemaSet/limits，不能用进程 current/default | ADR-0003 的 no-default/pinned SchemaSet；ADR-0004 的 exact reader/horizon；model/reopen/old-set fixtures |
| Concurrency | 若 lifecycle 在锁外读取再调用现有 append，会产生 TOCTOU。实现必须先建立 exact authority，再在同一 `BEGIN IMMEDIATE` 中验证/fold aggregate，然后执行幂等、guard 和 append；幂等只优先于 expected version/terminal，不优先于 authority 或 corruption | 两个独立 pool/barrier 争用同 expected sequence；同 ID retry 和不同 ID stale command matrix |
| Security | confused deputy、payload scope shadow、替代 SchemaSet、错误回显 Manifest、伪造 parent/actor、终态迟到结果是主要风险 | authority opaque/private；错误只返回 code/subject safe IDs；negative tests 覆盖所有 scope、actor、stream、parent 与 reader replacement |
| Failure/recovery | 验证/append/commit 前失败不得产生部分 Run；commit response uncertainty 只允许 exact retry。损坏历史不能“跳过坏事件继续”；fold 必须 fail closed | real file transaction drop/reopen、fresh connection visibility、row drift、missing first event、illegal stored transition fixtures |
| Compatibility/migration | 使用现有 events 表，无 DB schema migration；新增内容寻址 SchemaSet 并保留旧 set。旧普通 Event stream 不解释为 lifecycle stream；旧 Manifest 只按其 exact set 验证 | schema generation byte identity、retained sets、Event Store regression；unknown lifecycle major/reader missing fail closed |
| Performance | 每次命令 fold lifecycle stream 是 O(events) 且 SQLite 单 writer；首片优先正确性。REQ-0006 Projection/Snapshot 以后优化读取，但不能改变权威判定 | 记录不同 event-count 的 fold/transition observation；无阈值前不宣称优化；达到可测瓶颈再评审 snapshot-assisted validation |
| Dependency/operations | 不需要新第三方依赖或 DB migration；新增 Schema 增加发布资产和 retained registry 占用 | 继续使用 std/Serde/sqlx/Tokio/sha2；离线全门禁；不增加 Provider/网络依赖 |
| Documentation | index、EPIC-0002、ARCH-0003、Requirement/Spec/RFC/ADR 和 active work 必须区分 planned 与 implemented | 本设计变更同步链接；README 在实现完成前继续声明状态机未实现 |
| Rollback | 实现/数据写入前可 revert。首个 lifecycle Run 持久化后，writer 可停用但 v1 SchemaSet、Manifest/lifecycle reader 和 Event Store migration 必须保留；不能删历史或回填默认值 | RFC-0004/ADR-0005 rollback；未来修复用新 event/schema 或向前 DB migration，不原位改事件 |

## Downstream contract

- REQ-0006 必须以 `fold(events[1..=horizon])` 作为 Projection/Snapshot 正确性 oracle；snapshot 不得参与幂等真相或允许跳过未验证事件。
- REQ-0007 在 `LifecycleAuthority` 构造前加入 capability/delegation、预算和取消授权；不得让 capability payload 自授权，也不得把 lifecycle `cancelled` 等同于外部 effect 已停止。
- Provider/Agent Loop 只能提交命令并消费结构化结果，不能直接 append lifecycle event。
- REQ-0018 可增加 Task dependency/plan revisions，但 immutable parent ownership、Task ID isolation 和既有 terminal semantics 保持兼容。

# Compatibility and migration

本切片不改变 SQLite v1 表。协议生成器发布包含 lifecycle 1.0 types/bindings 的新内容地址 SchemaSet；REQ-0003/0004 已发布集合 byte-identical 保留。Reader 以 RunCreated row 持久化的 exact set/limits 选择 decoder；不使用 newest compatible set 替换。允许的旧 writer → 新 reader minor 演进仍需 compatibility checker 与 fixture；状态 enum、合法迁移、authority 或 Manifest 首事件语义变化默认升级 major 并要求新 RFC/ADR。

旧 Event Store 文件可原样打开。没有 sequence-1 `run-created` 的 stream 不是 lifecycle aggregate，不能猜测 Manifest；未知 lifecycle event、gap、重复 Task、from-state 不匹配或违反 parent guard 的历史标为 corrupt/unsupported，不能跳过。发布后 rollback 只停止新 writer并保留 v1 reader；如果需要修复错误事实，只能追加受审补偿事件或新增版本，不能 update/delete 旧事件。

# Test traceability

| Acceptance | Scope/layer | Scenario | Planned evidence |
|---|---|---|---|
| AC-01 | Focused protocol/contract + Core security | 八角色、SchemaSet/budget/limits/policy/mode 全部 exact；逐字段 missing/extra/wrong trusted pin/default/current substitution 拒绝 | `cargo test -p pareto-protocol lifecycle_manifest_contract --offline`; `cargo test -p pareto-kernel lifecycle::manifest --offline` |
| AC-02 | Focused real-SQLite integration | create success fresh connection visible；validation error、transaction drop、injected append/commit-before-return uncertainty 无孤立 Manifest/Run；same ID retry | `cargo test -p pareto-kernel lifecycle::creation_atomicity --offline` |
| AC-03 | Focused unit/table | Run 6 状态、Task 7 状态的全部有向状态对；只允许声明边；终态无 outgoing | `cargo test -p pareto-kernel lifecycle::state_machine --offline` |
| AC-04 | Core compile-fail/security | 外部 crate 无 authority/append API；策略/payload/裸 ValidatedEvent、自报 scope/actor/stream 全拒绝；owner 正例 | `cargo test -p pareto-kernel --doc --offline`; `cargo test -p pareto-kernel lifecycle::authority --offline` |
| AC-05 | Focused/Core concurrency/idempotency | exact retry、same-ID mutation、different-ID same expected version；两个 pool/barrier 竞争只一个 commit；commit response uncertain retry | `cargo test -p pareto-kernel lifecycle::idempotency --offline`; `cargo test -p pareto-kernel lifecycle::concurrency --offline` |
| AC-06 | Focused negative/model | exact retry优先；terminal 后新 ID 同目标/其他目标、cancel/fail 后 late success、重复目标均无新行 | `cargo test -p pareto-kernel lifecycle::terminal_and_late --offline` |
| AC-07 | Focused model/property | parent 先创建、同 Run、ID 唯一和无环；child/parent/Run guard；对 bounded command alphabet/sequence 做模型化枚举并比较 reference model 与 fold | `cargo test -p pareto-kernel lifecycle::model_sequences --offline`; `cargo test -p pareto-kernel lifecycle::hierarchy --offline` |
| AC-08 | Core transaction/replay contract | 锁内 fold-and-append；失败 event count 不变；相同固定 event range 重复 fold 得相同 state digest；无第二权威 state 表 | `cargo test -p pareto-kernel lifecycle::transaction --offline`; `cargo test -p pareto-kernel lifecycle::fold_contract --offline` |
| AC-09 | Impacted/Core recovery/compatibility | close/reopen恢复 Manifest 和多 Task 状态；missing first、gap、illegal stored edge、wrong/missing/alternate SchemaSet、row drift fail closed | `cargo test -p pareto-kernel lifecycle::recovery --offline`; `cargo test -p pareto-kernel lifecycle::compatibility --offline` |
| AC-10 | Core isolation/security | tenant/user presence+value/workspace/run/agent/actor/stream/Task 逐字段 swap；跨 scope operation ID 和 payload shadow 不越界 | `cargo test -p pareto-kernel lifecycle::isolation --offline` |
| AC-11 | Impacted protocol/golden/migration | 新 set 发布、旧 set byte identity/reader、old-writer/new-reader fixture、unknown major；DB `user_version` 不变且旧 Event DB 可打开 | `cargo test -p pareto-protocol --all-targets --all-features --offline`; `cargo test -p pareto-kernel event_store --offline`; schema generation diff |
| AC-12 | Focused/Impacted/Core | 单元、全部边、负向、模型、幂等、竞争、迟到、Manifest、原子、隔离、恢复、fold contract 全矩阵 | `cargo test -p pareto-kernel --all-targets --all-features --offline` plus named commands above |
| AC-13 | Impacted/Core regression | protocol、Event Store、workspace、治理、格式/静态、Schema byte identity 全通过；dependency direction 无新增反向边 | repository completion gates in PLAN/VALIDATION |

# Approval

RFC-0004 因状态语义、持久化首事件、并发事务和未来 Replay 依赖而必需；ADR-0005 已接受该设计。2026-08-24 架构专项自审为当前会话 self-review，不冒充独立代码评审；其中 5 个 Major 设计风险均在提案中关闭，0 open Blocker/Major。Spec 获批后已创建 Plan、Tasks 和 active Handoff；任何 Runtime 实现仍必须按任务分层验证，并在完成后交给独立 Agent/新会话执行 code-review。
