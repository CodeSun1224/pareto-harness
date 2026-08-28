---
id: REQ-0008
title: Observer、Gate 与 Transform Hook 骨架
status: specified
owners: [runtime-kernel]
created: 2026-08-28
updated: 2026-08-28
links: [EPIC-0002, REQ-0004, REQ-0007, SPEC-0007, RFC-0007, RFC-0008, ADR-0008]
risk: high
work: .agents/work/active/REQ-0008-observer-gate-transform-hooks
---

# Context and user

可信内核、后续 Effect、Provider、Coding Tool、Sandbox、Agent Loop、Memory、Task DAG、Evidence Graph 与 WASI Guest 的实现者需要一个统一、可审计且不能绕过 Kernel 的扩展观察与决策边界。REQ-0004 已交付 Kernel 私有 append-only Event Store；REQ-0007 已交付默认拒绝 Capability、原子预算、取消、deadline、迟到结果隔离和 Recorded replay 零执行合同。

# Problem

当前没有版本化 Hook 注册、稳定执行顺序、Observer/Gate/Transform 语义或 Hook 决定的恢复/重放路径。若后续组件各自插入 callback，就可能直接写 Event Store、改写已提交事件或权威字段、自我提权、重复预算核算、绕过取消/deadline、跨隔离域读写、在 Recorded replay 中重新执行，或形成第二套权威状态。

# Desired outcome

提供 Kernel 拥有的最小 Hook 骨架：Run Manifest 固定 Hook revision/config；Kernel 在明确生命周期点按稳定顺序调用 Rust Fake Observer、Fake Gate 与 Fake Transform；调用前重建 bounded authority 并原子预留预算；调用后把全部输出视为不可信输入重新验证，记录结构化 Hook 决定；支持取消、deadline、崩溃恢复、迟到/重复/乱序隔离；Recorded replay 只消费已记录决定，绝不重新执行 Hook 或重复核算。

# Acceptance criteria

