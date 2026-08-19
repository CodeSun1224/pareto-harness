---
name: rfc-authoring
description: Propose significant Pareto Harness technical designs with interfaces, invariants, alternatives, failure modes, evaluation, migration, and rollback. Use for cross-cutting, security-sensitive, public-contract, or hard-to-reverse changes.
---

# Author an RFC

1. Confirm an accepted Requirement exists.
2. Copy `.agents/templates/rfc.md` to `docs/rfcs/RFC-####-short-title.md`.
3. Describe the data flow and identify which responsibilities belong to the trusted kernel, strategy layer, or extension boundary.
4. Define public types and interfaces only as far as required to remove implementation ambiguity.
5. Analyze concurrency, determinism, effects, permissions, version compatibility, failure recovery, and rollback.
6. Compare at least two viable alternatives, including the status quo.
7. Define evaluation workloads and separate quality, cost, and latency acceptance.
8. Link the resulting ADR after acceptance and run the document checks.
