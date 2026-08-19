---
name: project-orientation
description: Navigate Pareto Harness goals, documents, decisions, active work, and validation rules. Use when starting work in this repository, resuming an unfamiliar task, or deciding where a fact or change belongs.
---

# Orient to the project

1. Read `README.md`, `AGENTS.md`, and `docs/index.md`.
2. Read the relevant accepted Requirement and its linked RFC/ADR.
3. Inspect `.agents/work/active/` and `git status --short`; preserve unrelated changes.
4. Classify the requested work as research, requirement, design, implementation, fix, or incident.
5. Load only the documents needed for that class of work.
6. State the governing acceptance criteria and validation commands before editing.

Do not treat chat history or a temporary work plan as durable project truth. If instructions conflict, apply repository `AGENTS.md`, then the closest scoped instruction, then the accepted decision record.
