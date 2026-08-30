---
id: REQ-0009
title: Effect Intent/Receipt 与幂等效果
status: approved
owners: [runtime-kernel]
created: 2026-08-30
updated: 2026-08-30
links: [EPIC-0002, REQ-0004, REQ-0007, REQ-0008, SPEC-0008, RFC-0009, ADR-0010, REVIEW-0012]
risk: high
work: .agents/work/active/REQ-0009-effect-intent-receipt-idempotent-effects
---

# Context and user

可信内核、后续 Provider、Coding Tool、Sandbox 与 Agent Loop 的实现者需要一个不可绕过、可恢复且可对账的外部效果边界。REQ-0004 已交付 Kernel 私有 append-only Event Store；REQ-0007 已交付默认拒绝 Capability、原子预算、取消、deadline、迟到结果隔离与 Kernel-private operation/reservation 合同；REQ-0008 已交付 Kernel 治理的 Fake Hook 骨架，但明确不执行外部效果。

# Problem

SQLite 事务不能与文件、进程、网络、模型或其他外部系统形成同一原子提交。若调用方在没有持久化 Intent 时直接执行，或在响应丢失、进程崩溃和 timeout 后盲目重试，就可能重复产生不可逆效果；若把外部返回值直接视为权威 Receipt 或预算结算证据，又会允许伪造成功、跨域串用、重复核算或终态反转。当前也没有稳定方式区分“尚未执行”“已执行但结果未知”“已观察结果”“需要人工或自动对账”。

# Desired outcome

提供 Kernel 拥有的最小 Effect Intent/Receipt 纵向切片：受保护的确定性 Fake Effect 只有在授权、预算和生命周期准入后，先持久化绑定 exact operation 的 Intent，才能通过不可伪造的 dispatch lease 执行；相同幂等键只能代表同一个规范化效果；结果以不可信 Receipt observation 返回，经版本固定的可信 admission 验证后形成唯一权威状态和预算结算；响应丢失、部分成功、崩溃、取消、timeout、迟到结果与重复提交进入可恢复的 reconciliation 流程；Recorded replay 只读取既有事实，绝不再次执行效果。

# Acceptance criteria

