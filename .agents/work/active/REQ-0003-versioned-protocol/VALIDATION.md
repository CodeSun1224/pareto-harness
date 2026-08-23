# Validation Evidence

## Subject

- Requirement: REQ-0003 (`implementing`)
- Spec: SPEC-0002 (`approved`)
- RFC/ADR: RFC-0002 / ADR-0003 (`accepted`)
- Reviewed revision: `ff614b59385125fd3438a725388aa15998db68e8`（independent verdict: changes-requested）
- Environment: Windows, Rust/Cargo 1.96.0, rustfmt 1.9.0, Python 3.14.5, Git 2.43.0

## Results

| Scope/layer | Command or procedure | Result | Artifact/reference | Notes/risk |
|---|---|---|---|---|
| Focused static | `cargo fmt --all -- --check` | passed | local command result | Rust formatting |
| Focused static/security | `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | passed | local command result | Includes MSRV compatibility lint |
| Focused unit/contract | `cargo test --workspace --all-targets --offline` | passed | 17 tests: 5 unit + 12 integration | Canonicalization, closed parsing, IDs/digests, real Draft 2020-12 validation, schema/Serde presence parity, compatibility, scope, event binding, limit N/N+1, deterministic error truncation and replay lineage |
| Focused golden | `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas` followed by checked-in schema golden test | passed | 9 schemas + SchemaSet manifest/ref | Exact file-set/byte golden prevents stale output; cross-platform atomic publication remains open as F-009 |
| Impacted dependency | `cargo metadata --offline --format-version 1` and `cargo tree -p pareto-protocol --offline` | passed | Cargo.lock / metadata | Direct dependencies: jsonschema, schemars, serde, serde_json, sha2; no Runtime/DB/provider dependency |
| Core governance unit | `python -m unittest discover -s scripts/tests -p "test_*.py"` | passed | 18 tests | Existing SDD positive/negative fixtures |
| Core document | `python scripts/check_docs.py` | failed | stale REVIEW-0001 | Expected gate: tracked AGENTS/README/ARCH/EPIC/index changes require exact-revision independent Review; do not weaken checker |
| Whitespace | `git diff --check` | passed | tracked diff | Untracked files are additionally compiled/generated/read by tests but ordinary Git diff does not cover them until staged/committed |
| Cross-platform | `.github/workflows/protocol-matrix.yml` | pending | Windows/Linux/macOS matrix added at `ff614b5`; not yet pushed/executed | Workflow fetches the lockfile once, then runs locked/offline Rust gates, Schema/digest golden and governance checks identically on all three OSes |
| Full Runtime | Event Store/Replay executor/E2E | skipped | out of scope | REQ-0003 provides protocol contracts only; later Requirements own Runtime consumers |

## Acceptance trace summary

- AC-01/03/06/07/08/09：typed closed contracts and semantic/security negative tests pass on Windows.
- AC-02：nine deterministic public Schemas plus SchemaSet manifest/ref are checked in and byte/file-set golden tested.
- AC-04：UTF-16 JCS ordering, unsafe number rejection, fixed digest/revision vectors, complete SchemaRef/type domain separation and artifact domain tests pass.
- AC-05：conservative old-writer/new-reader proof accepts only optional property additions; narrowing/required/composition mutations fail closed.
- AC-10：crate has no Runtime/DB/provider dependency and Windows gates pass; Linux/macOS evidence is still missing, so Requirement cannot be verified.

## Remaining gates

1. Resolve independent review Blocker/Major findings and run focused re-review.
2. Complete independent review of exact commits and close all Blocker/Major findings.
3. Push and obtain passing Windows/Linux/macOS workflow evidence.
4. Re-run all completion commands and make `check_docs.py` pass without weakening REVIEW freshness.
