# Validation Evidence

## Subject

- Requirement: REQ-0005 (`reviewing`)
- Spec: SPEC-0004 (`approved`)
- RFC/ADR: RFC-0004 / ADR-0005 (`accepted`)
- Git revision or diff: initial exact implementation `d0011d064b88cf4be8e6b70ae781f3637bb15161`; REVIEW-0004 remediation currently in working tree and must be committed to a new exact revision before re-review
- Environment: Windows PowerShell, 2026-08-24, Asia/Shanghai

## Results

| Scope/layer | Command or procedure | Result | Artifact/reference | Notes/risk |
|---|---|---|---|---|
| Project orientation | Read required durable docs, REQ-0003/0004 design/review/archive evidence, protocol and Event Store code/tests; inspect active work and Git status | passed | PLAN current state; SPEC-0004 impact table | REQ-0004 done; startup worktree clean; no other Runtime Requirement implementing |
| Architecture specialist self-review | Apply `architecture-review` to REQ/SPEC/RFC against trusted-kernel, identity, event, replay, permission, concurrency and rollback invariants | passed | `ARCHITECTURE-REVIEW.md` | 0 open Blocker/Major; explicitly non-independent and not implementation approval |
| Document/governance | `python scripts/check_docs.py` | passed | 147 Markdown files, 38 formal IDs | All formal IDs, states, links, work paths and existing Review freshness checks passed |
| Governance unit | `python -m unittest discover -s scripts/tests -p "test_*.py"` | passed | 18 tests, 0 failed | Existing SDD/document checker behavior unchanged |
| Hygiene | `git diff --check`; `git status --short` | passed | design-only status inspection | No whitespace error; status contains only four new REQ-0005 formal docs and its active work directory |
| Implementation-entry reorientation | Full read of `README.md`, `AGENTS.md`, `docs/index.md`, REQ/SPEC/RFC/ADR/work records for REQ-0005, REQ-0003/0004 durable contracts and Reviews, protocol implementation/tests, and current Event Store implementation/tests; `git status --short`; active-work inspection | passed | 2026-08-24, Windows PowerShell, repository worktree | REQ-0003/0004 are `done`; REVIEW-0002/0003 are approved with 0 open Blocker/Major; no other Runtime Requirement is implementing; user-owned untracked REQ-0005 SDD files preserved |
| Focused protocol lifecycle | `cargo test -p pareto-protocol lifecycle_manifest_contract --offline` | passed | 1 passed; 17 filtered contract tests | `TaskId`, exact state enums, closed payload, lifecycle binding and typed `RunCreatedPayload` decode proved |
| Impacted protocol | `cargo test -p pareto-protocol --all-targets --all-features --offline` | passed | 9 unit + 18 contract; 1 observation ignored | No protocol regression; expected publisher drift subprocess emits an error while its parent negative test passes |
| Schema publication and retained identity | `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas`; repeat same command; `git diff --exit-code -- schemas/sets/sha256-68535b... schemas/sets/sha256-7adfe3b...` | passed | new set `sha256-dae028a86b31c5ab341240a0768e5166ac36cd4104bfa7e8c759230add368a71` | Repeated publication is byte-identical; both retained REQ-0003 sets have zero tracked-byte diff |
| Event Store refactor regression (initial) | `cargo test -p pareto-kernel event_store --offline` | failed, corrected | 2 passed, 15 failed before fixture correction | Existing REQ-0004 fixture appended a custom binding after the new sorted lifecycle bindings; admission correctly rejected unsorted registry. Added only `event_bindings.sort()` to the fixture, then reran. |
| Event Store refactor regression (rerun) | `cargo test -p pareto-kernel event_store --offline` | passed | 17 passed | Existing migration/append/read/idempotency/concurrency/recovery tests plus initial lifecycle tests all pass after fixture ordering correction |
| Kernel API surface | `cargo test -p pareto-kernel --doc --offline` | passed | 1 compile-fail doctest | No external Event Store/lifecycle authority, sqlx, or raw transaction API exposed |
| Focused lifecycle named tests | `cargo test -p pareto-kernel lifecycle::{manifest,state_machine,creation_atomicity,hierarchy,idempotency,terminal_and_late,concurrency,authority,isolation,transaction,recovery,compatibility,fold_contract,model_sequences} --offline` as individual commands | passed | 14 named tests, each 1 passed | Covers exact Manifest pins, all directed state pairs and all legal edges, hierarchy, atomicity, conflict priority, two-pool race, terminal late result, full scope/actor isolation, persisted exact reader, corrupt history, deterministic fold, and 343 bounded action triples |
| Kernel full + static | `cargo test -p pareto-kernel --all-targets --all-features --offline`; `cargo clippy -p pareto-kernel --all-targets --all-features --offline -- -D warnings` | passed | 30 tests; clippy 0 warnings | Includes 15 REQ-0004 tests and 15 lifecycle tests; no dependency or DB migration added |
| Lifecycle latency observation | `cargo test -p pareto-kernel lifecycle::performance_observation --offline -- --nocapture`; lifecycle concurrency emits contention observation | observation only | Windows local: create 5.9639 ms; transition at 21 prior events 122.5141 ms; exact reader/fold 22 events 17.3804 ms | No threshold or optimization claim; no model/provider/token cost |
| Named traceability rerun | All 15 PLAN named protocol/kernel/doc commands executed individually | passed | 1 matching test per lifecycle filter; protocol contract 1; doctest 1 | Final implementation snapshot before independent review |
| Core/governance/static batch | `python -m unittest discover -s scripts/tests -p "test_*.py"`; `python scripts/check_docs.py`; `cargo fmt --all -- --check`; workspace clippy `-D warnings`; workspace all-target/all-feature test | partial: expected review gate | Python 18 passed; fmt passed; clippy passed; workspace kernel 30 + protocol 9/18 passed; docs checker reports REVIEW-0001/0002/0003 stale | Staleness is caused by this substantive implementation and must be cleared by independent REVIEW-0004, not by implementer self-approval; final rerun remains mandatory |
| Independent REVIEW-0004 first pass | Fresh Agent used `code-review` against requested `5ef949d..d0011d0`, including direct-parent product diff `8d37f41..d0011d0`, and independently reran focused/workspace/static/governance/schema gates | changes requested | `docs/reviews/REVIEW-0004-run-task-state-machine-run-manifest.md`; 0 Blocker / 4 Major / 1 accepted Minor | F-001 priority; F-002 sequence numeric domain; F-003 fold identity binding; F-004 actual old-reader evidence. Implementer did not change finding dispositions. |
| REVIEW-0004 F-001/F-002 remediation | `cargo test -p pareto-kernel lifecycle:: --offline` | passed | 18 passed | Adds real-SQLite `conflict_priority` and `sequence_boundaries`: stale sequence precedes Task existence/state; CreateTask/Run/Task reject `i64::MAX`, `-1`, `0` without panic/append; impossible-sequence event-ID collision remains generic idempotency conflict. |
| REVIEW-0004 F-003 remediation | `cargo test -p pareto-kernel lifecycle::fold_identity --offline`; included in full lifecycle command | passed | 1 focused; lifecycle 18/18 | Validated events retain exact SchemaSet/limits admission identity; fold rejects mixed tenant, user presence/value, workspace, run, agent, actor, stream and Manifest/envelope binding, while identical range state/canonical digest remains deterministic. |
| REVIEW-0004 F-004 protocol evidence | `cargo test -p pareto-protocol checked_in_old_writer_manifests_use_their_exact_retained_reader --offline` | passed | 1 passed | New reader code loads both checked-in old directories (`sha256-68535b...`, `sha256-7adfe3...`) from exact manifest/doc bytes, validates an old Manifest with its exact reader, and proves current reader substitution is rejected. |
| REVIEW-0004 F-004 Event Store evidence | `cargo test -p pareto-kernel event_store --offline` | passed | 33 passed | REQ-0004 fixture now evolves from actual checked-in old set `sha256-68535b...`; persisted old-writer event reopens with exact registry, current alternate reader is rejected, and `PRAGMA user_version` remains 1. Lifecycle compatibility additionally injects an internally consistent unknown major row and proves exact load fails closed. |
| Remediation static check (first run) | workspace clippy `-D warnings` | failed, corrected | unused test-only imports | Moved `canonical`/`fingerprint` imports into the included test scope; no product suppression added. |
| Remediation static check (rerun) | `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | passed | 0 warnings | No dependency, DDL, public raw SQL/transaction, or scope expansion added. |

## Pending final reruns

The remediation-focused and impacted suites above are complete. The full workspace, governance,
schema-generation byte identity, documentation freshness and hygiene completion commands remain
mandatory after the remediation is fixed to an exact Git revision and before final handoff.

## Remaining limitations

- REVIEW-0004 is independent but remains `changes-requested`; only the independent reviewer may close F-001 through F-004 after re-reviewing the new exact remediation revision.
- Full Revision repository/content resolution, Projection/Snapshot/Replay, Capability/delegation/cancellation effects, Provider, Agent Loop and Task DAG remain future Requirements.
- Performance values are local Windows observations only; no cross-platform or optimization claim is made.
