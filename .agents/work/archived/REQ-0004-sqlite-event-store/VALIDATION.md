# REQ-0004 Validation

## Design phase

- `python scripts/check_docs.py`: pending rerun after Plan/Tasks creation.
- `git diff --check`: passed before approval; rerun required after implementation.
- Independent architecture review: final 0 Blocker / 0 Major after four focused re-reviews.

## Implementation evidence

- Focused `cargo test -p pareto-kernel --all-targets --all-features --offline`: 第二轮 remediation 后 passed, 15 tests, 0 failed。
- API surface `cargo test -p pareto-kernel --doc --offline`: passed, 1 compile-fail doctest，证明外部 crate 不可访问 authority-bearing Event Store 类型。
- Static `cargo clippy -p pareto-kernel --all-targets --all-features --offline -- -D warnings`: passed。
- 真实 SQLite 测试覆盖 migration/identity/trigger、append/read/reopen、exact retry/conflict、sequence gap、同 Stream 多连接竞争、causation rollback、append-only、离线 row drift、固定 horizon、cursor binding 与 VACUUM 后续读。
- 依赖解析：经用户批准联网生成锁文件并下载 sqlx/Tokio 传递依赖；后续验证均使用 `--offline`。
- Impacted `cargo test -p pareto-protocol --all-targets --all-features --offline`: passed（9 unit、17 contract，1 个无阈值 observation test 按既有设计 ignored）。
- Core `cargo test --workspace --all-targets --all-features --offline`: 第二轮 remediation 后 passed（kernel 15、protocol 9+17；0 failed，既有 observation ignored）。
- Governance `python -m unittest discover -s scripts/tests -p "test_*.py"`: passed, 18 tests。
- Hygiene `git diff --check`: passed。
- 可复现本机 observation（Windows、debug、临时真实 SQLite、WAL/FULL、2026-08-23）：50 次顺序 append `100.245ms`，一次读取并重验 50 条 `22.0485ms`。这是观察基线，不是阈值或优化声明；Token/Provider cost 不适用。
- Independent Code Review：REVIEW-0003 approved；精确提交 `b7cf277f4232515bebbe15d6a237654336b95271`，0 Blocker / 0 Major。初审 5 个 Major 均由同一 independent reviewer 在两轮 focused re-review 后关闭。

## Results

| Scope/layer | Command or procedure | Result | Artifact/reference | Notes/risk |
|---|---|---|---|---|
| Focused integration/security | `cargo test -p pareto-kernel --all-targets --all-features --offline` | passed | 15 tests | 真实 SQLite、migration、authority、幂等、并发、恢复、隔离、cursor、retained reader、drift 与 observation |
| API surface | `cargo test -p pareto-kernel --doc --offline` | passed | 1 compile-fail doctest | 外部 crate 无 authority-bearing read/write API |
| Impacted protocol | `cargo test -p pareto-protocol --all-targets --all-features --offline` | passed | 9 unit + 17 contract | REQ-0003 回归；1 个无阈值 observation 按设计 ignored |
| Core workspace | `cargo test --workspace --all-targets --all-features --offline` | passed | kernel 15 + protocol 26 | 0 failed |
| Static | `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | passed | local commands | 全 workspace |
| Governance | `python -m unittest discover -s scripts/tests -p "test_*.py"` | passed | 18 tests | SDD/document checker regressions |
| Independent review | REVIEW-0003 exact `b7cf277f4232515bebbe15d6a237654336b95271` | passed | 0 Blocker / 0 Major | independent fresh-agent review + two focused re-reviews |
| Performance observation | 50 FULL/WAL appends + 50 validated reads | passed | 100.245 ms / 22.0485 ms | 本机 debug observation；不是阈值或优化声明 |
| Provider/token cost | No Provider/model calls | skipped | not applicable | 本需求不声称 Token/费用优化 |
