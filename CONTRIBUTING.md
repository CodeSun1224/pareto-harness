# Contributing

Pareto Harness uses trunk-based development around `main` with short-lived branches and reviewable commits.

## Before changing the repository

1. Find or create the Requirement that defines acceptance.
2. Use an RFC for cross-cutting or hard-to-reverse design.
3. Create an ADR after a design decision is accepted.
4. Keep temporary plans in `.agents/work/active/` and archive them when done.

## Pull requests

A pull request must identify linked decisions, risks, evidence, benchmark impact, compatibility, and rollback. Use Conventional Commit prefixes (`feat`, `fix`, `docs`, `refactor`, `test`, `chore`) without treating commit text as a substitute for design records.

Before submission run `python scripts/check_docs.py` and `git diff --check`. Runtime-specific gates will be added when runtime code is introduced.

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) and [SECURITY.md](SECURITY.md).
