---
id: RFC-0008
title: Kernel 治理的 Observer、Gate 与 Transform Hook 合同
status: proposed
owners: [runtime-kernel]
created: 2026-08-28
updated: 2026-08-28
links: [REQ-0008, SPEC-0007, REQ-0004, REQ-0007, RFC-0007, ADR-0008, ARCH-0002, ARCH-0003, ARCH-0004]
---

# Summary

建立由 Rust 可信 Kernel 治理、实现语言中立的 Hook 合同。Run Manifest 固定 Hook registry revision 与配置；Kernel 在显式 Hook point 按稳定顺序调用 Observer、Gate、Transform，调用前从持久身份、lifecycle 和 Runtime Control 重建最小 authority 并原子预留预算，调用后把结果当作不可信输入重验并记录版本化决定。Observer 不能影响权威决定；Gate 采用 deny 优先、required 显式 allow 和默认拒绝；Transform 只能修改明确允许的非权威 proposal 字段。Recorded replay 只消费已记录决定，不执行 handler 或重复核算。

首片只实现进程内 Rust Fake handler 和 Fake Clock，不冻结 Rust ABI、子进程、HTTP、MCP、WASI 或外部 Worker transport。真实 Effect、Provider、Tool、Sandbox、Loop、Memory、DAG、Evidence Graph 和 WASI 执行仍由后续 Requirement 批准。

# Motivation and requirements

REQ-0007 已冻结默认拒绝 Capability、trusted resource envelope、原子预算、取消/deadline、opaque lease、late result 和 Recorded replay 零执行。若 Hook 在这些边界外自行运行，任何 Observer/Gate/Transform 都可能成为第二 authority、预算旁路或 replay 副作用入口。REQ-0008 必须在第一个下游执行器之前冻结 Hook 类型、point、顺序、组合、失败、版本、恢复与隔离语义。

本 RFC 满足 REQ-0008 AC-01 至 AC-20，并继承 RFC-0007/ADR-0008：Rust 拥有 authority/admission 而非所有 handler 代码；reference implementation 不构成语言强制；真实跨语言 runtime 必须另行批准认证、Schema、资源、取消、late、隔离、升级和回滚。

# Proposed design

## 1. Roles and points

Observer 只获得只读、脱敏、版本化 view，并返回 observation/warning/failure；其业务输出永不进入组合值。Gate 只返回 allow/deny/abstain。Transform 只接受非权威 proposal 并返回同类型的受限新版本。初始 point 与允许矩阵以 SPEC-0007 为规范；未知组合默认拒绝。

point 分成 pre-commit 与 post-commit。pre-commit failure 可阻止尚未提交的后续阶段；post-commit Observer 不能回滚或修改已提交 Event，其 fail-closed 只阻止下一阶段。外部 handler 执行期间不得持有 SQLite write transaction：Kernel 先原子 reserve 并记录 invocation，释放锁调用 handler，返回后重新取得 writer lock 并 refold。这避免 hung handler 长期持锁，并通过持久 invocation/lease/terminal 处理 TOCTOU 与 crash。

## 2. Manifest-pinned registry and deterministic order

`HookRegistryRevisionV1` 是内容寻址闭合记录，包含排序后的 `HookRegistrationV1` 与 registry/config digest。Run Manifest 通过新 revision role `hook_registry` 固定 registry，SchemaSet 固定 registry、Hook Event 和 Projection reader。排序 tuple 为 `(point ordinal, priority ascending, hook logical ID bytes, hook revision bytes)`；相同完整 tuple 非法，不使用注册时间、map 顺序或进程地址。

每个 proposal 在 point 开始时固定 source cursor 和 ordered invocation list；本轮 registry 不能热更新。handler implementation registry 只能按 exact revision/config/compatibility identity 解析，missing、unknown 或 current substitution 在调用前 fail closed。

## 3. Gate composition

Gate 串行执行。每个结果先验证 limits、Schema、producer/lease、point 与 identity，再参与组合：

1. 有效 deny 立即确定 point deny；剩余 Gate 不执行并记录规范 skip reason。
2. handler failure、timeout、非法输出、unknown reader/revision 都转换为安全 deny，不转换为 abstain。
3. 无 deny 时，每个 required Gate 都必须 allow；required abstain 或 missing 决定 deny。
4. optional Gate 可 allow 或 abstain，但不能弥补 required Gate 缺失。
5. 只有 registry 对该 point 显式声明 `gate_requirement=none` 时才允许空 required set，否则 deny。

最终 `HookPointDecisionV1` 记录 ordered component decision IDs、short-circuit 位置、final outcome、稳定 reason 与 input/registry/source identity。它是 Kernel 决定，不是某个 Gate 自签 authority。

## 4. Transform protection

