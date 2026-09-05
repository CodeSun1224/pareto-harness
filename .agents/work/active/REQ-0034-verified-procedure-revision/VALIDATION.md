# REQ-0034 Route Design Validation Evidence

## Subject

- Trusted baseline: `e7a939cad71a85ada97c3b60d61ba5c024d85ab9`.
- Initial candidate: `cfdc65af64675b8066b9bc429fbf998d588231bc`.
- First independent review record: `72a0f8b597996688f31007bd2fc7f613528f5cdc`; REVIEW-0018 recorded 1 Blocker/3 Major.
- Remediation content revision: `499116a8e93e00a737f0c112d0a0104eb9386840`.
- Approved remediation/evidence revision: `660cfca9e230f1440505c8e3bfd9a07bf17529ab`.
- Accepted RFC/Spec/Requirement and ADR closure revision: `6df161ff5d5fc150cfa09f48ae54b7501cababcb`.
- Final same-reviewer record commit: `1f3078c521118635ddc498f2767ab51daec709fd`.
- Scope: product/architecture/roadmap documents, REQ-0034/SPEC-0010/RFC-0013, EPIC-0007 and active work records only.
- Environment: Windows PowerShell, 2026-09-05, Asia/Shanghai; all Cargo commands use `--offline`.

## Results

| Scope/layer | Command or procedure | Result | Artifact | Evidence/risk |
|---|---|---|---|---|
| Governance tests | `python -m unittest discover -s scripts/tests -p "test_*.py"` | passed | 27 tests | Document and repository governance regressions passed |
| Document structure | `python scripts/check_docs.py` | passed | 209 Markdown files / 74 formal IDs | Same independent Reviewer refreshed REVIEW-0001..0007 and REVIEW-0010..0013 to exact closure revision; no stale review remains |
| Rust format | `cargo fmt --all -- --check` | passed | exit 0 | No Rust formatting change |
| Rust static | `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | passed | workspace clippy | Zero warnings |
| Workspace tests | `cargo test --workspace --all-targets --all-features --offline` | passed | Kernel 185 passed/1 ignored; Protocol 9 unit + 25 contract passed; 1 performance baseline ignored | Existing ignored cases are non-threshold performance observations |
| Schema identity | generator twice plus `git diff --exit-code -- schemas` | passed | retained schema tree | No schema byte changed |
| Scope | `git diff --exit-code -- Cargo.toml Cargo.lock crates schemas scripts` | passed | exit 0 | No product code, dependency, schema or script diff |
| Hygiene | `git diff --check` | passed | exit 0 | No whitespace error |
| AC traceability | PowerShell parse of REQ-0034 AC rows and SPEC-0010 trace rows | passed | 12 requirements / 12 mapped | Every REQ-0034 AC has an owned SPEC test row; no downstream AC is used for REQ-0034 verification |
| Backlog topology | PowerShell parse of all explicit prerequisite cells | passed | 35 backlog Requirements plus accepted REQ-0001 | Every prerequisite exists and precedes its consumer; no dependency cycle or backward owner reference found |
| Review preservation | `git diff --exit-code 72a0f8b597996688f31007bd2fc7f613528f5cdc -- docs/reviews/REVIEW-0018-verified-procedure-roadmap.md` | passed | exit 0 | Designer did not edit the independent findings or verdict |

The protocol publisher prints `existing content-addressed schema set differs byte-for-byte` inside its negative test; the test and overall command exit successfully, proving drift rejection.

## Design evidence

- Actual code search finds only optional `RunManifest.plan_revision` storage/equality and no Procedure, Task DAG or Node state runtime.
- `EvidenceRecord` exists as a validated protocol type, but there is no execution Evidence coverage fold or completion gate.
- REQ-0009 provides the retained Effect/recovery/reconciliation/fixed-horizon replay boundary that future Node execution must extend.
- The Backlog dependency graph is acyclic and moves Plan/DAG, Node state and minimal Evidence before the single-Agent executor.
- The design distinguishes non-authoritative memory, project guidance, Procedure/Verified Procedure, Plan/DAG, Evidence, Behavior promotion, four recovery/rollback classes and three replay modes.
- F-001 remediation makes REQ-0034 independently verifiable and assigns Manifest/Plan, Node, Evidence, execution and promotion to unique downstream owners.
- F-002 remediation freezes exact template witnesses, node/branch cardinality and non-expansion of nodes, edges, Schema, Evidence, Capability, budget, retry, recovery and compensation.
- F-003 remediation chooses content-addressed user/pure-builder Plan input with zero Harness external I/O; sequence-1 Manifest admission is the only first-version planning authority.
- F-004 remediation binds a complete review subject and principal-root role assignment with four-way mandatory independence/quorum, freshness, invalidation and revocation.
- F-005 remediation corrects REQ-0009 Effect Schema/Runtime status in the authoritative version/event model.
- F-006 fixed exact validation/Task lineage; F-007 corrected lifecycle ownership; F-008 removed the duplicate RFC compatibility paragraph.
- REVIEW-0018 independently approved exact `6df161ff5d5fc150cfa09f48ae54b7501cababcb`; every F-001 through F-008 is closed, with 0 open Blocker and 0 open Major.

## Design closure

- RFC-0013 is accepted, SPEC-0010 is approved, REQ-0034 design is approved, and ADR-0012 records the durable decision.
- This approval does not implement REQ-0034 or authorize REQ-0010 runtime work. REQ-0034 requires a separately reviewed implementation Plan/Tasks before code or schema changes.
- The next session must start from the accepted roadmap and redesign REQ-0010 as a distinct authority-first Requirement; the archived Provider path is not a cherry-pick source.
