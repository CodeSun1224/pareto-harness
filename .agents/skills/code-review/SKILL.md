---
name: code-review
description: Perform an independent, evidence-first review of Pareto Harness requirements and diffs. Use after implementation, before verification, or for focused re-review of correctness, data isolation, API compatibility, permissions, regressions, concurrency, tests, and unrelated changes.
---

# Review independently

1. Prefer a fresh Agent/session and use `.agents/agents/code-reviewer.md`. If that is impossible, set `independence: self-review`.
2. Read the Requirement, Spec, relevant RFC/ADR, diff, changed source, and raw validation evidence. Do not rely on the implementer's conclusions.
3. Verify every acceptance criterion and trace direct callers and indirect consumers.
4. Review API/schema/event/snapshot/replay compatibility and migration.
5. Review capability, permissions, data isolation, secrets, paths, network, prompt injection, and confused-deputy risks.
6. Review errors, cancellation, timeout, retry, idempotency, partial success, concurrency, leases, and late results.
7. Review Focused, Impacted, Core, E2E, security, and performance test adequacy.
8. Identify unrelated changes, dependency growth, dead code, duplication, and abstractions not required by the Spec.
9. Review an exact Git commit, then create `docs/reviews/REVIEW-####-topic.md` using the template. Put findings first, label them Blocker, Major, Minor, or Note, and record open counts.
10. Approve only when no open Blocker or Major remains. On re-review, inspect the new diff and evidence rather than accepting a closure statement.

Remain read-only during review. Remediation is a separate implementation task.
