# REQ-0007 Architecture and Security Self-review

Independence: self-review. This record approves design entry only and is not an independent implementation review.

# Findings

| ID | Severity | Violated invariant / risk | Testable correction | Status |
|---|---|---|---|---|
| AR-F-001 | Major | Manifest owner could become an implicit operation bypass, defeating default deny | SPEC/RFC require owner grant for protected operation；only explicit management commands use owner authority；default-deny test covers owner without grant | closed |
| AR-F-002 | Major | Child grant could widen Task/resource/operation/time/usage/depth or use stale parent state | Kernel proves every subset dimension and revalidates full parent chain/revocation/expiry inside reserve transaction；widening matrix required | closed |
| AR-F-003 | Major | Delegated requester cannot safely become EventEnvelope actor under existing v2 reader, inviting actor bypass or hidden DB change | Envelope signer remains persisted Manifest owner；requester/subject are closed payload fields exact-checked against principal；future multi-signer requires new migration/RFC | closed |
| AR-F-004 | Major | Multi-scope reserve outside one lock, unchecked arithmetic or Provider usage trust can oversell/underreport | Single `BEGIN IMMEDIATE` all-account reserve, checked u64 equations, Provider observation non-authoritative, unknown consumes full reserve；two-pool/model tests | closed |
| AR-F-005 | Major | Cancel request, acknowledgement and lifecycle cancelled could be conflated；completion/timeout could create double terminal | Three facts remain distinct；one terminal state machine；deadline equality timeout；lock-serialized race model；no implicit lifecycle cascade | closed |
| AR-F-006 | Major | Persisting or comparing monotonic ticks across restart, or using wall only, breaks deterministic deadline | Live monotonic lease is process-local；Event persists absolute UTC；restart discards old tick and establishes new lease from trusted wall；FakeClock only/no sleep | closed |
| AR-F-007 | Major | Late callback could mutate outcome/budget/effect or leak sensitive result data | Exact retry/mutation rules；new late callback writes digest/redaction audit only；budget/outcome/effect counters invariant tests | closed |
| AR-F-008 | Major | Recorded replay could share live dispatch/reserve code or current reducer and repeat effects/accounting | Separate read/reducer API has no executor/append type；exact retained source/output reducer；before/after counter/event/account assertions | closed |

# Constitutional trace

1. Request → persisted lifecycle/control authority → Capability decision is exact and default-deny.
2. Approved request → atomic budget reservation → one authoritative Event; Fake Operation occurs only after commit.
3. Callback/cancel/timeout → lock-time refold → one settlement/audit Event → Projection/recovery.
4. Every behavior-affecting Schema/reducer/budget/clock policy is versioned and persisted; old readers remain.
5. Run/Task/Actor/Workspace/user/tenant/control stream and business IDs are exact isolation keys or aggregate-local references.
6. Cancellation, timeout, budget exhaustion, partial/unknown usage, crash and late results have distinct fail-closed behavior.
7. Recorded replay has no Operation/Effect reachability and does not append or alter Q/C/L facts.

# Verdict

Approved for implementation planning with 0 open Blocker and 0 open Major. A fresh independent Agent must review the exact implementation commit and raw validation evidence; this self-review cannot close implementation findings.
