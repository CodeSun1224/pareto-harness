---
id: ADR-0009
title: 采用 Kernel 治理的 Observer、Gate 与 Transform Hook 合同
status: accepted
owners: [maintainers]
created: 2026-08-28
updated: 2026-08-28
links: [REQ-0008, SPEC-0007, RFC-0008, RFC-0007, ADR-0008, REQ-0004, REQ-0007, ARCH-0002, ARCH-0003, ARCH-0004, REVIEW-0010]
---

# Context

REQ-0004 已交付 Kernel 私有 append-only Event Store，REQ-0007 已交付默认拒绝 Capability、原子预算、取消、deadline、唯一 terminal、迟到结果隔离和 Recorded replay 零执行。后续 Effect、Provider、Tool、Sandbox、Agent Loop、Memory、Task DAG、Evidence Graph 与 WASI Guest 都需要在这些权威边界内观察或提出非权威变更；若各自直接注册 callback，就会分裂顺序、权限、预算、失败、恢复和 replay 语义，甚至形成第二权威状态。

RFC-0007/ADR-0008 已决定 Rust 拥有权威控制面而不强制所有 handler 使用 Rust。REQ-0008 的 fresh independent REVIEW-0010 对 exact design revision `3aee02adf8815466b02f51de247ae19922efc126` 批准，0 open Blocker/Major；本 ADR 只接受该设计，不表示 Runtime 已实现。

# Decision

采用由 Rust 可信 Kernel 编排、实现语言中立的 Observer、Gate 与 Transform Hook 合同：

- Run Manifest exact 固定内容寻址的 Hook registry revision、完整有序注册和配置；unknown、missing、current substitution 或运行中热替换均 fail closed。
- Hook point 的跨类型 phase 固定：`before_proposal_admission` 为 Transform → Gate → Observer，`before_authoritative_commit` 为 Gate → Observer，`after_*` 只有 Observer；phase 内按 priority、logical ID、revision 稳定排序，并持久化 input lineage 与 point finalization。
- Observer 只读且业务输出无权威效力；其 failure policy 仅为 `warn-and-continue | fail-closed`，后者只能改变分离的 execution status 和下游推进，不能改写已经固定的业务决定或已提交事实。
- Gate 只返回 allow/deny/abstain，deny 优先，全部 required Gate 必须明确 allow；Gate-bearing point 的零 Gate 或仅 optional Gate 无条件 deny，不存在 `none` 绕过字段。失败、timeout、非法输出和 unknown version 固定 fail closed。
- Transform 只串行转换 point mask 明确允许的非权威 proposal 字段；identity、scope、authority、Capability、budget、deadline/cancellation、Receipt、Evidence、terminal、Manifest、Schema 和任何 unknown 字段都受保护。失败固定拒绝整个 proposal，不接受原输入继续，也不留下部分权威修改。
- Kernel 每次调用前从认证 principal、persisted Manifest、lifecycle/control/history 和完整隔离 scope 重建不可伪造、不可序列化、可收窄的 invocation authority；handler不能取得 Event transaction、Capability/lease constructor、budget、timeout recovery、Effect/Evidence admission或终态 authority。
- budget reserve与invocation admission、operation settlement与Hook terminal分别使用Kernel-private双stream atomic pair command。pair固定identity/fingerprint、双cursor/sequence、两个Event bytes和交叉引用；zero才原子写两者、two只允许exact retry、mutation冲突、one即损坏，任一写入/commit失败全rollback。Hook operation的通用single-stream terminal入口必须拒绝，不能先结算再补写Hook事实。
- 全部输出先受限再按Schema、scope、identity、producer、lease和保护字段重验；拒绝与日志只保留稳定reason、安全ID、digest、revision、cursor和redaction policy，不保存敏感payload。
- Event Store仍是唯一事实源；Hook stream与Projection采用exact retained reader和pure fold。Recorded replay只消费已记录决定，不加载handler/writer、不reserve/settle、不追加Event；Simulated/Reexecute在本切片稳定拒绝且不能污染source Run。

首个实现只允许进程内Rust Fake Observer/Gate/Transform与Fake Clock。合同不暴露Rust ABI、SQLite布局或Kernel私有对象，也不选择shell、Python、TypeScript、HTTP、MCP、WASI、队列、RPC或外部Worker transport；任何真实外部Runtime由其首次调用Requirement另行批准。

# Alternatives

- 各模块直接注册 callback：会形成多个顺序、权限、失败和replay解释；拒绝。
- Observer/Gate/Transform 共用任意JSON返回值：会让Observer隐式成为Gate或让Transform改写authority；拒绝。
- Gate majority、first-answer或空集合默认allow：失败、abstain和缺配置可能放行；拒绝。
- Transform使用任意JSON Patch或失败后回退原proposal：会允许保护字段走私或静默绕过必需转换；拒绝。
- 在handler执行期间持有SQLite write transaction：hung handler会扩大锁和故障域；拒绝。
- 用补偿Event修复control/Hook单边成功：会产生预算与invocation/terminal暂时或永久分叉；拒绝，采用atomic pair。
- Recorded replay重新运行“确定性”Hook：会重释历史并为未来外部handler打开重复执行/核算入口；拒绝。
- 立即冻结通用跨语言transport：缺少真实调用方、隔离和收益证据；延期。

# Consequences

Hook 顺序、准入、预算、取消、失败、恢复和 replay 都由同一个可信边界裁决，后续需求可复用稳定 proposal/observation/decision 接口而不能取得 authority。代价是实现必须重构当前自提交的 Runtime Control 私有准入为 transaction-local helper，并增加双stream pair、input lineage、pure fold、兼容reader、SQLite fault injection、并发model和负向安全测试；这些复杂度不能通过public SQL、补偿表或弱化默认拒绝规避。

SQLite预期保持v2并复用events表；若实现需要DB v3、outbox/decision table、alternate Event actor、public authority、background recovery writer或新外部依赖，必须停止实施并回到RFC。发布首个Hook Event后，rollback必须保留Schema、registry、reader/reducer和历史决定解释；旧Run不得后加Hook或升级到current registry。

# Revisit triggers

- Atomic pair无法在现有Event Store事务边界内证明双写rollback和exact retry。
- 某个真实外部handler需要跨进程认证、资源强制、隔离或取消语义。
- 并行Hook有可复现的质量/成本/延迟收益，足以承担merge与确定性复杂度。
- 需要新增Hook point、可配置Transform失败策略、Gate策略或Simulated/Reexecute执行。
- SQLite v2、单writer或单Run Hook stream无法满足已批准的恢复与性能目标。
