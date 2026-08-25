---
id: REVIEW-0002
title: REQ-0003 版本化协议类型和 JSON Schema 独立代码评审
status: approved
owners: [independent-reviewer]
created: 2026-08-23
updated: 2026-08-25
links: [REQ-0003, SPEC-0002, RFC-0002, ADR-0003]
independence: independent
reviewed_revision: 1d271549c2607f9c00377bdaa0fa999a131dafe3
open_blockers: 0
open_majors: 0
---

# Verdict

Approved for the REQ-0003 independent code-review gate. Reviewer independently inspected the Requirement, approved Spec, accepted RFC/ADR, exact revisions and diffs, public API boundaries, generated and retained SchemaSets, tests, raw local evidence, and GitHub Actions run `32642574089`. All Blocker and Major findings are closed at exact revision `6447c37610a0fd3a6ce3b8e3154b0653650f77ef`.

This Review does not itself change the Requirement lifecycle. Event Store, Replay executor, persistent migration, and Runtime E2E remain later-Requirement work. The latency measurements are reproducible observations only; they are not thresholds or evidence of a performance optimization.

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Blocker | SchemaSet admission and trusted context | Bootstrap/self-signing or forgeable context could admit unauthorized contracts. | Pinned meta-schema/root, exact member/manifest verification, capability-safe public authorizer, malicious and external-consumer fixtures. | closed |
| F-002 | Blocker | Event admission | Untyped or misbound payloads could enter an authoritative event path. | Exact EventTypeBinding, supported typed decoder registry, payload Schema/digest/context negatives, typed dispatch. | closed |
| F-003 | Blocker | Compatibility identity | Malformed or same-version mutations could be accepted as compatible. | Canonical identity before equality, strictly newer minor, protected graph and breaking mutation fixtures. | closed |
| F-004 | Blocker | Review revision | An uncommitted snapshot could not receive formal approval. | Exact committed revision and focused re-review. | closed |
| F-005 | Major | `ProtocolRecord` admission | Generic validation could bypass trusted Event/Run/Evidence semantics. | Seal the trait, remove context-sensitive records from generic admission, require dedicated boundaries, and preserve compile-fail/semantic negatives. | closed |
| F-006 | Major | Digest and revision identity | Incorrect domains, absent-field handling, or incomplete vectors could break replay identity. | Frozen preimages, identity validation, and fixed payload/revision/raw/artifact vectors. | closed |
| F-007 | Major | Boundary inventory/reconciliation and replay | Unsupported SchemaRefs, wrong source scope/run/policy, or wrong inventory could forge lineage. | Exact top/hash Schema and SchemaSet binding; validated source Manifest/scope/policy; Validated-inventory-only reconciliation/replay; mutation and lineage matrix. | closed |
| F-008 | Major | Protocol limits | Expensive Schema/digest work or typed boundaries could precede size rejection. | Exact limits profile; limits-before-Schema/digest; Event/Run/Evidence record and Event payload N/N+1 vectors. | closed |
| F-009 | Major | SchemaSet publication | Retained sets could drift or concurrent publication could expose incomplete content. | Immutable content-addressed publication, verifier over every retained set, exact file/digest checks, concurrency/stale-staging/conflict tests. | closed |
| F-010 | Major | Cross-platform and completion evidence | Missing platform or documentation evidence could overstate AC-10 and readiness. | Exact Windows/Ubuntu/macOS successful matrix, complete local gates, applicable official JCS vectors, named negative suites, and reproducible observation baseline. | closed |
| F-011 | Minor | Delivery documentation | Stale Plan/Handoff/Validation wording could misstate implementation status. | Reconcile commands, counts, handoff, and freshness records. | closed |

# Acceptance trace

- AC-01/AC-03: closed Rust types, explicit Schema identity, exact top-level admission, structured errors, Schema/Serde and malicious-input fixtures.
- AC-02: deterministic generated Schemas, manifest/ref identity, byte/file-set golden, and verification of every retained content-addressed SchemaSet.
- AC-04: RFC 8785 protocol subset plus fixed, domain-separated payload, revision, raw artifact, and manifest vectors.
- AC-05: conservative old-writer/new-reader checker with canonical identity, generated-Schema positive fixture, and fail-closed mutation matrix.
- AC-06: exact envelope/payload Schema, typed decoder binding, digest, sequence, scope, actor, stream, Run, and trusted-context validation.
- AC-07: complete Run pins and execution modes; finalized inventory/reconciliation identity; exact source scope/run/SchemaSet/policy and replay inventory binding.
- AC-08: closed Evidence contract, exact admitted Schema/scope/limits, canonical timestamp, required text, verdict and digest fields.
- AC-09: opaque validated/trusted values, bootstrap and evolved-set admission, exact isolation/context negatives, and no Runtime/network/file/secret dependency in validation.
- AC-10: exact three-platform matrix and local completion evidence pass; protocol crate remains independent of Runtime, database, and Provider SDKs.

# Compatibility, permission, and isolation review

Schema identity includes type, major, minor, and digest; same-version mutation and unsupported protected-graph changes fail closed. Published SchemaSets are immutable and retained readers are verified rather than overwritten.

Schema validation is not authorization. Admission authority, trusted context, and validated wrappers have opaque construction boundaries; Event, Run, Evidence, inventory, reconciliation, and replay compare exact trusted scope and pinned identities. No new database, Provider, Runtime, network resolver, process, clock, filesystem-read, or secret access was introduced into protocol validation.

# Regression and test review

