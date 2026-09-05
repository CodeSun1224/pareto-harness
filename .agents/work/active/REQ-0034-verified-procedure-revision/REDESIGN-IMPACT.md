# REQ-0034 Route Redesign Impact

## Baseline and classification

- Trusted baseline: `origin/main` exact `e7a939cad71a85ada97c3b60d61ba5c024d85ab9`.
- Classification: high-risk requirement and architecture redesign; no runtime implementation is authorized in this phase.
- Archived failed attempt: `archive-req0010-adapter-first-20260905` exact `fea9080fcc5529f4bdc1edf10e0bb4c5fc19f0cd`; it is not an implementation source branch.
- Current code fact: `plan_revision` is only an optional Manifest identity checked for equality. No Plan/Task DAG, Procedure, Node state machine, execution Evidence Gate, Provider, Tool, Workspace/Sandbox or CLI exists on this baseline.

## Direct document impact

- Product: `README.md`, `docs/index.md`, project charter and capabilities.
- Architecture: overview, kernel constitution, version/event model and technology selection.
- Roadmap: roadmap, backlog, EPIC-0003 through EPIC-0006, and new EPIC-0007.
- New durable contracts: REQ-0034, SPEC-0010 and RFC-0013.
- Work evidence: this directory only; formal Review remains Reviewer-owned.

## Indirect runtime impact for later Requirements

- Run Manifest gains forward-only exact Verified Procedure and Plan pins.
- PlanRevision/Task DAG moves before the single-Agent executor and out of the Multi-Agent ownership boundary.
- Node lifecycle/checkpoint becomes a Kernel contract before model/tool execution.
- Minimal Evidence Gate becomes a prerequisite for the first Agent executor; full Evidence Graph remains later.
- Procedure promotion/default rollback is separated from Behavior evolution and from recovery/compensation.
- Provider, Tool, Workspace and Sandbox requests must be Node-bound; any direct adapter path is a Blocker.

REQ-0034 itself ends at pure retained registry admission and returns only an opaque non-executable token. REQ-0018 uniquely owns Plan conformance, the no-external-I/O bootstrap and procedure-capable Manifest; REQ-0035 owns Node authority; REQ-0016 owns execution Evidence; REQ-0014 owns Agent execution; REQ-0036 owns promotion/default selection. None is an acceptance dependency of REQ-0034.

## Evidence trail

- `rg` under `crates/` found `plan_revision` only in Manifest/lifecycle/tests and found no `ProcedureRevision`, Task DAG or Node state implementation.
- `EvidenceRecord` and `GateDecisionV1` exist as protocol/hook types, but no execution Evidence service or completion coverage fold exists.
- Behavior exists as a generic revision role; no Behavior promotion/canary runtime exists.
- REQ-0009 implements Effect recovery/reconciliation and recorded replay boundaries that future procedure execution must reuse rather than bypass.

## Risk responses

- Identity/API: forward major only; retain every old SchemaSet and reader.
- Permissions: REQ-0034 uses Kernel-only registry/pure opaque admission; REQ-0035 later owns Node-scoped opaque leases.
- Isolation: bind tenant/user/workspace/run/task/plan/procedure/node/agent/evidence/effect exact lineage.
- Persistence/replay: append-only events, explicit inclusive horizons, no hidden mutable workflow status table.
- Concurrency: writer-lock revalidation for node claims; MVCC for promotion pointers.
- Security: Memory/model/Planner/adapter remain non-authoritative; zero external effect on failed admission.
- Rollback: separate version selection rollback from Run/Workspace recovery and Effect compensation.
- Scope: no runtime, schema, dependency or archived Provider cherry-pick in the route-design candidate.