Transform 使用稳定串行 pipeline。`TransformContractV1` 为每个 point 固定 allowed field IDs 及字段 Schema，禁止 wildcard、任意 JSON Patch 或代码表达式。每一步 Kernel 解析闭合 output，验证 scope、subject、proposal type 和大小，再比较 `ProtectedProposalHashViewV1`。

保护 view 摘要绑定 Schema/registry/Manifest/Event identity、tenant/user/workspace/run/task/actor/principal、Capability/grant/lease、budget plan/account/reservation/usage、cancellation/deadline、Effect intent/receipt、Evidence、terminal 和所有 unknown 字段。allowed mask 只是对非保护字段的进一步收窄，不能使保护字段可写。失败时整个 proposal admission 失败；中间结果只保留安全 digest，永不提交部分权威修改。

## 5. Authority, budget, and handler boundary

Kernel 从认证 principal 与 persisted Manifest/lifecycle/control 构造 invocation context，忽略 handler 自报 scope。每次调用映射为 REQ-0007 protected operation：retained trusted contract 产生完整 finite envelope；Kernel 在同一 writer transaction 完成 lifecycle/cancel/deadline/Capability、Run/Task/Actor/per-operation 全账户 reserve，并追加 control reservation 与 Hook invocation-reserved 事实。

两条 stream 的追加必须在同一 SQLite transaction 内完成或由 Event Store 证明为一个不可分割的 private command；不允许 control 已 reserve 但 Hook invocation identity 未记录的部分成功。实现若不能满足，必须退回 RFC，不得以补偿事件替代原子准入。

Kernel 随后向 Fake handler 发放不可序列化 `HookInvocationLease`，只暴露 read-only input/probe 与结果返回能力。handler 不能 append、refund、确认其他 operation 的取消、构造 timeout recovery 或执行 Effect。Kernel meter 包围 Fake 执行，settlement 复用 REQ-0007 verified/unknown 规则。

handler 运行期间 Run/Task 可请求取消或到达 deadline。结果返回后 Kernel 重新 fold 两条 stream 并验证 lease、producer、process epoch、attempt、terminal 与 Clock；loser 成为 late/rejected audit。hung/uninterruptible handler 由 REQ-0007 显式 timeout recovery 终结，Hook 层只引用最终 operation outcome，不建立第二 timeout 机制。

## 6. Events, projection, recovery, and replay

每 Run 使用 `stream_hooks-<run suffix>`；Event envelope actor 仍为 Manifest owner 这个 Kernel authority signer，真实 subject/handler producer 在 payload 中 exact 验证。sequence 1 初始化 registry/source contract。状态与事件族以 SPEC-0007 为规范，结果只包含 safe digest，不保存敏感 payload。

Hook state 只由 Hook stream pure fold；budget/operation terminal 只由 control stream pure fold。Hook reducer 交叉验证 invocation 引用的 control operation/reservation、outcome 与 source cursor。Kernel load/recovery 在一致 read transaction 中读取 exact horizons；悬空或矛盾引用 fail closed，不信任单边 Projection。

Recorded replay 只加载 Event reader/reducer；类型上不接受 handler registry implementation、Hook lease、control writer、timeout authority 或 Effect executor。它消费已记录 component/point decisions，绝不重新执行 Gate/Transform。Simulated/Reexecute 会产生新 Run 与新预算/effect 谱系；本切片没有 resolver/executor，必须在 dispatch 前拒绝。

## 7. Failure and redaction

Observer 注册策略只有 warn-and-continue 与 fail-closed；Gate 固定 fail-closed；Transform 固定 reject-whole-proposal。拒绝 reason 是版本化枚举；unknown reason/major 拒绝。自由文本只可作为受限、脱敏 annotation 摘要，不参与 authority。

所有外部 bytes 先受 transport ceiling，再受 semantic record/payload limits。错误不回显 payload、secret、path、SQL、DB、其他 scope 或 budget balance。日志由 Kernel 从 safe IDs、reason、digest、revision、cursor 和 redaction policy 构造；handler 日志不是权威 audit，首片不导入。

# Interfaces, data flow, and invariants

```text
Manifest-pinned Hook registry
  -> stable ordered point invocations
  -> Kernel persisted identity/lifecycle/control admission
  -> atomic control reserve + Hook invocation fact
  -> opaque bounded lease + Fake handler
  -> Kernel output limits/Schema/scope/protected-field validation
  -> Observer audit OR Gate component decision OR transformed proposal
  -> deterministic point composition
  -> Kernel-only authoritative Event admission
  -> Hook Projection/recovery
  -> Recorded replay read/fold only
```

