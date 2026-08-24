---
id: REVIEW-0004
title: REQ-0005 Run/Task 状态机与 Run Manifest 独立代码评审
status: approved
owners: [independent-reviewer]
created: 2026-08-24
updated: 2026-08-24
links: [REQ-0005, SPEC-0004, RFC-0004, ADR-0005, REQ-0003, REQ-0004, REVIEW-0002, REVIEW-0003]
independence: independent
reviewed_revision: 675e3f8fe6888c1d01fec14dda8e0f9164bb8a1b
open_blockers: 0
open_majors: 0
---

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `crates/pareto-kernel/src/event_store/lifecycle.rs:355-368`; `crates/pareto-kernel/src/event_store/lifecycle/tests.rs:733-788` | 初审发现 `transition_task` 在 aggregate sequence 前查询 Task。`38f0beb` 先判 sequence，再解析 Task并判 expected state；真实 SQLite test 同时覆盖 stale+existing、stale+missing 和 current+missing，拒绝后event count保持2。 | 独立复跑 `cargo test -p pareto-kernel lifecycle::conflict_priority --offline`：1 passed；源码顺序与SPEC固定优先级一致。 | closed |
| F-002 | Major | `crates/pareto-kernel/src/event_store/lifecycle.rs:225-230,273-278,325-330,397-419`; `crates/pareto-kernel/src/event_store/lifecycle/tests.rs:790-858` | 初审的unchecked `expected_sequence + 1` 已替换为同事务 `command_sequence`：checked add只接受可形成established event的sequence；非法数值在authority/fold之后按全局ID存在性保持idempotency-conflict优先，否则返回optimistic conflict。新增矩阵覆盖三种命令的MAX/-1/0、正常exact retry、ID collision和无append。 | 独立复跑 `cargo test -p pareto-kernel lifecycle::sequence_boundaries --offline`：1 passed；workspace debug测试无panic，代码不存在release wrap。 | closed |
| F-003 | Major | `crates/pareto-kernel/src/event_store/lifecycle.rs:581,603-655`; `crates/pareto-protocol/src/validation.rs:658-722`; `crates/pareto-kernel/src/event_store/lifecycle/tests.rs:1385-1521` | `675e3f8` 让crate-private `fold_lifecycle`显式接收persisted identity解析出的exact `SchemaSet`，在构造任何`LifecycleState`前对首事件nested Manifest执行`validate_run_manifest`，再校验该set与opaque `ValidatedEvent` admission identity一致。loader与standalone fold因此复用同一完整Manifest semantic admission；scope/run/actor/stream/reader binding、determinism和model oracle保持不变。新增wrong Manifest schema_ref、self-referential `RecordedReplay`、缺source的Derived simulation三种负测，均先成功通过Event JSON admission，再由fold返回`AggregateCorrupt`。 | 独立逐行检查exact diff `38f0beb..675e3f8`；复跑`cargo test -p pareto-kernel lifecycle:: --offline`为18 passed，workspace为Kernel 33、Protocol unit 9、contract 19 passed，doctest/clippy/fmt/governance及diff/schema/dependency检查均passed。未改变事务边界、public API、wire/Schema、DDL、reader identity表示或依赖。 | closed |
| F-004 | Major | `crates/pareto-protocol/tests/protocol_contract.rs:440-543`; `crates/pareto-kernel/src/event_store/tests.rs:42-132,883-945`; `crates/pareto-kernel/src/event_store/lifecycle/tests.rs:1173-1284` | `38f0beb` 现在从两个真实checked-in旧目录加载manifest/documents并用各自exact reader验证旧Manifest、拒绝current reader；REQ-0004 Event fixture从历史`sha256-68535b...` set演化，持久事件重开后exact reader成功、current/错误reader失败且`user_version=1`；lifecycle测试还注入unknown major并确认load fail closed。 | 独立复跑旧Manifest定向测试1 passed、`lifecycle::compatibility` 1 passed、`cargo test -p pareto-kernel event_store --offline` 33 passed；retained路径、digest、reader方向和DB版本均由assertion而非日志声明证明。 | closed |
| F-005 | Minor | `.agents/work/active/REQ-0005-run-task-state-machine/VALIDATION.md:3-9,44-70`; `.agents/work/active/REQ-0005-run-task-state-machine/PLAN.md:12-14`; `HANDOFF.md`; `TASKS.md` | 旧的design-only/Runtime未开始/全部skipped矛盾已删除，四份work记录一致承认reviewing与首轮open findings。残余陈旧点是exact `38f0beb` 已提交后，VALIDATION/PLAN/TASKS/HANDOFF仍称remediation在working tree、等待“新exact revision”；这是非产品Minor，不能冒充最终handoff新鲜度。 | 最终handoff前把remediation subject和四份记录固定到`38f0beb...`或后续closure revision；保持accepted Minor，不影响本轮Major计数。 | accepted |

# Verdict

