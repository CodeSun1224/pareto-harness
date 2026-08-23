# Validation Evidence

## 2026-08-23 exact `f8275e0` re-review and CI preflight

- 独立 reviewer 已对 `f8275e09103fe7702188c8298c5c2a791b9118b8` 完成 exact re-review：F001/F002/F003/F004/F006/F011 closed；F005/F007/F008/F009/F010 Major open。实现者未自行改变 finding disposition。
- `gh --version`：passed，`gh version 2.98.0 (2026-08-20)`。
- `gh auth status`：failed，keyring 中 `CodeSun1224` token invalid。
- `gh run list --workflow protocol-matrix.yml --limit 20`：沙箱内失败，GitHub API 连接被 `127.0.0.1:9` proxy 拒绝；批准后的沙箱外重试超时，尚未取得 run/job 原始结果。
- 因此 `f8275e0` Windows/Ubuntu/macOS 状态仍为 **未核实**；不得据此声明跨平台验证完成。后续在有效 GitHub CLI 认证/网络可用后记录 run ID、head SHA、每个 job conclusion 和原始命令输出。

## 2026-08-23 remaining-major impact/test delta

- F005 直接影响 public admission API；外部实现 `ProtocolRecord` 和 context-sensitive record 的通用 `Validated<T>` 是权限/语义绕过面。回滚为恢复旧 API，但不得发布该不安全表面。
- F007 直接影响 boundary/replay identity，间接影响未来 REQ-0006；测试必须覆盖 exact admitted top/hash SchemaRef、content mutation、wrong inventory/cross-scope 与 intent/partial/cancel/late 结果。
- F008 直接影响 untrusted typed resource boundary；Event/Run/Evidence 必须在 Schema、digest 和语义工作前执行 record limit，Event 另先执行 payload limit，并以精确 N/N+1 证明。
- F009 直接影响不可变 Schema reader；所有保留 set 都必须独立重算目录/manifest/member digest 和精确文件集，并验证并发幂等、冲突失败与 stale staging 不破坏已发布目录。

## 2026-08-23 remaining-major remediation evidence (pre-commit)

- F005：`ProtocolRecord` 已 sealed；EventEnvelope、RunManifest、EvidenceRecord 不再实现通用 record admission，必须使用各自 trusted-context boundary。compile-fail doctest 固定该 API 不可绕过；V1 limits profile 通用 admission 另要求完整 preimage exact equality。
- F007：`boundary_record_admission_binds_exact_top_and_hash_schemas` passed；inventory/reconciliation 的顶层 `metadata.schema_ref`、嵌套 `hash_schema_ref` 和 inventory `schema_set_ref` 都 exact-match admitted set；wrong hash/top/content mutation 失败，received、intent/partial-without-receipt、cancelled 与 late reconciliation 均覆盖。
- F008：`typed_event_payload_and_record_bytes_are_exact` 与 `typed_run_and_evidence_record_bytes_are_exact` passed；Event payload、Event/Run/Evidence record 均覆盖 semantic bytes N/N+1，另覆盖 minified/pretty 同一语义边界和 escape 解码。
- F009：`every_retained_schema_set_is_complete_and_content_addressed` passed，逐个检查 2 个保留 set 的目录 digest、manifest digest、所有 member digest/Draft compilation 和精确文件集合；publisher 并发同 digest、stale staging、幂等及 existing-target byte conflict 负例 passed。
- RFC 8785 官方 property ordering 与 literals/string escaping 两组适用 vector passed；浮点/unsafe integer 按本协议冻结子集继续 fail closed。
- 完整本地 Rust 结果：9 unit + 17 contract passed；1 个 release observation baseline 在普通 suite 中按设计 ignored。Python governance：18 passed。fmt、locked/offline clippy、locked/offline all-target/all-feature tests、Schema generation、`git diff --exit-code -- schemas/`、`git diff --check` passed。
- `python scripts/check_docs.py` 在 pre-commit 工作树仍只因 REVIEW-0001 freshness 对未提交 substantive paths 报错。下一步提交 exact revision 后交独立 reviewer 更新 freshness 和 finding disposition；在此之前不声明 completion。

