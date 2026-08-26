---
id: REQ-0007
title: Capability、预算、取消与超时
status: reviewing
owners: [maintainers]
created: 2026-08-25
updated: 2026-08-26
links: [EPIC-0002, REQ-0003, REQ-0004, REQ-0005, REQ-0006, SPEC-0006, RFC-0006, ADR-0007, REVIEW-0006]
risk: high
work: .agents/work/active/REQ-0007-capability-budget-cancellation-timeout
---

# Context and user

可信内核、后续 Hook、Effect、Provider、Tool、Sandbox、Agent Loop、Task DAG 与不可信插件需要一个不可绕过、可恢复且可审计的授权、资源核算和停止边界。REQ-0005 已交付完整 Manifest、owner-only lifecycle authority 与同事务 fold-and-append；REQ-0006 已交付 exact-reader Projection/Snapshot/Recorded replay。本需求在这些已完成前置条件之上建立第二个可运行的可信内核纵向切片。

# Problem

当前 Kernel 只能证明 lifecycle owner 可迁移 Run/Task，没有通用 Capability 的签发、委托、收窄、撤销或到期；`RunManifest.budget_revision` 只固定身份，没有可并发安全核算的预算账本；lifecycle `cancelled` 只表示状态结果，不表达请求取消、确认停止、deadline、不可中断边界或迟到 callback。若这些语义由 Hook、Provider、Tool 或策略各自实现，就会形成默认允许、自我提权、跨 Run/Workspace 混用、预算超卖、重试重复扣费、取消/完成竞态和 replay 重复执行。

# Desired outcome

提供 Kernel 私有的最小 Runtime Control 纵向切片：在已创建 Run/Task 上签发并固定最小 Capability；对受保护 Fake Operation 在可信内核内执行默认拒绝和 exact scope 判定；在同一 SQLite 事务中原子预留多作用域预算；按可验证用量确认消费、释放或显式退款；传播 Run/Task/operation 取消和 deadline；以确定性规则处理取消/完成竞态；终态后的迟到或重复结果只产生脱敏审计事实而不改变操作、预算或生命周期；关闭/重开后从权威事件和版本化 Projection 恢复；Recorded replay 不再次执行操作或重复核算。

# Acceptance criteria

