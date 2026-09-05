---
id: REQ-0034
title: 不可变已验证流程版本
status: specified
owners: [runtime-kernel]
created: 2026-09-05
updated: 2026-09-05
links: [EPIC-0007, REQ-0003, REQ-0005, REQ-0006, REQ-0007, REQ-0009, SPEC-0010, RFC-0013]
risk: high
work: .agents/work/active/REQ-0034-verified-procedure-revision
---

# Context and user

需要重复执行高价值 Coding Agent 任务的平台工程团队，希望把一次成功路径从非权威会话轨迹提升为可审计、可复用且不可绕过的流程合同。REQ-0003 至 REQ-0009 已交付版本、事件、生命周期、Runtime Control 与 Effect 基础，但尚未实现流程版本、Plan/Task DAG、节点状态机或执行期 Evidence Gate。

# Problem

当前路线可以继续形成安全、可审计的普通 Agent Runtime，却没有一个正式对象表达“哪条成功路径已由证据和独立审批证明可复用”。`RunManifest.plan_revision` 目前只是可选 ID；Memory、Markdown 指令、模型输出或一次成功 Run 都不能约束后续运行。若先实现自由 Agent Loop，再补 Task DAG、Evidence Gate 与 Promotion，模型仍可能跳过节点、依赖或完成条件。

# Desired outcome

定义不可变、内容寻址的 `ProcedureRevision` 与 `VerifiedProcedureRevision`：前者描述可执行流程内容，后者引用 exact 流程、验证证据与独立审批结论。后续受治理运行由 `RunManifest` 固定 exact 已验证流程版本；模型、Planner、Memory、Provider、Tool 与外部 Worker 只能提出动作或返回 observation，不能修改流程、绕过节点准入或自行宣告完成。

# Acceptance criteria

- AC-01：`ProcedureRevision` 具有稳定逻辑 ID、不可变 Revision ID、父谱系、Schema 版本、规范化内容摘要、创建者与来源；同内容跨进程得到同一内容身份，任何行为字段变化产生新 Revision。
- AC-02：流程内容闭合描述节点、依赖、合法转移、输入/输出 Schema、Capability、预算类别、Evidence 要求、checkpoint、恢复、重试、终止与可选补偿引用；自由文本说明不能替代闭合字段。
- AC-03：`VerifiedProcedureRevision` 是不可变审批包，固定 exact `ProcedureRevision`、适用 Task 类别、验证 evidence set、独立 Review decision、兼容范围、限制和批准者；成功 Run、模型声明或测试通过本身都不能构造该身份。
- AC-04：只有 Kernel 保留注册表中的 exact `VerifiedProcedureRevision` 可供 verified execution 使用；未知、撤销、内容替换、digest 不符、审批缺失或不兼容版本默认拒绝，且拒绝发生在 Run 创建或任何外部效果之前。
- AC-05：新的 procedure-capable `RunManifest` major 必须固定 exact `VerifiedProcedureRevision`、`PlanRevision` 和既有 task/behavior/workspace/environment/context/model/tool/schema/budget/boundary 身份；旧 Manifest 与旧 Run 语义和字节保持不变。
- AC-06：`PlanRevision` 是 Task-specific 实例化：绑定 exact Task 与已验证流程，展开具体 Task DAG、参数、预算和节点输入；Plan 不能修改流程合同，任何偏离必须产生新 Plan，并按兼容与审批策略重新准入。
- AC-07：流程节点的请求、开始、Evidence admission、成功、失败、暂停、恢复和补偿状态由 Kernel 事件与状态机拥有；本 Requirement 只冻结接口与身份，具体节点状态机由 REQ-0035 交付。
- AC-08：Evidence 要求是流程合同的一部分，但 Evidence 只在 Manifest-pinned producer/verifier、subject、artifact digest、scope 与 freshness 通过 Kernel admission 后才可推动状态；最小执行门禁由前移的 REQ-0016 交付，完整 Evidence Graph 仍由 REQ-0026 交付。
- AC-09：对话记忆、用户偏好、项目经验、操作说明和 Memory retrieval 均显式为非权威上下文；它们可提出流程候选或帮助填充 Plan，但不能满足依赖、Evidence、Capability、终态或审批条件。
- AC-10：Provider、Tool、Workspace 与 Sandbox 只能消费 Kernel 针对当前 Run/Plan/Node 签发的用途受限能力；任何绕过节点状态机或直接取得 Event Store、Manifest、Budget、Evidence、terminal 或系统网络/秘密的路径默认拒绝。
- AC-11：tenant、user presence/value、Workspace、Run、Task、Plan、Procedure、Node、Agent、Evidence、Capability 与 Effect identity 必须 exact 隔离；跨域探测、复用或混合 lineage 不得泄漏存在性或改变目标状态。
- AC-12：术语和合同明确区分行为/流程版本回退、运行恢复、工作区恢复、外部效果对账与补偿；任何一种都不能删除或重解释已发生的 Event 或外部事实。
- AC-13：Recorded replay 只读取固定 horizon 的已记录决策、节点、Evidence 与 boundary facts，零 Provider/Tool/Workspace 外部执行；reexecute 创建固定 source lineage 的新 Run；simulated 创建固定 fixture revisions 的新 Run，三者不得互相冒充。
- AC-14：基线实现只建立版本、注册与 Manifest 准入的确定性纵向切片，不实现 Agent Loop、完整工作流语言、自适应重规划、Multi-Agent、自动流程晋升、Canary 或 Behavior 优化；流程晋升/默认选择/回退由 REQ-0036 交付。
- AC-15：兼容与迁移采用前向 major、保留 reader/SchemaSet、旧 Run 只读可解释和新 writer 可停用策略；不得原位修改已发布 Procedure、Plan、Manifest、Event 或审批包。
- AC-16：测试覆盖内容身份、替换/撤销/跨域拒绝、Manifest exact pin、Plan/Procedure 不一致、Evidence/Memory 非权威、外部能力前置拒绝、恢复术语边界与三种 replay 模式；每个命名测试过滤器必须证明非零命中。

# Quality, cost, and latency guardrails

- 质量：流程遵循保证是“Kernel 只允许合同规定且证据满足的状态转移”，不是对现实结果百分百正确的保证；任何缺失、伪造、过期或跨域证据都必须 fail closed。
- Token/费用：本 Requirement 不调用真实模型或付费服务；记录流程版本对 Manifest 与 Event 大小的影响，不声明 Token 或费用优化。
- 延迟：记录注册、Run admission 与固定 horizon replay 的本机分布；设计优先确定性和 fail-closed，不设置无基线支持的性能收益阈值。

# Non-goals

- 不把 Memory、Prompt、Skill、README、聊天轨迹或自然语言 Plan 升格为权威流程。
- 不保证外部世界结果必然正确、模型必然找到解法或所有任务都可复用同一流程。
- 不在本 Requirement 实现节点执行器、完整 Evidence Graph、多 Agent、Context DAG、自动挖掘成功轨迹、Canary、Behavior Promotion 或真实 Provider。
- 不把 compensation 描述为撤销已发生的外部效果，也不把 reexecute 描述为确定性 replay。

# Risks and open questions

该能力触及 Manifest、Revision、事件、权限、隔离、Evidence、Replay 与 Promotion，风险为 high。实现前必须由独立架构评审确认：流程与 Plan 没有双重 authority；审批包不可由运行者自签；旧 Manifest/SchemaSet 保留；模型和所有 adapter 无旁路；REQ-0035/REQ-0016/REQ-0036 的责任边界闭合。
