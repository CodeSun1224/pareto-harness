---
title: REQ-0007 Capability、预算、取消与超时交付计划
status: active
owner: maintainers
updated: 2026-08-25
links: [REQ-0007, SPEC-0006, RFC-0006, ADR-0007]
---

# Goal and acceptance

按REQ-0007 AC-01至AC-20交付最小纵向切片：创建Run/Task → 初始化control aggregate并固定最小Capability/Budget → 默认拒绝与delegation收窄 → protected Fake Operation授权 → 原子多scope reserve → Fake执行 → verified/unknown settlement或release/refund → Run/Task/operation cancellation与FakeClock deadline → terminal race → late/duplicate safe audit → reopen Projection恢复 → Recorded control replay零执行/零重复核算。

# Current state

REQ-0003至REQ-0006均done，REQ-0005前置满足；启动时Git工作区clean，`.agents/work/active`只有`.gitkeep`，无其他Runtime Requirement。REQ-0007 high-risk影响分析已完成，RFC-0006/ADR-0007 accepted，SPEC-0006 approved；architecture/security self-review 0 open Blocker/Major且不冒充实施后独立review。现有基线：18 governance passed，docs 160 Markdown/44 formal IDs passed，workspace Kernel 68 passed/1 ignored、Protocol 9 unit + 21 contract/1 ignored。Runtime实现尚未开始。

# Plan

1. 在`pareto-protocol`增加强ID、Capability/Budget/Clock/Operation/Control Projection和control payload闭合类型；扩展builtin decoder和Schema生成，发布新内容地址SchemaSet，证明四个旧set byte-identical。
2. 在`pareto-kernel::event_store`内新增crate-private `runtime_control`模块：derived stream、sequence-1 initialization、exact persisted reader、pure fold、source reducer registry和full-provenance Projection；不改DB v2 DDL/trigger或公开API。
3. 实现root issuance、delegation subset、revocation/expiry与default-deny decision；control Event envelope保持Manifest owner signer，payload requester对认证principal exact验证。
4. 实现checked multi-dimensional Run/Task/Actor/per-operation budget model，同一`BEGIN IMMEDIATE`内capability/cancel/deadline检查和全账户reserve；实现verified/unknown settlement、release与owner refund。
5. 实现Run/Task/operation cancellation request/ack、cooperative/uninterruptible boundary、FakeClock wall/monotonic deadline和唯一terminal race；实现late/duplicate/out-of-order digest audit无状态/预算副作用。
6. 实现Fake Operation vertical service与read-only Recorded control replay；用counter证明replay无dispatch/event append/accounting变化，reopen从Event/Projection恢复pending和terminal状态。
7. 按SPEC-0006 traceability完成Focused、Impacted、Core、real SQLite、concurrency/model/compatibility tests；记录exact环境、命令、结果、Schema/DB identity及quality/cost/latency观察到VALIDATION。
8. 提交exact implementation revision，由fresh independent Agent使用`code-review`检查Requirement/Spec/RFC/ADR、exact diff和raw evidence；实现者只修复，Blocker/Major由reviewer focused re-review关闭。
9. 0 open Blocker/Major且完整completion gates通过后，同步README/index/EPIC/ARCH implemented facts和既有Review freshness，将REQ依次reviewing→verified→done并归档work；此前不启动REQ-0008。

# Validation

- Protocol contracts: `cargo test -p pareto-protocol capability_budget_contract --offline`; `cargo test -p pareto-protocol runtime_control_projection_contract --offline`; `cargo test -p pareto-protocol --all-targets --all-features --offline`
- Capability: `cargo test -p pareto-kernel runtime_control::capability_table --offline`; `cargo test -p pareto-kernel runtime_control::default_deny --offline`; `cargo test -p pareto-kernel runtime_control::delegation --offline`; `cargo test -p pareto-kernel runtime_control::revocation_and_expiry --offline`; `cargo test -p pareto-kernel runtime_control::denial_audit --offline`
- Budget: `cargo test -p pareto-kernel runtime_control::budget_model --offline`; `cargo test -p pareto-kernel runtime_control::reserve --offline`; `cargo test -p pareto-kernel runtime_control::budget_concurrency --offline`; `cargo test -p pareto-kernel runtime_control::settlement --offline`; `cargo test -p pareto-kernel runtime_control::refund --offline`; `cargo test -p pareto-kernel runtime_control::usage_authority --offline`
- Cancel/time/race: `cargo test -p pareto-kernel runtime_control::cancellation_propagation --offline`; `cargo test -p pareto-kernel runtime_control::interruptibility --offline`; `cargo test -p pareto-kernel runtime_control::deadline --offline`; `cargo test -p pareto-kernel runtime_control::terminal_race --offline`; `cargo test -p pareto-kernel runtime_control::model_sequences --offline`
- Idempotency/late/isolation: `cargo test -p pareto-kernel runtime_control::idempotency --offline`; `cargo test -p pareto-kernel runtime_control::late_and_duplicate --offline`; `cargo test -p pareto-kernel runtime_control::isolation --offline`
- Recovery/projection/replay: `cargo test -p pareto-kernel runtime_control::recovery --offline`; `cargo test -p pareto-kernel runtime_control::compatibility --offline`; `cargo test -p pareto-kernel runtime_control::projection --offline`; `cargo test -p pareto-kernel runtime_control::recorded_replay --offline`
- API surface: `cargo test -p pareto-kernel --doc --offline`
- Impacted: `cargo test -p pareto-kernel event_store --offline`; `cargo test -p pareto-kernel lifecycle:: --offline`; `cargo test -p pareto-kernel projection:: --offline`; `cargo test -p pareto-kernel --all-targets --all-features --offline`
- Core/governance/static: `python -m unittest discover -s scripts/tests -p "test_*.py"`; `python scripts/check_docs.py`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`; `cargo test --workspace --all-targets --all-features --offline`
- Schema identity: `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas`; rerun; verify all retained sets byte-identical and generated tree has only intended new set.
- DB/clock/scope inspection: assert `PRAGMA user_version=2` and exact v2 DDL/trigger bytes unchanged；source inspection confirmsruntime tests useFakeClock and no `sleep`、Provider、Tool、network/process dependency。
- Hygiene: `git diff --check`; `git status --short`，classify every file as intended/generated/reviewer-owned and reject unrelated changes.

# Handoff notes

Stop and return to impact/SPEC/RFC if implementation needs public authority/raw SQL, DB v3, alternate Event actor semantics, wildcard/ABAC, mutable balance/state table, multiple control streams, background timeout, real Effect/Provider/Tool, Control Snapshot, caller-selected reader/reducer, automatic crash release/reexecute, lifecycle cascade, sensitive late payload storage, or new third-party dependency. REQ-0008+ are consumers only.
