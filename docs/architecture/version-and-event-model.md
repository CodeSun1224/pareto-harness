---
id: ARCH-0003
title: 版本、事件与证据模型
status: proposed
owners: [maintainers]
created: 2026-08-20
updated: 2026-08-20
links: [RFC-0001, ADR-0001]
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

- `event_id`, `stream_id`, `sequence`, `run_id`
- `causation_id`, `correlation_id`
- `event_type`, `schema_version`, `occurred_at`
- `actor`, `payload`, `payload_digest`

`RunManifest`

- `task_revision`, `behavior_revision`, `plan_revision?`
- `workspace_revision`, `environment_revision`
- `context_graph_revision`, `model_snapshot`, `tool_set_revision`
- `kernel_version`, `schema_set`, `budget`, `replay_mode`

`EvidenceRecord`

- `requirement_id`, `claim`, `evidence_type`
- `producer_revision`, `verifier_revision`, `subject_revision`
- `artifact_digest`, `verdict`, `scope`, `freshness`, `limitations`

`EvolutionProposal`

- `proposal_id`, `base_behavior_revision`, `candidate_revision`
- `hypothesis`, `target_metrics`, `quality_floor`
- `evaluation_suite_revision`, `budget`, `risk`, `rollback_condition`

字段名是设计契约，序列化细节在实现 RFC 中冻结。公开数据必须携带 Schema 版本，不直接暴露 Rust 内部布局。

## 事件族

- 生命周期：Run/Task/Node started, paused, resumed, completed, failed, cancelled。
- 决策：Plan proposed, context projected, model routed, retry selected。
- 效果：Capability requested/granted/denied，Effect intended/received/failed。
- 证据：Evidence requested/recorded/verified/invalidated。
- 资源：Budget reserved/consumed/exhausted/released。
- 演化：Proposal created/evaluated，Candidate canaried，Behavior promoted/rolled back。

事件包含事实而不是可变视图。当前状态、DAG、成本和证据覆盖率由投影器构建。

## Replay 模式

- `recorded`：复用已记录的模型和外部效果结果，验证状态投影和策略消费逻辑。
- `reexecute`：重新调用外部系统，比较新旧结果并明确标为新 Run。
- `simulated`：使用 Fake/Fixture，验证状态机和失败路径。

任何模式都不得覆盖原 Run；派生 Run 通过 `derived_from_run_id` 建立谱系。
