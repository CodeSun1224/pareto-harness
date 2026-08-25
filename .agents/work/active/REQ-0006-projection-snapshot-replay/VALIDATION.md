# Validation Evidence

## Subject

- Requirement: REQ-0006 (`reviewing`)
- Spec: SPEC-0005 (`approved`)
- RFC/ADR: RFC-0005 / ADR-0006 (`accepted`)
- Git revision or diff: initial implementation `5c4f6e7f304c55fb61b6cc7e08d5bbe902b8d82c`; independent review record `a94d756`; remediation working tree based on `a94d756`，exact remediation revision pending
- Environment: Windows PowerShell, 2026-08-25, Asia/Shanghai

## Results

| Scope/layer | Command or procedure | Result | Artifact/reference | Notes/risk |
|---|---|---|---|---|
| Project orientation | Read required roadmap/epic/architecture, REQ-0003..0005 Requirement/Spec/RFC/ADR/Review, REQ-0004/0005 archived Plan/Tasks/Handoff/VALIDATION, protocol/Event Store/lifecycle implementation and tests; inspect Git and active work | passed | REQ-0006 Requirement/SPEC impact evidence | REQ-0003/4/5 done; startup worktree clean; active only `.gitkeep`; prerequisites satisfied |
| Risk classification | Apply project-orientation, impact-analysis and sdd-delivery | passed | REQ-0006 `risk: high`; SPEC-0005 impact table | events/schema, persistence, replay, concurrency and isolation require high-risk path |
| Requirement/test planning | Apply requirement-authoring and test-planning; map every AC and identified risk to named test commands | passed | REQ-0006 AC-01..14; SPEC-0005 Test traceability; PLAN Validation | No vague “relevant tests” entries |
| RFC decision | Apply rfc-authoring because reducer/snapshot/replay/DB contracts are cross-requirement and hard to reverse | passed | RFC-0005 accepted; ADR-0006 accepted | Alternatives, failure/effects, migration, rollback and separate quality/cost/latency covered |
| Architecture specialist self-review | Apply architecture-review to trusted boundary, effect reachability, versions, replay, concurrency, isolation and rollback | passed | `ARCHITECTURE-REVIEW.md` | 0 open Blocker/Major; explicitly non-independent and not implementation approval |
| Independent architecture review | Fresh reviewer fixed exact SHA-256, inspected protocol/Event Store/lifecycle contracts and independently ran Event Store/lifecycle baselines; author remediation received focused independent re-review and final-byte freshness confirmation | passed | `INDEPENDENT-ARCHITECTURE-REVIEW.md`; IAR-F-001..F-008 closed; final hashes fixed; 0 Blocker / 0 Major | Findings and final freshness closed only by reviewer; implementation still requires a fresh independent code review |
| Document/governance | `python scripts/check_docs.py` | passed | 159 Markdown files, 43 formal IDs | New Requirement/work path, Spec/RFC/ADR links and statuses valid |
| Governance unit | `python -m unittest discover -s scripts/tests -p "test_*.py"` | passed | 18 tests, 0 failed | Existing SDD/document checker behavior unchanged |
| Design-phase hygiene | `git diff --check`; `git status --short` before implementation | passed | five untracked REQ-0006 SDD/work paths only | Historical design-entry evidence; superseded by implementation diff and final hygiene gate |
| Protocol Projection/Snapshot contracts | `cargo test -p pareto-protocol projection_ --offline` | passed | 2 passed | Closed Schema contract plus seed/1/N prefix-suffix digest golden |
| Fresh independent code review | Reviewer inspected exact `5c4f6e7` diff/contracts and independently reran focused/impacted/core gates | changes requested | `docs/reviews/REVIEW-0005-projection-snapshot-replay.md`; 0 Blocker / 3 Major / 1 accepted Minor | F-001 exact reducer retention，F-002 actual DDL/migration proof，F-003 AC matrix/default-parallel stability；REQ-0006保持reviewing |
| F-001 remediation | Replace dynamic registration/current shared fold with version-controlled exact source-key → reducer/output/limits/implementation allowlist；test evolved set、alternate output order、missing/current substitution和第二implementation dispatch | passed, pending independent closure | `projection::reducer_resolution`; `projection::authority` | Persisted source exact reader remains separate；unrelated SchemaSet addition preserves source contract key，changed binding has no mapping and fails closed |
| F-002 remediation | Preserve the originally published v2 ledger checksum while independently freezing/comparing actual Snapshot table/index and trigger SQL；inject rollback after each of six v2 DDL stages；migrate two exact lifecycle Events and Manifest bytes；held-open v1 writer rejection + v2 success | passed, pending independent closure | `snapshot::migration`; `snapshot::migration_rolls_back_each_v2_ddl_stage_with_history_intact`; `snapshot::snapshot_actual_ddl_drift_is_rejected`; `snapshot::already_open_v1_writer` | CHECK/UNIQUE/type/index-order drift rejected；every injected failure leaves v1 version/schema/ledger/history unchanged，then migration/reopen succeeds；validator hardening does not strand initial-v2 databases |
| F-003 remediation | Add non-zero authority test, retained REQ-0005 source fixture, invalid sequence/schema/lifecycle matrix, candidate/isolation/comparison matrices and two-connection barrier MVCC test；all corruption writes use one controlled connection/transaction；repeat default-parallel suites | passed, pending independent closure | Projection 3× 35 passed/1 ignored；Event Store 3× 68 passed/1 ignored | No `--test-threads=1` workaround；former trigger DROP/CREATE race did not recur in six exact-remediation default-parallel runs |
| Focused Projection/Snapshot/Replay | `cargo test -p pareto-kernel projection:: --offline` | passed | 35 passed / 1 observation ignored，repeated 3 times | Full fold, explicit resolver/golden, historical source reader, authority, Snapshot candidate/isolation/prefix proof, replay comparison, barrier concurrency, recovery and migration matrices |
| Impacted Event Store/lifecycle | `cargo test -p pareto-kernel event_store --offline` | passed | 68 passed / 1 observation ignored，repeated 3 times | Includes REQ-0004/0005 plus REQ-0006 real-SQLite tests；default parallel stable |
| REQ-0003 regression | `cargo test -p pareto-protocol --all-targets --all-features --offline` | passed | 9 unit + 21 contract; 1 observation ignored | New set published; all retained sets complete and content-addressed |
| Static analysis | `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | passed | 0 warnings | No new dependency or public authority surface |
| Schema publication/retention | `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas`; repeat; `git diff --exit-code` on all three retained sets | passed | new set `sha256-4ce3872926ce61209fdc5ed48deceeec9703ccfe94ea83be485eb8ef7512ff97`; 28 files / 122365 bytes | Existing `68535b...`, `7adfe3...`, `dae028...` sets byte-identical; repeat publication idempotent |
| Local quality/latency/storage observation | `cargo test -p pareto-kernel projection::performance_observation --offline -- --ignored --nocapture` | passed | full 1/10/100/1000: 7.9178/6.9264/48.8606/439.3494 ms; Snapshot create 1/10/100/1000: 7.597/9.5602/75.1801/1316.729 ms; suffix 1/10: 7.1572/13.2117 ms; recorded replay 1000: 553.0014 ms; Snapshot JSON 75800 bytes; DB 2732032 bytes | Windows debug/local SQLite observation only; assisted still validates/hashes prefix and no optimization threshold or token/provider claim |
| Completion: governance | `python -m unittest discover -s scripts/tests -p "test_*.py"` | passed | 18 passed | Governance behavior unchanged |
| Completion: Rust/schema after remediation | `cargo fmt --all -- --check`; workspace clippy `-D warnings`; `cargo test --workspace --all-targets --all-features --offline`; schema generator | passed | Kernel 68 passed/1 observation ignored; Protocol 9 unit + 21 contract/1 observation ignored; generated bytes stable | Full offline language gates passed on 2026-08-25；protocol publisher drift stderr is expected negative fixture and process succeeded |
| Completion: hygiene after remediation | `git diff --check`; `git status --short` and scope classification | passed | five Kernel implementation/test files plus SPEC/active work evidence；review record separate in `a94d756` | No dependency, Provider, Effect, CLI, Memory, DAG or unrelated change；exact remediation commit/focused re-review pending |
| Completion: durable Review freshness | `python scripts/check_docs.py` before independent code review | expected pending | REVIEW-0001..0004 report substantive paths newer than their reviewed revision | Must be closed by fresh reviewer on exact implementation revision; implementer will not self-refresh reviewer-owned dispositions |

## Skipped tests

No planned Runtime, regression, migration, isolation or default-parallel test is skipped. Two explicitly ignored observation tests remain non-gating by design. `check_docs.py` remains expected pending because REVIEW-0003 freshness may only be advanced by the independent reviewer after F-002/F-003 closure；final documentation/status/archive gates run after focused re-review。

## Remaining limitations

- Simulated fixture resolver, reexecute and CLI remain intentionally unimplemented; simulated requests fail before any Effect entry point.
- Fresh independent code review completed with 3 Major；author remediation evidence is fixed in this working tree，but only the same independent reviewer may close F-001..F-003 on an exact remediation commit.
- Performance/storage numbers and any optimization claim are absent until the planned real-SQLite observations run.
- REQ-0007 Capability/Budget/Cancel, Effect execution, Workspace content, Memory and Context DAG remain future Requirements.
