# Contributing

Pareto Harness uses trunk-based development around `main` with short-lived branches and reviewable commits.

## Before changing the repository

1. Classify the change as lightweight, standard, or high risk using `AGENTS.md`.
2. Find or create the Requirement that defines acceptance.
3. For standard/high work, complete an approved Spec, impact analysis, test matrix, Plan, and Tasks before implementation.
4. Use an RFC for cross-cutting or hard-to-reverse design and an ADR after acceptance.
5. Keep execution state in one Requirement work directory under `.agents/work/active/` and archive it when done.
6. Run layered tests and an independent Review before verification.

## Pull requests

A pull request must identify its risk class, linked Epic/Requirement/Spec/decisions, direct and indirect impacts, evidence, compatibility, permissions, data isolation, regression scope, irrelevant changes, and rollback. Blocker or Major Review findings prevent merge.

Use Conventional Commit prefixes (`feat`, `fix`, `docs`, `refactor`, `test`, `chore`) without treating commit text as a substitute for design records.

Before submission run the commands in `AGENTS.md`. Runtime-specific gates will be added when runtime code is introduced.

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) and [SECURITY.md](SECURITY.md).
