---
title: REQ-0005 Run/Task 状态机与 Run Manifest 交付计划
status: active
owner: maintainers
updated: 2026-08-24
links: [REQ-0005, SPEC-0004, RFC-0004, ADR-0005]
---

# Goal and acceptance

交付 REQ-0005 的最小可运行纵向切片：完整 Manifest → 原子 `RunCreated` → Task 创建 → owner-only 合法迁移 → 同事务事件追加 → 关闭/重开后从 exact Event Store range 恢复 → 非法、异义重复、并发落后和终态迟到迁移拒绝。验收以 REQ-0005 AC-01 至 AC-13 和 SPEC-0004 traceability 为准。

# Current state

REQ-0003 与 REQ-0004 均为 done，且没有其他 Runtime Requirement implementing。RFC-0004/ADR-0005 已接受，SPEC-0004 approved。REQ-0005 当前为 `reviewing`：首轮 exact implementation `d0011d0` 的 4 个 Major 经 `38f0beb710fdd05ffd7b7047db6e3cb7cb7a2f79` remediation 后，独立 focused re-review 已关闭 F-001、F-002、F-004，保留 F-003 这 1 个 Major。实现者正在补全 standalone fold 与 loader 相同的 exact SchemaSet RunManifest semantic admission 及负测。独立复审关闭 F-003 前不得进入 verified/done。

# Plan

1. 在 `pareto-protocol` 增加 Task ID、Run/Task state 和四类 lifecycle payload/EventTypeBinding，发布新的不可变 SchemaSet并保留全部旧 set。
2. 在 `pareto-kernel` 重构 Event Store transaction-local shared append/check，增加 persisted-row-driven sequence-1 Manifest bootstrap read；保持 authority-bearing API crate-private且不暴露 sqlx。
3. 实现 pure lifecycle fold、create-only/established authority、Run/Task create与两套状态机 guard，并把幂等、expected version/state、fold和单事件append放在同一 `BEGIN IMMEDIATE`。
4. 完成 Focused unit/model/negative 与真实SQLite manifest/atomicity/idempotency/concurrency/terminal/isolation/reopen/compatibility测试。
5. 执行 Impacted protocol/Event Store回归和Core全仓门禁，记录确切环境、命令、结果、Schema byte identity及质量/费用/延迟观察到 `VALIDATION.md`。
6. 由独立 Agent/新会话执行 `code-review`，提交 Requirement/Spec/RFC/ADR、exact diff与测试证据；实现者只修复，不自关闭 Blocker/Major，完成focused re-review。
7. 通过全部completion gates后同步README/index/EPIC/ARCH implemented facts，将Requirement依次更新为reviewing/verified/done并归档work directory；在此之前不进入REQ-0006实现。

# Validation

- Focused protocol: `cargo test -p pareto-protocol lifecycle_manifest_contract --offline`
- Focused state/unit/model: `cargo test -p pareto-kernel lifecycle::state_machine --offline`; `cargo test -p pareto-kernel lifecycle::model_sequences --offline`; `cargo test -p pareto-kernel lifecycle::hierarchy --offline`
- Focused SQLite/manifest: `cargo test -p pareto-kernel lifecycle::manifest --offline`; `cargo test -p pareto-kernel lifecycle::creation_atomicity --offline`; `cargo test -p pareto-kernel lifecycle::transaction --offline`
- Concurrency/idempotency/late: `cargo test -p pareto-kernel lifecycle::idempotency --offline`; `cargo test -p pareto-kernel lifecycle::concurrency --offline`; `cargo test -p pareto-kernel lifecycle::terminal_and_late --offline`
- Isolation/recovery/compatibility: `cargo test -p pareto-kernel lifecycle::isolation --offline`; `cargo test -p pareto-kernel lifecycle::recovery --offline`; `cargo test -p pareto-kernel lifecycle::compatibility --offline`; `cargo test -p pareto-kernel lifecycle::fold_contract --offline`
- API surface: `cargo test -p pareto-kernel --doc --offline`
- Impacted: `cargo test -p pareto-protocol --all-targets --all-features --offline`; `cargo test -p pareto-kernel event_store --offline`; `cargo test -p pareto-kernel --all-targets --all-features --offline`
- Core/governance/static: `python -m unittest discover -s scripts/tests -p "test_*.py"`; `python scripts/check_docs.py`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`; `cargo test --workspace --all-targets --all-features --offline`
- Schema identity: `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas`; verify retained old sets byte-identical and no unintended schema diff.
- Hygiene: `git diff --check`; `git status --short`，逐项确认无无关修改。

# Handoff notes

实现若需要数据库新表/迁移、公开transaction/raw SQL、multi-event cascade、非owner actor、进程default补Manifest、替代SchemaSet reader或不同状态边，立即停止并退回SPEC/RFC。REQ-0006、0007、0018只作为合同消费者，不在本Requirement提前实现。
