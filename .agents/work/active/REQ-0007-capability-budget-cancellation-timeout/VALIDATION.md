# Validation Evidence

## Subject

- Requirement: REQ-0007 (`planned`)
- Spec: SPEC-0006 (`approved`)
- RFC/ADR: RFC-0006 / ADR-0007 (`accepted`)
- Git revision or diff: Runtime implementation paused；首轮设计commit `05dd7ca`已被REVIEW-0006要求修改；focused re-review revision待提交
- Environment: Windows PowerShell, 2026-08-25, Asia/Shanghai

## Results

| Scope/layer | Command or procedure | Result | Artifact/reference | Notes/risk |
|---|---|---|---|---|
| Project orientation | Read README/AGENTS/index/roadmap/backlog/EPIC/kernel architecture；REQ-0003..0006 Requirement/Spec/RFC/ADR/Review；REQ-0005/0006 archived Plan/Tasks/Handoff/VALIDATION；protocol/Event Store/lifecycle/projection/snapshot/replay code/tests；inspect Git/active work | passed | REQ-0007/SPEC-0006 evidence paths | REQ-0005/0006 done；startup clean；active only `.gitkeep`；prerequisite satisfied |
| Risk classification | Apply project-orientation, impact-analysis and sdd-delivery | passed | REQ-0007 `risk: high`; SPEC-0006 impact table | permissions/security/resources/concurrency/cancel/replay require high-risk path |
| Requirement/test planning | Apply requirement-authoring and test-planning；map every AC/risk to named non-zero command | passed | REQ-0007 AC-01..20；SPEC-0006 Test traceability；PLAN Validation | No vague “relevant tests” entry |
| RFC/ADR | Apply rfc-authoring to cross-requirement authority/budget/time/late/replay semantics | passed | RFC-0006 accepted；ADR-0007 accepted | Alternatives, failure, compatibility, rollback and separate Q/C/L covered |
| Architecture/security self-review | Apply architecture-review to request→capability→budget→operation→event→projection→recovery | superseded as gate | `ARCHITECTURE-REVIEW.md` | 仅历史self-review；不能批准设计或实施 |
| Independent design review | Fresh Agent review exact `05dd7ca` with architecture-review | failed / changes requested | `docs/reviews/REVIEW-0006-capability-budget-cancellation-timeout-design.md` | 0 Blocker、6 Major：lifecycle、cancel authority、callback producer、timeout recovery、Manifest/SchemaSet、trusted envelope；Runtime paused |
| Design remediation candidate | Revise REQ/SPEC/RFC/ADR/Plan for REVIEW-0006 F-001..F-007；remove all post-gate Runtime declarations；`python -m unittest discover -s scripts/tests -p "test_*.py"`; `git diff --check` | passed | design working tree before focused commit | 18 governance tests passed；diff check passed；worktree contains documents only |
| Design remediation docs gate | `python scripts/check_docs.py` | failed: freshness only | checker output before focused commit | REVIEW-0001..0005 reviewed revisions predate new REQ-0007 design paths；same independent reviewer must substantively verify unchanged earlier contracts and restore freshness；no parser/link/finding-format error reported |
| Existing governance baseline | `python -m unittest discover -s scripts/tests -p "test_*.py"`; `python scripts/check_docs.py` | passed | 18 tests；160 Markdown/44 formal IDs before REQ-0007 | Existing checker behavior green before edits |
| Existing Core baseline | `cargo test --workspace --all-targets --all-features --offline` | passed | Kernel 68 passed/1 ignored；Protocol 9 unit + 21 contract/1 ignored | Expected publisher-drift stderr belongs to passing negative fixture |
| Approved design docs | `python scripts/check_docs.py`; `git diff --check` before active work creation | passed | 164 Markdown files；48 formal IDs | Only REQ-0007 Requirement/Spec/RFC/ADR present；no Runtime code |

## Skipped tests

All implementation, compatibility, concurrency, recovery and completion commands remain pending because independent design approval has not passed and Runtime implementation is paused. No result is inferred from the candidate design.

## Remaining limitations

- REVIEW-0006 remains `changes-requested` until the same independent reviewer approves an exact design-remediation commit with 0 open Blocker/Major.
- The historical architecture/security self-review cannot approve design or implementation.
- ProductionClock、background timeout、real Effect/Provider/Tool、Control Snapshot、distributed budget and downstream frameworks are intentionally absent.
- Fresh independent implementation code review with a new Review ID remains mandatory after exact implementation and raw validation evidence exist；it cannot reuse or overwrite REVIEW-0006.
