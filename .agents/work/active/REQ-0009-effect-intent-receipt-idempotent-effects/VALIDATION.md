# REQ-0009 Validation Evidence

## Subject

- Requirement: REQ-0009 (`reviewing`，REVIEW-0013整改后待同一Reviewer复审)
- Spec/RFC/ADR: SPEC-0008 / RFC-0009 / ADR-0010
- Accepted design revision: `60cee6ed44d150185bf99ca3095a8ce803bcc0d3`
- Initial implementation candidate: `6cad604ffe5ec2126f9745bf22ece713f2c0ce85`
- Remediation candidate: 包含FIX-0002、本文件、Effect修复和最终SchemaSet的exact Git commit；提交后作为REVIEW-0013同一independent reviewer的固定输入
- Environment: Windows PowerShell，2026-08-31至2026-09-01，Asia/Shanghai；全部Cargo命令使用`--offline`

## Results

| Scope/layer | Command or procedure | Result | Notes |
|---|---|---|---|
| Effect focused | 19个`assert_cargo_test_filter.py`命令 + Effect模块全集 | passed；每个原始filter matched 1并运行1/1；模块23/23 | 同时覆盖REVIEW-0013九项整改负测 |
| Protocol focused | `cargo test -p pareto-protocol --test protocol_contract effect_contract_manifest_events_and_inventory_v2_are_closed --offline -- --exact` | passed；1/1 | Manifest v3、Effect contracts/events、Inventory V2 |
| Event Store impacted | `cargo test -p pareto-kernel event_store --offline` | passed；184 passed、1 ignored | ignored为既有非阈值performance observation |
| Workspace full | `cargo test --workspace --all-targets --all-features --offline` | passed | Kernel 184 passed/1 ignored；Protocol 9 unit + 25 contract passed/1 ignored |
| Scope/static | `python scripts/check_req0009_scope.py` | passed | SQLite v2、retained sets、Fake-only、Replay read-only、依赖不变 |
| Schema identity | generator运行两次并逐文件SHA-256比较 | passed；88 files byte-identical | remediation set `sha256-70389ae3f20ce4428ee0a8b1ecd6ddf1b6c48474982d6372ffea69e6fc7ba390`；初始`ed548…` set未改写 |

## Focused and layered tests

19个Kernel exact filters均由wrapper先列出测试并证明`matched: 1`：`default_deny`、
`intent_before_dispatch`、`idempotency`、`dispatch_lease`、`fake_outcomes`、
`receipt_admission`、`state_model`、`partial_success`、`crash_recovery`、
`reconciliation`、`atomic_settlement`、`cancellation_timeout`、`late_receipts`、
`fold_contract`、`isolation`、`projection_recovery`、`recorded_replay`、
 `compatibility`和`lifecycle_success_guard`。完整路径均为
`event_store::effect_runtime::tests::<name>`；没有把零测试Cargo成功计作证据。

REVIEW-0013整改新增或强化的独立证明为
`claim_revalidates_cancellation_and_deadline_under_writer_lock`、
`authenticated_invalid_receipt_settles_unknown_and_is_audited`、
`pair_counterpart_loss_fails_effect_and_control_reads_closed`、
`projection_reopens_losslessly_for_unclaimed_and_partial_effects`及
`effect_v3_digest_golden`；原有`dispatch_lease/fake_outcomes/crash_recovery/reconciliation/
lifecycle_success_guard/digest_golden`也扩展了wrong implementation、fault terminal、new-sample、
伪造source、Task scope与历史identity断言。Effect模块全集23/23通过。

