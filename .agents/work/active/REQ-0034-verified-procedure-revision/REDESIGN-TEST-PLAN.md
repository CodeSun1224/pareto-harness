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

## Future implementation layers

### Focused

- Protocol canonical identity and closed-field tests for Procedure, Verified Procedure, registry and procedure-capable Manifest.
- Kernel default-deny tests for missing/unretained/self-signed/revoked/substituted approval packages.
- Exact Plan/Task/Procedure compatibility tests and zero-write/zero-effect assertions.

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

SPEC-0010 maps every REQ-0034 acceptance criterion to the lowest deterministic proof and identifies downstream REQ-0035/REQ-0016/REQ-0014/REQ-0036 integration gates.
