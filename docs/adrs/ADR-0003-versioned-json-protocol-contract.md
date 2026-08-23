---
id: ADR-0003
title: 采用闭合版本化 JSON 协议与可信上下文验证
status: accepted
owners: [maintainers]
created: 2026-08-22
updated: 2026-08-22
links: [REQ-0003, SPEC-0002, RFC-0002, ADR-0001, ADR-0002]
---

# Context

Event Store、Run Manifest、Replay、Evidence 和未来跨语言消费者需要一个在持久化前即可验证、跨平台可复算且不能被 payload 绕过权限或隔离边界的公共合同。Serde 默认布局、开放未知字段和隐式当前 Schema 都会破坏版本身份与重放诚实。

# Decision

接受 RFC-0002：公共协议使用 JSON Schema Draft 2020-12 的闭合合同；顶层记录显式携带完整 SchemaRef 和 IsolationScope；规范化使用 RFC 8785 JCS，digest 使用 SHA-256、UTF-8 length-prefix、类型域和完整 SchemaRef 域分离。

Kernel 通过 bootstrap trust root admission SchemaSet，并从认证 principal、已 admission SchemaSet、RunManifest 和命令目标派生 TrustedValidationContext。协议验证 exact scope、actor、stream、SchemaSet membership 和 EventTypeBinding；通过结构验证不等于 capability 授权。

RunManifest 固定 task/behavior/workspace/environment/context/model/tool/kernel/SchemaSet、budget、protocol limits 和 boundary recording policy。live 边界事实通过 Event Log 追加，终止后形成不可变 BoundaryInventoryRevision；recorded replay/reexecute 固定 source inventory，不能回填 Manifest。

Schema 兼容只允许保守 checker 能证明的白名单变化；无法证明则 fail closed 并要求 major bump 或新的受审规则。旧 Schema、reader 和 fixtures 在存在引用时保留。

# Alternatives

- Serde 默认 JSON 和派生 Schema：偶然暴露内部布局，缺少稳定线合同。
- 开放对象并忽略未知字段：可能形成权限降级和 unknown-field smuggling。
- Protobuf/gRPC 首发：增加工具链并偏离已接受的 JSON 基线。
- 私有 JSON canonicalization：跨语言维护成本和歧义高于采用 JCS。
- 只用内容 digest、不表达 major/minor：无法描述 reader/writer 支持窗口和迁移政策。

# Consequences

SchemaSet 的文件发布采用不可变内容地址目录 `schemas/sets/sha256-<manifest-digest>/`，不设置跨平台可变
`current` 指针。并发发布同一 digest 只在字节完全相同时幂等成功，不同 digest 保留并由 RunManifest 精确选择。

获得明确的跨语言身份、兼容、隔离、Replay 与失败语义，并能用 golden、mutation 和负向 fixture 验证。代价是协议实现必须维护 Schema generator、保守 compatibility checker、多版本 writer/reader、bootstrap trust root、严格 limits 和更多测试资产。

协议层保持无文件、网络、进程、时钟或秘密访问；Event Store 原子顺序、ID 唯一性、状态迁移和实际 capability 判定仍属于后续 Kernel Requirement，不能由本 ADR 的结构验证替代。

# Revisit triggers

- JSON Schema/JCS 无法表达必需的兼容或吞吐需求。
- 跨语言实现无法在规定 limits 内稳定复现 golden vectors。
- 真实持久化数据证明 closed-world、多版本 reader/writer 成本不可接受。
- 需要改变 canonicalization、digest preimage、Schema identity、unknown-field、bootstrap root 或隔离语义。

触发后必须创建新 Requirement/RFC/ADR，提供数据迁移、Replay/权限负例、回滚和独立架构评审；不得原位重释已发布 Schema。
