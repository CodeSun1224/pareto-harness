# Pareto Harness Agent Guide

This file contains the repository's current operating rules. Durable product and architecture facts live under `docs/`; implementation status lives in `docs/status.md`.

## Mission

Turn evidence-backed successful task runs into reusable immutable verified procedures, then improve strategies on a quality, token/cost, and latency Pareto frontier without weakening the trusted kernel.

## Stable Kernel Baseline

REQ-0003 through REQ-0009 are the Stable Kernel Baseline: versioned protocol and JSON Schema, SQLite Event Store, Run/Task lifecycle and Manifest, projection/snapshot/recorded replay, capability/budget/cancellation/timeout, governed hooks, and Effect Intent/Receipt/recovery/reconciliation.

Do not modify their runtime semantics or code unless an active Requirement explicitly changes a kernel contract or demonstrates an invariant violation. Documentation may update references and status without reopening the baseline.

## Start a task

1. Read `README.md`, `docs/status.md`, and `docs/index.md`.
2. Inspect existing changes and preserve unrelated user work.
3. Read only the Requirement, Spec, ADR/RFC, and skill relevant to the change.
4. Prefer the smallest runnable vertical slice; do not scaffold future layers.

## Change workflow

- `lightweight`: documentation, governance, templates, checks, refactors, and fixes that do not change product/runtime contracts. Record scope in the change and run focused checks; no new Requirement, Spec, work directory, or Review record is required.
- `product`: observable product or runtime behavior. Require an accepted Requirement, an approved Spec, a short Plan, mapped tests, and review.
- `kernel-contract`: authority, permissions, isolation, events/schemas, persistence, replay, concurrency, secrets, or promotion. Add explicit negative tests and specialist review.

An approved Spec freezes when implementation starts. After that point, implementation and review must be judged against the frozen contract. A discovered requirement gap returns the work to design through an explicit Spec amendment; a reviewer may not invent new acceptance criteria inside a finding.

Historical records remain valid snapshots. Do not rewrite them only to match the latest workflow wording.

## Review

- Review an exact implementation commit against the frozen Requirement and Spec.
- Keep `implementation_commit`, `reviewed_commit`, and `review_record_commit` distinct. A Review record or its own commit is never evidence that the implementation works.
- `Blocker` and `Major` findings block approval. `Minor` and `Note` findings never block and may remain open or be accepted with stated risk.
- Remediation changes implementation/tests/evidence, not the Reviewer's original finding text.
- Re-review focuses on the remediation diff plus affected regression evidence; it does not restart an unbounded full review or add unrelated requirements.
- After two remediation rounds, any new or still-open Major finding classifies the work as `DESIGN_NOT_CONVERGED` and returns it to design instead of continuing the review loop.
- Use a fresh reviewer when practical. If not, record `independence: self-review`; never claim independence that did not occur.

## Runtime invariants

- The kernel alone advances authoritative state and admits identity, capability, budget, evidence, effects, replay, recovery, and promotion decisions.
- Models, planners, memory, providers, tools, workers, and plugins propose actions or return observations; they cannot self-authorize or self-declare completion.
- Every Run pins behavior-affecting versions in its Manifest.
- Externally visible effects use Intent/Receipt and explicit recovery/reconciliation semantics.
- Optimizations require reproducible evidence against a named baseline, with quality, token/cost, and latency reported separately.

## Validation

For documentation-only changes, run:

```text
python -B -m unittest discover -s scripts/tests -p "test_*.py"
python -B scripts/check_docs.py
git diff --check
git status --short
```

For runtime changes, add the focused and impacted tests named by the Plan, then run the relevant Rust gates. Run the full workspace, schema-generation, provider, or performance suites only when the changed scope can affect them. Never weaken a gate to make a change pass.

## Writing

- Chinese is authoritative for core design documents; concise English summaries are optional.
- Distinguish implemented fact, evidence, inference, hypothesis, and target.
- Link to one source of truth instead of duplicating status or procedure text.
- Keep governance proportional to risk and keep active work records short.