Reviewer-observed local evidence includes fmt; locked/offline all-target/all-feature clippy and workspace tests; 9 unit and 17 contract tests; deterministic Schema generation and unchanged `schemas/`; 18 Python governance tests; document validation; and whitespace checks. The ordinary suite intentionally ignores one release observation baseline, which was explicitly run and reported separately without a pass threshold.

Reviewer directly queried GitHub Actions run `32642574089`, exact head SHA `6447c37610a0fd3a6ce3b8e3154b0653650f77ef`, status `completed`, conclusion `success`:

- Windows job `97201827527`: completed/success.
- Ubuntu job `97201827618`: completed/success.
- macOS job `97201827652`: completed/success.

On all three jobs, checkout/toolchain setup, locked dependency fetch, fmt, clippy, workspace tests, Schema generation, Schema unchanged check, digest golden, governance tests, document validation, whitespace, and completion steps succeeded.

# Scope and unrelated changes

The implementation is a protocol-only vertical slice. No Event Store, state machine, persistent Replay executor, Provider integration, CLI Runtime, or database layer was added. Dependency growth is limited to the reviewed protocol/Schema/canonicalization stack and does not reverse the dependency direction.

Remaining Notes/limitations:

- Runtime persistence, migration, replay execution, and end-to-end state-transition evidence are explicitly outside REQ-0003 and must be supplied by later Requirements.
- The JCS implementation intentionally supports the frozen protocol subset and rejects floating-point and unsafe integer values; the applicable official property-ordering and literals/string-escaping vectors pass.
- Latency results establish a reproducible local observation baseline only. No quality, cost, or latency optimization is claimed.

# Re-review history

- Initial advisory review found 4 Blocker, 6 Major, and 1 Minor findings.
- Exact re-reviews progressively closed bootstrap admission, typed events, canonical compatibility identity, digest vectors, documentation drift, sealed admission, boundary identity, limits, and immutable publication findings.
- Exact `201e19c` introduced validated source-inventory lineage; exact `175bed4` added recording-policy binding and closed F-007.
- Exact `72914b7` plus Actions run `32642138568` closed F-010 after all three platforms and local completion gates passed.
- Exact `b00928d` and `6447c37` contain only independent review/freshness records; no product or test behavior changed after the last product re-review.
- Exact `e5d1b9b36de61a961b42b0c8148142cb98ae816f` is a lifecycle/archive-only closure: it records REQ-0003 as done, links this Review, archives the completed work directory, and synchronizes final delivery facts. No product, Schema, test, CI, permission, isolation, replay, or compatibility behavior changed; approval and zero open findings are retained at the newer exact revision.
- Exact `5ef949dd084b1e6ae82015f4c66adb8281aebf65` freshness/lifecycle re-review: since `e5d1b9b`, `crates/pareto-protocol/` and `schemas/` are byte-diff unchanged. REQ-0004 adds a separate `pareto-kernel` consumer with SQLite/Tokio dependencies; dependency direction remains Kernel -> protocol and protocol gains no database, Runtime, network, Provider, filesystem-reader, or secret dependency. Its authority, retained SchemaSet reader, compatibility and isolation behavior received independent REVIEW-0003 approval and protocol 9 unit + 17 contract regressions passed. `b7cf277..5ef949d` itself is docs/status/work-archive only. REVIEW-0002 remains approved with 0 open Blocker/Major at the newer exact revision.
- Exact `b5850b76325bbc31825303215224d60c931e27c6` final freshness re-review: REQ-0005's intervening protocol/Event additions were independently reviewed in REVIEW-0004, published a new immutable SchemaSet without changing retained sets, preserved Kernel -> protocol dependency direction, and passed protocol 9 unit + 19 contract tests including exact old-reader substitution negatives. The exact closure diff `675e3f8..b5850b7` changes no `crates/`, Schema bytes, Cargo manifests/lock, dependencies, public protocol API, reader identity, or replay semantics; it is docs/status/work-archive only. REVIEW-0002 remains approved with 0 open Blocker/Major and freshness advances to exact `b5850b7`.
- Exact `5c4f6e7f304c55fb61b6cc7e08d5bbe902b8d82c` substantive freshness re-review: REQ-0006 additively introduces closed Projection/Snapshot/reducer/history records, ten generated member Schemas and immutable content-addressed set `sha256-4ce3872926ce61209fdc5ed48deceeec9703ccfe94ea83be485eb8ef7512ff97`. Independent comparison confirms the three previously retained sets are byte-diff unchanged; protocol remains independent of Kernel/sqlx/Runtime/network/Provider/filesystem readers and Cargo dependencies are unchanged. Reviewer reran 9 protocol unit + 21 contract tests, including retained-set completeness, generation identity and old-writer exact readers; all passed (one observation ignored), and schema generation left the worktree unchanged. Open exact reducer/migration/test findings are owned by REVIEW-0005 and do not weaken REQ-0003's already approved canonicalization, SchemaSet admission, limits, isolation or retained-reader contract. REVIEW-0002 remains approved with 0 open Blocker/Major and freshness advances to exact `5c4f6e7`.
- Exact `1d271549c2607f9c00377bdaa0fa999a131dafe3` substantive freshness re-review: remediation changes Kernel reducer registration/migration tests and REQ-0006 traceability only；`crates/pareto-protocol/`、`schemas/`、Cargo manifests/lock及public protocol API相对`5c4f6e7` byte-diff unchanged。Reviewer独立复跑protocol 9 unit + 21 contract、retained `dae028...` old Run/Snapshot exact reader及Schema generation byte identity；显式source-contract registration仍使用已批准SchemaSet admission和exact digest，不放宽REQ-0003 canonicalization/limits/isolation。REVIEW-0002保持approved、0 open Blocker/Major，freshness前移至exact `1d27154`。
