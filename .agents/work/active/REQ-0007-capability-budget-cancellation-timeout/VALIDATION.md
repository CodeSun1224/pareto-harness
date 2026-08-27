# Validation Evidence

## Subject

- Requirement: REQ-0007 (`reviewing`)
- Spec: SPEC-0006 (`approved`)
- RFC/ADR: RFC-0006 / ADR-0007 (`accepted`)
- Git revision or diff: exact implementation candidate `9f979f0ccaa6be0431ca794f584fd0c6df83af9c`
- Environment: Windows PowerShell, 2026-08-27, Asia/Shanghai

## Results

| Scope/layer | Command or procedure | Result | Artifact/reference | Notes/risk |
|---|---|---|---|---|
| Project orientation | Read README/AGENTS/index/roadmap/backlog/EPIC/kernel architecture；REQ-0003..0006 Requirement/Spec/RFC/ADR/Review；REQ-0005/0006 archived Plan/Tasks/Handoff/VALIDATION；protocol/Event Store/lifecycle/projection/snapshot/replay code/tests；inspect Git/active work | passed | REQ-0007/SPEC-0006 evidence paths | REQ-0005/0006 done；startup clean；active only `.gitkeep`；prerequisite satisfied |
| Risk classification | Apply project-orientation, impact-analysis and sdd-delivery | passed | REQ-0007 `risk: high`; SPEC-0006 impact table | permissions/security/resources/concurrency/cancel/replay require high-risk path |
| Requirement/test planning | Apply requirement-authoring and test-planning；map every AC/risk to named non-zero command | passed | REQ-0007 AC-01..20；SPEC-0006 Test traceability；PLAN Validation | No vague “relevant tests” entry |
| RFC/ADR | Apply rfc-authoring to cross-requirement authority/budget/time/late/replay semantics | passed | RFC-0006 accepted；ADR-0007 accepted | Alternatives, failure, compatibility, rollback and separate Q/C/L covered |
| Architecture/security self-review | Apply architecture-review to request→capability→budget→operation→event→projection→recovery | superseded as gate | `ARCHITECTURE-REVIEW.md` | 仅历史self-review；不能批准设计或实施 |
| Independent design review | Fresh Agent review exact `05dd7ca` with architecture-review | failed / changes requested | `docs/reviews/REVIEW-0006-capability-budget-cancellation-timeout-design.md` | 0 Blocker、6 Major：lifecycle、cancel authority、callback producer、timeout recovery、Manifest/SchemaSet、trusted envelope；Runtime paused |
| Design remediation candidate | Revise REQ/SPEC/RFC/ADR/Plan for REVIEW-0006 F-001..F-007；remove all post-gate Runtime declarations；`python -m unittest discover -s scripts/tests -p "test_*.py"`; `git diff --check` | passed | design working tree before focused commit | 18 governance tests passed；diff check passed；worktree contains documents only |
| Design remediation docs gate | `python scripts/check_docs.py` | failed: freshness only | checker output before focused commit | REVIEW-0001..0005 reviewed revisions predate new REQ-0007 design paths；same independent reviewer must substantively verify unchanged earlier contracts and restore freshness；no parser/link/finding-format error reported |
| F-004 identity remediation candidate | Freeze TimeoutKey/command-event ID/fingerprint、not_due、response-loss、same/different-ID priority in REQ/SPEC/RFC/ADR；`python -m unittest discover -s scripts/tests -p "test_*.py"`; `git diff --check`; `python scripts/check_docs.py` | governance/diff passed；docs freshness pending | working tree after REVIEW-0006 focused re-review | 18 governance tests passed；diff check passed；docs checker only reports REVIEW-0001..0005 stale against the new unreviewed F-004 design paths，expected until next exact independent re-review |
| Independent design approval | Same fresh reviewer focused re-review of exact `a4e34785908207e622365250ae1466b85b4baecb`; `python scripts/check_docs.py`; `git diff --check` | passed | REVIEW-0006 approved | F-001..F-006 closed；0 open Blocker/Major；170 Markdown/49 IDs；review freshness substantively restored；Runtime implementation unlocked |
| Existing governance baseline | `python -m unittest discover -s scripts/tests -p "test_*.py"`; `python scripts/check_docs.py` | passed | 18 tests；160 Markdown/44 formal IDs before REQ-0007 | Existing checker behavior green before edits |
| Existing Core baseline | `cargo test --workspace --all-targets --all-features --offline` | passed | Kernel 68 passed/1 ignored；Protocol 9 unit + 21 contract/1 ignored | Expected publisher-drift stderr belongs to passing negative fixture |
| Approved design docs | `python scripts/check_docs.py`; `git diff --check` before active work creation | passed | 164 Markdown files；48 formal IDs | Only REQ-0007 Requirement/Spec/RFC/ADR present；no Runtime code |