- AC-01：Effect 合同闭合、版本化且不可变，至少固定 effect kind/revision、内容地址executor descriptor/revision/config、完整隔离域、Run/可选 Task、subject/executor、已批准 operation/reservation、规范化请求摘要、幂等键、deadline、重试/对账策略和 redaction policy；请求 payload、幂等键、兼容摘要或 Receipt 本身都不等于 authority。executor identity必须贯穿Manifest registry、Effect/request identity、Intent、claim、lease、Receipt admission、Projection与reopen，same key替换executor稳定冲突。
- AC-02：所有 Effect 默认拒绝。只有 Kernel 从认证 principal、persisted Manifest、exact lifecycle/control history、Capability、reservation 与 retained exact SchemaSet 构造的私有 admission 才能创建 Intent 和 dispatch authority；插件、Hook、Provider、Tool、裸协议值和自报 scope 只能提出请求。
- AC-03：每次外部 dispatch 前必须先原子提交唯一 Intent，并绑定当时仍有效的 lifecycle、授权、预算 reservation、operation、deadline 和规范化请求；Intent 提交失败、未提交或随后无法证明 exact Intent 时不得执行。不得声称 SQLite Event 与外部效果形成跨边界原子事务。
- AC-04：幂等键在版本化 effect kind 和完整隔离域内唯一。exact 请求重试返回原 Intent/终态且不重复 reserve、dispatch 或 settlement；同键异请求、异 operation/reservation、异 scope 或异版本稳定冲突。调用方自选键不能跨 Effect 类型或隔离域去重，也不能覆盖 Kernel 派生身份。
- AC-05：只有绑定 exact Intent/operation/reservation/executor descriptor、用途受限且不可序列化伪造的单次 dispatch lease 能进入 Manifest-pinned Fake Effect executor；lease 的签发、领取、提交结果和失效规则可审计，不能被重放到另一executor、请求、attempt、Run、Task、Actor、process epoch或Workspace。
- AC-06：首片提供确定性 Fake Effect，能够分别模拟成功、业务失败、执行前失败、外部已接受但本地响应丢失、部分成功、timeout 和返回后进程崩溃；不以真实文件、进程、网络、Provider、Tool 或 Sandbox 副作用冒充合同证明。
- AC-07：Receipt 明确是外部结果 observation，包含 effect/attempt identity、外部幂等 identity、结果分类、观察时间、版本、摘要、可验证 usage/evidence 与 limitation，不自动成为权威成功或预算结算。只有 Kernel 注册并由 Manifest/SchemaSet 固定的 producer/evidence adapter 或明确的保守 unknown policy 能接纳 Receipt。
- AC-08：Effect 状态闭合区分至少 `intent-recorded`、`dispatching`、`succeeded`、`failed-before-effect`、`failed-after-possible-effect` 与 `reconciliation-required`；成功、确定未执行、可能已执行和结果未知不得互相伪装。每个 Intent 只有一个权威 terminal/reconciliation outcome，后到结果不能反转已确定终态。
- AC-09：部分成功必须保留已确认的外部效果和未确认部分、限制与对账状态；不得把部分成功整体标记为未执行后自动重试。是否允许补偿、重试或查询由版本化 effect policy 明确决定，默认不得对可能已发生的不可逆效果重新 dispatch。
- AC-10：响应丢失或 crash/reopen 后，Kernel 从连续 exact Event history恢复pending/unknown Effect；不得由临时调用方重建authority。每个Intent持久化recovery contract，claim后扩展为绑定scope/effect/attempt/claim process epoch/operation/reservation/executor/policy/deadline/source identity的recovery key；Kernel-private recovery command固定canonical Clock/process-loss/usage evidence、domain-separated fingerprint和双Event IDs，并按integrity/isolation → same-ID exact/mutation → existing terminal → eligibility/due判定。not-eligible不消费identity；commit-response-loss exact retry或新sample terminal no-op均不重复settle。Intent未claim且可信证明旧process epoch已失效时可确定not-applied；claim后只能partial/unknown并进入reconciliation。首片不授权再次dispatch。
- AC-11：reconciliation 是独立、可审计、默认拒绝的 Kernel-private 命令，固定来源、证据、查询/补偿 policy 与 command fingerprint；exact retry 幂等，same ID 异内容冲突。Receipt、查询观察或人工声明只有经过受审 producer/admission 后才能关闭对账，且不得删除 Intent、attempt、部分结果或 gross budget history。
- AC-12：Effect 完成按 REQ-0007 的 verified/unknown 规则 consume/release reservation；部分或未知 usage 保守结算，Receipt 自报 usage 不能降低已证明消耗上界。Effect terminal 与 Runtime Control operation terminal/settlement 必须原子一致，禁止单边终态、重复结算、refund 或重新打开 operation。
- AC-13：取消、deadline 与 timeout 阻止新的 Intent 或尚未领取的 dispatch；Intent存在但未claim时，history证明Kernel从未交付executor lease，deadline/取消recovery必须唯一结论为`not_applied`；claim后已越过可能执行边界，只能记录 cancellation pending/partial/unknown并等待返回或reconciliation，不能虚假确认外部效果停止。恰好 deadline 的竞争、callback 与 timeout/recovery 由同一 writer serialization 和稳定优先级产生唯一合法权威结果。
- AC-14：终态后的 exact duplicate 返回原处理结果；不同 ID 的迟到、乱序或矛盾 Receipt 只形成绑定原 Effect 的脱敏 audit/reconciliation observation，不改变 Effect/operation outcome、lifecycle 或 budget。敏感 payload、秘密、路径和外部原始错误不进入权威事件。
- AC-15：Effect Intent、dispatch attempt、Receipt admission/rejection、terminal、partial/unknown 与 reconciliation 都由版本化 Event 表达并按一个连续 exact validated Effect stream pure fold；不得建立第二权威 mutable outbox/status/receipt 表。实现可使用非权威索引或投影，但必须能从完整权威历史验证并重建。
- AC-16：tenant、user presence/value、Workspace、Run、Task、Actor、effect/attempt、operation/reservation、idempotency/dispatch/reconciliation command identity 全部 exact 隔离；跨域复用、payload shadowing、兼容 SchemaSet 替代或未授权探测不能读取、执行、确认、结算或泄漏目标存在性。
- AC-17：Effect Projection 在 crash/reopen 后恢复 Intent、attempt、executor/recovery identity、Receipt、terminal、partial/unknown、reconciliation 与 budget binding；缺 first event、gap、非法顺序、双 terminal、错误 binding、unknown reader/reducer、row identity 漂移或 current-version substitution 均 fail closed。Projection读取必须接受显式inclusive cursor/horizon并把source history digest纳入identity，不能默认为stream最新。
- AC-18：Effect-capable Run使用新的versioned Boundary Inventory/record major，固定exact Effect stream cursor、history digest与逐Effect request/attempt/external identity，并无损表达`applied | not_applied | partial | unknown`、confirmed/unknown摘要、limitations和reconciliation binding；旧Inventory/reader bytes保持不变，partial/unknown不得序列化为旧`Failed-before-receipt`。Recorded replay只读该inventory固定horizon内的source Effect事实并逐项核对；inventory之后的late/reconciliation Event不参与同一pin，不改变结果。Replay不创建Intent、lease，不执行/查询Effect，不reserve/settle/append；Simulated/reexecute入口稳定拒绝。
- AC-19：发布 Run Manifest v3、Effect/Executor/Event/Projection/Boundary Inventory v2 Schema 与内容地址 SchemaSet时保留全部既有Manifest、Inventory、SchemaSet、reader/reducer、SQLite v2 DDL/trigger和历史字节；只有 Manifest exact 固定 Effect-capable SchemaSet、effect registry/policy及executor descriptor的Run能初始化。旧Run不后加Effect、不静默升级。
- AC-20：测试覆盖默认拒绝、Intent-before-dispatch、幂等 exact retry/same-key/executor mutation、全隔离矩阵、dispatch lease复用、部分成功、响应丢失、commit-response-loss、Intent未claim/claim已提交/executor已应用/terminal pair前后crash、recovery not-eligible/exact-mutation/new-sample terminal no-op、unknown outcome、cancel/deadline/timeout/receipt竞态、late/duplicate/out-of-order、reconciliation admission、预算原子结算、单边终态损坏、crash/reopen、old/unknown版本、Inventory v1/v2与fixed-horizon Recorded replay零执行/零核算，以及 Event Store/Lifecycle/Runtime Control/Hook/Projection/Snapshot/Replay回归；每个命名过滤命令必须证明至少命中一个测试。
- AC-21：稳定、版本化、默认拒绝的 Kernel-private proposal/intent/dispatch/receipt/reconciliation/projection接口为 REQ-0010 Provider、REQ-0011 Tool、REQ-0013 Sandbox与REQ-0014 Agent Loop留出接入点，但不实现这些需求，也不授予外部组件 Event、Capability、Budget、Receipt/Evidence、Replay 或 terminal authority。
- AC-22：Run/Task `succeeded` 迁移必须在同一 writer transaction 中折叠 exact Effect history；目标范围存在未结清 operation、尚未确定的 dispatch 或 `reconciliation-required` Effect 时拒绝成功。`failed | cancelled` 可在 Runtime Control operation 已结清后保留待对账 Effect，后续 Receipt/reconciliation 不得重开生命周期；transition/Intent/dispatch/terminal 竞争产生唯一合法提交顺序。

