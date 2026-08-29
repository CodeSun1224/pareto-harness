# REQ-0008 Validation Evidence

## Subject

- Requirement: REQ-0008 (`done`)
- Spec/RFC/ADR: SPEC-0007 / RFC-0008 / ADR-0009
- Accepted design revision: `3318cbc6fe8bc8c9717a5a2b4aea1153f0d281d6`
- Initial implementation candidate: `dfeee45f286c4dce4bdd950bf98d30cbc4b00fb8`
- REVIEW-0011 remediation candidate: the exact Git commit containing the updated evidence below; the same independent reviewer must re-review that full revision
- Baseline: initial worktree clean at `b1f626ed52ffd7e0e6b4ad9c0cb457c12e8760d7`; `origin/main` fetched at `754798de3a7f0f09d38c466b8f09199c7ebda9d1` (ahead 8, behind 0)
- Environment: Windows PowerShell, 2026-08-29, Asia/Shanghai; Cargo commands offline

## Results

| Scope/layer | Command or procedure | Result | Artifact/reference | Notes/risk |
|---|---|---|---|---|
| Independent implementation review | Fresh reviewer审查REQ/SPEC/RFC/ADR、完整diff和原始证据并多轮复审 | passed | REVIEW-0011；exact `e4877834fb54e3db936677f3b87c5fdf9e1d2d97` | F-001至F-006全部closed；0 open Blocker/Major |
| Hook focused | `cargo test -p pareto-kernel hook_runtime:: --offline` | passed | 39 passed | 包含Kernel-owned纵切、隔离、不同pair竞态、恢复、replay与重封负例 |
| Full regression | `cargo test --workspace --all-targets --all-features --offline` | passed | Kernel 160 passed/1 ignored；Protocol 9 unit + 24 contract/1 ignored | ignored项均为非阈值观察；publisher drift stderr属于绿色负向夹具 |
| Governance and scope | Python governance；REQ-0008 scope；fmt；clippy；schema generator；diff check | passed | Python 24；scope/schema/fmt/clippy/diff green | SQLite v2、retained SchemaSet、Fake-only和REQ-0009边界未漂移 |
| Final completion state | fact sync、Requirement done、active→archived | passed | 本归档目录、README/index/EPIC/architecture/REQ-0008 | 真实外部Hook Runtime与REQ-0009未开始 |

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
| Hook aggregate | `cargo test -p pareto-kernel hook_runtime:: --offline` | 39 passed after REVIEW-0011 remediation |
| Kernel-owned vertical execution | `cargo test -p pareto-kernel hook_runtime::kernel_owned_ --offline` | 6 passed: Transform→Gate→Observer plus exact no-write retry and full-command mutation rejection, point-start crash recovery, Gate deny short-circuit+skip, cancellation winner+late audit+skip, deadline timeout authority+late audit+skip, and invalid-kind rejection audit with a seven-field resealed-lineage matrix |
| Validly resealed negative history | `cargo test -p pareto-kernel hook_runtime::resealed_history_rejection --offline` | passed: wrong point/lineage, wrong final digest, pair mutation, and mutually resealed cross-stream payload IDs that disagree with the actual Hook envelope all fail closed |
| Hook isolation | `cargo test -p pareto-kernel hook_runtime::isolation --offline` | passed: tenant, user presence/value, workspace, run, scope actor, authenticated actor, task, subject proposal and Hook identity mutations are rejected with both pair Events absent; the task binding is checked against the admitted Control reservation |
| Different-pair writer race | `cargo test -p pareto-kernel hook_runtime::budget_concurrency --offline` | passed in both call orders: two distinct pair identities with the same initial cursors produce exactly one commit and one operation/reservation, without double reserve |
| Event Store | `cargo test -p pareto-kernel event_store --offline` | 160 passed, 1 existing performance observation ignored |
| Lifecycle | `cargo test -p pareto-kernel lifecycle:: --offline` | 18 passed |
| Projection/replay | `cargo test -p pareto-kernel projection:: --offline` | 35 passed, 1 existing performance observation ignored |
| Runtime Control | `cargo test -p pareto-kernel runtime_control:: --offline` | 53 passed |
| Kernel all targets/features | `cargo test -p pareto-kernel --all-targets --all-features --offline` | 160 passed, 1 ignored |
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
candidate under one writer transaction with strict registry-aware fold, validates both actual
Control and Hook envelope identities plus pair payloads at one MVCC horizon, short-circuits
Gate/Observer failure, and derives cancellation/timeout terminal winners through Runtime Control.
The point-start Event identity now binds a canonical fingerprint of every execute-command field, so
finalized and start-only mutation retries conflict without writes. Exact finalized retry is
no-write/no-handler and deliberately returns no redacted Transform payload;
a persisted point-start is resumed without duplicating the start fact. Registry order includes the
full canonical Hook-point vector before phase-local order. Nested JSON pointers, pair kind/identity,
next sequences, canonical Event preimages and both payloads are closed. Recorded replay remains
projection-only and the live vertical test proves replay causes no handler, Event, operation,
account or budget change. Rejection fold admission binds decision, point, Hook identity/revision,
subject, source cursor, input digest, registry and redaction policy to the current invocation.
The Hook-specific isolation matrix and a distinct-pair race in both call orders now exercise the
registry-aware writer path directly; test-only registry-free helpers remain limited to pair
mechanics fixtures and are not used as production identity admission evidence.

## Completion gates and independent review

| Command | Result |
|---|---|
| `python -m unittest discover -s scripts/tests -p "test_*.py"` | 24 passed |
| `python scripts/check_docs.py` | implementation review reports only REVIEW-0001 through REVIEW-0007 stale; final closure freshness is performed after fact sync/archive without rewriting their original findings |
| `cargo fmt --all -- --check` | passed |
| `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | passed |
| `cargo test --workspace --all-targets --all-features --offline` | passed: Kernel 160 passed/1 ignored; Protocol 9 unit + 24 contract passed/1 ignored |
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

## Final review and closure

Fresh independent REVIEW-0011 approved exact revision
`e4877834fb54e3db936677f3b87c5fdf9e1d2d97` with 0 open Blocker and 0 open Major after the same
Reviewer closed F-001 through F-006. Final fact sync and archival do not add runtime behavior;
REQ-0009 remains unstarted. The closure revision receives a final docs-freshness check before the
work is considered complete.
