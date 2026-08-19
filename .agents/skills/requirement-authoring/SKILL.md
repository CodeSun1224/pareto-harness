---
name: requirement-authoring
description: Draft or revise verifiable Pareto Harness requirements. Use for new capabilities, behavior changes, success metrics, scope decisions, or acceptance criteria before technical design or implementation begins.
---

# Author a requirement

1. Copy `.agents/templates/requirement.md` to `docs/requirements/REQ-####-short-title.md` using the next unused ID.
2. Identify the user, observable problem, desired outcome, constraints, and non-goals without prescribing an implementation.
3. Make every acceptance criterion independently testable or inspectable.
4. Express quality, token/cost, and latency guardrails separately; label unmeasured values as targets.
5. Link dependent requirements and any later RFC/ADR instead of duplicating their content.
6. Run `python scripts/check_docs.py`.

Reject vague criteria such as "works well," "is fast," or "uses fewer tokens" unless a baseline, workload, statistic, and threshold are named.
