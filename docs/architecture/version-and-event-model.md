---
id: ARCH-0003
title: 版本、事件与证据模型
status: accepted
owners: [maintainers]
created: 2026-08-20
updated: 2026-09-06
links: [RFC-0001, RFC-0002, RFC-0003, RFC-0004, RFC-0005, RFC-0009, RFC-0013, ADR-0001, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ADR-0010, ADR-0012, REQ-0003, REQ-0004, REQ-0005, REQ-0006, REQ-0009, REQ-0034, SPEC-0002, SPEC-0003, SPEC-0004, SPEC-0005, SPEC-0008, SPEC-0010, REVIEW-0004, REVIEW-0005, REVIEW-0012, REVIEW-0018]
---

# 版本、事件与证据模型

## 身份规则

每个对象同时拥有稳定逻辑 ID 与不可变 Revision ID。Revision 包含父版本、Schema 版本、规范化内容摘要、创建者和来源。逻辑 ID 用于追踪概念，Revision ID 用于精确执行。

```text
TaskRevision
  ├── acceptance criteria
  └── PlanRevision ── task-specific Node DAG

ProcedureRevision ── reusable nodes/dependencies/gates/recovery
  └── VerifiedProcedureRevision ── evidence + independent approval

BehaviorRevision
  ├── strategy revisions
  ├── prompt/skill revisions
  └── routing/retry configuration

ContextGraphRevision ── ContextProjectionRevision
WorkspaceRevision      EnvironmentRevision
ModelSnapshot          ToolSetRevision
              \        /
               RunManifest ── exact VerifiedProcedure + Plan
```

## 最小公共类型

`EventEnvelope`

- `schema_ref`, `scope`, `event_id`, `stream_id`, `sequence`, `run_id`
- `causation_id`, `correlation_id`
- `event_type`, `event_major`, `event_minor`, `occurred_at`
- `actor`, `payload_schema_ref`, `payload`, `payload_digest`

`RunManifest`

- `schema_ref`, `scope`, `revisions`（闭合角色集合）、`plan_revision?`
- `schema_set_ref`, `budget_revision`, `protocol_limits_ref`
- `boundary_recording_policy_ref`, `execution_mode`

`ProcedureRevision` / `VerifiedProcedureRevision`

- Procedure：metadata/hash schema、nodes、dependencies、transitions、I/O schemas
- capability/budget/evidence/checkpoint/retry/recovery/terminal/compensation refs
- Verified wrapper：exact procedure、task classification、evidence set、independent review decision、approval policy、limitations

`PlanRevision` / `Node`

- exact Task/Verified Procedure binding、instantiated DAG、parameters、budgets
- node identity、dependencies、lease、state、evidence coverage、checkpoint 与 recovery lineage

`EvidenceRecord`

- `schema_ref`, `scope`, `requirement_id`, `claim`, `evidence_type`
- `producer_revision`, `verifier_revision`, `subject_revision`
- `artifact_digest`, `verdict`, `evidence_scope`, `freshness`, `limitations`

`EvolutionProposal`

- `proposal_id`, `base_behavior_revision`, `candidate_revision`
- `hypothesis`, `target_metrics`, `quality_floor`
- `evaluation_suite_revision`, `budget`, `risk`, `rollback_condition`

字段名是设计契约；序列化、SchemaSet、规范化/digest、可信验证上下文、兼容与 Replay lineage 由 RFC-0002/ADR-0003 冻结。公开数据携带完整 SchemaRef 和 IsolationScope，不直接暴露 Rust 内部布局。实现状态统一见[项目状态](../status.md)。

## 事件族

- 生命周期：Run/Task 状态集合与合法边由 RFC-0004/ADR-0005 冻结；Procedure/TaskClass/Verified package、Plan/DAG 与 Node lifecycle 使用独立版本和闭合迁移。
- 决策：Plan proposed, context projected, model routed, retry selected。
- 效果：Capability/Budget/取消/超时与 Effect intended/dispatch-claimed/receipt-admitted/attempt-concluded/reconciliation-required/reconciled 保持独立事实；partial/unknown 必须保留并进入对账。
- 证据：requested/recorded/verified/invalidated Event 绑定 exact subject、producer、verifier、scope 和 freshness；只有 Kernel-admitted evidence 可推动完成。
- 资源：Budget reserved/consumed/exhausted/released。
- 演化：Proposal created/evaluated，Candidate canaried，Behavior promoted/rolled back。

事件包含事实而不是可变视图。当前状态、DAG、成本和证据覆盖率由投影器构建。

## Replay 模式

- `live`：首次实时执行，不声明 source Run。
- `recorded_replay`：引用 source Run 和已终结的 `BoundaryInventoryRevision`，复用已记录边界结果。
- `reexecute`：重新调用外部系统，比较新旧结果并明确标为新 Run。
- `simulated`：固定非空 Fixture revisions，并显式区分 standalone/derived lineage。

任何模式都不得覆盖原 Run；派生模式以 `source_run_id` 和 exact inventory revision 建立谱系。迟到结果写入独立 audit/reconciliation revision，不修改已终结 inventory。

Procedure-capable replay 必须固定 source Procedure/Plan/Node/Evidence horizons。Run recovery 保持同一 Manifest；Workspace recovery 产生或重建明确 revision；Effect compensation 是新的受治理 Effect；Procedure/Behavior rollback 只改变后续 Run 选择。四者不得互相替代。