- AC-01：Hook 合同闭合区分 `observer | gate | transform`，每个注册固定稳定 logical ID、不可变 revision、配置摘要、类型、允许生命周期点、排序键、failure policy、resource contract、input/output Schema 与 redaction policy；Run Manifest 固定完整有序注册集合及配置，运行中不得替换为 current/compatible revision。
- AC-02：Kernel 明确 Hook point 与允许类型矩阵。Observer 只接收已提交事实或已准入只读视图且不能影响权威决定；Gate 只返回 `allow | deny | abstain`；Transform 只转换该 Hook point 明确允许的非权威 proposal/input。未注册类型、point 或组合默认拒绝。
- AC-03：注册和执行顺序由版本化闭合排序规则决定，至少绑定 Hook point、显式 priority、stable logical ID 与 revision；相同输入、Manifest 和已记录历史产生相同顺序。重复注册、排序键冲突、缺 revision/config 或 unknown major fail closed。
- AC-04：Gate 组合规则固定为 deny 优先、否则全部 required gate 必须明确 allow，abstain 不等于 allow；空 required gate 集、失败、timeout、非法输出、unknown version 或缺失决定默认 deny。短路只能跳过尚未调用的 Gate，且跳过原因和最终决定可审计。
- AC-05：Observer 失败策略只能在注册时选择 `warn-and-continue | fail-closed`；失败不得隐式改变业务决定。Gate 一律 fail closed。Transform 失败、timeout 或非法输出不得留下部分权威修改，并按注册策略返回原 proposal 的拒绝结果或整体失败，不能静默接受未转换输入。
- AC-06：Transform 允许字段由每个 Hook point 的版本化 transform mask/schema 明确列举；Kernel 对结果重新验证 Schema、size、scope、identity 与限制。Transform 永远不能创建或改写已提交 Event、Manifest、SchemaSet、identity、principal/authority、Capability/grant/lease、budget/reservation/accounting、deadline/cancellation、Effect Receipt、Evidence 或 Run/Task/operation terminal。
- AC-07：每次调用前 Kernel 从认证 principal、persisted Manifest、exact lifecycle/control history及目标 Workspace/Run/Task/Actor重建调用上下文；扩展自报 scope 不参与授权。Hook 只获得不可伪造、不可序列化、可收窄的 bounded invocation authority，不能签发、委托或扩大 Capability。
- AC-08：tenant、user presence/value、Workspace、Run、Task、owner/subject Actor、Hook registration/invocation/attempt/decision ID 全部 exact 隔离；跨域 ID、payload shadow、兼容 SchemaSet 或 Hook cache/state 不能读取、执行、确认或改变目标决定。未授权探测不得向目标 stream 写入或泄漏存在性。
- AC-09：每次 Hook invocation 使用 REQ-0007 trusted operation contract、Kernel meter、Run/Task/Actor/per-operation account 与同一 SQLite writer transaction 原子 reserve；proposal 自报用量不能降低上界，任何维度不足全部拒绝且不调用 Hook，并发调用不得超卖。
- AC-10：Hook 成功、失败、取消和 timeout 按 REQ-0007 verified/unknown 规则 consume/release；同一 invocation/attempt 的 exact retry 不重复 reserve/consume/release。Hook 无权 refund、自增预算、绕过 operation limit 或把 observation 升级为权威 usage。
- AC-11：Hook invocation 继承 Run/Task/operation cancellation 与 absolute/monotonic deadline，支持 cooperative probe；hung/uninterruptible Hook 只能在返回 Kernel 或由 Kernel timeout/recovery authority按已持久化 deadline 结清。所有测试使用可注入 Fake Clock，不依赖真实 sleep。
- AC-12：完成、取消、timeout、Kernel 重启与 response loss 由同一 writer serialization 和稳定 command identity决定唯一 terminal。终态后的 late、duplicate、retry、out-of-order Hook 结果不得改变最终 Hook decision、权威状态、lifecycle、budget或Effect，只可形成绑定原 invocation 的脱敏审计事实。
- AC-13：Hook 输出、错误和日志均按不可信输入处理；闭合大小/深度/集合限制在反序列化与业务验证前执行。结构化拒绝只保存稳定 reason、安全 ID、摘要、版本与 redaction policy，不保存敏感 payload、秘密、路径、SQL或跨域身份；output injection不能变成 Kernel instruction/authority。
- AC-14：Hook registration、invocation、attempt、result/decision、skip、late/rejected audit 由版本化 Event 表达并按连续 exact validated Hook stream pure fold；不得建立可被 Hook 写入的数据库、mutable decision table、共享 cache 或第二权威状态。最终权威事件仍只由 Kernel 在全部准入后提交。
- AC-15：Kernel crash/reopen 从 Event Store 与版本化 Hook Projection 恢复 pending/terminal invocation、组合状态和 budget binding；响应丢失后 exact command重试幂等。缺 first event、gap、非法顺序、双 terminal、错误 authority/budget binding、unknown reader/reducer或 current substitution均 fail closed。
- AC-16：Recorded replay 只读完整已记录 Hook 决定并重建 Projection，不调用 Hook handler、不 reserve/settle、不追加 Event；普通与 Recorded 仅在完整 provenance 相同才可比较。Simulated/Reexecute 必须创建独立派生 Run、显式选择模式并由后续 Requirement 批准，不能污染 source Run；本切片稳定拒绝其 Hook 执行入口。
- AC-17：发布新的 Hook Schema/Event binding与内容地址 SchemaSet时保留全部既有 set、RunTask/RuntimeControl reducer、SQLite v2 DDL/trigger和历史字节。只有 Manifest exact 固定 Hook-capable SchemaSet及Hook registry revision的 Run 可初始化；旧 Run 不后加 Hook、不静默升级。
- AC-18：Rust Fake Hook 是 reference implementation，接口只表达版本化 request/result/authority语义，不暴露 Rust ABI、SQLite布局或内核对象；不选择或实现 shell、Python、TypeScript、HTTP、MCP、WASI、队列、RPC或外部 Worker transport。
- AC-19：稳定、版本化、默认拒绝的 Kernel-private接口为 REQ-0009、0010、0011、0013、0014、0015、0018、0026、0033 留出 proposal/observation/gate/transform接入点，但不实现这些需求，也不授予其 Event、Capability、Budget、Effect/Evidence、Replay或终态 authority。
- AC-20：测试覆盖 Hook 判定表、稳定排序/组合/短路、默认拒绝、自我提权、Transform权威字段保护、全隔离矩阵、Capability撤销/过期/收窄、budget reserve/settle与并发防超卖、cancel/deadline/timeout/terminal race、hung recovery、late/duplicate/retry/out-of-order、crash/reopen、Event Store/Lifecycle/Projection/Snapshot/Replay回归、Recorded replay零执行/零核算、old/unknown版本、output injection/敏感日志/oversized output及bounded command/concurrency model；每个命名过滤命令必须证明非零命中。

# Quality, cost, and latency guardrails

- 质量：默认拒绝、Observer无权威影响、Transform不可触碰保护字段、预算不超卖、唯一终态、跨域隔离和 Recorded replay 零执行是硬门禁；所有决定必须由 exact Event range 重算。
- Token/费用：不调用真实模型、Provider或Tool；Fake Hook usage仍由Kernel账本核算。本需求不宣称费用或Token优化。
- 延迟：记录单Hook、N Hook排序/组合、争用、timeout recovery、完整fold与Recorded replay的本机观察；首版不设无证据阈值，不引入background worker或远程transport。

# Non-goals

- 不实现真实 shell/Python/TypeScript/HTTP/MCP/WASI Hook Runtime、外部 Worker、Rust动态库ABI或通用插件执行器。
- 不实现真实Provider、Coding Tool、外部Effect、Sandbox、Agent Loop、Memory、Task DAG、Evidence Graph或WASM/WASI隔离。
- 不允许Hook直接写Event Store、修改已提交事件、构造Capability/lease/recovery authority、管理budget、决定Run/Task终态或自报Evidence/Receipt为权威。
- 不实现Simulated fixture resolver、reexecute执行器、Hook background scanner、Hook Snapshot、远程数据库或多节点调度。

# Risks and open questions

Hook point集合、Gate默认拒绝与组合、Transform mask、调用/决定Event顺序、invocation identity、budget/cancel/timeout复用、失败恢复、redaction、版本兼容和Recorded replay是跨需求且难逆合同。SPEC-0007与RFC-0008必须经fresh independent设计评审批准并达到0 open Blocker/Major后，才能批准本Requirement并创建Plan/Tasks/active Handoff；此前禁止Runtime功能代码。