核心不变量：Hook 没有 Event/Capability/Budget/terminal authority；Observer 业务无效力；Gate default deny；Transform 保护字段不可写；顺序由 Manifest 版本固定；每次调用先 reserve 且只结算一次；取消/deadline/terminal 唯一；late 无状态/effect 污染；跨域 no-write；Event Store 唯一事实源；Recorded 零 handler/零核算；reference Rust 不等于 ABI/transport 承诺。

# Failure modes and security

| Failure | Required behavior and recovery |
|---|---|
| registry/config missing、duplicate 或 unknown | Run 初始化/point 调用前 fail closed；不选 current，不部分注册 |
| Observer 试图 deny/mutate/cancel | invalid output；按注册 failure policy 处理，业务组合值不变 |
| Gate failure/timeout/required abstain | stable deny；剩余 Gate 按 short-circuit 规则 skip；无 authoritative effect |
| Transform 保护字段/unknown mutation | protected hash 与 mask 验证拒绝整个 proposal；无 partial commit |
| self-signed scope/capability/lease | Kernel 从 persisted facts 重建；unknown producer no-write 且不能触发 unknown usage 扣账 |
| budget 不足或并发竞争 | handler 前全账户原子拒绝；reverse-winner 测试证明不超卖 |
| crash after reserve | pending 保留；reopen 从双 stream 恢复并显式 reconcile；不自动 release/reexecute |
| cancel/complete/timeout race | writer refold 决定唯一 terminal；late 只写安全 digest audit |
| response loss/duplicate/out-of-order | exact command retry 返回原结果；mutation conflict；late 不改 decision/budget |
| oversized/injection/sensitive output | pre-decode limits 与 closed Schema 拒绝；Kernel-only redaction；文本不成为 instruction/authority |
| old/unknown Schema/reducer | exact retained reader；missing fail closed；旧 Run 不后加 Hook |
| Recorded replay | API 无 handler/writer；counter/event/budget 前后相等；source Run 不覆盖 |

# Alternatives considered

1. 各模块直接注册 callback：分散顺序、权限、预算、取消和 replay 语义，形成绕过点；拒绝。
2. 所有 Hook 使用通用 JSON 输入输出：Observer 可隐式变 Gate，Transform 可改 authority；拒绝。
3. Gate majority 或 first-answer-wins：失败/abstain 可能隐式允许；拒绝，采用 deny 优先与 required explicit allow。
4. Transform 任意 JSON Patch：可改 identity/authority并进行 unknown-field smuggling；拒绝。
5. 并行执行同 point Hook：引入 merge、预算、取消和确定性复杂度且无性能证据；首片采用稳定串行。
6. 在 write transaction 内调用 handler：hung handler 会持锁并扩大故障域；拒绝。
7. Recorded replay 重新运行“确定性”Rust Hook：未来会重复外部调用/核算并重释历史；拒绝。
8. 立即建立 JSONL/MCP/WASI runtime：缺少真实调用方的认证、隔离和升级证据；拒绝提前选 transport。
9. Hook mutable state/decision table：形成第二 authority 和 crash/replay 歧义；拒绝。
10. 等到 REQ-0009/0014 再设计：Effect 或 Loop 会先形成非受控 callback；拒绝。

# Compatibility, migration, and rollback

新增 Hook 协议以新内容地址 SchemaSet 发布，保留全部既有 sets、readers、reducers 和 fixtures。新增 `hook_registry` Manifest role 应使用 RunManifest 新 major，除非保守 compatibility checker 能证明显式 minor；不得用进程 default 为历史 Manifest 补齐。只有新 Run 使用 Hook，旧 Run 生命周期、control、Projection/Snapshot/Recorded replay 保持原解释。

SQLite 预期仍为 v2，Hook stream 复用 events 表。若原子双 stream command 需要 DDL、outbox 表、actor 模型或 public SQL 变化，实施必须停止并回到 RFC。回滚 writer 时保留 Hook Schema/reader/reducer/registry config；pending invocation 不得静默 release，abstain/失败不得重释 allow，已记录决定不得重新执行或覆盖。

# Evaluation and acceptance

- 质量：AC-01 至 AC-20 全部映射到命名、非零、确定性测试；重点是 default deny、Observer 无权威效力、Transform 保护字段、全隔离、防超卖、唯一 terminal、crash recovery 与 Recorded 零执行。
- Token/费用：无真实模型/Provider/Tool；记录 Fake usage 和 unknown 保守核算，不声明优化。
- 延迟：记录 1/N Hook 串行、short-circuit、reserve/settle、争用、fold/replay 观察；无基线前不并行化或设阈值。
- 设计门禁：fresh independent reviewer 对 exact design commit 检查 Requirement/Spec/RFC、现有代码/测试与架构边界；0 open Blocker/Major 后才能接受 RFC、创建 ADR-0009、批准 Spec/Requirement并创建 Plan、Tasks 和 active Handoff。此前禁止 Runtime 功能代码。