Approved。第二次focused independent re-review的精确 remediation revision `675e3f8fe6888c1d01fec14dda8e0f9164bb8a1b` 有0个open Blocker、0个open Major，通过REQ-0005 independent code-review gate。F-003 required proof完整：standalone fold显式接收exact SchemaSet并在任何状态fold前执行与loader相同的完整Manifest semantic admission；JSON-admitted但Manifest语义非法的三类首事件均fail closed，原mixed identity、determinism、model、load及全仓回归不退化。F-005仍是accepted Minor，留待最终handoff freshness更新，不阻断批准。

首轮评审按用户指定基线检查了`5ef949dd084b1e6ae82015f4c66adb8281aebf65..d0011d064b88cf4be8e6b70ae781f3637bb15161`全diff；第一次focused re-review检查`d0011d0..38f0beb`；本轮逐行检查exact remediation diff `38f0beb710fdd05ffd7b7047db6e3cb7cb7a2f79..675e3f8fe6888c1d01fec14dda8e0f9164bb8a1b`并独立复跑required proof。`675e3f8`的direct parent与本轮baseline均为exact `38f0beb`。

# Acceptance trace

| Acceptance | Review result | Evidence |
|---|---|---|
| AC-01 | 满足主要合同 | Create authority逐项 exact 比对 scope、八角色、plan、SchemaSet、budget、limits、recording policy和mode；协议层闭合 Manifest再验证。缺省补齐路径未发现。 |
| AC-02 | 实现满足主要原子路径，证据不完整 | Manifest只存在 sequence-1 `RunCreated` payload；单次 insert/commit在一个 SQLite事务中且fresh connection可见。现有 creation test未单独覆盖计划中的transaction drop/injected commit uncertainty，但底层REQ-0004 RAII/retry回归通过。 |
| AC-03 | 满足 | Run 6态、Task 7态和声明边与Spec一致；terminal无outgoing，guard实现与表格一致。 |
| AC-04 | 满足当前owner-only范围 | 生命周期模块和authority均crate-private；persisted first event建立exact owner，wrong actor/scope先于全局event-ID查询拒绝；未宣称通用Capability。 |
| AC-05 | 满足 | same-ID exact retry、mutation/cross-aggregate conflict、two-pool single winner、Task stale priority和非法sequence域均通过；`command_sequence`在完整authority/fold后保持idempotency先于expected。 |
| AC-06 | 实现主逻辑满足，负测矩阵偏窄 | exact retry先于terminal/version；新ID terminal transition不追加。测试主要覆盖cancelled late success，未完整枚举failed late/same-target，但共享terminal逻辑明确。 |
| AC-07 | 满足主要合同 | parent必须更早存在、Task ID唯一、Run created-only creation、parent/child/Run guards及无隐式cascade均由代码和真实SQLite hierarchy路径覆盖。 |
| AC-08 | 当前command path满足；standalone fold受F-003阻断 | established commands确在同一 `BEGIN IMMEDIATE` 内reader/authority/fold/idempotency/expected/guards/append；无第二Manifest/state表。fold已绑定event admission identity，但尚未绑定完整Manifest semantic admission。 |
| AC-09 | 满足当前loader/recovery合同 | reopen恢复、missing registry、非法edge、gap、unknown major、wrong/current reader、REQ-0004 row drift均fail closed；loader显式执行`validate_run_manifest`。 |
| AC-10 | command/reader路径满足；standalone fold仍受F-003阻断 | 完整scope/actor/derived stream和跨aggregate ID负测通过；新增fold mixed-identity矩阵通过，但语义非法首Manifest仍可由standalone fold接受。 |
| AC-11 | 满足 | 新set确定生成、旧set byte-identical；两个checked-in old Manifest exact reader、current substitution拒绝、历史Event fixture reopen、unknown major和`user_version=1`均有执行断言。 |
| AC-12 | 部分满足，受F-003阻断 | 原状态/model/hierarchy/race/terminal矩阵及新增priority、sequence、mixed identity、old-reader测试均通过；Manifest semantic fold负测缺失。 |
| AC-13 | Runtime/static regression通过，completion未通过 | workspace 33+9+19、doctest、fmt、clippy、governance、Schema generation和diff-check通过；本Review仍有1 open Major，旧Reviews freshness与最终handoff门禁不得关闭。 |

# Compatibility, permission, and isolation review

- `pareto-protocol`继续不依赖Kernel/sqlx；Cargo manifests/lock在requested baseline到reviewed revision间无变化，没有新增第三方依赖、Provider、网络、CLI或真实模型调用。
- `ValidatedEvent`新增private SchemaSet/limits admission fields和只读accessors；值只能由opaque trusted context成功admission产生，不新增wire字段、JSON Schema或外部constructor，现有协议Schema/digest byte-identical。该API是additive，workspace与retained-reader回归通过。
- SQLite v1 DDL、`user_version`、append-only triggers和store identity未改变；重构仅抽出transaction-local `PreparedEvent`、idempotency check和insert helper。未发现第二Manifest/state权威表或public raw sqlx/transaction API。
- established path顺序保持 `BEGIN IMMEDIATE` → sequence-1 persisted reader → exact Manifest/owner authority → exact-stream row validation/fold → command sequence/idempotency → expected/guards → one append。非法numeric sequence时，`command_sequence`只在authority/fold后查询global event-ID presence以维持generic idempotency优先；它不返回原aggregate/result，也未在authority/fold前查询。
- Manifest/Event row SchemaSet和limits exact match由首行读取、registry exact resolution、`validate_row`及nested Manifest equality共同检查；unknown/missing/current替代reader均fail closed。实际checked-in retained readers与历史Event fixture已关闭F-004。
- Run/Workspace/tenant/user/agent/actor隔离由SQL完整key和authority实现；pure fold现在继承event identity binding，但F-003指出脱离loader后仍未继承完整Manifest semantic admission，不能据此扩张REQ-0006声明。

