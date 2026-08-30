---
id: ARCH-0003
title: 版本、事件与证据模型
status: accepted
owners: [maintainers]
created: 2026-08-20
updated: 2026-08-30
links: [RFC-0001, RFC-0002, RFC-0003, RFC-0004, RFC-0005, RFC-0009, ADR-0001, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ADR-0010, REQ-0003, REQ-0004, REQ-0005, REQ-0006, REQ-0009, SPEC-0002, SPEC-0003, SPEC-0004, SPEC-0005, SPEC-0008, REVIEW-0004, REVIEW-0005, REVIEW-0012]
---

# 版本、事件与证据模型

## 身份规则

每个对象同时拥有稳定逻辑 ID 与不可变 Revision ID。Revision 包含父版本、Schema 版本、规范化内容摘要、创建者和来源。逻辑 ID 用于追踪概念，Revision ID 用于精确执行。

```text
TaskRevision
  ├── acceptance criteria
  └── PlanRevision ── Task DAG

BehaviorRevision
  ├── strategy revisions
  ├── prompt/skill revisions
  └── routing/retry configuration

ContextGraphRevision ── ContextProjectionRevision
WorkspaceRevision      EnvironmentRevision
ModelSnapshot          ToolSetRevision
              \        /
               RunManifest
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

`EvidenceRecord`

- `schema_ref`, `scope`, `requirement_id`, `claim`, `evidence_type`
- `producer_revision`, `verifier_revision`, `subject_revision`
- `artifact_digest`, `verdict`, `evidence_scope`, `freshness`, `limitations`

`EvolutionProposal`

- `proposal_id`, `base_behavior_revision`, `candidate_revision`
- `hypothesis`, `target_metrics`, `quality_floor`
- `evaluation_suite_revision`, `budget`, `risk`, `rollback_condition`

字段名是设计契约；序列化、SchemaSet、规范化/digest、可信验证上下文、兼容与 Replay lineage 已由 RFC-0002/ADR-0003 冻结。公开数据携带完整 SchemaRef 和 IsolationScope，不直接暴露 Rust 内部布局。REQ-0004 已实现 Kernel 私有 SQLite append-only Event Store；REQ-0005 已实现 derived lifecycle stream 的 sequence-1 完整 Manifest、Run/Task 闭合状态机、owner-only authority、exact reader、pure fold 与同一 `BEGIN IMMEDIATE` 内的幂等/版本/guard/单事件追加。REQ-0006 已实现由 persisted source contract 显式解析 retained reducer/output reader 的 `RunTaskProjection`，以及绑定 store/full scope/stream/cursor/source/output/reducer/history/digest 的 immutable Snapshot；assisted load 仍重读并验证完整 prefix，只跳过 prefix reducer fold。

## 事件族

- 生命周期：已实现 `run-created`、`task-created`、`run-state-transitioned`、`task-state-transitioned` 1.0；Run/Task 状态集合与合法边由 RFC-0004/ADR-0005 冻结。Node lifecycle 留待后续 Requirement。
- 决策：Plan proposed, context projected, model routed, retry selected。
- 效果：Capability requested/granted/denied；REQ-0009已接受Effect intended/dispatch-claimed/receipt-admitted/attempt-concluded/reconciliation-required/reconciled的设计，但尚未实现Schema或Runtime。Effect/Executor/Boundary Inventory v2 identity、partial/unknown与fixed replay horizon由RFC-0009/ADR-0010冻结。
- 证据：Evidence requested/recorded/verified/invalidated。
- 资源：Budget reserved/consumed/exhausted/released。
- 演化：Proposal created/evaluated，Candidate canaried，Behavior promoted/rolled back。

事件包含事实而不是可变视图。当前状态、DAG、成本和证据覆盖率由投影器构建。

## Replay 模式

- `live`：首次实时执行，不声明 source Run。
- `recorded_replay`：引用 source Run 和已终结的 `BoundaryInventoryRevision`，复用已记录边界结果。
- `reexecute`：重新调用外部系统，比较新旧结果并明确标为新 Run。
- `simulated`：固定非空 Fixture revisions，并显式区分 standalone/derived lineage。

任何模式都不得覆盖原 Run；派生模式以 `source_run_id` 和 exact inventory revision 建立谱系。迟到结果写入独立 audit/reconciliation revision，不修改已终结 inventory。

当前 Runtime 只交付 Run/Task `recorded_replay`：忽略 Snapshot、从完整已验证 Event 历史只读重建 Projection，不接受 Effect/Provider/Tool executor，也不追加 Event。`simulated` 在 fixture resolver 交付前稳定拒绝；`reexecute` 和外部边界执行仍属于后续 Requirement。