## Implementation candidate results

| Scope/layer | Command or procedure | Result | Artifact/reference | Notes/risk |
|---|---|---|---|---|
| Runtime focused | `cargo test -p pareto-kernel runtime_control --offline --no-fail-fast` | passed | 32 passed | FakeClock only；default deny、delegation、budget race、cancel/deadline/timeout、late/idempotency、reopen/replay |
| Named non-zero filters | `python scripts/assert_cargo_test_filter.py pareto-kernel <filter>` for PLAN filters | passed | each reported `matched: 1` | initial zero-match for `budget_concurrency` was rejected；test renamed/split then rerun green |
| Protocol contracts | capability/budget and Runtime Projection filters；all targets/features | passed | 9 unit + 23 contract；1 ignored observation | publisher drift stderr is expected passing negative fixture |
| Schema identity | generate final SchemaSet twice and hash all files | passed | `sha256-a1f960…`; 45 files stable | four retained sets untouched；stale agent-generated candidates removed |
| Event Store/Lifecycle | Event Store and Lifecycle filters | passed | 15 + 18 | DB v2/migration/authority/concurrency/recovery green |
| Projection/Snapshot/Replay | `cargo test -p pareto-kernel projection:: --offline --no-fail-fast` | passed after compatibility repair | 35 passed；1 ignored observation | new source set resolves lifecycle reducer while retained `4ce387…` output contract remains explicit |
| API/static scope | Kernel doctest；Kernel clippy `-D warnings`; `check_req0007_scope.py`; `git diff --check` | passed | doctest 1；scope checker green | no real I/O/sleep/dependency growth；frozen DB constants exact HEAD |
| Governance unit | `python -m unittest discover -s scripts/tests -p "test_*.py"` | passed | 20 tests | includes non-zero filter and scope helper tests |
| Full language gates | `cargo fmt --all -- --check`; workspace clippy `-D warnings`; `cargo test --workspace --all-targets --all-features --offline` | passed | Kernel 100 passed/1 ignored；Protocol 9 + 23 passed/1 ignored | no warning suppression or gate weakening |
| Docs freshness before implementation review | `python scripts/check_docs.py` | expected gate failure | only REVIEW-0001..0005 stale against implementation paths | fresh independent code review must substantively restore freshness；not treated as pass |

## REVIEW-0007 repair candidate

独立实现评审 REVIEW-0007 在 `1b40e92` 上记录 0 Blocker、9 open Major。FIX-0001
按 F-001 至 F-009 完成修复；本节结果属于实现者证据，不能自行关闭 finding。