| Layer | Command | Result |
|---|---|---|
| Lifecycle | `cargo test -p pareto-kernel lifecycle:: --offline` | 18 passed |
| Projection/replay/snapshot | `cargo test -p pareto-kernel projection:: --offline` | 35 passed、1 ignored |
| Runtime Control | `cargo test -p pareto-kernel runtime_control:: --offline` | 53 passed |
| Hook regression | `cargo test -p pareto-kernel hook_runtime:: --offline` | 39 passed |
| Kernel all targets/features | workspace命令中的Kernel target | 184 passed、1 ignored |
| Protocol all targets/features | `cargo test -p pareto-protocol --all-targets --all-features --offline` | 9 unit + 25 contract passed、1 ignored |
| Public boundary doctest | `cargo test -p pareto-kernel --doc --offline` | 1 compile-fail doctest passed |

## Scope and identity

- `EffectRequestV1`在Kernel admission中按exact retained SchemaSet验证顶层与内层request
  Schema、canonical size、subject/Task/deadline/correlation；Kernel重算request digest及绑定
  full scope/registry/effect revision/kind/idempotency key的Effect ID。
- Intent前先完成lifecycle/capability/budget准入，并以Control+Effect双Event原子提交；claim
  writer锁内重验Run/Task、reservation、cancellation与deadline；提交后才生成crate-private sealed
  dispatch lease。exact claim retry无lease且executor counter不变。
- Fake implementation compatibility digest与Manifest descriptor固定；Kernel私有orchestration执行
  claim→invoke→terminal admission，所有Fake fault均关闭Control reservation或进入显式reconciliation。
- wrong Receipt producer/adapter no-write；authenticated malformed/oversized/非规范结果形成脱敏audit，
  并以unknown conservative settlement原子关闭Control+Effect pair。Clock、result bytes、usage排序和
  schema均验证。
- recovery current epoch绑定canonical Clock；same IDs异命令冲突，terminal后的新sample/different IDs
  为ExistingTerminal式no-op。reconciliation逐source Event验证并由Kernel重算evidence fingerprint。
- Effect/Control权威读取双向验证counterpart、pair、prepared digests和pair fingerprint；Task success
  guard按exact Task过滤；Effect Projection无损保留identity、budget、pair、Receipt与evidence字段。
- Recorded replay只读取Inventory V2固定Effect cursor/history digest，Inventory之后late Event不改变
  同一pin；Reexecute拒绝，未获得writer/executor/reserve/settlement authority。
- 相对设计接受基线，workspace/crate manifests与`Cargo.lock`无diff；`cargo tree --workspace
  --offline`成功。已删除三个未提交的中间Schema候选目录；它们可由generator重建，最终只保留
  `sha256-70389...`整改候选、初始`sha256-ed548...`集合及全部更早retained sets。

## Completion gates before independent review

| Command | Result |
|---|---|
| `python -m unittest discover -s scripts/tests -p "test_*.py"` | 27 passed |
| `python scripts/check_docs.py` | expected pre-re-review failure：FIX状态已修正；当前行为diff尚未固定到REVIEW-0013复审revision，因此旧Review freshness被正确判定stale；不得表示为通过 |
| `cargo fmt --all -- --check` | passed |
| `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | passed |
| `cargo test --workspace --all-targets --all-features --offline` | passed |
| `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas` | passed连续两次；88 files byte-identical |
| `git diff --check` | passed |
| dependency manifest diff against `60cee6e` | passed；no diff |

Protocol publisher负向测试会打印`existing content-addressed schema set differs byte-for-byte`，
其测试与进程退出码仍为绿色；该输出证明tampered existing set被拒绝。

## Quality, cost, and latency

- Quality：REVIEW-0013 F-001..F-009均已实现候选修复并有确定性负测；尚未由Reviewer关闭，故不把
  independent approval表示为完成。
- Cost：无模型、Provider或付费外部系统调用；unknown usage保守核算，不声明成本优化。
- Latency：测试只使用FakeClock，无真实sleep；既有SQLite/protocol performance observation保持
  ignored的非阈值观察，不声明延迟改善。

## Pending gate

下一步将整改固定为exact commit，由REVIEW-0013同一independent Reviewer逐项复审F-001..F-009。
只有Reviewer将open Blocker/Major归零并批准后，才同步最终freshness、复跑docs门禁并完成归档。
