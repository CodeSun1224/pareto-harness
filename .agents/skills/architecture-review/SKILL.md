---
name: architecture-review
description: Review Pareto Harness designs and changes against trusted-kernel boundaries, version identity, event integrity, evidence, replay, permissions, and controlled evolution. Use during RFC review, architecture changes, or cross-module code review.
---

# Review architecture

Check, in order:

1. Trace each effect from request through capability check, event, state transition, evidence, and recovery.
2. Verify kernel invariants cannot be replaced or bypassed by a strategy or plugin.
3. Verify every run pins task, behavior, workspace, environment, model, tool, and schema versions.
4. Identify nondeterministic inputs and define how replay records or bounds them.
5. Check cancellation, timeout, budget exhaustion, partial failure, concurrency conflict, and rollback.
6. Confirm promotion requires historical replay and canary evidence rather than self-reported improvement.
7. Check that quality, cost, and latency remain separately observable.

Report findings by severity with a concrete violated invariant and a testable correction. Do not approve architecture based only on diagram plausibility.
