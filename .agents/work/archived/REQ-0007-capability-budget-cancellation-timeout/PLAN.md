---
title: REQ-0007 Capability、预算、取消与超时交付计划
status: completed
owner: maintainers
updated: 2026-08-27
links: [REQ-0007, SPEC-0006, RFC-0006, ADR-0007, REVIEW-0006, REVIEW-0007, FIX-0001]
---

# Goal and acceptance

按REQ-0007 AC-01至AC-20交付最小纵向切片：创建由Manifest固定control-capable SchemaSet的Run/Task → 初始化control aggregate并固定最小Capability/Budget/source contract → lifecycle准入、默认拒绝与delegation收窄 → trusted resource envelope → 原子多scope reserve → opaque lease绑定的Fake执行 → authorized callback或显式timeout recovery settlement/release/refund → Run/Task/operation cancellation authority与FakeClock deadline → terminal/lifecycle race → late/duplicate safe audit → reopen Projection恢复 → Recorded control replay零执行/零重复核算。

# Current state

REQ-0003至REQ-0007均done。REVIEW-0006独立批准设计固定提交 `a4e3478`；fresh independent REVIEW-0007独立批准实现固定候选 `80249cc5c73575a3f92027f843cc657536905b9e`，F-001至F-009全部关闭，0 open Blocker/Major。Capability/Budget/Cancellation/Timeout、late-result隔离、Projection恢复和Recorded replay最小纵向切片已实现，完整completion gates通过，工作证据归档；REQ-0008未提前实现。

# Plan

1. 先把仅含设计修订与REVIEW-0006记录的exact commit交同一independent reviewer focused re-review；0 open Blocker/Major和docs/diff门禁恢复前不写Runtime。
2. 在`pareto-protocol`增加强ID、Capability/Budget/Clock/Operation/Control Projection和control payload闭合类型；扩展builtin decoder和Schema生成，发布新control-capable内容地址SchemaSet，证明四个旧set byte-identical及Manifest exact binding。
3. 在`pareto-kernel::event_store`内新增crate-private `runtime_control`模块：derived stream、sequence-1 source contract、exact persisted reader、pure fold、source reducer/operation-contract registry和full-provenance Projection；不改DB v2 DDL/trigger或公开API。
4. 实现lifecycle admission matrix与runtime-aware lifecycle transition in-flight guard；实现root issuance、delegation subset、revocation/expiry与default-deny，control Event envelope保持Manifest owner signer。
5. 实现trusted resource envelope/Kernel meter、checked Run/Task/Actor/per-operation budget及同一`BEGIN IMMEDIATE`原子reserve；实现opaque operation lease、authorized producer verified/unknown settlement、release与owner refund。
6. 实现Run/Task/operation cancellation request authority、probe/ack lease binding、FakeClock wall/monotonic deadline、TimeoutKey与确定性recovery command/event fingerprint、显式Kernel timeout recovery和唯一terminal/lifecycle race；实现not_due/commit-response-loss/same-ID mutation/different-ID terminal优先级及late audit无状态/预算副作用。
7. 实现Fake Operation vertical service与read-only Recorded control replay；用counter证明replay无dispatch/event append/accounting变化，reopen从Event/Projection恢复pending并显式reconcile到期operation。
8. 按SPEC-0006 traceability完成Focused、Impacted、Core、real SQLite、concurrency/model/compatibility tests；每个filter先由helper证明非零命中，记录exact环境、命令、结果、Schema/DB identity及quality/cost/latency观察到VALIDATION。
9. 提交exact implementation revision，由fresh independent Agent使用`code-review`检查Requirement/Spec/RFC/ADR、exact diff和raw evidence；代码评审使用下一个可用Review ID，实现者只修复，Blocker/Major由reviewer focused re-review关闭。
10. 0 open Blocker/Major且完整completion gates通过后，同步README/index/EPIC/ARCH implemented facts和既有Review freshness，将REQ依次reviewing→verified→done并归档work；此前不启动REQ-0008。

# Validation

所有下列`pareto-kernel`命名filter都通过计划新增的`python scripts/assert_cargo_test_filter.py pareto-kernel <filter>`执行：helper先运行Cargo list、断言命中数大于0，再运行该filter并把count/result写入VALIDATION；不得以Cargo的0 tests成功充当证据。

- Protocol contracts: `cargo test -p pareto-protocol capability_budget_contract --offline`; `cargo test -p pareto-protocol runtime_control_projection_contract --offline`; `cargo test -p pareto-protocol --all-targets --all-features --offline`
- Capability/lifecycle: filters `runtime_control::capability_table`, `runtime_control::default_deny`, `runtime_control::delegation`, `runtime_control::revocation_and_expiry`, `runtime_control::denial_audit`, `runtime_control::lifecycle_admission`, `runtime_control::lifecycle_reserve_race`
- Budget/envelope/producer: filters `runtime_control::budget_model`, `runtime_control::resource_envelope`, `runtime_control::reserve`, `runtime_control::budget_concurrency`, `runtime_control::settlement`, `runtime_control::refund`, `runtime_control::usage_authority`, `runtime_control::callback_authority`
- Cancel/time/race: filters `runtime_control::cancellation_authority`, `runtime_control::cancellation_propagation`, `runtime_control::interruptibility`, `runtime_control::deadline`, `runtime_control::timeout_recovery`（含TimeoutKey/ID golden、not_due、response-loss、same/different-ID priority）, `runtime_control::terminal_race`, `runtime_control::model_sequences`
- Idempotency/late/isolation: filters `runtime_control::idempotency`, `runtime_control::late_and_duplicate`, `runtime_control::isolation`
- Recovery/schema/projection/replay: filters `runtime_control::recovery`, `runtime_control::compatibility`, `runtime_control::schema_manifest_binding`, `runtime_control::projection`, `runtime_control::recorded_replay`
- API surface: `cargo test -p pareto-kernel --doc --offline`
- Impacted: `cargo test -p pareto-kernel event_store --offline`; `cargo test -p pareto-kernel lifecycle:: --offline`; `cargo test -p pareto-kernel projection:: --offline`; `cargo test -p pareto-kernel --all-targets --all-features --offline`
- Core/governance/static: `python -m unittest discover -s scripts/tests -p "test_*.py"`; `python scripts/check_docs.py`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`; `cargo test --workspace --all-targets --all-features --offline`
- Schema identity: `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas`; rerun; verify all retained sets byte-identical and generated tree has only intended new set.
- DB/clock/scope inspection: `python scripts/check_req0007_scope.py` asserts `PRAGMA user_version=2` and exact v2 DDL/trigger bytes unchanged，Runtime tests useFakeClock且无真实`sleep`、Provider、Tool、network/process依赖，Recorded replay API无executor/writer/recovery authority。
- Hygiene: `git diff --check`; `git status --short`，classify every file as intended/generated/reviewer-owned and reject unrelated changes.

# Handoff notes

Stop and return to impact/SPEC/RFC if implementation needs public authority/raw SQL, DB v3, alternate Event actor semantics, wildcard/ABAC, mutable balance/state table, multiple control streams, background timeout scanner、除显式Kernel timeout recovery之外的writer, real Effect/Provider/Tool, Control Snapshot, caller-selected reader/reducer/operation contract, automatic crash release/reexecute, lifecycle cascade, sensitive late payload storage, or new third-party dependency. REQ-0008+ are consumers only.
