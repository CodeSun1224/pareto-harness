---
id: REVIEW-0002
title: REQ-0003 版本化协议类型和 JSON Schema 独立代码评审
status: approved
owners: [independent-reviewer]
created: 2026-08-23
updated: 2026-08-25
links: [REQ-0003, SPEC-0002, RFC-0002, ADR-0003]
independence: independent
reviewed_revision: 1748f69d01044a936727b3b5b7659882981b9129
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
- Exact closure `907eee7295a7c3e7c2fa408a035c52d684f52fb4` freshness-only re-review: `14b5438..907eee7`只更新REQ-0006 durable status/navigation/architecture facts和归档work；`crates/pareto-protocol/`、`schemas/`、Cargo manifests/lock、public API、SchemaSet bytes与reader identity零差异。新增文案准确描述已批准current output set、retained source reader和Recorded replay边界，不扩大协议合同。REVIEW-0002保持approved、0 open Blocker/Major，freshness前移至exact `907eee7`。
- Exact candidate `cfa7a06c3588a6ad975a9511140d0984f5eb1b8f` substantive freshness re-review: `907eee7..cfa7a06`仅设计新的REQ-0007 closed protocol、control-capable SchemaSet和retained operation/source contracts；没有修改`crates/pareto-protocol/`、`schemas/`、Cargo manifests/lock、public API、四个既有SchemaSet bytes、canonicalization、limits、isolation或reader实现。新合同仍要求Manifest exact pin、旧set不升级和protocol不依赖Kernel；实现及golden证据尚未发生且受REVIEW-0006阻塞。REVIEW-0002保持approved、0 open Blocker/Major，freshness前移至exact `cfa7a06`。
- Exact candidate `a4e34785908207e622365250ae1466b85b4baecb` substantive freshness re-review: `cfa7a06..a4e3478`只在REQ-0007设计层冻结`TimeoutKeyV1`与`TimeoutRecoveryCommandV1`的未来closed wire identity；没有修改`crates/pareto-protocol/`、`schemas/`、Cargo manifests/lock、public API、四个既有SchemaSet bytes、canonicalization、limits、isolation或reader实现。新增类型、Schema与golden仍须在实施中生成并独立评审，既有协议批准事实未改变。REVIEW-0002保持approved、0 open Blocker/Major，freshness前移至exact `a4e3478`。
- Exact implementation candidate `1b40e92be11e73a497ec821118b7cb4e0c1af1ce` substantive freshness re-review: 新增closed Runtime Control v1类型、43个生成Schema member和内容地址set `sha256-a1f960...`；四个retained set byte-diff无变化，protocol仍不依赖Kernel/SQLite，Cargo依赖无变化。Reviewer独立复跑Protocol 9 unit + 23 contract，retained-set completeness、old-writer exact reader和Schema generation worktree identity均通过。REVIEW-0007对新REQ-0007 v1完整性保持open Major，但没有放宽REQ-0003已批准canonicalization、limits、retained reader或依赖方向。REVIEW-0002保持approved、0 open Blocker/Major，freshness前移至exact `1b40e92`。
- Exact remediation candidate `ab2fbc6d2e979ef12bcffd5df1cfe76b975a9684` substantive freshness re-review: `1b40e92..ab2fbc6`补全未发布REQ-0007 control source/operation/meter/projection identities并产生最终内容地址set `sha256-19566903…`；四个既有retained set仍完整且由exact reader读取，protocol不依赖Kernel/SQLite，Cargo依赖未变。Reviewer独立复跑Protocol 9 unit + 23 contract、retained completeness、old writer reader和生成一致性，全部通过（1 observation ignored）。REVIEW-0007仍阻塞新settlement/late authority合同，但REQ-0003既有canonical JSON、closed Schema、limits、isolation和retained-reader合同未回退。REVIEW-0002保持approved、0 open Blocker/Major，freshness前移至exact `ab2fbc6`。
- Exact second-repair candidate `26b63ca2abb99bf3d6216d395994d006c1b3e2b5` substantive freshness re-review: 未发布中间REQ-0007 set `195669…`由补全lifecycle/callback authority的final set `c3e2fda5…`替换；此前五个published retained sets完整且exact old-reader测试通过，protocol仍不依赖Kernel/SQLite，Cargo依赖不变。Reviewer独立复跑Protocol 9 unit + 23 contract、content addressing、generation和old writer tests；新REQ-0007 semantic fold仍由REVIEW-0007阻塞，但REQ-0003 canonical JSON、closed Schema、limits/isolation与retained-reader合同未回退。REVIEW-0002保持approved、0 open Blocker/Major，freshness前移至exact `26b63ca`。
- Exact third-repair candidate `97bca8b7b34ceadd5ab4f8ad01f49e10b3377adb` substantive freshness re-review: `26b63ca..97bca8b`未修改`crates/pareto-protocol/`、`schemas/`、Cargo manifests/lock、public protocol API或任何retained set；final control set仍为`c3e2fda5…`。Reviewer独立复跑Protocol 9 unit + 23 contract、content addressing、old writer reader及Schema generator，生成后tracked Schema byte-identical。REVIEW-0007的F-007仅涉及Kernel pure semantic admission，不放宽REQ-0003 canonical JSON、closed Schema、limits/isolation、dependency direction或retained-reader合同。REVIEW-0002保持approved、0 open Blocker/Major，freshness前移至exact `97bca8b`。
- Exact fourth-repair candidate `80249cc5c73575a3f92027f843cc657536905b9e` substantive freshness re-review: `CallbackAuthorityV1`新增closed必填`decision_monotonic_millis`，未发布`c3e2fda5…`由content-addressed final set `a95c824d…`替换；五个此前published retained sets `68535…/7adfe…/dae028…/4ce387…/a1f960…`未改写。Protocol 9 unit + 23 contract、old writer exact reader、retained completeness/content addressing和generator byte identity通过；protocol仍不依赖Kernel/SQLite且Cargo依赖未变。REVIEW-0002保持approved、0 open Blocker/Major，freshness前移至exact `80249cc`。
- Exact closure candidate `87be5391c40fdaa5b423c921747e7c941f7e2d42` substantive freshness re-review: `f18f410..87be539`对`crates/pareto-protocol/`、`schemas/`、Cargo manifests/lock、public API和全部retained sets零差异；文档继续准确固定final source set `a95c824d…`和五个既有published sets。closure仅因归档Validation格式由REVIEW-0007 F-010阻塞，不改变REQ-0003 canonical JSON、closed Schema、limits/isolation、dependency direction或retained-reader批准。REVIEW-0002保持approved、0 open Blocker/Major，freshness前移至exact `87be539`。
- Exact F-010 remediation `53338a836f646cdcefb6858ce07b0b0e8e12b11e` substantive freshness re-review: `828f9aa..53338a8`只重组归档Validation历史失败叙述，对protocol、schemas、Cargo、API和retained sets零差异；历史设计拒绝与最终批准均保留。F-010关闭不改变REQ-0003批准合同。REVIEW-0002保持approved、0 open Blocker/Major，freshness前移至exact `53338a8`。
- Exact architecture clarification `8bb885bda678f5f785706e9eb335f472b5244974` substantive freshness re-review: `53338a8..8bb885b`对protocol、schemas、Cargo、API和retained sets零差异；`ARCH-0004`只要求未来离线工具依赖版本化公共协议或artifact，并明确不冻结Worker transport。REQ-0003 canonical JSON、closed Schema、dependency direction与retained-reader合同不变。REVIEW-0002保持approved、0 open Blocker/Major。
- Exact polyglot design remediation `1748f69d01044a936727b3b5b7659882981b9129` substantive freshness re-review: `8bb885b..1748f69`对`crates/pareto-protocol/`、`schemas/`、Cargo manifests/lock、public API、canonicalization、limits及retained readers零差异。RFC-0007仍禁止首个真实跨语言Requirement之前发布wire Schema或暴露Rust ABI/SQLite layout，并要求unknown/old version fail closed。REVIEW-0002保持approved、0 open Blocker/Major。
