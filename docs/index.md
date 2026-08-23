# 文档导航

本目录是 Pareto Harness 的长期事实来源。当前基线日期为 2026-08-20。

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

## 交付

- [评测协议](benchmarks/protocol.md)
- [Evidence-gated 路线图](roadmap/roadmap.md)
- [Requirement Backlog](roadmap/requirement-backlog.md)
- [Epic 目录](epics/EPIC-0001-engineering-foundation.md)

## 记录类型

`epics/` 组织路线结果；`requirements/` 定义需求；`specs/` 固化行为、影响和测试合同；`rfcs/` 保存重大技术提案；`adrs/` 保存已接受决策；`reviews/` 保存独立评审；`fixes/` 保存缺陷根因和回归证据；`postmortems/` 保存系统性事故复盘。模板位于 `.agents/templates/`。
