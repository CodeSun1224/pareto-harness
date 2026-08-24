---
id: REVIEW-0004
title: REQ-0005 Run/Task 状态机与 Run Manifest 独立代码评审
status: changes-requested
owners: [independent-reviewer]
created: 2026-08-24
updated: 2026-08-24
links: [REQ-0005, SPEC-0004, RFC-0004, ADR-0005, REQ-0003, REQ-0004, REVIEW-0002, REVIEW-0003]
independence: independent
reviewed_revision: d0011d064b88cf4be8e6b70ae781f3637bb15161
open_blockers: 0
open_majors: 4
---

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `crates/pareto-kernel/src/event_store/lifecycle.rs:337-346` | `transition_task` 在比较 aggregate `expected_sequence` 前先查询 Task。对“stale sequence + 不存在 Task”的 authorized 命令，当前实现返回 `invalid_transition`，而 SPEC-0004 固定优先级要求在幂等之后先返回 `optimistic_concurrency_conflict`。这使 Task 路径与 Run/CreateTask 路径的并发合同不一致，并让 stale caller 观察到本不应进入 guard 阶段的 Task-existence 分类。 | 先独立比较 aggregate sequence，再解析 Task/比较 expected state；增加真实 SQLite 负测证明 stale sequence + missing/existing Task 均为 optimistic conflict、current sequence + missing Task 才为 invalid transition，且所有拒绝均不追加事件。 | open |
| F-002 | Major | `crates/pareto-kernel/src/event_store/lifecycle.rs:237,279,326` | 三个 established command 路径在幂等和 expected-version 判定前直接计算 `expected_sequence + 1`。`i64::MAX` 在当前 debug/test 配置中 panic，在 release 中可能 wrap；负数也会先走 event validation 并被误分类为 `manifest_invalid`。Kernel 因此不能对完整命令域稳定返回结构化错误，且未保持 idempotency → expected sequence 的合同顺序。 | 使用 checked sequence derivation或先完成不溢出的 command admission；对 CreateTask、Run transition、Task transition 增加 `i64::MAX`、负数/零边界和正常 exact retry 测试，证明无 panic、无 append，并返回合同允许且优先级正确的稳定错误。 | open |
| F-003 | Major | `crates/pareto-kernel/src/event_store/lifecycle.rs:557-628`; `crates/pareto-kernel/src/event_store/lifecycle/tests.rs:1090-1165` | 承诺给 REQ-0006 作为 Projection/Replay oracle 的 `fold_lifecycle` 不是 aggregate-bound fold。它只检查首事件 type/sequence、后续连续 sequence、payload state/guards；不检查首个 Manifest 与首 envelope 的 scope/run/derived stream/actor binding，也不检查后续已验证事件与首事件属于同一 tenant/user/workspace/run/agent/stream。两个 aggregate 各自合法的 `ValidatedEvent` 只要 sequence 和状态衔接即可被混合并成功 fold。当前 `load_established` 的 SQL/read admission 会预先收窄事件，但 pure fold 自身无法安全成为 SPEC-0004 所声明的独立 correctness oracle，REQ-0006 必须重新解释/复制隔离前置条件。 | 引入不可伪造的 aggregate-bound validated range（或让 fold 显式验证完整首事件 binding 和每条 envelope identity），并增加 mixed tenant、user presence/value、workspace、run、agent、actor、stream、Manifest/envelope mismatch 负测；同一 exact range 的 deterministic state/digest 正例继续通过。 | open |
| F-004 | Major | `crates/pareto-kernel/src/event_store/lifecycle/tests.rs:1044-1088`; `crates/pareto-protocol/tests/protocol_contract.rs:368-446`; `.agents/work/active/REQ-0005-run-task-state-machine/VALIDATION.md` | AC-11/SPEC-0004 要求旧 SchemaSet byte identity **和 reader**、old-writer → new-reader、unknown lifecycle major及旧 Event DB reopen。现有 retained-set 测试只重算磁盘文件/digest；lifecycle compatibility 只覆盖空 registry和一个非法 edge；Event Store 的“retained reader”使用本次生成集派生的测试 set，不实例化两个已发布的 checked-in old sets。因而“旧目录仍在”已证明，但“旧 Manifest/Event 仍由 exact reader 读取且不被 current set 替代”没有原始执行证据。 | 从至少一个真实已发布旧内容地址目录加载 exact manifest/documents，构造 retained registry，读取/验证旧 Manifest 与旧 Event Store fixture并拒绝 current/alternate reader；增加 unknown lifecycle major fail-closed fixture，并记录旧 set 路径、digest、writer/reader方向和旧 DB `user_version` 不变的 reviewer-rerunnable evidence。 | open |
| F-005 | Minor | `.agents/work/active/REQ-0005-run-task-state-machine/VALIDATION.md:3-9,35-41`; `.agents/work/active/REQ-0005-run-task-state-machine/PLAN.md:12-14` | 交付记录内部矛盾：VALIDATION subject 与 skipped section仍称 design-only、Runtime 未开始/未运行，而同一文件中部已记录实现、30个Kernel测试和全仓门禁；PLAN current state也仍称 `planned`、尚未修改 Runtime/Schema。该陈旧文本会误导后续 reviewer 和 completion gate。 | 将 subject 固定到 exact implementation revision，删除/改写过期的 design-only/skipped 叙述，并使 PLAN/VALIDATION/HANDOFF/TASKS 对当前 reviewing 状态、实际执行命令和预期 docs freshness failure一致。 | accepted |