## 2026-08-23 independent exact `72b61b7` disposition and Actions

- 独立 reviewer 只读复审 `72b61b70abef8c6ba1bd448aad5b59638a791b47`：F005/F008/F009 closed；F007/F010 Major open；0 Blocker、2 Major。reviewer 未修改文件，实施者未自行关闭 finding。
- F007 剩余原因：reconciliation 通用 admission 尚未把 `inventory_revision` 对照可信 finalized inventory；inventory 尚缺 source manifest/expected scope 的 cross-scope binding。下一轮以 dedicated admission API 修复。
- GitHub run `32640519929` exact head SHA `72b61b70abef8c6ba1bd448aad5b59638a791b47`：Windows job `97196795698`、Ubuntu `97196795800`、macOS `97196795835` 的 checkout/toolchain/fetch/fmt/clippy/Rust tests/Schema generation+diff/digest/Python governance 全部 success；三者均仅 `Validate repository documents` failure，whitespace 因前序失败 skipped。workflow conclusion=failure，不能声明三平台完整通过。
- 历史 run `32638266269` exact head SHA `f8275e09103fe7702188c8298c5c2a791b9118b8` 呈相同结论：三平台协议/产品步骤通过、document validation 失败。

## 2026-08-23 F007 trusted-lineage remediation (pre-commit)

- `SchemaSet::validate_boundary_inventory` 先执行 inventory record limits，再验证 source RunManifest 与 expected source scope，随后 exact 绑定 source run、SchemaSet、top-level Schema 和 inventory hash-view Schema。
- `SchemaSet::validate_boundary_reconciliation` 只接受 `Validated<BoundaryInventoryRevision>`，并 exact 对照 `inventory_revision`、top-level Schema、reconciliation hash-view Schema、content/revision identity。
- `ExecutionMode::validate_inventory` 同样只接受 `Validated<BoundaryInventoryRevision>`，阻止 replay lineage 使用未经过 source scope/SchemaSet admission 的自报 inventory。
- focused negative matrix：wrong expected workspace scope、wrong source run、wrong replay source、wrong inventory revision、wrong top/hash SchemaRef、content mutation 均 fail closed；received、partial-effect-without-receipt、cancelled、late-result reconciliation 正例通过。
- 完整本地门禁：fmt passed；locked/offline clippy passed；locked/offline workspace all-target/all-feature tests passed（9 unit + 17 contract，1 ignored observation baseline）；Schema generation 与 `git diff --exit-code -- schemas/` passed；Python governance 18 passed；`git diff --check` passed。
- `check_docs.py` 仍按设计仅报 REVIEW-0001 freshness stale；必须在 exact product commit 后由独立 reviewer 更新 review disposition/freshness，实施者不自行批准。

## 2026-08-23 remediation working-tree evidence

- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`：passed。
- `cargo test --workspace --all-targets --all-features --locked --offline`：passed，6 unit + 14 contract；另有 1 个 ignored observation baseline。
- `python -m unittest discover -s scripts/tests -p "test_*.py"`：passed，18 tests。
- 生成器发布 12 个当前公共 Schema 到 `schemas/sets/sha256-<manifest-digest>/`；旧 digest set 保留。幂等发布和既有目标 byte drift 负例通过。
- limits N/N+1 覆盖 depth、decoded string、array/object collection、object-name、raw pretty transport 与 escape；Run/Evidence 在 Schema 前执行 record limits；V1 profile digest 从公开 preimage 重算。
- RFC 8785 §3.2.3 官方 property-sorting vector 通过。
- release baseline 命令：`cargo test -p pareto-protocol --test protocol_baseline --release --locked --offline -- --ignored --nocapture`。Windows 本次观测：parse 1,779,200 ns/1000；Schema validate 4,214,000 ns/1000；canonicalize 1,281,600 ns/1000；digest 4,401,000 ns/1000；Schema generation 100,885,400 ns/100。仅为可复现基线，无性能通过阈值或优化声明。
- `python scripts/check_docs.py` 当前预期失败：工作树尚未形成 exact commit，REVIEW-0001 freshness gate 检出本轮 substantive paths；commit 后由独立 reviewer 更新 exact revision，禁止实现者自行绕过。
- GitHub Windows/Linux/macOS matrix 必须在本轮 exact commit push 后取得结果；当前仍 pending。

## Subject

- Requirement: REQ-0003 (`implementing`)
- Spec: SPEC-0002 (`approved`)
- RFC/ADR: RFC-0002 / ADR-0003 (`accepted`)
- Reviewed revision: `ff614b59385125fd3438a725388aa15998db68e8`（independent verdict: changes-requested）
- Environment: Windows, Rust/Cargo 1.96.0, rustfmt 1.9.0, Python 3.14.5, Git 2.43.0

## Results

| Scope/layer | Command or procedure | Result | Artifact/reference | Notes/risk |
|---|---|---|---|---|
| Focused static | `cargo fmt --all -- --check` | passed | local command result | Rust formatting |
| Focused static/security | `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | passed | local command result | Includes MSRV compatibility lint |
| Focused unit/contract | `cargo test --workspace --all-targets --offline` | passed | 17 tests: 5 unit + 12 integration | Canonicalization, closed parsing, IDs/digests, real Draft 2020-12 validation, schema/Serde presence parity, compatibility, scope, event binding, limit N/N+1, deterministic error truncation and replay lineage |
| Focused golden | `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas` followed by checked-in schema golden test | passed | 9 schemas + SchemaSet manifest/ref | Exact file-set/byte golden prevents stale output; cross-platform atomic publication remains open as F-009 |
| Impacted dependency | `cargo metadata --offline --format-version 1` and `cargo tree -p pareto-protocol --offline` | passed | Cargo.lock / metadata | Direct dependencies: jsonschema, schemars, serde, serde_json, sha2; no Runtime/DB/provider dependency |
| Core governance unit | `python -m unittest discover -s scripts/tests -p "test_*.py"` | passed | 18 tests | Existing SDD positive/negative fixtures |
| Core document | `python scripts/check_docs.py` | failed | stale REVIEW-0001 | Expected gate: tracked AGENTS/README/ARCH/EPIC/index changes require exact-revision independent Review; do not weaken checker |
| Whitespace | `git diff --check` | passed | tracked diff | Untracked files are additionally compiled/generated/read by tests but ordinary Git diff does not cover them until staged/committed |
| Cross-platform | `.github/workflows/protocol-matrix.yml` | pending | Windows/Linux/macOS matrix added at `ff614b5`; not yet pushed/executed | Workflow fetches the lockfile once, then runs locked/offline Rust gates, Schema/digest golden and governance checks identically on all three OSes |
| Full Runtime | Event Store/Replay executor/E2E | skipped | out of scope | REQ-0003 provides protocol contracts only; later Requirements own Runtime consumers |

## Acceptance trace summary

- AC-01/03/06/07/08/09：typed closed contracts and semantic/security negative tests pass on Windows.
- AC-02：nine deterministic public Schemas plus SchemaSet manifest/ref are checked in and byte/file-set golden tested.
- AC-04：UTF-16 JCS ordering, unsafe number rejection, fixed digest/revision vectors, complete SchemaRef/type domain separation and artifact domain tests pass.
- AC-05：conservative old-writer/new-reader proof accepts only optional property additions; narrowing/required/composition mutations fail closed.
- AC-10：crate has no Runtime/DB/provider dependency and Windows gates pass; Linux/macOS evidence is still missing, so Requirement cannot be verified.

## Remaining gates

1. Resolve independent review Blocker/Major findings and run focused re-review.
2. Complete independent review of exact commits and close all Blocker/Major findings.
3. Push and obtain passing Windows/Linux/macOS workflow evidence.
4. Re-run all completion commands and make `check_docs.py` pass without weakening REVIEW freshness.
