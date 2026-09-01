# REQ-0009 Validation Evidence

## Subject

- Requirement: REQ-0009 (`done`，fresh independent REVIEW-0013 approved)
- Spec/RFC/ADR: SPEC-0008 / RFC-0009 / ADR-0010
- Accepted design revision: `60cee6ed44d150185bf99ca3095a8ce803bcc0d3`
- Initial implementation candidate: `6cad604ffe5ec2126f9745bf22ece713f2c0ce85`
- Final reviewed implementation: `25e84603f09c3e3c47c29846e9cc3ef1fe6a4d72`
- Environment: Windows PowerShell，2026-08-31至2026-09-01，Asia/Shanghai；全部Cargo命令使用`--offline`

## Results

| Scope/layer | Command or procedure | Result | Artifact | Evidence/risk |
|---|---|---|---|---|
| Effect focused | 19个`assert_cargo_test_filter.py`命令 + Effect模块全集 | passed | 每个原始filter matched 1并运行1/1；模块24/24 | 覆盖REVIEW-0013九项整改及hybrid lineage负测 |
| Protocol focused | `cargo test -p pareto-protocol --test protocol_contract effect_contract_manifest_events_and_inventory_v2_are_closed --offline -- --exact` | passed | 1/1 | Manifest v3、Effect contracts/events、Inventory V2 |
| Event Store impacted | `cargo test -p pareto-kernel event_store --offline` | passed | 185 passed、1 ignored | ignored为既有非阈值performance observation |
| Workspace full | `cargo test --workspace --all-targets --all-features --offline` | passed | Kernel 185 passed/1 ignored；Protocol 9 unit + 25 contract passed/1 ignored | 无失败；ignored均为既有非阈值观察 |
| Scope/static | `python scripts/check_req0009_scope.py` | passed | SQLite v2、retained sets、Fake-only、Replay read-only、依赖不变 | 无边界扩张 |
| Schema identity | generator运行两次并逐文件SHA-256比较 | passed | 89 files byte-identical | 当前`sha256-0d323…`；`70389…`、`ed548…`及更早set未改写 |

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
伪造source、Task scope与历史identity断言；最终新增hybrid lineage fail-closed/no-write。Effect模块全集24/24通过。

REVIEW-0013对`7eeb5f6d4095b7d2fdc6cc225e9b60c89482063f`的同一Reviewer复审关闭
F-001/F-006/F-007/F-008/F-009，保留F-002/F-003/F-004/F-005四个Major。第二轮候选进一步证明：
Kernel用固定implementation digest解析sealed concrete Fake executor，wrong pin为0调用；
`CrashAfterReturn`在claim后/terminal前中断，关闭并reopen后只走recovery且调用计数不增加；
authenticated-invalid Receipt的Control terminal、Effect terminal与mandatory rejection audit在同一事务，
terminal后/audit前故障为零写入；recovery只接受KernelRecoveryClock签发并绑定scope/effect/attempt/cause的
authority，篡改authority no-write；reconciliation resolution只来自Manifest-pinned producer/adapter/
implementation生成的sealed query observation，command不再携带resolution/source/producer，wrong
implementation/producer/resolution与不存在lineage均no-write。上述第二轮修复待同一Reviewer关闭。

同一Reviewer对`fc1e3d968b87a8cfea987bea306f53b6a5b8c468`独立复审后确认F-002/F-003/F-004
可关闭，并发现F-005的最后一个可达性缺口：claim后recovery Unknown不含Receipt identity，旧admission
却只接受Receipt-backed source。最终整改把source验证严格分成完整Receipt-backed与Kernel recovery-backed
两种闭合形态；`fake_outcomes`现证明CrashAfterReturn→close/reopen→recovery Unknown→Manifest-pinned
sealed query observation→ResolvedNotApplied全链路可达，同时executor counter保持1。该最终整改待Reviewer关闭。

Reviewer对`f3bf18e0129f397c998032979b0bf19dc055ca56`确认上述可达性，但保留F-005一个Major：
Receipt-backed形态未要求result digest/合法reason，且writer/fold/source没有共用互斥validator。最新候选已将
exact validator接入三处；direct shape matrix覆盖missing result/identity与recovery+Receipt hybrid，数据库
reseal hybrid Event后projection/reconcile fail closed且reconcile Event count不变。定向hybrid、reconciliation、
fake_outcomes exact tests与Kernel clippy通过，待完整门禁和同一Reviewer最终关闭。

| Layer | Command | Result |
|---|---|---|
| Lifecycle | `cargo test -p pareto-kernel lifecycle:: --offline` | 18 passed |
| Projection/replay/snapshot | `cargo test -p pareto-kernel projection:: --offline` | 35 passed、1 ignored |
| Runtime Control | `cargo test -p pareto-kernel runtime_control:: --offline` | 53 passed |
| Hook regression | `cargo test -p pareto-kernel hook_runtime:: --offline` | 39 passed |
| Kernel all targets/features | workspace命令中的Kernel target | 185 passed、1 ignored |
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
  `sha256-0d323...`第二轮候选、`sha256-70389...`首轮整改集合、初始`sha256-ed548...`
  集合及全部更早retained sets。

## Completion gates before independent review

| Command | Result |
|---|---|
| `python -m unittest discover -s scripts/tests -p "test_*.py"` | 27 passed |
| `python scripts/check_docs.py` | pre-closure仅报告REVIEW-0001..0013 freshness；归档格式及其他文档检查通过。closure revision后刷新Review并要求最终exit 0 |
| `cargo fmt --all -- --check` | passed |
| `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings` | passed |
| `cargo test --workspace --all-targets --all-features --offline` | passed |
| `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas` | passed连续两次；89 files byte-identical |
| `git diff --check` | passed |
| dependency manifest diff against `60cee6e` | passed；no diff |

Protocol publisher负向测试会打印`existing content-addressed schema set differs byte-for-byte`，
其测试与进程退出码仍为绿色；该输出证明tampered existing set被拒绝。

## Quality, cost, and latency

- Quality：Reviewer已关闭F-001/F-006..F-009；F-002..F-005第二轮候选修复均有确定性负测，
  但尚未由Reviewer关闭，故不把independent approval表示为完成。
- Cost：无模型、Provider或付费外部系统调用；unknown usage保守核算，不声明成本优化。
- Latency：测试只使用FakeClock，无真实sleep；既有SQLite/protocol performance observation保持
  ignored的非阈值观察，不声明延迟改善。

## Final review and closure

Fresh independent REVIEW-0013批准exact `25e84603f09c3e3c47c29846e9cc3ef1fe6a4d72`，
F-001至F-009均由同一Reviewer关闭，最终0 Blocker、0 Major。Requirement已按
`reviewing → verified → done`闭环；本轮fact sync与归档不增加runtime行为。closure revision形成后刷新
全部Review freshness并复跑最终docs/仓库门禁。