# Verdict

Changes requested。精确 revision `d0011d064b88cf4be8e6b70ae781f3637bb15161` 有 0 个 open Blocker、4 个 open Major，不能通过 REQ-0005 independent code-review gate。实现已正确建立 Manifest 首事件、同一 `BEGIN IMMEDIATE` 的 persisted reader/authority/fold/idempotency/guard/append 主路径、owner-only authority、单流状态机、并发单赢、terminal fail-closed、新不可变 SchemaSet和无第二权威表；但 F-001 至 F-004 分别阻断固定冲突语义、全命令域结构化失败、下游 pure-fold 隔离合同和旧 reader 兼容证据。

评审按用户指定基线检查了 `5ef949dd084b1e6ae82015f4c66adb8281aebf65..d0011d064b88cf4be8e6b70ae781f3637bb15161` 全 diff。Git 实际记录显示 `d0011d0` 的直接 parent 是 `8d37f416a827ea11a5f6b4200c28c39202f98b80`；`5ef949d..8d37f41` 仅为 REVIEW-0001/0002/0003 freshness 记录，产品实现 diff 为 `8d37f41..d0011d0`。两段均已审查，本 Review 仍只批准或拒绝 exact `reviewed_revision`。

# Acceptance trace

| Acceptance | Review result | Evidence |
|---|---|---|
| AC-01 | 满足主要合同 | Create authority逐项 exact 比对 scope、八角色、plan、SchemaSet、budget、limits、recording policy和mode；协议层闭合 Manifest再验证。缺省补齐路径未发现。 |
| AC-02 | 实现满足主要原子路径，证据不完整 | Manifest只存在 sequence-1 `RunCreated` payload；单次 insert/commit在一个 SQLite事务中且fresh connection可见。现有 creation test未单独覆盖计划中的transaction drop/injected commit uncertainty，但底层REQ-0004 RAII/retry回归通过。 |
| AC-03 | 满足 | Run 6态、Task 7态和声明边与Spec一致；terminal无outgoing，guard实现与表格一致。 |
| AC-04 | 满足当前owner-only范围 | 生命周期模块和authority均crate-private；persisted first event建立exact owner，wrong actor/scope先于全局event-ID查询拒绝；未宣称通用Capability。 |
| AC-05 | 部分满足，受F-001/F-002阻断 | same-ID exact retry、mutation conflict、跨aggregate ID reuse和two-pool single winner通过；Task stale conflict priority及sequence全域不满足。 |
| AC-06 | 实现主逻辑满足，负测矩阵偏窄 | exact retry先于terminal/version；新ID terminal transition不追加。测试主要覆盖cancelled late success，未完整枚举failed late/same-target，但共享terminal逻辑明确。 |
| AC-07 | 满足主要合同 | parent必须更早存在、Task ID唯一、Run created-only creation、parent/child/Run guards及无隐式cascade均由代码和真实SQLite hierarchy路径覆盖。 |
| AC-08 | 当前command path满足；downstream fold受F-003阻断 | established commands确在同一 `BEGIN IMMEDIATE` 内load/fold后idempotency/expected/guards/append；无第二Manifest/state表。独立pure fold尚未携带aggregate binding。 |
| AC-09 | 部分满足 | reopen恢复、missing registry、非法stored edge、gap fold和REQ-0004 row drift回归通过；exact old reader/alternate lifecycle set和完整corruption矩阵不足，见F-004。 |
| AC-10 | 当前SQL/authority路径满足主要隔离；pure fold受F-003阻断 | tenant、user presence/value、workspace、run、agent、actor均有负测，stream由Run确定性派生，跨aggregate ID复用为通用conflict。mixed-range pure fold仍可跨边界。 |
| AC-11 | 部分满足，受F-004阻断 | 新 set `sha256-dae028...`确定生成，两个旧set byte diff为零，DB DDL/user_version不变；actual retained old reader未执行。 |
| AC-12 | 部分满足 | 状态pair、hierarchy、idempotency、race、terminal、recovery和343个三步模型序列通过；priority/overflow/mixed-fold/old-reader矩阵缺失。 |
| AC-13 | Runtime回归通过，completion未通过 | protocol、Event Store、workspace tests、fmt、clippy、governance和Schema generation通过；`check_docs.py`按预期因REVIEW-0001/0002/0003 freshness相对本substantive commit而失败，且本Review有open Major。 |

