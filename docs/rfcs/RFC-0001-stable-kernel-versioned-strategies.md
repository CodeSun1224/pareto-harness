---
id: RFC-0001
title: 稳定机制内核与版本化策略
status: accepted
owners: [maintainers]
created: 2026-08-20
updated: 2026-08-20
links: [REQ-0001, ADR-0001, ARCH-0001, ARCH-0002]
---

# Summary

采用窄而稳定的可信内核管理事件、版本、状态、权限、资源、重放、证据和晋升；将规划、上下文、路由、工具选择、重试、评测和记忆实现为不可变且可评测的策略版本。

# Motivation and requirements

全部组件插件化提高组合性，却会让安全和一致性语义也变成可替换实现。完全固定的 Agent Loop 又阻碍高频实验。设计必须同时满足 REQ-0001 的可实施性，以及后续行为演化的隔离、归因和回滚。

# Proposed design

内核只暴露带 capability、预算和事件语义的接口。策略注册时声明兼容协议、输入输出、权限和降级行为；运行时通过 `BehaviorRevision` 固定策略集合。策略输出是提议或决策，不直接写权威状态。

所有执行由 `RunManifest` 固定 Task、Behavior、Workspace、Environment、Context、Model 和 Tool 版本。Task、Context、Evidence 和 Cost 视图均从 Event Log 投影。

# Interfaces, data flow, and invariants

核心接口为 `EventStore`、`SnapshotStore`、`Strategy`、`PolicyRegistry`、`ModelProvider`、`EvidenceVerifier`、`Sandbox` 和 `EvolutionController`。具体字段见 ARCH-0003；不变量见 ARCH-0002。

# Failure modes and security

- 策略失效：使用显式降级或终止，不绕过检查。
- 版本不兼容：运行创建前失败，不做隐式转换。
- 外部效果部分成功：以 intent/receipt 和幂等键对账。
- 预算耗尽：内核拒绝新预留并传播取消。
- 候选越权：Sandbox 拒绝并产生安全证据，禁止晋升。
- 基线已变化：MVCC 冲突，要求显式 rebase/merge。

# Alternatives considered

1. **万物插件化**：组合性最高，但安全协议、初始化顺序和调试路径容易隐式化；拒绝。
2. **固定完整 Agent Loop**：初期简单，但上下文、路由和评测实验需修改核心；拒绝。
3. **外置优化器，不版本化 Runtime**：可运行实验，但难以归因和生产回滚；拒绝。

# Compatibility, migration, and rollback

公共 Schema 使用显式版本。已发布 Revision 不原位修改。策略晋升更新默认指针，旧 Run 始终引用原版本；回滚只切换默认指针并记录事件，不删除历史。

# Evaluation and acceptance

实现阶段必须通过状态机属性测试、事件幂等与并发测试、录制重放一致测试、权限拒绝测试、预算/取消测试、Evidence 准入测试和 Promotion/Rollback 测试。
