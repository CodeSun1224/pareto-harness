---
name: postmortem-authoring
description: Write blameless Pareto Harness postmortems for escaped or systemic failures. Use when an incident affected users, corrupted evidence or state, bypassed safety, caused material regression, or exposed a missing organizational guardrail.
---

# Author a postmortem

1. Preserve primary evidence before interpretation.
2. Copy `.agents/templates/postmortem.md` to `docs/postmortems/PM-####-short-title.md`.
3. Build a timestamped timeline and quantify impact, detection delay, and recovery.
4. Separate trigger, root cause, contributing conditions, and failed or missing defenses.
5. Avoid attributing failure to an individual; identify system conditions that made the action unsafe.
6. Assign every corrective action an owner, priority, due condition, and verifiable completion signal.
7. Link Fix, Requirement, RFC, and ADR records; do not duplicate their details.
8. Review whether the trusted kernel constitution or completion gates need strengthening.
