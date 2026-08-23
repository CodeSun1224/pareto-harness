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
- Core `cargo test --workspace --all-targets --all-features --offline`: remediation 后 passed（kernel 14、protocol 9+17；0 failed，既有 observation ignored）。
- Governance `python -m unittest discover -s scripts/tests -p "test_*.py"`: passed, 18 tests。
- Hygiene `git diff --check`: passed。
- 可复现本机 observation（Windows、debug、临时真实 SQLite、WAL/FULL、2026-08-23）：50 次顺序 append `100.245ms`，一次读取并重验 50 条 `22.0485ms`。这是观察基线，不是阈值或优化声明；Token/Provider cost 不适用。
- Independent Code Review：`9aab861` 初审 0 Blocker / 5 Major；`a7c73b3` focused re-review 已关闭 F-001 至 F-004，剩余 F-005。第二轮 remediation 将 SchemaSet/limits pin 移入 trusted authority，由 retained registry 按 exact identity 选择 reader；新增旧 set 重启读取、wrong digest、alternate admitted set 替换拒绝，并明确覆盖 RAII drop rollback、busy 与提交响应丢失后的跨重启同 event-id retry。等待新精确提交 focused re-review；Major 仅可由原 reviewer 关闭。
