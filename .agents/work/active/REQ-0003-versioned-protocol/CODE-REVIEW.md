# REQ-0003 Independent Code Review

## Initial review

- Reviewer: fresh sub-agent `/root/req0003_code_review`
- Independence: independent, read-only
- Reviewed revision: unavailable; working tree advisory review
- Baseline HEAD: `66fe760ce88b3817b6869ebdc5b509be22e73770`
- Verdict: changes-requested
- Open: 4 Blocker, 6 Major, 1 Minor

| ID | Severity | Summary | Status |
|---|---|---|---|
| F-001 | Blocker | SchemaSet bootstrap self-signing and public-forgeable trusted context | open |
| F-002 | Blocker | Event payload/typed variant and envelope schema are not validated; empty sequence passes | open |
| F-003 | Blocker | Compatibility checker accepts breaking composition and ignores major identity | open |
| F-004 | Blocker | No exact reviewed Git revision | open |
| F-005 | Major | Generated Schema and Serde/semantic contracts diverge, including explicit null | open |
| F-006 | Major | RevisionHashView, ArtifactManifest and fixed digest vectors missing | open |
| F-007 | Major | Replay lineage and BoundaryInventory contracts incomplete | open |
| F-008 | Major | Pinned limits profile is not actually enforced across all paths | open |
| F-009 | Major | Schema publishing is non-atomic and golden check permits stale files | open |
| F-010 | Major | Named, negative, cross-platform and completion evidence incomplete | open |
| F-011 | Minor | Durable docs and active work status drift | open |

Only the independent Reviewer may close Blocker/Major findings after inspecting remediation.

## Focused re-review — 2026-08-22

- Reviewer: `/root/req0003_code_review`
- Independence: independent; source, generated assets, dependency metadata, and raw validation record inspected directly
- Reviewed revision: unavailable; baseline HEAD remains `66fe760ce88b3817b6869ebdc5b509be22e73770` with tracked modifications and untracked implementation
- Verdict: changes-requested; working-tree advisory review only
- Open after re-review: 4 Blocker, 6 Major, 1 Minor
- Test execution: not re-run by the Reviewer to preserve the requested read-only source/evidence review. The supplied Windows result says 16 Rust tests passed, but the current source contains 17 `#[test]` functions, so that result is not exact-snapshot evidence.

### Finding disposition

