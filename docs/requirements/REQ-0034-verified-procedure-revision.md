---
id: REQ-0034
title: 不可变已验证流程版本
status: specified
owners: [runtime-kernel]
created: 2026-09-05
updated: 2026-09-05
links: [EPIC-0007, REQ-0003, REQ-0004, REQ-0005, REQ-0007, REQ-0009, SPEC-0010, RFC-0013]
risk: high
work: .agents/work/active/REQ-0034-verified-procedure-revision
---

# Context and user

需要重复执行高价值 Coding Agent 任务的平台工程团队，希望把一次成功路径从非权威会话轨迹提升为可审计、可复用且不可绕过的流程合同。REQ-0003 至 REQ-0009 已交付版本、事件、生命周期、Runtime Control 与 Effect 基础，但尚未实现流程版本、Plan/Task DAG、节点状态机或执行期 Evidence Gate。

# Problem

当前路线可以继续形成安全、可审计的普通 Agent Runtime，却没有一个正式对象表达“哪条成功路径已由证据和独立审批证明可复用”。`RunManifest.plan_revision` 目前只是可选 ID；Memory、Markdown 指令、模型输出或一次成功 Run 都不能约束后续运行。若先实现自由 Agent Loop，再补 Task DAG、Evidence Gate 与 Promotion，模型仍可能跳过节点、依赖或完成条件。

# Desired outcome

定义不可变、内容寻址的 `ProcedureRevision`、`TaskClassRevision`、`VerifiedProcedureRevision` 与 Kernel-retained registry/admission：前者描述可执行流程内容，批准包引用 exact 流程、适用 Task class、验证证据与满足最低角色分离的独立审批。REQ-0034 自身只交付身份、注册表和纯 admission，不创建 Run、Plan、Node、Evidence、Capability 或 Effect；REQ-0018 使用其 opaque admission 结果建立第一版 procedure-capable `RunManifest`。

# Acceptance criteria

- AC-01：`ProcedureRevision` 具有稳定逻辑 ID、不可变 Revision ID、父谱系、Schema 版本、规范化内容摘要、创建者与来源；同内容跨进程得到同一内容身份，任何行为字段变化产生新 Revision。
- AC-02：流程内容闭合描述节点、依赖、合法转移、输入/输出 Schema、Capability、预算类别、Evidence 要求、checkpoint、恢复、重试、终止与可选补偿引用；自由文本说明不能替代闭合字段。
- AC-03：`TaskClassRevision` 只使用闭合 Task SchemaRef、canonical field predicates 与 limits 表达适用范围；第一版不执行模型分类器、任意代码、网络或 caller 自报标签，Task 内容变化必须重新匹配。
- AC-04：`VerifiedProcedureRevision` 是不可变审批包，固定 exact Procedure revision/digest、TaskClass revision/digest、evaluation evidence-set revision/digest、`PrincipalRoleAssignmentRevision`、review decision revision/digest、approval policy/authority revision、limitations 与 compatibility；任一 subject 字段变化使旧 review/approval 不可复用。
- AC-05：最低独立性不可被 policy 放宽：identity 以认证 `PrincipalRootId` 比较而非可变 Agent/Actor alias；candidate creator/proposer/runner 与 mandatory evidence producer 不得担任 verifier、reviewer 或 approver；mandatory evidence producer、verifier、reviewer、approver 四类 principal root 两两不同，且至少各一名形成 quorum。
- AC-06：review decision 必须绑定 AC-04 的完整 subject、reviewer principal root、verdict、observed_at、freshness horizon 与 limitations；只有 `approved` 且无 open Blocker/Major 可被 admission 接受。自然语言、自报 `independent`、成功 Run 或测试通过本身都不是 approval authority。
- AC-07：`ProcedureRegistryRevision` 以 canonical order 固定 exact verified packages、Task classes、approval authorities/policies、evidence/verifier roots、revocation 与 invalidation facts和 config digest。普通 runtime principal 不能新增 authority、注册包、撤销事实或选择 ambient/current registry。
- AC-08：Kernel 纯 admission 必须从显式 retained registry revision/digest、认证 scope、exact TaskRevision 与 requested verified revision 重算全部 identity、Task match、角色分离、quorum、freshness、invalidation/revocation 和 compatibility；成功只返回绑定这些输入的 crate-private opaque `AdmittedVerifiedProcedure`，不写 Event 或签发运行能力。
- AC-09：未知、撤销、内容替换、digest/config 不符、未批准、角色重叠/alias、旧 review 复用、evidence 失效、Task 不匹配、跨 scope 或 unknown Schema major 默认拒绝；错误有界且不得泄漏另一 scope 的 package、principal、evidence 或 revocation 是否存在。
- AC-10：所有成功与拒绝路径都必须是零 Run/Task/Node Event、零 Capability、零 reservation、零 Effect 和零 registry mutation；只有后续 REQ-0018 可消费 opaque admission 并在一个新 Manifest major 中建立 execution authority。
- AC-11：协议与 registry 实现只采用 forward Schema major 并保留全部旧 Revision、Manifest、SchemaSet、reader/reducer 与 SQLite v2 字节；已发布 Procedure、TaskClass、Verified package 与 registry revision 不得原位修改。
- AC-12：确定性测试逐项覆盖 canonical identity、Task predicate、完整 approval subject、每一种角色重叠/alias、quorum、freshness、invalidation/revocation、registry substitution、跨域、unknown major、opaque token mutation 与 zero-write/zero-effect；每个命名过滤器先证明非零命中。

# Quality, cost, and latency guardrails

- 质量：本 Requirement 只保证已验证流程身份和独立审批准入的可重算性，不声称节点执行或现实结果正确；任何缺失、伪造、过期、角色重叠或跨域输入都必须 fail closed。
- Token/费用：本 Requirement 不调用真实模型或付费服务；记录流程版本对 Manifest 与 Event 大小的影响，不声明 Token 或费用优化。
- 延迟：记录 registry resolution 与纯 admission 的本机分布；设计优先确定性和 fail-closed，不设置无基线支持的性能收益阈值。

# Non-goals

- 不把 Memory、Prompt、Skill、README、聊天轨迹或自然语言 Plan 升格为权威流程。
- 不保证外部世界结果必然正确、模型必然找到解法或所有任务都可复用同一流程。
- 不在本 Requirement 实现 Run Manifest procedure pin、Plan/DAG、节点状态机、执行期 Evidence Gate、节点执行器、replay、恢复、多 Agent、自动流程晋升、Canary、Behavior Promotion 或真实 Provider。
- 不把 compensation 描述为撤销已发生的外部效果，也不把 reexecute 描述为确定性 replay。

# Risks and open questions

该能力触及 Revision、权限、隔离、Evidence identity 与后续 Promotion，风险为 high。实现前必须由独立架构评审确认：审批 subject 完整、principal-root 角色分离不可放宽、registry bootstrap 无 runtime writer、opaque admission 无法伪造、旧 SchemaSet 保留。REQ-0018、REQ-0035、REQ-0016、REQ-0014 与 REQ-0036 分别拥有 Manifest/Plan、Node、Evidence、execution 与 promotion 验收，不作为 REQ-0034 的完成条件。
