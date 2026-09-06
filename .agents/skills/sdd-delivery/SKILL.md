---
name: sdd-delivery
description: Deliver product or kernel-contract changes through a proportional spec-driven workflow. Do not use for documentation-only governance or behavior-neutral maintenance.
---

# Deliver with SDD

1. Read `AGENTS.md`; use this workflow only for `product` or `kernel-contract` changes.
2. Use an accepted Requirement with independently verifiable acceptance criteria.
3. Approve the Spec, map acceptance criteria to tests, and freeze it before implementation starts.
4. Keep Plan, Tasks, Handoff, and validation evidence as short execution aids, not duplicate product truth.
5. Implement the smallest runnable vertical slice and run focused/impacted tests.
6. Review the exact implementation commit. Resolve Blocker/Major findings; Minor/Note never block.
7. Re-review the remediation diff and affected evidence. After two non-converging remediation rounds, classify `DESIGN_NOT_CONVERGED` and return to design.
8. Verify only when the frozen acceptance criteria have evidence and no Blocker/Major remains.

Do not create a Requirement or RFC for behavior-neutral governance simplification. Do not present self-review or a Review record commit as implementation evidence.
