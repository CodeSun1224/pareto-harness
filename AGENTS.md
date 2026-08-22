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

## Spec-driven delivery

Classify every change before editing:

- `lightweight`: spelling, comments, links, or behavior-neutral metadata. Record impact and run basic checks; a formal Review record is optional.
- `standard`: runtime behavior, tests, tools, public documentation structure, or automation. Require Requirement, Spec, Plan, Tasks, layered tests, and independent Review.
- `high`: permissions, sandboxing, data isolation, events/schemas, persistence, concurrency, replay, secrets, or promotion. Add the relevant specialist review and negative tests.

For standard and high-risk work, follow this state path:

```text
proposed → impact-analyzed → specified → approved → planned
→ implementing → reviewing → verified → done
```

Before implementation, use `impact-analysis`, complete the Spec impact matrix, map every acceptance criterion to a test, and create the Requirement work directory. Do not infer impact only from the files requested by the user; inspect callers, consumers, schemas, permissions, isolation boundaries, persistence, and regression surfaces.

## Architectural constitution

- Keep event integrity, version identity, state transitions, permissions, budgets, cancellation, replay, MVCC, evidence admission, and promote/rollback protocols in the trusted kernel.
- Put planner, context selection, model routing, tool ranking, retry, evaluator, and memory policies behind versioned strategy interfaces.
- Plugins may request capabilities; they may not bypass the kernel or mutate authoritative state directly.
- Every externally visible effect must be represented by an event or an explicitly documented non-replayable boundary.
- Every run must pin task, behavior, workspace, environment, model, tool, and schema versions in a run manifest.
- Do not claim an optimization without reproducible evidence against a named baseline.

## One home for each fact

- Epic: roadmap outcome and ordered Requirement set.
- Requirement: desired behavior and acceptance criteria.
- Spec: approved behavior contract, impact analysis, and test traceability.
- RFC: proposed significant design.
- ADR: accepted durable decision and rationale.
- Fix: defect reproduction, root cause, repair, and regression proof.
- Postmortem: escaped/systemic failure, timeline, and guardrails.
- Review: independent findings, evidence, and approval state.
- `.agents/work`: Plan, Tasks, Handoff, and test evidence for active execution; never the sole source of durable product truth.

Use stable IDs (`EPIC-####`, `REQ-####`, `SPEC-####`, `RFC-####`, `ADR-####`, `FIX-####`, `PM-####`, `REVIEW-####`). Change `status` metadata rather than moving a formal document between lifecycle folders.

## Change workflow

- Non-trivial product behavior requires an accepted Requirement.
- Every standard/high Requirement requires an approved Spec, impact analysis, test matrix, Plan, Tasks, validation evidence, and independent Review.
- Cross-cutting or hard-to-reverse design requires an RFC and, once accepted, an ADR.
- Bug fixes require a Fix document unless the change is self-evident and local.
- Update architecture and benchmark documents in the same change when contracts or metrics change.
- Record assumptions, rejected alternatives, failure modes, rollback, and validation evidence.
- Prefer a small vertical slice over empty package scaffolding.

## Layered testing

- `Focused`: changed behavior and minimal reproduction.
- `Impacted`: direct and indirect callers/consumers identified by impact analysis.
- `Core`: kernel invariants, permissions, isolation, event/replay, and critical CLI flows.
- `Full`: milestone and release suite, including real-provider and performance runs where applicable.

Within those scopes select the appropriate static, unit, component/contract, integration, E2E, replay/compatibility, security/isolation, and performance tests. Every Plan must name concrete commands; “run relevant tests” is not sufficient.

## Independent review gate

- Run `code-review` for every standard/high Requirement after implementation and tests.
- Prefer a fresh Agent/session. Give the reviewer the Requirement, Spec, RFC/ADR, diff, and test evidence—not the implementer's conclusions.
- Review data isolation, API/schema compatibility, permissions, concurrency, regression scope, irrelevant changes, dependency growth, rollback, and quality/cost/latency.
- `Blocker` and `Major` findings must be closed and re-reviewed before verification. The implementing Agent may not self-close them.
- If independent execution is unavailable, record the review as non-independent; do not represent it as an independent approval.

## Completion gates

Run:

```text
python -m unittest discover -s scripts/tests -p "test_*.py"
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
