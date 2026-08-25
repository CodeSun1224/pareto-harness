# REQ-0006 Independent Architecture Review Remediation

Author record only. Finding disposition remains owned by the independent reviewer.

## Focused re-review subject

Baseline remains `bb395ad78f762b53d5f486c742194dd8d551dc61`. Author remediation exact SHA-256:

| Subject | SHA-256 |
|---|---|
| `docs/requirements/REQ-0006-projection-snapshot-replay.md` | `4ffe8b85f215e577d1ae3391afe3df12404cfdc0e552312d419f141bd36494c4` |
| `docs/specs/SPEC-0005-projection-snapshot-replay.md` | `bd7577bd52b9bfa2c0253398be597b7165ad8185e53206a1e002d42dd47973d7` |
| `docs/rfcs/RFC-0005-projection-snapshot-replay-contract.md` | `b0022117eaac3fd7e51d172b67f3cafd7b7669b1d00d1510242e9f17952782ea` |
| `docs/adrs/ADR-0006-versioned-projection-snapshot-recorded-replay.md` | `33e8ee86886fd791e84db3f0edd1abd38bc8c73fdde9069b5913bbcd2b1fe092` |

## Finding responses

| Finding | Author remediation | Added/strengthened planned proof |
|---|---|---|
| IAR-F-001 | Assisted load must exact reread/validate `[1..cursor]` and independently recompute the rolling history chain before accepting a seed; prefix corruption is Event failure, not cache miss. Optimization is explicitly limited to skipping prefix reducer fold. | `snapshot::prefix_validation`; `snapshot::prefix_corruption`; controlled envelope/admission/sequence/cursor drift must fail closed. |
| IAR-F-002 | Frozen `digest_json` domains and closed seed/step/projection/snapshot/descriptor hash-view Schemas. `H0`/`Hi`, fields, order, absence and incremental continuation are explicit. | `projection_digest_golden` covers empty/1/N/prefix+suffix, identity mutation and all digest families. |
| IAR-F-003 | Frozen machine-readable reducer descriptor and `SourceReducerKeyV1 -> exact ProjectionReducerRef` registry. Historical descriptor/implementation/output reader retention and missing/wrong/current substitution failure are mandatory. | `projection::reducer_resolution`; `projection::compatibility`; old/current/missing reducer matrix. |
| IAR-F-004 | Projection/Snapshot bind exact output/snapshot SchemaSetRef and ProtocolLimitsRef separately from source Event identity; row bootstrap resolves retained output registry before record validation. | `snapshot::output_reader`; retained exact/alternate/current/missing output set fixtures. |
| IAR-F-005 | Projection and its digest now include Kernel-read store ID and full scope/stream/source/reducer/output provenance; comparison checks all fields before digest and returns safe not-comparable for cross-store/scope. | `replay::cross_store_not_comparable`; two-store same-bytes clone and per-field swap matrix. |
| IAR-F-006 | SQLite v2 adds `events.writer_epoch DEFAULT 1` plus checksum BEFORE INSERT trigger requiring epoch 2; v2 SQL binds 2, so a migration-before-open v1 pool's old INSERT defaults to 1 and is rejected after migration. | `snapshot::already_open_v1_writer`; v1 pool held open across migration, then old append rejected and v2 append/reopen/rollback remain valid. |
| IAR-F-007 | v2 Snapshot UPDATE and DELETE are both checksum-trigger rejected; no GC/delete API in this slice. “Discardable” means ignored/rebuilt only. | Snapshot append-only trigger execution and open drift checks under `snapshot::migration`/`snapshot::atomicity`. |
| IAR-F-008 | Requirement/RFC/Spec statuses were moved back to `specified/proposed/draft`; after independent focused re-review they advanced to `planned/accepted/approved`, and the same independent reviewer confirmed the final bytes without design drift. | `check_docs.py`; `INDEPENDENT-ARCHITECTURE-REVIEW.md` owns closure, approval, and final hashes. |

## Author validation before re-review

- `python scripts/check_docs.py`: passed, 158 Markdown files / 43 formal IDs.
- `git diff --check`: passed.
- No Runtime, Schema, Cargo, dependency or existing test files changed.