# Quality, cost, and latency guardrails

- 质量：Intent-before-dispatch、同键异请求拒绝、可能已发生的效果不盲重试、部分结果不丢失、唯一终态/原子结算、跨域隔离与 Recorded replay 零执行是硬门禁；所有权威判断必须能由 exact Event range 重算。
- Token/费用：本需求不调用模型、真实 Provider 或付费外部系统；Fake usage 仍由 Kernel 账本核算。未知 usage 保守结算，不宣称 Token/费用优化。
- 延迟：记录 Intent admission、dispatch/Receipt terminal pair、reconciliation、竞争、完整 fold 与 Recorded replay 的本机观察；沿用 SQLite 有限 busy 上界，首片不设无证据吞吐收益阈值，不引入 background worker。

# Non-goals

- 不实现真实文件、进程、网络、模型、Provider、Coding Tool、Sandbox、Agent Loop、自动补偿器或通用工作流引擎；只交付 Fake Effect 的内核合同纵切。
- 不承诺 exactly-once 外部执行；目标是在可控边界实现持久 Intent、幂等 dispatch、at-most-once-or-reconcile 语义和可验证对账。
- 不把 Receipt 或外部系统自报 usage/evidence直接提升为权威事实，不允许扩展直接写 Event Store、管理预算或决定 terminal。
- 不实现 background outbox poller/scanner、远程队列、分布式事务、两阶段提交、跨 Run 去重、自动redispatch、通用 saga/compensation language、Effect Snapshot、Simulated fixture resolver 或 reexecute执行器；recovery仅提供显式Kernel-private command。

# Risks and open questions

Intent 与 reservation 的原子边界、幂等键作用域、dispatch lease、Receipt producer trust、部分成功、unknown outcome、Effect/operation 双 stream 原子终结、对账准入、取消竞态、redaction、版本兼容与 replay 零执行会被多个后续 Requirement 依赖，属于跨模块、安全敏感且难以回退的合同。影响分析已识别 Runtime Control Hook-specific pair 字段、lifecycle success guard、Run Manifest v3、Effect stream/Projection、Boundary Inventory 与 retained SchemaSet 为直接影响。REVIEW-0012对初始fixed `9f8bf23`提出4个Major；Requirement/RFC/SPEC经多轮同Reviewer复审，在fixed `b7acbd82824d8410d432117c89be1bd56c8ce05c`关闭F-001至F-004并达到independent approved、0 Blocker、0 Major。ADR-0010接受该合同；实现仍未开始。
