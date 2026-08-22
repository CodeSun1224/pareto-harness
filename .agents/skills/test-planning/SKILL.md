---
name: test-planning
description: Design layered, risk-based tests and requirement traceability for Pareto Harness changes. Use before implementation, when impact changes, or when selecting focused, impacted, core, E2E, replay, compatibility, security, and performance regression tests.
---

# Plan tests

1. Convert each acceptance criterion and impact risk into at least one observable scenario.
2. Prefer the lowest deterministic layer that proves the behavior, then add cross-boundary tests where integration risk exists.
3. Classify scope:
   - Focused: changed behavior and minimal reproduction.
   - Impacted: direct and indirect callers/consumers.
   - Core: kernel invariants, permissions, isolation, event/replay, and critical CLI flows.
   - Full: milestone/release suite and costly real-provider/performance runs.
4. Select layers: static, unit, component/contract, integration, E2E, replay/compatibility, security/isolation, and performance/Pareto.
5. Include positive, negative, boundary, failure, cancellation, retry, partial-success, concurrency, and migration cases where applicable.
6. Write concrete commands or procedures in the Plan and map them in the Spec test traceability table.
7. Use Fake/recorded providers for every-PR determinism; reserve real-provider tests for scheduled, milestone, or release gates unless the requirement specifically changes the provider.
8. Record actual command, environment, result, artifact, and any skipped test in `VALIDATION.md`.

Do not accept "existing tests pass" without identifying which acceptance criteria and regression risks those tests prove.
