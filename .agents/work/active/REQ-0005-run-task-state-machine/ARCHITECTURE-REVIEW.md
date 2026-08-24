# REQ-0005 Architecture Review

Reviewer independence: self-review in the current design session using the repository `architecture-review` skill. This is a specialist design review, not the independent post-implementation code review required before verification.

Scope: REQ-0005, SPEC-0004, RFC-0004, accepted ADR-0001/0003/0004, REQ-0003 protocol public API, REQ-0004 Event Store implementation/tests, and downstream REQ-0006/0007/0018 boundaries. No REQ-0005 Runtime code existed during review.

## Verdict

Approved for design entry: 0 open Blocker, 0 open Major. RFC-0004 may be accepted, ADR-0005 recorded, and SPEC-0004 approved. Implementation must stop and return to design if it needs a mutable authoritative state/Manifest table, lock-external state validation, public raw transaction API, implicit multi-Task cascade, broader actor permissions, or different state/event semantics.

## Findings

| ID | Severity | Violated invariant and impact | Testable correction | Status |
|---|---|---|---|---|
| AR-F-001 | Major | A separate Manifest/state table plus events would create two authoritative facts and permit partial or divergent writes, violating event integrity and replay reconstruction | Put the complete closed Manifest in sequence-1 `RunCreated`; derive all state by fold; real SQLite rollback/fresh-connection tests prove no orphan or second state authority | closed in proposal |
| AR-F-002 | Major | Established read requiring caller-supplied current SchemaSet/limits creates a bootstrap cycle and permits historical identity substitution | Load sequence 1 by exact isolation/derived stream, obtain persisted SchemaSet/limits from the row, exact-resolve retained registry, then cross-check nested Manifest; missing/wrong/alternate reader negative tests | closed in proposal |
| AR-F-003 | Major | Reading/folding with REQ-0004 pagination before opening append transaction creates TOCTOU; global event ID before aggregate authority can leak cross-scope results, while checking it before full fold can hide corrupt history | Resolve persisted Manifest and owner authority first, then use a crate-private `BEGIN IMMEDIATE` transaction for exact read/fold, same-aggregate idempotency, guards and one append; cross-aggregate reuse gets only generic conflict; add two-pool, corrupt-history retry and cross-scope same-ID tests | closed in proposal |
| AR-F-004 | Major | Implicit Run cancel/fail cascade would require multi-entity event batch idempotency and could leave partially terminal children | First version has no cascade: Task transitions are explicit and retryable; Run terminal guards require all Tasks terminal with deterministic failure/cancel/success precedence | closed in proposal |
| AR-F-005 | Major | Treating existing private `KernelAuthority` or any authenticated actor as full authorization would overclaim REQ-0007 and enable confused deputy paths | Separate create/established lifecycle authorities; exact owner actor only; payload/ValidatedEvent cannot construct authority; compile-fail and full scope/actor matrix | closed in proposal |
| AR-F-006 | Minor | Event-only O(n) fold can grow transition latency before Snapshot exists | Record fold observations by event count; do not set an unsupported gain target; use the pure fold as REQ-0006 oracle and revisit only on measured pressure | accepted |
| AR-F-007 | Note | Rejected commands are not persisted, so they are not replayable audit evidence | Document rejection as a no-state-effect command-response boundary; REQ-0007/0009 may add separate audit event types without changing lifecycle fold | accepted |

## Constitution trace

1. Request → approval → event → state → recovery is fully traced; external effects/evidence are explicitly outside this slice.
2. Lifecycle authority, transition guards and Event Store transaction remain trusted-kernel private; strategies/plugins only request.
3. Run Manifest pins the closed eight revision roles plus SchemaSet, budget, limits, recording policy and execution lineage; persisted values are never default-filled.
4. `occurred_at` and operation ID are command inputs fixed before retry; external boundary outcomes remain for REQ-0007/0009 and Replay inventory for REQ-0006.
5. Cancellation/late results, busy/rollback, commit-response uncertainty, hierarchy failure, writer competition and rollback are explicit and test-mapped.
6. Promotion is not in scope and no optimization claim is made.
7. Quality, token/cost and latency are separately stated; only quality has correctness gates, token/cost is N/A, latency is an observation baseline.

## Required post-implementation independent review

An independent Agent/new session must review the exact implementation revision, Requirement, Spec, RFC/ADR, diff and validation evidence. It must focus on state-machine completeness, transaction atomicity, concurrency/idempotency, terminal/late behavior, data isolation, protocol/old-Schema compatibility, unrelated changes and whether REQ-0006 can implement Projection/Replay without reinterpreting history. Blocker/Major findings require independent closure and re-review.
