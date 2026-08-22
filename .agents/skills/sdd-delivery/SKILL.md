---
name: sdd-delivery
description: Deliver Pareto Harness changes through the complete spec-driven workflow. Use for any standard or high-risk requirement that must progress from impact analysis and specification through planning, implementation, layered testing, independent review, and evidence-backed verification.
---

# Deliver with SDD

1. Read `AGENTS.md`; classify the change as lightweight, standard, or high risk.
2. Find or create an Epic and Requirement with independently verifiable acceptance criteria.
3. Run `impact-analysis`; create and approve a Spec before editing behavior.
4. Run `test-planning`; map every acceptance criterion to concrete tests.
5. Create `.agents/work/active/REQ-####-topic/PLAN.md`, `TASKS.md`, and `HANDOFF.md`.
6. Implement tasks as small vertical slices. Update Tasks and Handoff after each material step.
7. Run Focused tests first, then Impacted and mandatory Core gates. Record exact results in `VALIDATION.md`.
8. Run `code-review` with a fresh reviewer when available. Resolve Blocker/Major findings and request re-review.
9. Mark the Requirement `verified` only after all acceptance criteria have evidence; mark `done` only after documentation and work records are finalized.
10. Archive the work directory and run repository completion gates.

Do not use an RFC for every requirement. Add one only for cross-cutting, security-sensitive, public-contract, or hard-to-reverse decisions. Do not present self-review as independent review.
