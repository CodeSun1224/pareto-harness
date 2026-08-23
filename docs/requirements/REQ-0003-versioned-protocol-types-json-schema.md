---
id: REQ-0003
title: 建立版本化协议类型和 JSON Schema
status: done
owners: [maintainers]
created: 2026-08-22
updated: 2026-08-22
links: [EPIC-0002, REQ-0002, SPEC-0002, RFC-0001, RFC-0002, ADR-0001, ADR-0002, ADR-0003, REVIEW-0002]
risk: high
work: .agents/work/archived/REQ-0003-versioned-protocol
---

# Context and user

可信内核、Runtime、适配器和未来跨语言 SDK 的实现者需要共享一组可审查、可生成、可持久化且可演进的协议合同。REQ-0004 及其后的 Event Store、Run Manifest、Replay、Capability、Evidence 和演化能力都依赖该合同。

# Problem

当前架构文档只列出最小公共类型和字段方向，尚未冻结机器可检查的 JSON 表示、Schema 身份、版本兼容规则和规范化摘要规则。若各消费者自行定义，持久化事件可能无法可靠读取或重放，跨语言实现也无法判断兼容性。

# Desired outcome

提供一个不依赖 Runtime 实现的版本化协议包及发布的 JSON Schema 集，使生产者、消费者、持久化层和测试能够用同一合同表达 Revision 身份、Event Envelope、Run Manifest 和 Evidence，并在不兼容数据进入权威状态前明确拒绝。

# Acceptance criteria

- AC-01：协议包定义带显式 Schema 身份的基础标量/标识类型、Revision 元数据、`EventEnvelope`、`RunManifest` 和 `EvidenceRecord`，公共 JSON 不泄漏语言内部布局。
- AC-02：每个公开类型都有确定性生成、纳入版本控制的 JSON Schema；Schema 集具有可固定的身份和清单，重复生成不产生差异。
- AC-03：Schema 和实现验证必填字段、格式、枚举、数值边界及未知字段策略；畸形、越界或版本不支持的输入在进入权威状态前返回结构化错误。
- AC-04：同一规范化内容产生相同摘要，不同类型或 Schema 身份不能因内容相同而共享 Revision 身份；测试固定规范化与摘要 golden vectors。
- AC-05：兼容性规则区分可接受的向后兼容变更与必须升级主 Schema 身份的不兼容变更，并由旧/新 Schema fixture 的自动测试证明。
- AC-06：`EventEnvelope` 明确事件、流、Run、因果、关联、序号、事件类型、发生时间、actor、payload Schema 和 payload 摘要边界；验证拒绝跨 Run/Stream 身份混用、非法序号和 payload 合同不匹配。
- AC-07：`RunManifest` 固定 Task、Behavior、Workspace、Environment、Context、Model、Tool、Kernel 和 Schema 集身份；可选 Plan 及非确定性边界必须显式表达，禁止靠进程默认值补齐已持久化 Manifest。
- AC-08：`EvidenceRecord` 固定 Requirement/claim、producer/verifier/subject Revision、artifact digest、verdict、scope、freshness 和 limitations；不得把自然语言自信当作可验证 verdict。
- AC-09：权限与隔离负例证明反序列化/验证不访问文件、网络、进程或秘密，且 Run、Workspace、Agent/actor、用户/租户作用域字段不会被默认值或 payload 覆盖。
- AC-10：在 Windows、Linux 和 macOS 目标上执行格式化、静态检查、单元、Schema golden、兼容性、序列化往返和负向 fixture 测试；后续 REQ-0004/0005 可只依赖公开协议，不反向引入存储或 Runtime 依赖。

# Quality, cost, and latency guardrails

- 质量：所有公开 Schema 与 golden fixtures 都纳入版本控制；任何不兼容变更必须由测试阻止静默发布。
- Token/费用：本需求不调用真实模型或外部 Provider；该维度不宣称优化结果。
- 延迟：协议验证和 Schema 生成必须可在普通开发机的每次 PR 门禁中运行；具体性能阈值在实现基线形成后测量，本需求不虚构目标收益。

# Non-goals

- 不实现 Event Store、状态机、Projection、Snapshot/Replay 执行器或 CLI。
- 不定义 Provider、Tool、Agent Message 或 Evolution Proposal 的完整业务协议；它们由后续 Requirement 扩展基础合同。
- 不承诺任意未来 Schema 自动迁移，也不引入数据库迁移。
- 不创建没有行为和测试的 Runtime 空模块。

# Risks and open questions

这是公共 Schema、持久化和 Replay 的高风险基础合同。规范化 JSON、摘要域分离、版本号语义、未知字段策略、时间/数字表示及 Schema 兼容算法会影响所有后续消费者；SPEC-0002 批准前必须通过专项 RFC 冻结这些决策并接受架构评审。