| ID | Severity | Re-review evidence and remaining impact | Required proof | Status |
|---|---|---|---|---|
| F-001 | Blocker | Partial remediation: `SchemaAdmissionAuthority` and `TrustedValidationContext` are opaque outside the crate; admission now requires exact member documents, recomputes schema/member/manifest digests, requires the manifest schema as a member, and compiles Draft 2020-12 schemas (`validation.rs:51-145,425-483`). It remains open because the bootstrap root only compares the `$schema` URI and calls `jsonschema::validator_for`; it neither validates with the crate's distinct Draft 2020-12 meta-schema API nor pins/verifies the accepted meta-schema digest required by RFC-0002. The only authority constructor is dead `pub(crate)` test/future code, so no production Kernel-owned capability path is demonstrated. Existing tests cover missing documents and wrong manifest digest, not malformed-meta-schema, external/self reference, wrong capability, or the first admitted set. | Pin the bootstrap meta-schema bytes/digest and allowed algorithm; invoke explicit Draft 2020-12 meta-validation before compilation; prove a usable Kernel-only authority path and add malicious bootstrap/capability fixtures. | open |
| F-002 | Blocker | Partial remediation: the manifest now pins an exact event-envelope SchemaRef, payloads are run through `jsonschema`, and empty sequence is rejected (`types.rs:199-209`; `validation.rs:181-255`). It remains open because `variant_id` is only checked for non-empty text and never selects or validates a typed event variant; the returned payload is still untyped `Value`. The payload negative test mutates payload without recomputing `payload_digest`, so it would fail on digest alone and does not prove the runtime Schema branch. | Bind each admitted `variant_id` to an actual typed decoder/semantic validator, and add a wrong-shape payload with a recomputed matching digest that asserts a `/payload` Schema error; retain exact envelope, binding, scope, actor, stream, and empty-sequence negatives. | open |
| F-003 | Blocker | Composition and major-name counterexamples now fail closed (`compatibility.rs:12-27`; `protocol_contract.rs:106-124`). The checker still accepts an optional-property mutation when old and new have the same complete `$id`, allowing in-place bytes/digest changes forbidden by RFC-0002, because it compares only name and major and skips `$id` in `prove_object`. It also rejects every otherwise-safe change to a real generated Schema merely because an unchanged `$ref` exists anywhere, so the inline no-ref fixture does not prove compatibility for published contracts. | Require same schema type, strictly newer minor within the same major, canonical URN/version syntax, and unchanged-ref-graph proof rather than blanket `$ref` presence. Add same-ID mutation rejection and old/new fixtures derived from an actual generated public Schema. | open |
| F-004 | Blocker | No remediation is possible in this snapshot: all implementation and formal documents remain based on an uncommitted working tree. `git diff --check` still cannot inspect untracked contents, and no exact `reviewed_revision` can be recorded. | Commit the complete intended tree after all findings are repaired, rerun every gate, and obtain focused re-review of that exact commit. | open |
| F-005 | Major | Explicit-null handling and many generated patterns improved, and the IsolationScope null parity test is valid. Schema/Serde parity is still false: `RunId::parse("run_a_b")` succeeds because the Rust parser permits internal underscores (`types.rs:57-68`), while the generated ID pattern rejects them (`schema.rs:152-157`). `SchemaRef.type`, artifact/final sequence, Evidence/Revision timestamps, and several non-empty/format contracts remain unconstrained in generated Schema; RevisionMetadata has no semantic validator. Run/Evidence validation checks only membership, not their exact top-level SchemaRef. | Add table-driven Schema-versus-Serde fixtures for every ID/digest/optional field and N/N+1 boundary; align the accepted ID grammar; harden all timestamp/decimal/SchemaRef contracts; validate each top-level record against its exact admitted Schema and semantic validator. | open |
| F-006 | Major | RevisionHashView, RevisionMetadata and ArtifactManifest Schemas plus fixed payload/revision vectors were added. The accepted digest contract is not yet proven: `derive_revision_id` always hashes `"parent_revision": null` when absent instead of the closed-world omitted field, and `digest_revision_content` introduces the undocumented `revision-content:<kind>` domain rather than the RFC's frozen revision domain. No validator proves a supplied `revision_id` equals the derived value; artifact tests still assert only non-collision and do not pin manifest/raw/artifact digest bytes. | Resolve the preimage/domain against RFC-0002 (or revise RFC/ADR through design review), omit absent fields, validate RevisionMetadata identity, and add fixed RevisionHashView content, raw SHA, ArtifactManifest, artifact identity, parent-present/absent, and metadata-mutation vectors. | open |
| F-007 | Major | Boundary inventory/reconciliation types and basic lineage tests were added. The model still permits `BoundaryOutcome::LateResult` inside the finalized inventory (`types.rs:241-304`), contradicting the rule that post-final-sequence results may only create audit events and a new reconciliation revision. Validation does not bind inventory metadata kind/digest, source scope, exact SchemaSet/policy, or replay source to the referenced finalized inventory; tests cover only self-reference, empty inventory, invalid sequence and empty reconciliation, not intent-without-receipt, partial/cancel/late receipt, duplicate outcomes, cross-scope or wrong inventory reference. | Remove late results from finalized inventory, validate inventory/reconciliation identities and exact replay context, and add the full SPEC-0002 AC-07 positive/negative matrix. | open |
| F-008 | Major | Full Run/Evidence value limits and depth-before-canonicalization were added. The pinned limits identity is still ornamental: validators compare only `profile == "protocol-limits-v1"` and accept any digest. Event validation does not enforce the full record ceiling, and it compiles/runs JSON Schema plus computes digest before applying payload depth/collection/byte limits (`validation.rs:193-255`), reversing the required resource boundary. Tests cover depth, string and raw transport only, not record/payload/collection bytes, escape semantics, exact limits digest or every typed path. | Define and verify the exact V1 profile digest; apply limits before Schema/digest work and to complete Event/Run/Evidence typed records; add all RFC N/N+1, typed/minified/pretty, collection/payload/record and wrong-profile-ref fixtures. | open |
| F-009 | Major | Exact directory-set golden checks now reject stale files and generation uses staging/backup directories. Publication is still not atomic: it first renames the published directory away and then renames staging into place (`generate_schemas.rs:36-47`), leaving an observable missing-output interval and a process-crash state where only backup remains. No test exercises concurrent publication, failure after backup rename, or restoration. | Use a publication design with an atomically switched immutable version/pointer on all supported platforms, or explicitly revise the RFC guarantee; add concurrent/fault-injection/rollback tests. | open |
| F-010 | Major | Named filters, more negative tests, runtime Draft validation and exact schema file-set checks improved coverage. Linux/macOS remain skipped; `check_docs.py` remains failed; there is no exact revision. The recorded 16-test result does not match the 17 test functions in the current tree. Official JCS vectors, true Schema-only payload proof, complete admission/scope/limits/replay fixtures and validation latency baselines remain absent. `jsonschema` adds a large but permissively licensed dependency graph; no Runtime/DB/provider/network resolver feature is enabled, but validators are compiled and discarded at admission then recompiled for every event, with no latency evidence. | Produce raw exact-commit Windows/Linux/macOS results, passing docs/completion gates, corrected test counts, complete named fixtures and reproducible parse/validate/canonicalize/digest/schema-generation baselines; cache admitted validators or justify repeated compilation with evidence. | open |
| F-011 | Minor | ARCH-0003 fields and replay vocabulary were mostly corrected and Tasks 07-09 are checked. Drift remains: PLAN steps 2-7 and handoff wording still describe implementation as future work; the exact unit-test filter for the module-qualified isolation test is incorrect; ARCH-0003 lists `scope` twice for Evidence instead of `evidence_scope`; VALIDATION reports 16 tests while source has 17. | Reconcile durable docs, Plan/Handoff, exact commands and validation counts with the final exact revision. | open |

