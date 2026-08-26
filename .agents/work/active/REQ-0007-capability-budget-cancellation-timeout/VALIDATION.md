# Validation Evidence

## Subject

- Requirement: REQ-0007 (`reviewing`)
- Spec: SPEC-0006 (`approved`)
- RFC/ADR: RFC-0006 / ADR-0007 (`accepted`)
- Git revision or diff: exact implementation candidate `9f979f0ccaa6be0431ca794f584fd0c6df83af9c`
- Environment: Windows PowerShell, 2026-08-26, Asia/Shanghai

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
| F-004 identity remediation candidate | Freeze TimeoutKey/command-event ID/fingerprint、not_due、response-loss、same/different-ID priority in REQ/SPEC/RFC/ADR；`python -m unittest discover -s scripts/tests -p "test_*.py"`; `git diff --check`; `python scripts/check_docs.py` | governance/diff passed；docs freshness pending | working tree after REVIEW-0006 focused re-review | 18 governance tests passed；diff check passed；docs checker only reports REVIEW-0001..0005 stale against the new unreviewed F-004 design paths，expected until next exact independent re-review |
| Independent design approval | Same fresh reviewer focused re-review of exact `a4e34785908207e622365250ae1466b85b4baecb`; `python scripts/check_docs.py`; `git diff --check` | passed | REVIEW-0006 approved | F-001..F-006 closed；0 open Blocker/Major；170 Markdown/49 IDs；review freshness substantively restored；Runtime implementation unlocked |
| Existing governance baseline | `python -m unittest discover -s scripts/tests -p "test_*.py"`; `python scripts/check_docs.py` | passed | 18 tests；160 Markdown/44 formal IDs before REQ-0007 | Existing checker behavior green before edits |
| Existing Core baseline | `cargo test --workspace --all-targets --all-features --offline` | passed | Kernel 68 passed/1 ignored；Protocol 9 unit + 21 contract/1 ignored | Expected publisher-drift stderr belongs to passing negative fixture |
| Approved design docs | `python scripts/check_docs.py`; `git diff --check` before active work creation | passed | 164 Markdown files；48 formal IDs | Only REQ-0007 Requirement/Spec/RFC/ADR present；no Runtime code |

## Implementation candidate results

| Scope/layer | Command or procedure | Result | Artifact/reference | Notes/risk |
|---|---|---|---|---|
| Runtime focused | `cargo test -p pareto-kernel runtime_control --offline --no-fail-fast` | passed | 32 passed | FakeClock only；default deny、delegation、budget race、cancel/deadline/timeout、late/idempotency、reopen/replay |
| Named non-zero filters | `python scripts/assert_cargo_test_filter.py pareto-kernel <filter>` for PLAN filters | passed | each reported `matched: 1` | initial zero-match for `budget_concurrency` was rejected；test renamed/split then rerun green |
| Protocol contracts | capability/budget and Runtime Projection filters；all targets/features | passed | 9 unit + 23 contract；1 ignored observation | publisher drift stderr is expected passing negative fixture |
| Schema identity | generate final SchemaSet twice and hash all files | passed | `sha256-a1f960…`; 45 files stable | four retained sets untouched；stale agent-generated candidates removed |
| Event Store/Lifecycle | Event Store and Lifecycle filters | passed | 15 + 18 | DB v2/migration/authority/concurrency/recovery green |
| Projection/Snapshot/Replay | `cargo test -p pareto-kernel projection:: --offline --no-fail-fast` | passed after compatibility repair | 35 passed；1 ignored observation | new source set resolves lifecycle reducer while retained `4ce387…` output contract remains explicit |
| API/static scope | Kernel doctest；Kernel clippy `-D warnings`; `check_req0007_scope.py`; `git diff --check` | passed | doctest 1；scope checker green | no real I/O/sleep/dependency growth；frozen DB constants exact HEAD |
| Governance unit | `python -m unittest discover -s scripts/tests -p "test_*.py"` | passed | 20 tests | includes non-zero filter and scope helper tests |
| Full language gates | `cargo fmt --all -- --check`; workspace clippy `-D warnings`; `cargo test --workspace --all-targets --all-features --offline` | passed | Kernel 100 passed/1 ignored；Protocol 9 + 23 passed/1 ignored | no warning suppression or gate weakening |
| Docs freshness before implementation review | `python scripts/check_docs.py` | expected gate failure | only REVIEW-0001..0005 stale against implementation paths | fresh independent code review must substantively restore freshness；not treated as pass |

## Skipped tests

Independent code review and post-review full gate rerun remain pending. Real Provider/Tool/network/performance claims are out of scope；ignored tests are observation-only baselines already marked by the repository。

## Remaining limitations

- REVIEW-0006 approved design only；implementation still requires a new independent Review ID and exact revision。
- The historical architecture/security self-review cannot approve design or implementation.
- ProductionClock、background timeout、real Effect/Provider/Tool、Control Snapshot、distributed budget and downstream frameworks are intentionally absent.
- Fresh independent implementation code review with a new Review ID remains mandatory after exact implementation and raw validation evidence exist；it cannot reuse or overwrite REVIEW-0006.