- AC-01：Capability 是闭合、版本化且不可变的 Kernel 合同，明确签发者、subject actor、完整 tenant/user presence-value/workspace/run scope、可选 Task scope、exact resource kind/identity、允许 operation 集合、约束、签发/生效/到期时间、delegation policy、parent grant 与 grant ID；Capability 或 payload 本身不等于 authority。
- AC-02：所有受保护操作默认拒绝。只有 Kernel 从认证 principal、persisted Manifest、目标 Task/operation、exact lifecycle history 和 retained exact SchemaSet 构造的私有 admission 才能判定与执行；初始化、管理、reserve/dispatch 与 settlement 分别遵守冻结的 Run/Task lifecycle 准入矩阵，Run/Task 非运行态或终态不能产生新的 dispatch authority。策略、插件、Hook、Provider、Tool、裸协议值和自报 scope 只能请求，不能签发、扩大或绕过 Capability。
- AC-03：root grant 只能由 Manifest owner 对同一隔离域签发；委托只能在父 grant 有效且允许委托时发生，child 的 subject、Task/resource/operation、时间窗、约束和 delegation 深度只能保持或收窄，不能扩大。撤销父 grant 会使其未终态 descendants 对后续新请求无效；到期与 not-before 使用注入 Clock 判定。
- AC-04：Capability 判定返回结构化 `allowed | denied` 与稳定 reason code。已建立且可认证的 Runtime Control aggregate 对默认拒绝、撤销、过期、约束或预算拒绝追加脱敏审计事件；跨 tenant/user/workspace/run/actor 的未授权探测不得向目标 aggregate 写入或泄漏其存在性。
- AC-05：Budget 合同分别表达 token、费用最小单位、elapsed time、tool-call 次数和版本化其他资源；固定 Run、Task、Agent/Actor 与 per-operation limit，硬限制拒绝超额预留，软限制只产生结构化 warning。每类可执行 operation 必须由 Kernel 信任且版本固定的 resource-envelope/meter 合同产生覆盖执行的资源上界；不可信 requested vector 不能降低该上界，无法证明上界就拒绝 dispatch。所有数值使用非负 canonical decimal，不能用浮点或负数。
- AC-06：reserve 在 lifecycle、授权、取消/deadline 与可信 resource-envelope 检查之后、Fake Operation 执行之前发生，并在同一 `BEGIN IMMEDIATE` 中对 exact lifecycle history、全部适用 Run/Task/Actor account 与 operation limit 原子判定和预留；任一状态或维度失败则全部不预留。多连接竞争同一余额或 lifecycle transition 时不得超卖，且 terminal/paused 后不能新 dispatch。
- AC-07：settlement 明确区分 `consume`、`release` 与 `refund`：成功、失败、取消、超时和部分完成均提交已验证实际用量并释放未使用预留；未验证或未知用量按保守规则消费该维度的全部预留并标记 limitation；refund 是 owner-authorized、引用既有 settlement、不可超过已消费且幂等的追加修正，不删除 gross audit history，也不重新打开终态操作。
- AC-08：Provider、Tool、插件或 Fake Operation 自报 usage 只能是 observation；只有绑定 exact operation/reservation/opaque lease、由 Kernel 注册的版本化 producer/evidence contract 接纳的 deterministic meter，或保守 unknown policy，能形成权威 accounted usage。首片只信任 Kernel meter；未来 Receipt/evidence adapter 必须另行受审。重试必须复用相同 command/callback ID；exact retry 返回原结果，same ID 异内容返回稳定冲突且不重复 reserve/consume/release/refund。
- AC-09：Run、Task 和 operation cancellation 分别持久化请求者、原因、requested-at 与目标；请求取消与确认取消是不同事实。Run/Task 请求首片只允许 Manifest owner，operation 请求允许 Manifest owner 或该 operation 的 exact subject；Gate/插件只能提出 proposal。ack 只能来自 reserve 时绑定的受信 executor lease 或 Kernel timeout/recovery 事实。Run 请求覆盖其所有当前和后续 Task/operation，Task 请求覆盖其操作，operation 请求只覆盖自身；请求本身不等同于 lifecycle `cancelled`，也不宣称不可中断操作已经停止。
- AC-10：cooperative operation 获得只读 Kernel cancellation probe，并由绑定 exact operation/reservation/lease 的受信执行边界确认；uninterruptible boundary 可以记录 cancellation pending，但只能在返回 Kernel 后确认或由 Kernel timeout/recovery command 按 deadline 终结。取消、失败、超时与成功是互斥操作终态且保持独立 reason，不互相伪装。
- AC-11：每个受保护操作固定 absolute UTC deadline 与进程内 monotonic deadline。活进程以 monotonic clock 执行 timeout；持久化只保存 UTC deadline、timeout policy 与 clock-observation boundary，重启用注入 wall clock 判定已过期并建立新的 monotonic deadline。Kernel-private timeout/recovery command 必须能在没有 callback 时锁内重新 fold、权威产生 `timed_out` settlement，并按 verified 或 unknown 规则结清 reservation；该命令只能由 Kernel recovery authority 构造，并固定由 persisted timeout key、canonical Clock sample与usage-evidence fingerprint确定性派生的command/event ID及canonical fingerprint。`not_due`不消费identity；提交响应丢失后exact command重试幂等。测试只用 Fake Clock，不依赖真实 sleep。
- AC-12：完成只能在 deadline 之前且没有更早提交的有效 cancellation 时成功；恰好到 deadline 时 timeout 胜。deadline 前取消与完成、lifecycle pause/terminal 与 reserve/dispatch、timeout recovery 与 callback 并发均由同一 SQLite writer serialization 的首个合法 commit 决定；后到命令不得反转终态或在非运行 lifecycle 下制造新 dispatch。timeout recovery 按 integrity/isolation → same event ID exact/mutation → existing terminal → clock due 判定的稳定优先级处理：same-ID异fingerprint冲突，不同ID晚到只返回既有terminal且不追加或核算。模型化并发测试必须接受可观察提交顺序中的唯一合法结果而不允许双终态。
- AC-13：已取消、超时、失败或成功终态后的 callback 不得改变 operation outcome、lifecycle state、budget net/gross accounting 或产生重复 Effect。exact duplicate 返回原处理结果；不同 callback ID 的迟到/乱序结果只可追加绑定完整隔离域和原 operation 的脱敏 `LateResultObserved` 审计事实，保存摘要与安全分类而非敏感 payload。
- AC-14：Runtime Control state 只由一个确定性派生 control stream 的连续 exact validated Event fold 得出；不得建立第二权威可变 state/balance/cancellation 表。重启后缺失首事件、未知 Schema/event/reducer、非法委托、预算负值/超额、重复 settlement、非法取消序列、gap、row identity 漂移或替代 current reader/reducer均 fail closed。
- AC-15：版本化 Runtime Control Projection 可从完整历史恢复 grants、revocations、budget account 的 reserved/gross-consumed/refunded/net-consumed、operations、cancellation/deadline 和 late-result audit counters；输出绑定 store、完整 scope/stream/cursor、source SchemaSet/limits、reducer与 digest。Recorded replay 忽略任何 cache、只读完整历史，Fake Operation 调用计数和所有预算数字均保持不变。
- AC-16：Run、Task、Agent/Actor、Workspace、tenant、user presence/value、control stream、capability/budget/reservation/operation/callback ID 全部 exact 隔离；跨域复用 ID、payload shadowing、父 grant、预算 account、取消或 callback 不能读取、授权、退款或改变目标状态。
- AC-17：发布新的 control event/payload、Capability/Budget/Projection Schema 和内容地址 SchemaSet，保留全部既有 SchemaSet、lifecycle reducer、Snapshot 与 DB v2 合同。只有 Manifest 已固定该 control-capable SchemaSet 的 Run 能初始化 control aggregate；sequence 1、每个 control row、reader limits 与 reducer source identity 必须 exact 等于 Manifest 固定值，不能选择 current/compatible/alternate set。旧 Run/Projection/Snapshot 仍由其 exact retained reader/reducer解释，未知 major 或旧 Capability/Budget 版本不被 current 版本静默升级。首片不需要 SQLite schema migration。
- AC-18：稳定的 Kernel-private capability request、authorize/reserve、cancellation request/probe/ack、callback settlement、带版本化timeout key/command fingerprint的timeout recovery、refund 与 projection/replay 接口，为 REQ-0008、REQ-0009、REQ-0010、REQ-0011、REQ-0013、REQ-0014、REQ-0018 与 REQ-0033 留出版本化扩展点；这些接口必须携带冻结的 lifecycle guard、management/callback authority、trusted resource envelope 与 exact SchemaSet 约束。本需求不得实现 Hook framework、真实 Tool/Provider/外部 Effect、Sandbox、Agent Loop、Memory、Task DAG 或 WASM/WASI 隔离。
- AC-19：测试覆盖 Capability 判定表、默认拒绝/越权、delegation 收窄/禁止提权、撤销/到期、Run/Task lifecycle 准入全状态表与 transition/reserve 竞争、全隔离矩阵、取消 request/ack 授权矩阵、callback/settlement producer 绑定、可信 resource envelope 的低报/漏维度/超界负例、预算 reserve/consume/release/refund、并发防超卖、重试/失败/取消/未知用量、三级取消传播、无 callback timeout recovery、`not_due` identity不消费、commit-response-loss exact retry、same-ID mutation与different-ID terminal no-op优先级、deadline/完成/取消竞态、迟到/重复/乱序结果、崩溃重开、Manifest/control SchemaSet exact 绑定、Event Store/Lifecycle/Projection/Snapshot/Replay 回归、Recorded replay 零执行/零重复核算、旧/未知 Schema 兼容及 bounded model/property 状态序列；每个过滤命令必须证明至少命中一个测试。
- AC-20：REQ-0003 至 REQ-0006 的协议、Schema golden、Event Store migration/append/read/authority、lifecycle state/fold、Projection/Snapshot/Replay/compatibility 全部回归通过；`pareto-protocol` 不反向依赖 Kernel/SQLite，且本切片不增加网络、Provider、Tool 或模型依赖。

