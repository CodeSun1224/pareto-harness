# 文档导航

本目录是 Pareto Harness 的长期事实来源。当前基线日期为 2026-08-27。

## 产品

- [项目章程与 PRD](product/project-charter.md)：目标、用户、范围、成功指标。
- [核心能力](product/capabilities.md)：能力地图、优先级和验收方向。
- [REQ-0001：建立 Pareto Harness 设计基线](requirements/REQ-0001-design-baseline.md)
- [REQ-0002：建立 SDD 与独立评审门禁](requirements/REQ-0002-sdd-review-gates.md)
- [SPEC-0001：SDD 与独立评审规范](specs/SPEC-0001-sdd-review-gates.md)
- [REQ-0003：版本化协议类型和 JSON Schema](requirements/REQ-0003-versioned-protocol-types-json-schema.md)
- [SPEC-0002：版本化协议类型和 JSON Schema 规范](specs/SPEC-0002-versioned-protocol-types-json-schema.md)
- [REQ-0004：SQLite append-only Event Store](requirements/REQ-0004-sqlite-append-only-event-store.md)
- [SPEC-0003：SQLite append-only Event Store 规范](specs/SPEC-0003-sqlite-append-only-event-store.md)
- [REQ-0005：Run/Task 状态机与 Run Manifest](requirements/REQ-0005-run-task-state-machine-run-manifest.md)
- [SPEC-0004：Run/Task 状态机与 Run Manifest 规范](specs/SPEC-0004-run-task-state-machine-run-manifest.md)
- [REQ-0006：Projection、Snapshot 与 Replay](requirements/REQ-0006-projection-snapshot-replay.md)
- [SPEC-0005：Projection、Snapshot 与 Replay 规范](specs/SPEC-0005-projection-snapshot-replay.md)
- [REQ-0007：Capability、预算、取消与超时](requirements/REQ-0007-capability-budget-cancellation-timeout.md)
- [SPEC-0006：Capability、预算、取消与超时规范](specs/SPEC-0006-capability-budget-cancellation-timeout.md)
- [REQ-0008：Observer、Gate 与 Transform Hook 骨架](requirements/REQ-0008-observer-gate-transform-hooks.md)
- [SPEC-0007：Observer、Gate 与 Transform Hook 骨架规范](specs/SPEC-0007-observer-gate-transform-hooks.md)

## 研究

- [竞品与论文洞察](research/landscape.md)：综合判断与可挖掘空间。
- [证据账本](research/evidence-register.md)：来源、等级、成熟度和受支持声明。

## 架构

- [总体架构](architecture/overview.md)
- [可信内核宪法](architecture/kernel-constitution.md)
- [版本与事件模型](architecture/version-and-event-model.md)
- [技术选型](architecture/technology-selection.md)
- [RFC-0001：稳定机制内核与版本化策略](rfcs/RFC-0001-stable-kernel-versioned-strategies.md)
- [ADR-0001：采用稳定内核和版本化策略边界](adrs/ADR-0001-stable-kernel-boundary.md)
- [ADR-0002：Rust 模块化单体与 SQLite 起步](adrs/ADR-0002-rust-modular-monolith.md)
- [RFC-0002：版本化协议、规范化 JSON 与 Schema 兼容合同](rfcs/RFC-0002-versioned-protocol-json-schema.md)
- [ADR-0003：闭合版本化 JSON 协议与可信上下文验证](adrs/ADR-0003-versioned-json-protocol-contract.md)
- [RFC-0003：SQLite Event Store 事务、幂等与读取合同](rfcs/RFC-0003-sqlite-event-store-contract.md)
- [ADR-0004：Kernel 私有 SQLite append-only Event Store 合同](adrs/ADR-0004-sqlite-event-store-transaction-contract.md)
- [RFC-0004：事件溯源 Run/Task 生命周期](rfcs/RFC-0004-event-sourced-run-task-lifecycle.md)
- [ADR-0005：采用事件溯源 Run/Task 生命周期](adrs/ADR-0005-event-sourced-run-task-lifecycle.md)
- [RFC-0005：Projection、Snapshot 与本地确定性 Replay 合同](rfcs/RFC-0005-projection-snapshot-replay-contract.md)
- [ADR-0006：采用版本化 Projection、同库 Snapshot 与只读 Recorded Replay](adrs/ADR-0006-versioned-projection-snapshot-recorded-replay.md)
- [RFC-0006：Capability、预算、取消与超时可信内核合同](rfcs/RFC-0006-runtime-control-capability-budget-cancellation.md)
- [ADR-0007：采用事件溯源 Runtime Control 可信内核合同](adrs/ADR-0007-event-sourced-runtime-control.md)
- [RFC-0007：Rust 权威控制面与多语言扩展边界](rfcs/RFC-0007-rust-authority-polyglot-extensions.md)
- [ADR-0008：采用 Rust 权威控制面与多语言扩展边界](adrs/ADR-0008-rust-authority-polyglot-extension-boundary.md)
- [RFC-0008：Kernel 治理的 Observer、Gate 与 Transform Hook 合同](rfcs/RFC-0008-kernel-governed-hook-contract.md)
- [ADR-0009：采用 Kernel 治理的 Observer、Gate 与 Transform Hook 合同](adrs/ADR-0009-kernel-governed-observer-gate-transform-hooks.md)

## 交付

- [评测协议](benchmarks/protocol.md)
- [Evidence-gated 路线图](roadmap/roadmap.md)
- [Requirement Backlog](roadmap/requirement-backlog.md)
- [工程基础 Epic](epics/EPIC-0001-engineering-foundation.md)
- [可信内核 Epic](epics/EPIC-0002-trusted-kernel.md)
- [REQ-0005 独立评审](reviews/REVIEW-0004-run-task-state-machine-run-manifest.md)
- [REQ-0006 独立代码评审](reviews/REVIEW-0005-projection-snapshot-replay.md)
- [REQ-0007 独立设计评审](reviews/REVIEW-0006-capability-budget-cancellation-timeout-design.md)
- [REQ-0007 独立实现评审](reviews/REVIEW-0007-capability-budget-cancellation-timeout-implementation.md)
- [Rust 权威控制面与多语言扩展边界独立设计评审](reviews/REVIEW-0009-rust-authority-polyglot-extensions-design.md)
- [REQ-0008 Observer/Gate/Transform Hook 独立设计评审](reviews/REVIEW-0010-observer-gate-transform-hooks-design.md)
- [REQ-0007 Runtime Control 修复记录](fixes/FIX-0001-runtime-control-authority-and-replay-contract.md)

## 记录类型

`epics/` 组织路线结果；`requirements/` 定义需求；`specs/` 固化行为、影响和测试合同；`rfcs/` 保存重大技术提案；`adrs/` 保存已接受决策；`reviews/` 保存独立评审；`fixes/` 保存缺陷根因和回归证据；`postmortems/` 保存系统性事故复盘。模板位于 `.agents/templates/`。