# Compatibility, permission, and isolation review

- `pareto-protocol`继续不依赖Kernel/sqlx；Cargo manifests/lock在requested baseline到reviewed revision间无变化，没有新增第三方依赖、Provider、网络、CLI或真实模型调用。
- SQLite v1 DDL、`user_version`、append-only triggers和store identity未改变；重构仅抽出transaction-local `PreparedEvent`、idempotency check和insert helper。未发现第二Manifest/state权威表或public raw sqlx/transaction API。
- established path顺序为 `BEGIN IMMEDIATE` → sequence-1 persisted reader → exact Manifest/owner authority → exact-stream row validation/fold → global event-ID idempotency → expected/guards → one append。未发现authority/fold前全局event-ID查询；跨aggregate ID reuse只返回generic idempotency conflict。
- Manifest/Event row SchemaSet和limits exact match由首行读取、registry exact resolution、`validate_row`及nested Manifest equality共同检查；unknown/missing reader fail closed。F-004仅指出已发布旧reader证据缺失，不表示current exact reader选择了替代set。
- Run/Workspace/tenant/user/agent/actor隔离由SQL完整key和authority实现；F-003指出脱离loader后的pure fold尚未继承这些绑定，不能据此扩张REQ-0006声明。

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

现有测试全部绿不关闭F-001至F-004：相关边界没有断言，且F-002是未被测试输入触发的deterministic overflow path。

# Scope and unrelated changes

direct-parent产品diff集中在protocol lifecycle types/Schemas、Kernel lifecycle aggregate、Event Store helper重构、真实SQLite测试及REQ-0005 SDD/work记录。未增加依赖、DDL、`user_version`、第二权威表、public SQL/API、Projection/Replay executor、Capability、Provider、Agent Loop、Task DAG或网络调用。

requested baseline额外包含三份既有独立Review的freshness-only commit `8d37f41`；未改变finding disposition或产品行为。F-005记录的交付文档陈旧文本属于REQ-0005范围，不是产品逻辑扩张。

# Re-review history

- 2026-08-24：fresh independent review of exact `d0011d064b88cf4be8e6b70ae781f3637bb15161` against requested baseline `5ef949dd084b1e6ae82015f4c66adb8281aebf65`；0 Blocker、4 Major、1 Minor，结论 changes requested。后续必须在新exact revision上检查remediation diff与原始测试证据；实现者不能自行关闭F-001至F-004。