| Scope/layer | Command or procedure | Result | Artifact/reference | Notes/risk |
|---|---|---|---|---|
| Trusted operation and meter | `cargo test -p pareto-kernel --all-features --offline runtime_control` | passed | 50 passed | retained contract reference only；Kernel meter/Fake Operation；超 envelope 在执行前拒绝；forged/unknown usage 负例 |
| Authority, time and recovery | 同一 focused command | passed | capability、cancel、callback、timeout、late、race、reopen 命名矩阵 | exact lifecycle cursor/event bindings；process epoch；frozen Clock sample；deterministic command ID；response-loss retry |
| Pure fold/projection/replay | 同一 focused command | passed | illegal capability/budget/cancel history、complete provenance、Recorded replay zero-dispatch | Schema-valid 但语义非法历史 fail closed；终态后结果只进入隔离审计 |
| Protocol/Schema | `cargo test -p pareto-protocol --all-features --offline`; schema generator twice | passed | 9 unit + 23 contract；1 ignored observation；`sha256:19566903…` | 新增 Kernel meter evidence；最终内容寻址 set 稳定；两个未发布中间 set 已删除 |
| Governance/static scope | `python -m unittest discover -s scripts/tests -p "test_*.py"`; `python scripts/check_req0007_scope.py`; workspace clippy `-D warnings` | passed | 21 tests；scope check green；clippy green | scope baseline 固定 `6de3598`，不再与 HEAD 自比较；DB v2/依赖/real I/O 未变 |
| Full regression | `cargo test --workspace --all-targets --all-features --offline` | passed | Kernel 118 passed/1 ignored；Protocol 9 + 23 passed/1 ignored | Projection golden 随最终 SchemaSet canonical identity 更新后全仓绿色 |
| Docs freshness before focused re-review | `python scripts/check_docs.py` | expected gate failure | only REVIEW-0001..0005 stale against repaired substantive paths | 原独立 REVIEW-0007 reviewer 必须在 exact repair revision 上复审并实质恢复 freshness |

## REVIEW-0007 second repair candidate

REVIEW-0007 对 `ab2fbc6d2e979ef12bcffd5df1cfe76b975a9684` focused re-review 后关闭
F-001/F-002/F-004/F-006，保留 F-003/F-005/F-007/F-008/F-009 五个 Major。
second repair implementation revision 固定为
`ea751c83d26d45ed91aa3cfbc1b2cd2c316e334e`。本轮实现者证据只证明候选具备复审条件，
finding 仍必须由同一独立 reviewer 关闭。

| Scope/layer | Command or procedure | Result | Artifact/reference | Notes/risk |
|---|---|---|---|---|
| Restart lease/recovery authority | `cargo test -p pareto-kernel runtime_control --all-features --offline` | passed | 52 passed | reopen 前 deadline 只读 rebind；旧 epoch/墙钟回退拒绝；recovery ack 必须携带 Kernel-sealed fact；无 re-dispatch/deadline extension |
| Pure fold and durable provenance | 同一 focused command | passed | forged reservation/cancel/lifecycle/adapter/timeout/settlement/late history fail closed | reservation、lifecycle checkpoint、producer/reservation/lease/process epoch authority 全部持久化并在 reopen/replay 重验 |
| Time/capability/model matrix | 同一 focused command | passed | exact not-before/expiry、deadline boundary、bounded complete/cancel/timeout sequences | 使用 FakeClock；每序列唯一终态、预算守恒、late audit 唯一且 replay 相等 |
| Protocol/Schema | `cargo test --workspace --all-targets --all-features --offline`; schema generator twice | passed | Protocol 9 unit + 23 contract；1 ignored observation；`sha256:c3e2fda5…` | 最终内容寻址 set 连续生成稳定；未批准中间 `195669…` 被替换；既有 retained sets 未改写 |
| Governance/static scope | governance unittest；`check_req0007_scope.py`；fmt；workspace clippy `-D warnings`；`git diff --check` | passed | 21 tests；scope/fmt/clippy/diff green | DB v2、依赖与 real I/O 边界未变 |
| Full regression | `cargo test --workspace --all-targets --all-features --offline` | passed | Kernel 120 passed/1 ignored；Protocol 9 + 23 passed/1 ignored | schema publisher drift stderr 是通过中的负例夹具 |

## REVIEW-0007 third repair candidate

