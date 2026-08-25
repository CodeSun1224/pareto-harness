---
title: REQ-0006 Projection、Snapshot 与 Replay 交付计划
status: completed
owner: maintainers
updated: 2026-08-25
links: [REQ-0006, SPEC-0005, RFC-0005, ADR-0006]
---

# Goal and acceptance

按REQ-0006 AC-01至AC-14交付最小纵向切片：单Run persisted exact lifecycle events → deterministic versioned reducer → Run/Task Projection → current-cursor Snapshot → Snapshot-assisted incremental recovery → full-history Recorded replay → exact digest comparison → untrusted Snapshot full fallback。仅支持单进程SQLite、本地确定性Replay。

# Current state

REQ-0003、REQ-0004、REQ-0005、REQ-0006均为done。独立架构评审0 open Blocker/Major；fresh independent code review初审3个Major，author remediation后由同一reviewer focused re-review全部关闭，最终REVIEW-0005 approved、0 open Blocker/Major。协议/output SchemaSet、显式retained reducer、SQLite v2 Snapshot、prefix-proved assisted recovery、Recorded replay与比较的最小纵向切片已实现；全部completion gates通过，工作证据归档。REQ-0007尚未开始实现。

# Plan

1. 在`pareto-protocol`增加Reducer descriptor/ref、SourceReducerKey、EventCursor、Projection/Snapshot及history seed/step、projection/snapshot hash-view闭合类型，冻结`digest_json`preimage与empty/1/N/prefix+suffix golden，发布新内容地址output SchemaSet并保留所有旧set byte-identical。
2. 将REQ-0005 lifecycle pure fold重构为exact versioned reducer与确定性Projection，建立persisted source contract→exact reducer/output reader registry及retention/current-substitution拒绝，复用完整Manifest admission/identity/state guards；authority-bearing API保持crate-private。
3. 将SQLite Event Store原子升级到v2：新增events writer_epoch列/v2 INSERT trigger、immutable snapshot table/index及UPDATE/DELETE triggers/checksums，保留历史Event JSON、store identity、append-only语义和old readers；证明migration前已打开的v1 writer在v2后fail closed。
4. 实现owner-only point-in-time Projection load、显式`BEGIN IMMEDIATE` Snapshot creation、exact output reader/candidate校验、prefix Event重新读取/validation/rolling-chain证明、Snapshot seed后suffix增量和candidate-only full fallback。
5. 实现read-only Recorded replay、source/history/projection digest comparison及Simulated-before-effect fail-closed gate；接口不得依赖或接受Effect/Provider/Tool executor。
6. 按SPEC-0005逐项完成Focused unit/contract/SQLite negative、Impacted REQ-0003/4/5回归与Core isolation/replay/migration/crash/concurrency测试，记录exact命令、环境、结果、Schema identity、DB bytes和quality/cost/latency观察到VALIDATION。
7. 由fresh Agent/session使用`code-review`独立审查Requirement/Spec/RFC/ADR、exact diff和证据。实现者只修复，不自关闭Blocker/Major；每轮修复由同等独立reviewer focused re-review。
8. 0 open Blocker/Major且全部completion gates通过后，同步README/index/EPIC/ARCH implemented facts，将REQ依次更新为reviewing→verified→done并归档work；此前不启动REQ-0007实现。

# Validation

- Protocol/reducer contract: `cargo test -p pareto-protocol projection_snapshot_contract --offline`; `cargo test -p pareto-protocol projection_digest_golden --offline`; `cargo test -p pareto-kernel projection::reducer --offline`; `cargo test -p pareto-kernel projection::reducer_resolution --offline`
- Full history/invalid history: `cargo test -p pareto-kernel projection::full_history --offline`; `cargo test -p pareto-kernel projection::invalid_history --offline`
- Snapshot create/atomic/incremental: `cargo test -p pareto-kernel snapshot::creation --offline`; `cargo test -p pareto-kernel snapshot::atomicity --offline`; `cargo test -p pareto-kernel snapshot::incremental --offline`
- Snapshot trust/fallback/no-snapshot: `cargo test -p pareto-kernel snapshot::output_reader --offline`; `cargo test -p pareto-kernel snapshot::prefix_validation --offline`; `cargo test -p pareto-kernel snapshot::prefix_corruption --offline`; `cargo test -p pareto-kernel snapshot::fallbacks --offline`; `cargo test -p pareto-kernel projection::no_snapshot --offline`
- Replay/effects/digest: `cargo test -p pareto-kernel replay::recorded_read_only --offline`; `cargo test -p pareto-kernel replay::simulated_no_effect --offline`; `cargo test -p pareto-kernel replay::recorded_determinism --offline`; `cargo test -p pareto-kernel replay::digest_equivalence --offline`; `cargo test -p pareto-kernel replay::cross_store_not_comparable --offline`
- Isolation/concurrency/recovery: `cargo test -p pareto-kernel projection::isolation --offline`; `cargo test -p pareto-kernel snapshot::isolation --offline`; `cargo test -p pareto-kernel projection::concurrency --offline`; `cargo test -p pareto-kernel snapshot::concurrency --offline`; `cargo test -p pareto-kernel snapshot::recovery --offline`
- Migration/compatibility: `cargo test -p pareto-kernel snapshot::migration --offline`; `cargo test -p pareto-kernel snapshot::already_open_v1_writer --offline`; `cargo test -p pareto-kernel snapshot::compatibility --offline`; `cargo test -p pareto-kernel projection::compatibility --offline`
- REQ-0003 regression: `cargo test -p pareto-protocol --all-targets --all-features --offline`
- REQ-0004 regression: `cargo test -p pareto-kernel event_store --offline`
- REQ-0005 regression: `cargo test -p pareto-kernel lifecycle:: --offline`
- Kernel/API: `cargo test -p pareto-kernel --all-targets --all-features --offline`; `cargo test -p pareto-kernel --doc --offline`
- Governance/static/core: `python -m unittest discover -s scripts/tests -p "test_*.py"`; `python scripts/check_docs.py`; `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`; `cargo test --workspace --all-targets --all-features --offline`
- Schema identity: `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas`; verify all retained sets byte-identical and no unintended schema diff.
- Hygiene: `git diff --check`; `git status --short`，逐项确认仅REQ-0006范围变更。

# Handoff notes

实现前重读HANDOFF并重新检查Git/active work。若需要省略prefix exact validation、改变rolling hash/reducer resolver/output admission/writer epoch、public raw SQL/transaction、external Snapshot import、mutable authoritative Projection、caller-selected reader/reducer、Effect dispatch/reexecute、自动后台Snapshot、不同lifecycle状态语义、历史Event JSON重写/DB降级、Snapshot DELETE/GC、通用Projection框架或新第三方依赖，立即停止并回到impact/SPEC/RFC。REQ-0007/0012/0015/0024仅作为接口消费者，不在本Requirement提前实现。
