# REQ-0008 Validation Evidence

## Subject

- Requirement: REQ-0008 (`implementing`)
- Spec/RFC/ADR: SPEC-0007 / RFC-0008 / ADR-0009
- Accepted design revision: `3318cbc6fe8bc8c9717a5a2b4aea1153f0d281d6`
- Initial implementation candidate: `dfeee45f286c4dce4bdd950bf98d30cbc4b00fb8`
- REVIEW-0011 remediation candidate: the exact Git commit containing the updated evidence below; the same independent reviewer must re-review that full revision
- Baseline: initial worktree clean at `b1f626ed52ffd7e0e6b4ad9c0cb457c12e8760d7`; `origin/main` fetched at `754798de3a7f0f09d38c466b8f09199c7ebda9d1` (ahead 8, behind 0)
- Environment: Windows PowerShell, 2026-08-29, Asia/Shanghai; Cargo commands offline

## Scope and identity

| Check | Result | Evidence |
|---|---|---|
| Requirement scope | passed | `python scripts/check_req0008_scope.py`: SQLite `user_version=2` and accepted DDL/trigger constants unchanged; retained SchemaSets byte-identical; Fake Hook only; Recorded replay read-only; no real runtime/network/process/sleep/new dependency or REQ-0009 implementation |
| Current SchemaSet | passed | `sha256-0efc2ecfafba4c683a08917f4f4d025731f70df7c1ec68827d5eedff46384771`; the prior Hook set `sha256-3a0c6e67a97675cf6bfcdc1fb9766b30a79ae62e662479d9ae1ef5d7b43ff99d` and every earlier retained set remain byte-identical |
| Historical compatibility | passed | retained v1 manifest remains exact eight roles with no Hook config; current v2 has nine roles and config digest; retained source/output reducers and schema sets pass protocol/kernel compatibility tests |
| Dependency growth | passed | no changes to workspace or crate manifests and `Cargo.lock` |
| Hygiene before review | passed | `git diff --check`; status classified as expected protocol/kernel source, tests, generated SchemaSet, scope checker, Requirement and active-work evidence only |

## Focused and layered tests

| Scope/layer | Command or procedure | Result |
|---|---|---|
| Named Hook filters | `python scripts/assert_cargo_test_filter.py pareto-kernel <filter>` for all 29 PLAN filters | passed; every filter independently reported `matched: 1` and ran 1/1 |
| Hook aggregate | `cargo test -p pareto-kernel hook_runtime:: --offline` | 37 passed after REVIEW-0011 remediation |
| Kernel-owned vertical execution | `cargo test -p pareto-kernel hook_runtime::kernel_owned_ --offline` | 4 passed: Transform→Gate→Observer, Gate deny short-circuit+skip, deadline timeout authority+late audit+skip, invalid-kind rejection audit |
| Validly resealed negative history | `cargo test -p pareto-kernel hook_runtime::resealed_history_rejection --offline` | passed: wrong point/lineage, wrong final digest, and cross-stream pair mutation fail closed |
| Event Store | `cargo test -p pareto-kernel event_store --offline` | 158 passed, 1 existing performance observation ignored |
| Lifecycle | `cargo test -p pareto-kernel lifecycle:: --offline` | 18 passed |
| Projection/replay | `cargo test -p pareto-kernel projection:: --offline` | 35 passed, 1 existing performance observation ignored |
| Runtime Control | `cargo test -p pareto-kernel runtime_control:: --offline` | 53 passed |
| Kernel all targets/features | `cargo test -p pareto-kernel --all-targets --all-features --offline` | 158 passed, 1 ignored |
| Kernel public-boundary doctest | `cargo test -p pareto-kernel --doc --offline` | 1 compile-fail doctest passed |
| Hook protocol contract | `cargo test -p pareto-protocol hook_contract --offline` | 1 matched and passed |
| Protocol all targets/features | `cargo test -p pareto-protocol --all-targets --all-features --offline` | 9 unit + 24 contract passed; 1 existing performance observation ignored |

The original 29 exact filters were: `kind_point_table`, `phase_order_lineage`, `ordering`,
`gate_composition`, `default_deny`, `failure_policy`, `observer_non_authority`,
`transform_chain_failure`, `transform_protected_fields`, `authority`, `isolation`,
`output_security`, `reserve_pair_atomicity`, `pair_fault_injection`, `budget_reserve`,
`budget_concurrency`, `settlement`, `idempotency`, `terminal_pair_atomicity`,
`cancellation_deadline`, `terminal_race`, `model_sequences`, `late_and_duplicate`,
`fold_contract`, `recovery`, `pair_corruption`, `compatibility`, `recorded_replay`, and
`unsupported_modes`. The first wrapper batch hit its outer shell time limit only after the
first 23 individual invocations had each completed green; the remaining six were then run
individually and completed green. No zero-test Cargo success is counted as evidence.

REVIEW-0011 initially returned `changes-requested` at exact revision `dfeee45` with 2 Blocker and
4 Major findings. Remediation adds one Kernel-owned execution path that resolves the pinned
registry/schema/handler before start, emits point/reserve/terminal/skip/final facts, admits each
candidate under one writer transaction with strict registry-aware fold, validates control+Hook
pairs at one MVCC horizon, short-circuits Gate/Observer failure, derives timeout settlement through
Runtime Control, recursively checks JSON pointers, and seals pair kind, identity, next sequences,
canonical event preimages and both payloads. Recorded replay remains projection-only and the
new live vertical test proves replay causes no handler, Event, operation, account or budget change.

## Completion gates before independent review

| Command | Result |
|---|---|
| `python -m unittest discover -s scripts/tests -p "test_*.py"` | 24 passed |
| `python scripts/check_docs.py` | expected pre-review failure only: REVIEW-0001 through REVIEW-0007 are stale against the new substantive implementation paths; a fresh independent implementation Review is required to restore freshness |
| `cargo fmt --all -- --check` | passed |
| `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | passed |
| `cargo test --workspace --all-targets --all-features --offline` | passed: Kernel 158 passed/1 ignored; Protocol 9 unit + 24 contract passed/1 ignored |
| `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas` | passed twice; generated bytes stable |
| `git diff --check` | passed |

The protocol publisher tests intentionally print `existing content-addressed schema set differs
byte-for-byte` while asserting that a tampered existing set is rejected; their process exit code
and enclosing test results are zero/green.

## Quality, cost, and latency

- Quality: deterministic protocol, atomic-pair, isolation, compatibility, replay and model tests
  pass; this is test evidence, not production-provider evidence.
- Cost: Recorded mode proves zero handler calls, zero Event/pair appends and no budget/account
  mutation. No token or monetary optimization claim is made.
- Latency: tests use FakeClock and no real sleep. Existing local SQLite/protocol performance
  observations remain explicitly ignored non-threshold observations; no latency improvement claim
  is made.

## Remaining gate

A fresh independent Agent created REVIEW-0011; it remains `changes-requested` until its 2 Blocker
and 4 Major findings are re-reviewed against the exact remediation commit. The same reviewer must
leave zero open Blocker/Major. Only then may docs freshness, the final completion gates,
Requirement fact sync, `done` transition and work archival occur.