### Acceptance trace after focused re-review

| Acceptance criterion | Result |
|---|---|
| AC-01 | partial: public types expanded, but Revision identity validation remains incomplete |
| AC-02 | partial: nine deterministic Schemas and exact file-set golden exist; atomic publication remains open |
| AC-03 | failed: Schema/Serde parity, exact top-level Schema and limits identity/order remain incomplete |
| AC-04 | failed: revision/artifact preimage and complete fixed vectors remain incomplete |
| AC-05 | failed: same-version mutation can be approved and real `$ref`-bearing compatibility is not proved |
| AC-06 | partial: exact envelope, payload Schema and sequence checks exist; typed variant proof remains open |
| AC-07 | failed: finalized/late-result separation and exact source-inventory lineage remain incomplete |
| AC-08 | partial: Evidence shape/verdict exist; artifact and exact Schema/limits admission remain incomplete |
| AC-09 | failed: bootstrap meta-schema trust root and malicious/capability evidence remain incomplete |
| AC-10 | failed: exact revision, Linux/macOS and passing completion gates are absent |

### Positive evidence retained

- Opaque fields prevent downstream safe Rust code from directly constructing `SchemaAdmissionAuthority` or `TrustedValidationContext`.
- Member Schema bytes, `$id`, digest, manifest membership and exact event-envelope reference are now checked; default `jsonschema` network/file resolver features are disabled.
- Generated Schema null removal matches the custom present-option deserializer for the tested IsolationScope case.
- Cargo metadata shows no Runtime, database, Provider, HTTP client or enabled filesystem resolver dependency. Transitive licenses expose permissive alternatives compatible with the repository license.
- No unrelated product implementation was found; remaining documentation changes are within REQ-0003 scope.

