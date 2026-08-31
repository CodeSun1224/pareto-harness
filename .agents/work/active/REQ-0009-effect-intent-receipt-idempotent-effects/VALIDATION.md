# REQ-0009 Validation Evidence

## Subject

- Requirement: REQ-0009 (`implementing`，待独立实现评审)
- Spec/RFC/ADR: SPEC-0008 / RFC-0009 / ADR-0010
- Accepted design revision: `60cee6ed44d150185bf99ca3095a8ce803bcc0d3`
- Implementation candidate: 包含本文件、Effect实现和最终SchemaSet的exact Git commit；提交后作为fresh independent reviewer的固定输入
- Environment: Windows PowerShell，2026-08-31，Asia/Shanghai；全部Cargo命令使用`--offline`

## Results

| Scope/layer | Command or procedure | Result | Notes |
|---|---|---|---|
| Effect focused | 19个`assert_cargo_test_filter.py`命令 | passed；每项matched 1并运行1/1 | 覆盖AC-02至AC-19、AC-22的命名测试 |
| Protocol focused | `cargo test -p pareto-protocol --test protocol_contract effect_contract_manifest_events_and_inventory_v2_are_closed --offline -- --exact` | passed；1/1 | Manifest v3、Effect contracts/events、Inventory V2 |
| Event Store impacted | `cargo test -p pareto-kernel event_store --offline` | passed；179 passed、1 ignored | ignored为既有非阈值performance observation |
| Workspace full | `cargo test --workspace --all-targets --all-features --offline` | passed | Kernel 179 passed/1 ignored；Protocol 9 unit + 25 contract passed/1 ignored |
| Scope/static | `python scripts/check_req0009_scope.py` | passed | SQLite v2、retained sets、Fake-only、Replay read-only、依赖不变 |
| Schema identity | generator运行两次并逐文件SHA-256比较 | passed；88 files byte-identical | current set `sha256-ed5482a4ce2e593782f8909cf3a11e75759aa656ce3673eac1a18b7e2d3ec241` |

## Focused and layered tests

19个Kernel exact filters均由wrapper先列出测试并证明`matched: 1`：`default_deny`、
`intent_before_dispatch`、`idempotency`、`dispatch_lease`、`fake_outcomes`、
`receipt_admission`、`state_model`、`partial_success`、`crash_recovery`、
`reconciliation`、`atomic_settlement`、`cancellation_timeout`、`late_receipts`、
`fold_contract`、`isolation`、`projection_recovery`、`recorded_replay`、
`compatibility`和`lifecycle_success_guard`。完整路径均为
`event_store::effect_runtime::tests::<name>`；没有把零测试Cargo成功计作证据。

| Layer | Command | Result |
|---|---|---|
| Lifecycle | `cargo test -p pareto-kernel lifecycle:: --offline` | 18 passed |
| Projection/replay/snapshot | `cargo test -p pareto-kernel projection:: --offline` | 35 passed、1 ignored |
| Runtime Control | `cargo test -p pareto-kernel runtime_control:: --offline` | 53 passed |
| Hook regression | `cargo test -p pareto-kernel hook_runtime:: --offline` | 39 passed |
| Kernel all targets/features | `cargo test -p pareto-kernel --all-targets --all-features --offline` | 179 passed、1 ignored |
| Protocol all targets/features | `cargo test -p pareto-protocol --all-targets --all-features --offline` | 9 unit + 25 contract passed、1 ignored |
| Public boundary doctest | `cargo test -p pareto-kernel --doc --offline` | 1 compile-fail doctest passed |

## Scope and identity

- `EffectRequestV1`在Kernel admission中按exact retained SchemaSet验证顶层与内层request
  Schema、canonical size、subject/Task/deadline/correlation；Kernel重算request digest及绑定
  full scope/registry/effect revision/kind/idempotency key的Effect ID。
- Intent前先完成lifecycle/capability/budget准入，并以Control+Effect双Event原子提交；claim
  提交后才生成crate-private sealed dispatch lease并调用确定性Fake executor。
- Receipt错误producer/adapter/schema/limits只产生脱敏`effect-message-rejected` audit；scope、
  effect、attempt或external identity不匹配不写入。tenant、user presence/value、Workspace、
  Run和Actor矩阵均fail-closed且Event总数不变。
- recovery exact retry核对已提交Control/Effect Event内容及command fingerprint；same IDs异命令
  冲突。未claim结论为not-applied；claim后unknown/partial打开显式reconciliation且不redispatch。
- Recorded replay只读取Inventory V2固定Effect cursor/history digest，Inventory之后late Event不改变
  同一pin；Reexecute拒绝，未获得writer/executor/reserve/settlement authority。
- 相对设计接受基线，workspace/crate manifests与`Cargo.lock`无diff；`cargo tree --workspace
  --offline`成功。已删除三个未提交的中间Schema候选目录；它们可由generator重建，最终只保留
  `sha256-ed548...`候选及全部历史retained sets。

## Completion gates before independent review

| Command | Result |
|---|---|
| `python -m unittest discover -s scripts/tests -p "test_*.py"` | 27 passed |
| `python scripts/check_docs.py` | expected pre-review failure：当前行为diff尚未固定到fresh implementation Review，因此REVIEW-0001..0011 freshness被正确判定stale；不得表示为通过 |
| `cargo fmt --all -- --check` | passed |
| `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | passed |
| `cargo test --workspace --all-targets --all-features --offline` | passed |
| `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas` | passed twice；88 files byte-identical |
| `git diff --check` | passed |
| dependency manifest diff against `60cee6e` | passed；no diff |

Protocol publisher负向测试会打印`existing content-addressed schema set differs byte-for-byte`，
其测试与进程退出码仍为绿色；该输出证明tampered existing set被拒绝。

## Quality, cost, and latency

- Quality：Intent-before-dispatch、same-key mutation、双stream原子pair、scope隔离、partial/unknown、
  crash recovery、reconciliation、success guard和fixed-horizon replay均有确定性测试证据；尚未把
  fresh independent code review表示为完成。
- Cost：无模型、Provider或付费外部系统调用；unknown usage保守核算，不声明成本优化。
- Latency：测试只使用FakeClock，无真实sleep；既有SQLite/protocol performance observation保持
  ignored的非阈值观察，不声明延迟改善。

## Pending gate

下一步由新的fresh independent Agent使用`code-review`检查固定实现候选。任何Blocker/Major必须
由实现者整改，并由同一Reviewer复审关闭；随后复跑完整门禁、同步implemented facts并完成归档。