# Regression and test review

Reviewer在Windows/PowerShell、2026-08-24、offline依赖条件下独立执行：

- `cargo test -p pareto-kernel lifecycle:: --offline`：15 passed。
- `cargo test --workspace --all-targets --all-features --offline`：Kernel 30 passed；Protocol unit 9 passed；contract 18 passed；observation 1 ignored；publisher drift子进程的预期stderr不代表suite失败。
- `cargo test -p pareto-kernel --doc --offline`：1 compile-fail passed。
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`：passed，0 warnings。
- `cargo fmt --all -- --check`：passed。
- `python -m unittest discover -s scripts/tests -p "test_*.py"`：18 passed。
- `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas`：passed；随后 `git diff --exit-code -- schemas`：passed，生成未改tracked bytes。
- `git diff --check 5ef949d..d0011d0`：passed。
- `python scripts/check_docs.py`：failed only on REVIEW-0001/0002/0003 freshness against the substantive REQ-0005 diff；这不是Runtime test failure，也不能在open Major存在时由实现者伪装为完成。

首轮测试全部绿仍未覆盖F-001至F-004；下列focused re-review以新增断言而非实现者closure声明重新判定。

Focused re-review在同一环境对exact `38f0beb`独立执行：

- `cargo test -p pareto-kernel lifecycle::conflict_priority --offline`：1 passed，关闭F-001。
- `cargo test -p pareto-kernel lifecycle::sequence_boundaries --offline`：1 passed，关闭F-002。
- `cargo test -p pareto-kernel lifecycle::fold_identity --offline`：1 passed；证明mixed identity拒绝，但未覆盖F-003剩余的Manifest semantic admission。
- `cargo test -p pareto-protocol checked_in_old_writer_manifests_use_their_exact_retained_reader --offline`：1 passed。
- `cargo test -p pareto-kernel lifecycle::compatibility --offline`：1 passed。
- `cargo test -p pareto-kernel event_store --offline`：33 passed，关闭F-004。
- `cargo test --workspace --all-targets --all-features --offline`：Kernel 33、Protocol unit 9、contract 19 passed；observation 1 ignored；publisher drift子进程预期stderr不代表失败。
- `cargo test -p pareto-kernel --doc --offline`：1 compile-fail passed。
- workspace clippy `-D warnings`、fmt、18 governance tests、Schema generation、`git diff --exit-code -- schemas`及`git diff --check d0011d0..38f0beb`全部passed。

全绿测试不关闭F-003：`fold_identity`只变异scope/actor/stream与Manifest scope，未构造JSON-Schema-valid但`validate_run_manifest`-invalid的首Manifest。

# Scope and unrelated changes

remediation diff集中在ValidatedEvent in-memory admission identity、lifecycle command/fold、retained-reader和真实SQLite负测及REQ-0005 work evidence。未增加依赖、Schema bytes、DDL、`user_version`、第二权威表、public SQL/transaction API、Projection/Replay executor、Capability、Provider、Agent Loop、Task DAG或网络调用。

requested baseline额外包含三份既有独立Review的freshness-only commit `8d37f41`；未改变finding disposition或产品行为。F-005记录的交付文档陈旧文本属于REQ-0005范围，不是产品逻辑扩张。

# Re-review history

- 2026-08-24：fresh independent review of exact `d0011d064b88cf4be8e6b70ae781f3637bb15161` against requested baseline `5ef949dd084b1e6ae82015f4c66adb8281aebf65`；0 Blocker、4 Major、1 Minor，结论 changes requested。后续必须在新exact revision上检查remediation diff与原始测试证据；实现者不能自行关闭F-001至F-004。
- 2026-08-24：focused independent re-review of exact `38f0beb710fdd05ffd7b7047db6e3cb7cb7a2f79` and diff `d0011d0..38f0beb`；F-001/F-002/F-004 closed，F-003 partial remediation后保持open，F-005 residual stale exact-revision wording保持accepted Minor。最终0 Blocker、1 Major，仍为changes requested。
- 2026-08-24：second focused independent re-review of exact `675e3f8fe6888c1d01fec14dda8e0f9164bb8a1b` and diff `38f0beb..675e3f8`；F-003 required proof完整并closed，F-005保持accepted Minor。最终0 Blocker、0 Major，结论approved；REQ-0006可直接复用带exact SchemaSet完整Manifest admission的pure fold，无需重释历史准入合同。
