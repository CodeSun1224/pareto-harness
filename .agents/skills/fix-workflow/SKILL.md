---
name: fix-workflow
description: Diagnose, repair, and document Pareto Harness defects and regressions. Use when observed behavior violates a requirement, replay diverges, evidence is wrong, or a quality, cost, latency, security, or compatibility regression appears.
---

# Fix a defect

1. Preserve the failing run manifest, event sequence, inputs, versions, and observed output.
2. Minimize the reproduction and identify the first violated invariant; do not implement a repair during diagnosis unless requested.
3. Copy `.agents/templates/fix.md` to `docs/fixes/FIX-####-short-title.md` for non-trivial defects.
4. Explain the causal root, not only the failing line.
5. Add a regression test that fails before the repair and passes after it.
6. Evaluate replay compatibility, stored-data migration, security, cost, and latency impact.
7. Record rollback and validate all repository gates.

Escalate an escaped or systemic failure to a Postmortem; keep the Fix focused on the defect and proof.
