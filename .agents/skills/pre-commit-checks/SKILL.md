---
name: pre-commit-checks
description: Validate Pareto Harness repository hygiene and completion gates before committing or handing off work. Use after edits, before a commit, or when reviewing whether a task is complete.
---

# Run completion gates

1. Run the focused checks for the changed scope. Documentation-only changes use the four commands in `AGENTS.md`.
2. Run `git diff --check`.
3. Run `git status --short` and classify every changed file as intended, generated, or unrelated.
4. Verify linked Requirement/RFC/ADR/Fix records only when the change affects their contract.
5. Confirm new claims have sources and targets are not described as achieved results.
6. For runtime changes, run the focused/impacted language gates named by the Plan; reserve full suites for affected scope.
7. Report exact commands and results; never claim a check was run when it was inferred.