## Second focused re-review — 2026-08-22

- Reviewer: `/root/req0003_code_review`
- Independence: independent source/test/evidence inspection; remediation summaries were not accepted as proof
- Reviewed revision: unavailable; dirty/untracked working tree at baseline HEAD `66fe760ce88b3817b6869ebdc5b509be22e73770`
- Verdict: changes-requested; advisory working-tree review only, **not** exact-revision approval
- Open: 4 Blocker, 5 Major, 1 Minor
- Closed this round by Reviewer: F-006
- Execution evidence: Reviewer did not rerun commands. Current `VALIDATION.md` records Windows 17 Rust tests (5 unit + 12 integration), 18 Python tests, fmt/clippy/schema generation/diff-check passing, and `check_docs.py` failing on stale REVIEW-0001. Exact commit and Linux/macOS evidence remain absent.

| ID | Severity | Independent disposition | Testable correction | Status |
|---|---|---|---|---|
| F-001 | Blocker | Explicit Draft 2020-12 meta-validation/compilation, exact `jsonschema=0.50.0`, correct pinned root meta-schema SHA-256, member-byte/digest verification, cached validators, embedded-root-only bootstrap, and a public `validate_event_at_boundary` now exist (`Cargo.toml:15`; `validation.rs:104-165,212-228,521-592`). The external event boundary is usable, but general SchemaSet admission/authority remains `pub(crate)` and the only public bootstrap set has no event bindings. No external Kernel consumer fixture proves an authorized evolved set can be admitted and used. | Compile a consumer-crate Kernel fixture that admits both the fixed initial root and an authorized evolved/event-bearing set through a capability-safe public boundary; retain malicious meta/member/self-sign/wrong-ref negatives. | open |
| F-002 | Blocker | Exact envelope and payload validators, exact EventTypeBinding, positive sequence, recomputed wrong-shape digest, `/payload` Schema assertion, cached validators and `ValidatedEvent` are present (`validation.rs:230-358,1034-1086`). `variant_id` is still arbitrary manifest text and success returns an untyped `Value`; admission has no reader-supported typed decoder/semantic-variant registry. | Bind admitted variant IDs to supported decoders/semantic validators and reject unknown/misbound variants; assert typed dispatch through the public boundary. | open |
| F-003 | Blocker | Same-ID mutation, major/name changes, composition/ref changes, nested `$id`, and a real generated-Schema optional addition are now covered (`compatibility.rs:12-105`; `protocol_contract.rs:122-154`). Identity parsing still accepts non-canonical major/minor spellings such as `01`, contrary to the claimed canonical URN/version identity; byte-identical malformed inputs also bypass identity validation via the early equality return. | Require canonical decimal version syntax and validate identity before the equality fast path; add leading-zero/malformed-identical fixtures. | open |
| F-004 | Blocker | There is still no exact Git revision. Untracked implementation bytes cannot receive formal approval. | Commit the intended tree after repairs, rerun all gates, and obtain independent re-review of that exact commit. | open |
| F-005 | Major | ID grammar, null handling, exact Event/Run/Evidence Schema checks, timestamp/decimal hardening, and new `SchemaRef`/`ArtifactManifest` `try_from` checks materially improve parity (`types.rs:77-123,228-276`; `schema.rs:146-333`; `protocol_contract.rs:25-47,212-243`). The parity evidence remains two isolated invalid fixtures plus one null fixture; `parse_bounded` still performs only limits+Serde and no exact admitted Schema/semantic validation for RevisionHashView/Metadata, ArtifactManifest, or boundary revisions. | Add boundary validators/constructors for every public top-level record and a table-driven two-way Schema/implementation matrix for IDs, digest, optional/null, timestamp, decimal, enum, empty and N/N+1 cases. | open |
| F-006 | Major | Revision content/metadata use `revision:<kind>`, absent parent is omitted, supplied IDs can be validated, and fixed payload/content/revision/raw/artifact vectors plus mutations are asserted (`digest.rs:74-158`; `types.rs:200-225`; `protocol_contract.rs:183-210,245-300`). | Preserve the vectors and rerun on the exact cross-platform revision. | closed by reviewer |
| F-007 | Major | Finalized outcomes exclude late results; inventory/reconciliation recompute content digests, bind metadata identity/kind, reject duplicate/final-sequence faults, and replay can exact-match source/inventory (`types.rs:336-496,544-574`). It remains open because no mutation test proves any inventory/reconciliation content change invalidates identity; the digest uses `metadata.schema_ref` (the fixture supplies `revision-metadata`) rather than a declared inventory/reconciliation hash-view Schema; the required intent-without-receipt, partial/cancel/late, wrong-inventory and cross-scope matrix is absent. | Freeze exact inventory/reconciliation hash-view Schemas/preimages, add field-by-field content-mutation tests, and complete the AC-07 boundary/lineage matrix. | open |
| F-008 | Major | Exact profile name+digest and Event record/payload limits are checked before Event Schema/digest (`validation.rs:24-42,230-254,691-744`). Run/Evidence still execute JSON Schema before record limits (`validation.rs:377-422,443-481`). N/N+1 coverage still only exercises depth, decoded string and raw pretty bytes, not record/payload bytes, array/object collection, object-name bytes, escape/minified/typed equivalence; the digest is not derived from a published complete profile preimage. | Move all typed limits ahead of Schema work and add exact N/N+1 tests for every limit and Event/Run/Evidence/raw/typed path; publish a profile preimage tied to the digest. | open |
| F-009 | Major | File-set golden now rejects stale files, but publisher renames live output away before installing staging (`generate_schemas.rs:36-47`), creating a missing-directory/crash window. No concurrent/fault/recovery test exists. | Use an atomically switched immutable version/pointer portable across all targets, or approve a weaker guarantee; add crash/concurrency/rollback tests. | open |
| F-010 | Major | Windows evidence/count and validator caching improved, and dependency direction remains clean/permissively licensed. Linux/macOS, exact commit, passing docs gate, official JCS vectors, full malicious/parity/limits/lineage fixtures and reproducible latency baselines remain absent. | Pass the complete named gates/fixtures on the repaired exact commit across Windows/Linux/macOS and record raw results/baselines. | open |
| F-011 | Minor | PLAN/VALIDATION counts improved, but `HANDOFF.md:11` still says implementation has not begun, PLAN's `--exact` unit-test filter is not module-qualified, and VALIDATION claims atomic publication despite F-009. | Reconcile Handoff/Plan/Validation with final source and rerun docs freshness. | open |

