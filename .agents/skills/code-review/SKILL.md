---
name: code-review
description: Perform an independent, evidence-first review of Pareto Harness requirements and diffs. Use after implementation, before verification, or for focused re-review of correctness, data isolation, API compatibility, permissions, regressions, concurrency, tests, and unrelated changes.
---

# Review independently

1. Prefer a fresh Agent/session and use `.agents/agents/code-reviewer.md`. If that is impossible, set `independence: self-review`.
2. Read the frozen Requirement/Spec, relevant RFC/ADR, exact implementation commit, changed source, and raw validation evidence. Do not rely on the implementer's conclusions or the Review record itself.
3. Verify every frozen acceptance criterion and trace direct callers and indirect consumers. Do not invent new requirements during review.
4. Review API/schema/event/snapshot/replay compatibility and migration.
5. Review capability, permissions, data isolation, secrets, paths, network, prompt injection, and confused-deputy risks.
6. Review errors, cancellation, timeout, retry, idempotency, partial success, concurrency, leases, and late results.
7. Review Focused, Impacted, Core, E2E, security, and performance test adequacy.
8. Identify unrelated changes, dependency growth, dead code, duplication, and abstractions not required by the Spec.
9. Record `implementation_commit`, `reviewed_commit`, and `review_record_commit` separately. The first two identify the implementation subject; the last identifies where the review was stored and is never correctness evidence.
10. Approve when no open Blocker or Major remains. Minor/Note never block.
11. On re-review, inspect only the remediation diff plus affected regression evidence. After round two, any new or open Major means `DESIGN_NOT_CONVERGED` and the work returns to design.

Remain read-only during review. Remediation is a separate implementation task.
