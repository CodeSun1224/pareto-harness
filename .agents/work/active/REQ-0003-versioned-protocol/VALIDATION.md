# Validation Evidence

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