### Second re-review acceptance trace

| AC | Result |
|---|---|
| AC-01 | partial — public contracts exist; complete top-level admission and evolved SchemaSet Kernel path remain open |
| AC-02 | partial — deterministic/stale-file golden exists; atomic publication remains open |
| AC-03 | failed — parity and limits ordering/coverage remain incomplete |
| AC-04 | locally satisfied — fixed vectors exist; exact cross-platform proof remains AC-10 |
| AC-05 | failed — non-canonical version identities can still be approved |
| AC-06 | partial — exact Schema/digest/context works; supported typed-variant proof remains open |
| AC-07 | failed — hash-view contract and boundary/lineage mutation matrix remain incomplete |
| AC-08 | partial — Evidence exact Schema improved; general admission/limits parity remains incomplete |
| AC-09 | partial — bootstrap/meta/context improved; evolved-set external Kernel admission proof remains open |
| AC-10 | failed — exact revision, docs pass, Linux/macOS and full evidence are absent |

## Exact-HEAD re-review — 2026-08-23

- Reviewer: `/root/req0003_code_review`
- Independence: independent; Requirement/Spec/RFC/ADR, `98e882a..ff614b5` history, `d1daa0d..ff614b5` increment, source, generated assets, dependency direction, tests, and raw evidence inspected directly
- Reviewed revision: `ff614b59385125fd3438a725388aa15998db68e8`
- Verdict: changes-requested
- Open: 3 Blocker, 5 Major, 1 Minor
- Closed: F-004 and F-006
- Formal REQ-0003 Review: not created because open Blocker/Major findings remain

