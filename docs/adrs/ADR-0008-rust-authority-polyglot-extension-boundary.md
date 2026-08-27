---
id: ADR-0008
title: 采用 Rust 权威控制面与多语言扩展边界
status: accepted
owners: [maintainers]
created: 2026-08-27
updated: 2026-08-27
links: [REQ-0001, RFC-0007, ADR-0001, ADR-0002, ARCH-0001, ARCH-0004, ROADMAP-0001, BACKLOG-0001, REVIEW-0009]
---

# Context

ADR-0001/0002 已选择 Rust 可信内核和模块化单体作为起点，但没有规定 G2 至 G5 的 Provider、Tool、Hook、Agent Worker、Memory、评测、SDK 和插件必须全部使用 Rust。若把实现语言直接等同于信任，会扩大内核、增加生态接入和研究成本，也不能替代权限准入、协议验证或 OS Sandbox。

# Decision

G1 至 G5 保持 Rust 权威控制面：Event、version identity、state transition、Capability、Budget、Cancellation、Effect/Evidence admission、Replay、Lease/MVCC、completion 和 Promote/Rollback 只能由 Rust 可信内核裁决并提交。

Provider、Tool、Hook handler、Agent Worker、Memory 检索/排序、评测、候选生成、SDK 和 WASI Guest 可以采用适合其生态与故障域的语言，但只能提出请求、观察、计算，或在 opaque bounded authority 内执行；不得直接写权威状态、自授权限、增加预算、决定终态、伪造 Receipt/Evidence 或晋升候选。

Rust reference implementation 不构成语言强制。每个真实跨语言 Runtime 必须由首次需要它的 accepted Requirement 单独冻结身份、Schema、权限、预算、取消、timeout、late result、Effect、Replay、隔离、升级与回滚合同，并以可复现的质量、成本或延迟收益覆盖协议、部署和调试复杂度。本 ADR 不选择 JSONL、MCP、WASI、RPC、队列、FFI、容器或远程服务。

Sandbox 的 policy、admission、deadline、预算和审计属于 Rust 控制面，实际 enforcement 可由 OS、容器、远程 Sandbox 或 WASI 实现。Memory 的 provenance、隔离、版本、失效与权威引用准入属于可信面；embedding、retrieval、rerank、compression 和 summarization 属于可演化计算面。

# Alternatives

- G2 至 G5 全部 Rust：保留为默认 reference path，但拒绝作为长期语言强制。
- G2 起改用 Python 或 TypeScript 主 Runtime：会复制或外移已建立的 authority，拒绝。
- 立即冻结通用跨语言 Worker 协议：缺少首个真实调用者与失败语义，拒绝提前选型。
- 所有组件同权插件化：会允许插件替换安全语义或形成第二事实源，拒绝。

# Consequences

可信面保持小且可审计，同时允许生态 adapter、研究计算和隔离 Worker 选择合适语言。代价是每个跨语言边界都需要版本化协议、身份验证、资源核算、取消恢复、兼容读者、供应链和部署证据；没有直接收益时继续使用进程内 Rust 实现。

ADR-0001/0002 继续有效。本决定只收窄“必须 Rust”的适用范围，不改变现有 Runtime、Schema、SQLite、Manifest 或 Requirement 顺序，也不授权任何尚未批准的外部 Runtime。

# Revisit triggers

- 某类扩展无法在不取得 authority 的情况下满足产品需求。
- 跨进程协议、部署、冷启动或调试成本持续超过多语言收益。
- WASI、容器或 OS enforcement 无法达到已声明的隔离强度。
- 可测量的吞吐、故障域、数据驻留或组织所有权要求需要新的控制面拆分。