# Quality, cost, and latency guardrails

- 质量：默认拒绝、不能自授予/提权、预算不超卖、终态不可反转和 replay 零执行是硬门禁；任一无法从 exact authoritative Event range 重算的余额、授权或取消结论均不合格。
- Token/费用：本需求不调用模型或真实 Provider；记录 Fake usage 与本地测试成本，不宣称 Token/费用优化。未知外部 usage 必须保守核算而非信任自报。
- 延迟：记录授权判定、单次 reserve/settle、并发争用、完整 control fold 和 Recorded replay 的本机观察；SQLite busy 等待沿用有限上界。首版不设无证据吞吐收益阈值。

# Non-goals

- 不实现 REQ-0008 Hook 执行框架、REQ-0009 真实 Effect Intent/Receipt/outbox、REQ-0010 Provider、REQ-0011 Coding Tools、REQ-0013 Sandbox、REQ-0014 Agent Loop、REQ-0018 Task DAG 或 REQ-0033 WASM/WASI 插件运行时。
- 不执行文件、网络、进程、模型、真实 Tool/Provider 或不可逆外部副作用；Fake Operation 只用于确定性内核合同测试。
- 不把 lifecycle `cancelled` 重释为 operation cancellation acknowledgement，不实现隐式跨 Task lifecycle cascade，不新增 scheduler、background timeout worker 或分布式 budget coordinator；只提供显式、Kernel-private 且可幂等调用的 operation timeout/recovery admission。
- 不建立通用 RBAC/ABAC policy language、团队管理 UI、全局管理员查询、动态货币换算、Provider 账单对账或自动 refund；只交付可版本化的最小 Capability 与多维整数预算机制。
- 不为 Runtime Control Projection 增加 Snapshot；首片以完整 control history fold 与 Recorded replay 作为 oracle，并保持 REQ-0006 既有 Run/Task Snapshot 合同不变。

# Risks and open questions

授权链与 confused-deputy、预算原子预留和退款、取消/完成/timeout 竞态、monotonic 与 wall-clock 持久化边界、迟到结果审计、控制事件版本和 replay 零执行会被多个后续 Requirement 依赖，属于跨模块且难以回退的高风险合同。REVIEW-0006 首轮对`05dd7ca`提出6个Major；经两轮修订，同一fresh independent reviewer已对固定提交`a4e3478`关闭全部Major并批准设计，0 open Blocker/Major。Runtime现仅按该合同实施最小纵向切片；完成后仍需新的fresh independent code review。
