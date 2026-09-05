# REQ-0034 Route Redesign Test Plan

## Design candidate checks

- `python -m unittest discover -s scripts/tests -p "test_*.py"`
- `python scripts/check_docs.py`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`
- `cargo test --workspace --all-targets --all-features --offline`
- `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas`
- `git diff --exit-code -- schemas`
- `git diff --check`
- `git status --short`
- `git diff --name-status e7a939cad71a85ada97c3b60d61ba5c024d85ab9...HEAD` after the exact candidate commit.

The schema generator is run only to prove this documentation-only candidate leaves `schemas/` byte-identical. Any runtime, Cargo or schema diff blocks design review.

## REQ-0034 owned implementation tests

### Focused

- `python scripts/assert_cargo_test_filter.py pareto-protocol procedure_revision_contract_is_closed`
- `python scripts/assert_cargo_test_filter.py pareto-protocol task_class_revision_is_closed_and_pure`
- `python scripts/assert_cargo_test_filter.py pareto-kernel verified_package_binds_complete_review_subject`
- `python scripts/assert_cargo_test_filter.py pareto-kernel verified_package_independence_matrix`
- `python scripts/assert_cargo_test_filter.py pareto-kernel verified_review_decision_default_deny`
- `python scripts/assert_cargo_test_filter.py pareto-kernel verified_registry_is_retained_and_kernel_owned`
- `python scripts/assert_cargo_test_filter.py pareto-kernel verified_procedure_admission_is_exact_and_opaque`
- `python scripts/assert_cargo_test_filter.py pareto-kernel verified_procedure_admission_negative_matrix`
- `python scripts/assert_cargo_test_filter.py pareto-kernel verified_procedure_admission_has_zero_effects`

These filters jointly own REQ-0034 AC-01 through AC-10. AC-11 uses generator/retained-set/scope checks; AC-12 requires every wrapper to report `matched > 0` plus the full offline gates. No Plan, Manifest, Node, execution Evidence or replay test is used to mark REQ-0034 verified.

## Downstream unique owners

| Requirement | Sole owned contract | Required named proof |
|---|---|---|
| REQ-0018 | Plan proposal provenance, closed Procedure instantiation, basic DAG and procedure-capable Manifest | `plan_instantiation_rejects_removed_required_node`, `plan_instantiation_rejects_altered_required_edge`, `plan_instantiation_rejects_effect_node_duplication`, `plan_instantiation_rejects_weaker_schema_or_evidence`, `plan_instantiation_rejects_expanded_capability_budget_retry_or_compensation`, `plan_instantiation_rejects_cross_task_binding`, `pre_manifest_external_requests_have_zero_effects`, `execution_manifest_binds_exact_plan_provenance` |
| REQ-0035 | Node Event/state/lease/checkpoint, recovery and Node-bound Effect | illegal transition, dependency skip, lease race, crash/reopen, late result and wrong Node Effect binding filters |
| REQ-0016 | minimal Evidence admission/coverage and Node/Run success gate | missing, forged, stale, wrong producer/verifier/subject/scope and incomplete coverage filters |
| REQ-0014 | single-Agent procedure executor with no adapter bypass | Fake E2E plus direct Provider/Tool/Workspace/Sandbox access and self-terminal rejection filters |
| REQ-0036 | candidate promotion, registry/default pointer MVCC and procedure rollback | role/quorum reuse, candidate isolation, compare-and-swap conflict, old Manifest stability and rollback selection filters |
| REQ-0015 | non-authoritative Memory | prompt injection, stale/cross-scope retrieval and attempted state/Evidence/terminal mutation filters |
| REQ-0017 | CLI orchestration only | real repository run/resume/inspect/replay E2E with Kernel decisions unchanged |

REQ-0018 must run each listed filter through `python scripts/assert_cargo_test_filter.py pareto-kernel <exact-filter>` before its impacted/core suites. The delete/edge/duplicate/schema/evidence/capability/budget/retry/compensation/cross-task cases must all reject before lifecycle sequence 1, Node lease or Effect. The pre-Manifest case also asserts zero Provider/Tool/Workspace/Sandbox calls, fees, external reads/writes, reservations and Events.

## Topological verification

```text
REQ-0003/0004/0005/0007/0009(done) -> REQ-0034
REQ-0005/0006/0007(done) + REQ-0034 -> REQ-0018
REQ-0009(done) + REQ-0018 -> REQ-0035
REQ-0008(done) + REQ-0011 + REQ-0035 -> REQ-0016
REQ-0010/0011/0012/0013 + REQ-0016/0018/0034/0035 -> REQ-0014
REQ-0006 + REQ-0014/0016/0034/0035 -> REQ-0036
REQ-0006 + REQ-0014/0036 -> REQ-0015
REQ-0014/0015/0016/0036 -> REQ-0017
```

Each arrow points only from a prerequisite that can be verified without consuming the dependent Requirement. A future machine-readable backlog checker should parse the explicit prerequisite cells and reject cycles, missing IDs and backward ownership references.

## Later impacted layers

### Impacted

- Lifecycle Manifest admission and success guard.
- Runtime Control capability/budget/cancellation propagation bound to Node identity.
- Effect Intent/claim/receipt/recovery/reconciliation bound to Node identity.
- Projection/Snapshot/Recorded replay fixed-horizon compatibility.

### Core security and replay

- Full isolation matrix over tenant, user presence/value, Workspace, Run, Task, Plan, Procedure, Node, Agent, Evidence and Effect.
- Memory/model/Planner/adapter attempts to write state, satisfy Evidence, skip dependencies or claim terminal must fail closed.
- Recorded replay has zero executor/writer/reserve/settlement calls; reexecute and simulated create new lineage.
- Run recovery, Workspace recovery, version rollback and external compensation emit distinct facts and cannot erase history.

### Full and milestone

- Fake end-to-end candidate execution, independent approval fixture, verified reuse, rejection of a divergent Plan and rollback of default procedure selection.
- Real Provider smoke remains optional and credential-gated under the redesigned REQ-0010; it is not a route-design gate.
- Quality, token/cost and latency are reported separately against named baselines.

## Traceability owner

SPEC-0010 maps every REQ-0034 acceptance criterion to its own lowest deterministic proof. The table above assigns Plan, Node, Evidence, executor, promotion, Memory and CLI acceptance to exactly one downstream Requirement.
