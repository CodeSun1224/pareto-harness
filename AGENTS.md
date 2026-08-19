# Pareto Harness Agent Guide

This file is the canonical instruction source for coding agents in this repository.

## Mission

Build a coding-agent harness that improves verified result quality while reducing token cost and latency. Treat quality, cost, and latency as separate dimensions and publish their Pareto frontier.

## Start every task

1. Read `README.md` and `docs/index.md`.
2. Read the accepted requirement and linked RFC/ADR before changing behavior.
3. Check `.agents/work/active/` for an active handoff.
4. Use the narrowest relevant skill from `.agents/skills/`.
5. Inspect existing changes and preserve unrelated user work.

## Architectural constitution

- Keep event integrity, version identity, state transitions, permissions, budgets, cancellation, replay, MVCC, evidence admission, and promote/rollback protocols in the trusted kernel.
- Put planner, context selection, model routing, tool ranking, retry, evaluator, and memory policies behind versioned strategy interfaces.
- Plugins may request capabilities; they may not bypass the kernel or mutate authoritative state directly.
- Every externally visible effect must be represented by an event or an explicitly documented non-replayable boundary.
- Every run must pin task, behavior, workspace, environment, model, tool, and schema versions in a run manifest.
- Do not claim an optimization without reproducible evidence against a named baseline.

## One home for each fact

- Requirement: desired behavior and acceptance criteria.
- RFC: proposed significant design.
- ADR: accepted durable decision and rationale.
- Fix: defect reproduction, root cause, repair, and regression proof.
- Postmortem: escaped/systemic failure, timeline, and guardrails.
- `.agents/work`: temporary execution state, never the sole source of durable truth.

Use stable IDs (`REQ-####`, `RFC-####`, `ADR-####`, `FIX-####`, `PM-####`). Change `status` metadata rather than moving a document between lifecycle folders.

## Change workflow

- Non-trivial product behavior requires an accepted Requirement.
- Cross-cutting or hard-to-reverse design requires an RFC and, once accepted, an ADR.
- Bug fixes require a Fix document unless the change is self-evident and local.
- Update architecture and benchmark documents in the same change when contracts or metrics change.
- Record assumptions, rejected alternatives, failure modes, rollback, and validation evidence.
- Prefer a small vertical slice over empty package scaffolding.

## Completion gates

Run:

```text
python scripts/check_docs.py
git diff --check
git status --short
```

When runtime code exists, add its formatter, linter, unit, contract, replay, and migration checks here. Do not weaken a gate to make a change pass.

## Writing rules

- Chinese is authoritative for core design documents; add concise English summaries where useful.
- Distinguish implemented facts, experimental evidence, inference, hypothesis, and target.
- Prefer direct official documentation, source code, and papers over secondary summaries.
- Add verification date and source URL to time-sensitive research claims.
- Keep agent instructions concise; link to durable documents instead of duplicating them.
