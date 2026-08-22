---
name: impact-analysis
description: Analyze direct and indirect change impact before implementing Pareto Harness requirements. Use when a change may affect callers, APIs, schemas, permissions, data isolation, persistence, replay, concurrency, security, performance, tests, or rollback.
---

# Analyze impact

1. Start from the Requirement, then inspect actual definitions, callers, consumers, tests, schemas, configuration, and documentation. Do not rely only on requested filenames.
2. Record direct changes and indirect effects separately in the Spec impact table.
3. Trace the call path across input validation, authorization, state mutation, external effects, events, projections, evidence, and recovery.
4. Check API/schema defaults, errors, compatibility, migration, snapshot, and replay.
5. Check capability scope, confused-deputy and TOCTOU risks, secrets, paths, network, and process access.
6. Check isolation keys and boundaries for Run, Workspace, Agent, user/tenant, cache, memory, event, artifact, and worktree data.
7. Check concurrency, idempotency, leases, duplicate delivery, cancellation, retry, partial success, and late results.
8. Identify performance, token, cost, latency, storage, dependency, operational, documentation, and rollback impact.
9. Cite evidence: symbols, paths, searches, schemas, tests, or commands. Mark unknowns and block implementation when an unknown changes the design.
10. Feed every identified risk into `test-planning` and specialist review triggers.

Use `high` risk for permissions, sandbox, isolation, events/schemas, persistence, concurrency, replay, secrets, or promotion even when the code diff is small.