Exact-HEAD local evidence passed: `cargo fmt --all -- --check`; locked/offline clippy with `-D warnings`; locked/offline workspace all-target/all-feature tests (5 unit + 12 integration); 18 Python governance tests; and `git diff-tree --check ff614b5^ ff614b5`. The three-platform workflow exists but is explicitly `pending` and has no remote run evidence.

| ID | Severity | Exact-HEAD disposition | Status |
|---|---|---|---|
| F-001 | Blocker | Exact bootstrap/meta/member validation remains sound locally, but `SchemaSet::admit`/authority is crate-private and the only public bootstrap bundle has `event_bindings: []`. An external future Kernel can call the boundary validator but cannot admit an authorized event-bearing/evolved set; no public consumer fixture proves AC-09/AC-10. | open |
| F-002 | Blocker | Cached exact envelope/payload Schema validation and recomputed wrong-shape digest proof pass. `variant_id` remains arbitrary manifest text; admission has no supported typed-decoder/semantic-variant registry and success retains untyped payload `Value`. | open |
| F-003 | Blocker | Nested `$id`, composition/ref and same-version mutations are protected, but RFC-0002's no-leading-zero version grammar is not enforced: `schema_identity` parses `01`, and `old == new` returns before any identity validation (`compatibility.rs:12-63`). | open |
| F-004 | Blocker | Implementation and CI increment are now pinned and inspected at exact HEAD `ff614b59385125fd3438a725388aa15998db68e8`. | closed by reviewer |
| F-005 | Major | Existing hardening remains, but raw/top-level admission is incomplete outside Event/Run/Evidence and parity tests still do not cover the approved public contract matrix. `parse_bounded<T>` is limits+Serde only. | open |
| F-006 | Major | Correct revision domains/preimages, omitted absent parent, identity validation, and fixed payload/revision/raw/artifact vectors remain present and pass at exact HEAD. | closed by reviewer |
| F-007 | Major | Inventory/reconciliation recompute content, but use `metadata.schema_ref` without a frozen per-kind hash-view Schema; the fixture uses generic `revision-metadata`. Content-mutation and the full intent/partial/cancel/late/wrong-inventory/cross-scope matrix remain absent. | open |
| F-008 | Major | Event ordering improved, but Run/Evidence still execute Schema before record limits. Complete record/payload/collection/object-name/escape N/N+1 coverage and a canonical limits-profile preimage remain absent. | open |
| F-009 | Major | Schema golden file-set checking passes, but live-directory-to-backup then staging-to-live publication retains the observable missing-directory/crash window and has no fault/concurrency recovery proof. | open |
| F-010 | Major | A Windows/Linux/macOS workflow was added at `ff614b5`, but VALIDATION correctly marks it pending; no Linux/macOS execution evidence exists. The workflow's whitespace step checks only `HEAD`, not the PR/push commit range, and official JCS vectors, complete security/limits/lineage fixtures and latency baselines remain absent. | open |
| F-011 | Minor | PLAN/VALIDATION acknowledge exact commits and pending CI, but HANDOFF remains pre-implementation, the unit `--exact` filter remains unqualified, VALIDATION still overclaims atomic schema publication, and its review candidate is `d1daa0d` rather than exact HEAD. | open |

REQ-0003 must remain `changes-requested`; F-001/F-002/F-003 and F-005/F-007/F-008/F-009/F-010 require remediation and another exact-commit independent re-review.
