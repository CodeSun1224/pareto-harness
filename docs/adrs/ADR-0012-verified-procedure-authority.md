---
id: ADR-0012
title: 采用已验证流程版本与 Kernel 流程权威
status: accepted
owners: [maintainers, runtime-kernel]
created: 2026-09-05
updated: 2026-09-05
links: [REQ-0034, SPEC-0010, RFC-0013, REVIEW-0018, PRD-0001, ARCH-0002, ADR-0001]
---

# Context

REQ-0003 至 REQ-0009 已交付版本、事件、生命周期、Runtime Control 与 Effect 基础，但原路线会在 Plan/DAG、Node 状态机和最小 Evidence Gate 之前实现自由 Agent Loop，并把完整 Behavior Promotion 放到很晚。这能形成更安全的普通 Coding Agent，却不能保证后续 Run 严格复用已由证据和独立审批支持的成功路径。

# Decision

采用 RFC-0013：

- 将 `ProcedureRevision`、`TaskClassRevision`、`VerifiedProcedureRevision`、完整 review subject 与 principal-root 独立性作为正式、不可变、内容寻址的合同；REQ-0034 只交付纯 retained registry admission。
- 将 REQ-0018 PlanRevision/基础 Task DAG 前移，并由它唯一拥有无外部 I/O 的 Plan bootstrap、closed Procedure instantiation 与 procedure-capable sequence-1 Run Manifest。
- 在单 Agent executor 前依次交付 REQ-0035 Kernel Node 状态机与 checkpoint、REQ-0016 最小 Evidence Gate；REQ-0014 只能执行 Node-scoped proposal，不能成为自由 Agent Loop。
- 由 REQ-0036 单独交付成功流程候选提升、固定复用和流程版本回退；REQ-0028 至 REQ-0032 的 Behavior 演化保持正交。
- Conversation Memory、用户偏好、项目经验和操作说明保持非权威；Provider、Tool、Workspace、Sandbox 和模型结果只能通过 Kernel capability/effect/evidence admission。
- 分别定义 Procedure/Behavior rollback、Run recovery、Workspace recovery、Effect reconciliation/compensation，以及 recorded replay、reexecute、simulated；任何一种都不能删除或重解释历史事实。

该决策保证 Kernel 可观察边界内的流程遵循和证据准入，不承诺现实结果百分百正确。

# Alternatives

- 保持原路线并先实现自由 Agent Loop：拒绝，因为后补 DAG/Evidence 无法证明已有执行路径没有 authority 旁路。
- 用 Memory、Prompt 或 Markdown 固化成功路径：拒绝，因为非权威文本不能强制状态、权限、证据、恢复或补偿。
- 只前移 Plan/DAG：拒绝，因为缺少跨 Run 的已验证流程身份、审批与复用合同。
- 一次实现完整 workflow language、Evidence Graph 与自动演化：拒绝，因为范围和迁移面过大，无法形成独立可验证纵切。

# Consequences

产品路径更早形成“流程版本 → Plan → Node → Evidence → executor → promotion/reuse”的闭环，避免把 Provider 或 Agent Loop 误当最终产品。代价是 G2 增加 REQ-0034 至 REQ-0036 和 EPIC-0007，原工期估算失效；每个边界需 forward Schema、兼容、隔离、恢复与独立评审。

第一版 pre-Run Plan 只能是用户输入或 pinned pure builder 生成的内容寻址 artifact，Harness 内不允许模型/工具/Workspace/Sandbox I/O。模型辅助 planning 必须以后续独立 Requirement 建立受治理 planning Run，不能绕过 sequence-1 authority。

# Compatibility and rollback

本 ADR 不修改 Rust、SQLite v2、SchemaSet 或已完成 REQ-0003 至 REQ-0009 的事实。既有 Requirement ID 不重用；新增 REQ-0034 至 REQ-0036。未来只使用 forward major；旧 Run 不补 Procedure pin，也不重新解释为 verified execution。

若该产品方向被替代，必须以新 Requirement/RFC/ADR supersede 本决策；不得删除本 ADR、改写历史 Run 或恢复归档 REQ-0010 的旧 authority 路径。

# Revisit triggers

- exact Procedure→Plan conformance 无法以闭合、可测试关系表达。
- principal-root 角色分离无法在目标部署环境可靠判定。
- Node-bound capability 无法覆盖 Provider、Tool、Workspace 或 Sandbox 的真实旁路。
- 最小 Evidence Gate 无法在不引入完整 Evidence Graph 的情况下阻止错误完成。
- 可复现实验证明该流程闭环在保持质量底线时无法满足可接受的成本或延迟范围。
