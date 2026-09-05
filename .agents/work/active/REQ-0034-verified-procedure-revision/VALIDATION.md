# REQ-0034 Route Design Validation Evidence

## Subject

- Trusted baseline: `e7a939cad71a85ada97c3b60d61ba5c024d85ab9`.
- Candidate: working tree before exact design commit.
- Scope: product/architecture/roadmap documents, REQ-0034/SPEC-0010/RFC-0013, EPIC-0007 and active work records only.
- Environment: Windows PowerShell, 2026-09-05, Asia/Shanghai; all Cargo commands use `--offline`.

## Results

| Scope/layer | Command or procedure | Result | Artifact | Evidence/risk |
|---|---|---|---|---|
| Governance tests | `python -m unittest discover -s scripts/tests -p "test_*.py"` | passed | 27 tests | Document and repository governance regressions passed |
| Document structure | `python scripts/check_docs.py` | skipped | pre-review output | Only REVIEW-0001..0007 and REVIEW-0010..0013 freshness checks fail because substantive formal docs changed; independent review must refresh them |
| Rust format | `cargo fmt --all -- --check` | passed | exit 0 | No Rust formatting change |
| Rust static | `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | passed | workspace clippy | Zero warnings |
| Workspace tests | `cargo test --workspace --all-targets --all-features --offline` | passed | Kernel 185 passed/1 ignored; Protocol 9 unit + 25 contract passed; 1 performance baseline ignored | Existing ignored cases are non-threshold performance observations |
| Schema identity | generator twice plus `git diff --exit-code -- schemas` | passed | retained schema tree | No schema byte changed |
| Scope | `git diff --exit-code -- Cargo.toml Cargo.lock crates schemas scripts` | passed | exit 0 | No product code, dependency, schema or script diff |
| Hygiene | `git diff --check` | passed | exit 0 | No whitespace error |

The protocol publisher prints `existing content-addressed schema set differs byte-for-byte` inside its negative test; the test and overall command exit successfully, proving drift rejection.

## Design evidence

- Actual code search finds only optional `RunManifest.plan_revision` storage/equality and no Procedure, Task DAG or Node state runtime.
- `EvidenceRecord` exists as a validated protocol type, but there is no execution Evidence coverage fold or completion gate.
- REQ-0009 provides the retained Effect/recovery/reconciliation/fixed-horizon replay boundary that future Node execution must extend.
- The Backlog dependency graph is acyclic and moves Plan/DAG, Node state and minimal Evidence before the single-Agent executor.
- The design distinguishes non-authoritative memory, project guidance, Procedure/Verified Procedure, Plan/DAG, Evidence, Behavior promotion, four recovery/rollback classes and three replay modes.

## Remaining gates

- Commit an exact candidate and record its revision.
- Fresh independent architecture-review and code-review of the exact candidate, including Plan/Tasks.
- Close all Blocker/Major findings with same-Reviewer re-review.
- Refresh existing approved Review freshness, accept RFC/SPEC/Requirement and add the resulting ADR only after approval.
- Re-run `python scripts/check_docs.py` to exit 0 and repeat final hygiene/status checks.
