# REQ-0006 Architecture Review

Reviewer independence: self-review in the current design session using the repository `architecture-review` skill. This is a high-risk specialist design review, not the independent post-implementation code review required before verification.

Scope: REQ-0006, SPEC-0005, RFC-0005, accepted ADR-0001/0003/0004/0005/0006, REQ-0003 protocol contracts, REQ-0004 Event Store implementation/tests, REQ-0005 lifecycle implementation/tests, and downstream REQ-0007/0012/0015/0024 boundaries. No REQ-0006 Runtime code existed during review.

## Verdict

Approved for implementation planning: 0 open Blocker, 0 open Major. RFC-0005 may be accepted, ADR-0006 recorded, and SPEC-0005 approved. Implementation must stop and return to design if it needs an external Snapshot import/write API, a mutable authoritative Projection table, a reader/reducer selected by the caller, Event history skipping, live Effect dispatch, a database downgrade, weaker isolation, a different lifecycle state contract, or a generic Projection framework.

## Findings

| ID | Severity | Violated invariant and impact | Testable correction | Status |
|---|---|---|---|---|
| AR-F-001 | Major | Treating Snapshot as authoritative or accepting caller-provided bytes lets cache bypass Event Store protocol validation and creates a second state truth | Snapshot is Kernel-created only from an exact validated range, immutable and fully bound; no import/public SQL; any untrusted candidate full-fallbacks and full fold remains oracle | closed in proposal |
| AR-F-002 | Major | A replay path that reuses command/effect execution could repeat Provider/Tool/file/network/process side effects while claiming determinism | Recorded replay has read/reducer dependencies only, ignores Snapshot, never receives an Effect callback and never appends; simulated rejects before dispatch until fixed fixtures exist; fake effect counter test | closed in proposal |
| AR-F-003 | Major | Building Snapshot outside the append transaction creates a TOCTOU cursor/state tear; mutable Projection writes can partially commit after crash | Create under `BEGIN IMMEDIATE` with full exact read/fold/insert; normal loads use one read transaction horizon; transaction-drop, two-pool race and reopen tests | closed in proposal |
| AR-F-004 | Major | An unversioned reducer or unordered Task map makes identical Event history produce platform/build-dependent state and lets new code reinterpret old Snapshot | Exact reducer kind/version/contract digest and output Schema; pure reducer and canonical TaskId order; digest golden and incompatible reducer fallback tests | closed in proposal |
| AR-F-005 | Major | Skipping unknown/illegal events, sequence gaps or old Schema with no reader hides corrupt history; a compatible-looking current reader can reinterpret it | Every incremental/full/replay Event uses persisted exact SchemaSet/limits; unknown/gap/illegal fails closed; Snapshot cannot mask missing source reader; retained/current-substitution tests | closed in proposal |
| AR-F-006 | Major | Snapshot/cursor/digest IDs used without the full tenant/user/workspace/run/actor/stream/store binding permit cross-scope cache reuse and existence leaks | Bind all isolation fields plus store identity in JSON, indexes, digest and authority; generic unauthorized/not-comparable errors; per-field swap, store-swap and payload-shadow negative matrix | closed in proposal |
| AR-F-007 | Major | An in-place DB downgrade or Event rewrite during v1→v2/rollback breaks append-only history and makes old binaries silently misread data | Atomic forward-only v2 migration adds cache structures only; event bytes preserved; failed migration rollback; old binary fails newer version; rollback retains v2 reader and old Schema/reducers | closed in proposal |
| AR-F-008 | Minor | Snapshot creation performs O(n) validation while holding SQLite's single writer lock and may worsen append latency | Explicit creation only; record 1/10/100/1000-event latency and contention observations; no unsupported optimization claim; revisit on measured pressure | accepted |
| AR-F-009 | Note | Snapshot rejection disposition is not a durable audit event, so it cannot be replayed as operational history | Treat it as an explicit read-response diagnostic; a future evidence/audit Requirement may persist separate facts without changing Projection truth | accepted |

## Constitution trace

1. Request → authority → persisted exact reader → reducer → optional cache/read result → recovery is fully traced. There is intentionally no state transition or external Effect edge.
2. Event validation, reducer registry, Snapshot store and Replay remain trusted-kernel private; strategies/plugins only request results.
3. The sequence-1 Manifest still pins task, behavior, workspace, environment, context, model, tool, kernel, SchemaSet, budget, limits and recording policy; Snapshot duplicates identities only for exact verification and never fills defaults.
4. Recorded replay consumes persisted facts only; Simulated requires fixed revisions and currently rejects before dispatch. Nondeterministic clock/path/timing is excluded from reducer and digest preimages.
5. Busy, cancellation by transaction drop, partial failure, commit uncertainty, concurrent append, corruption, missing reader and rollback all have explicit semantics and tests. Full Capability/Budget timeout behavior remains REQ-0007.
6. Promotion/evolution is out of scope; no self-reported optimization or canary claim is introduced.
7. Quality, Token/cost and latency are separate: correctness has gates, model cost is N/A, storage/test cost and latency are observations only.

## Required post-implementation independent review

A fresh Agent/session must receive the Requirement, Spec, RFC/ADR, exact diff and VALIDATION evidence and apply `code-review`. It must focus on: replay Effect reachability; reducer purity/determinism; Snapshot protocol bypass; cursor/version/digest completeness; concurrency/crash atomicity; tenant/user/workspace/run/actor isolation; Schema/API/DB compatibility; REQ-0003/4/5 regressions; dependency/unrelated changes; rollback; and whether REQ-0007/0012/0015/0024 remain evolvable. Blocker/Major findings may only be closed by the independent reviewer after remediation and focused re-review.