第二次 focused independent re-review 在 exact `26b63ca2abb99bf3d6216d395994d006c1b3e2b5`
关闭 F-003/F-008，保留 F-005/F-007/F-009 三个 Major。本节仍是实现者证据，不能自行
关闭 finding。third repair implementation revision 固定为
`693299cb1dbe5fa5c75729445bcd8f1054389731`。

| Scope/layer | Command or procedure | Result | Artifact/reference | Notes/risk |
|---|---|---|---|---|
| Stop/recovery boundary | `cargo test -p pareto-kernel runtime_control --all-features --offline` | passed | 53 passed；`interruptibility`、reopen/rebind tests | 删除仅凭epoch rebind形成`kernel_recovery` ack的通道；rebind只读且不写ack；uninterruptible保持pending直到executor return或deadline timeout terminal |
| Settlement pure admission | 同一 focused command | passed | `compatibility_rejects_schema_valid_illegal_terminal_winner_history` | validly resealed wrong namespace、cancelled-without-request、success-after-cancel、success-at-deadline、timeout-before-deadline、wrong monotonic equation在Projection/Recorded replay/reopen均fail closed |
| Bounded command/concurrency model | 同一 focused command | passed | `model_sequences` | 23组duplicate/双向冲突/三步request-ack-terminal序列 + 2组真实SQLite concurrent writer race；逐步唯一terminal、合法outcome、预算守恒，final late exact retry和Recorded replay相等 |
| Late exact idempotency regression | 同一 focused command | passed | bounded model每个case exact retry | 修复terminal后late callback同event ID重试误按`operation-settled`比较而产生`idempotency_conflict`的问题；不重复audit/effect/accounting |
| Governance/static/full | governance unittest；scope；fmt；clippy；workspace test；schema generator；diff check | passed | Python 21；Kernel 121 passed/1 ignored；Protocol 9+23 passed/1 ignored | final SchemaSet仍为`sha256:c3e2fda5…`且generator无diff；DB v2/依赖/real I/O未变 |

## REVIEW-0007 fourth repair candidate

第三次 focused independent re-review 在 exact `97bca8b7b34ceadd5ab4f8ad01f49e10b3377adb`
关闭 F-005/F-009，仅保留 F-007 一个 Major。第四轮把 callback/ack decision monotonic
时间纳入durable authority，并补齐live/pure一致性；finding仍须由同一reviewer关闭。

| Scope/layer | Command or procedure | Result | Artifact/reference | Notes/risk |
|---|---|---|---|---|
| Decision/lease authority | Runtime focused and illegal terminal history filter | passed | 53 passed；新增`lease-after-settlement`与`meter-epoch-mismatch` | pure fold要求decision wall/monotonic不早于lease，non-timeout monotonic decision早于deadline，meter evidence epoch等于callback authority epoch |
| Protocol/Schema identity | Protocol contracts；schema generator | passed | 9 unit + 23 contract；`sha256:a95c824d…` | 新必填`decision_monotonic_millis`进入closed callback-authority Schema；替换未发布candidate `c3e2fda5…`；既有published retained sets不改写 |
| Projection compatibility | projection digest golden and full workspace | passed | 六个canonical digest golden按新source identity重算 | output reducer语义不变；source schema identity变化显式进入projection/snapshot provenance |

## Skipped tests

REVIEW-0007 fourth focused independent re-review and post-review full gate rerun remain pending. Real Provider/Tool/network/performance claims are out of scope；ignored tests are observation-only baselines already marked by the repository。

## Remaining limitations

- REVIEW-0006 approved design only；implementation still requires a new independent Review ID and exact revision。
- The historical architecture/security self-review cannot approve design or implementation.
- ProductionClock、background timeout、real Effect/Provider/Tool、Control Snapshot、distributed budget and downstream frameworks are intentionally absent.
- Fresh independent implementation code review with a new Review ID remains mandatory after exact implementation and raw validation evidence exist；it cannot reuse or overwrite REVIEW-0006.
