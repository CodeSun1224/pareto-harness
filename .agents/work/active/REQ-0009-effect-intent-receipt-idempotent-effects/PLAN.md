---
title: REQ-0009 Effect Intent/Receipt 与幂等效果交付计划
status: active
owner: runtime-kernel
updated: 2026-08-30
links: [REQ-0009, SPEC-0008, RFC-0009, ADR-0010, REQ-0004, REQ-0007, REQ-0008, REVIEW-0012]
---

# Goal and acceptance

按REQ-0009 AC-01至AC-22交付最小Fake Effect纵向切片：Run/Task → Manifest v3 exact固定Effect registry、policy与executor descriptor → Runtime Control reserve与Effect Intent原子pair → 单次dispatch claim → Manifest-pinned Fake executor → 不可信Receipt observation → Kernel admission → Effect结论与operation terminal/settlement原子pair → crash/cancel/timeout恢复及reconciliation → Projection与Boundary Inventory V2固定horizon → Recorded replay零执行、零写入、零预算变化。

# Current state

REQ-0004、REQ-0007、REQ-0008均done。当前SQLite保持v2，REQ-0008最终Hook-capable SchemaSet为`sha256-0efc2ecfafba4c683a08917f4f4d025731f70df7c1ec68827d5eedff46384771`。REQ-0009 Requirement、影响矩阵、SPEC-0008、RFC-0009与AC→测试追踪已经完成；fresh independent REVIEW-0012曾提出4个Major，设计逐项整改后由同一Reviewer批准，最终设计exact `b7acbd82824d8410d432117c89be1bd56c8ce05c`，接受闭环exact `60cee6ed44d150185bf99ca3095a8ce803bcc0d3`，0 open Blocker/Major。

当前没有REQ-0009 Runtime、Schema、测试、依赖或真实外部效果实现。规划门禁已由fresh independent REVIEW-0012对exact `46772c7fbb30e82f0e8fd4fb50915e8414acaa65`批准；Requirement现推进为`implementing`，不得表示reviewing、verified或done。

# Plan

开始任何Protocol、Schema、Runtime、测试或脚本行为编辑前，先把Requirement从`planned`推进为`implementing`，并同步TASKS/HANDOFF当前执行状态；设计Review只解除设计门禁，不代替实施状态或代码Review。

1. 在`pareto-protocol`新增闭合Effect、executor descriptor、Intent/claim/Receipt/conclusion/reconciliation、Projection与Boundary Inventory/Record V2类型；发布Run Manifest v3和内容地址Effect-capable SchemaSet，保留全部旧Manifest、Inventory、SchemaSet、reader/reducer和历史字节。
2. 在`pareto-kernel::event_store`新增crate-private Effect derived stream初始化、exact retained reader、连续pure fold、Projection和显式inclusive cursor/history digest读取；不改SQLite v2 DDL/trigger，不增加可写outbox/status/receipt表。
3. 把REQ-0007 Runtime Control transaction-local helper扩展为互斥Hook/Effect binding；实现`control reserve + effect-intended`及`control terminal/settlement + effect conclusion`的双cursor/sequence/Event/fingerprint原子pair，覆盖zero/two exact、mutation、one-existing corruption、insert/commit fault与response-loss retry。
4. 实现Kernel-only admission、规范请求摘要、完整隔离域和幂等键：exact retry返回既有Intent/终态且不重复reserve/dispatch/settle；same key异请求、异operation/reservation/scope/version/executor稳定冲突。
5. 实现一次性不可伪造dispatch lease、claim process epoch与Manifest-pinned exact executor identity；只接入进程内确定性Fake executor，覆盖成功、业务失败、执行前失败、响应丢失、partial、timeout与返回后crash，不执行真实文件、进程、网络、Provider、Tool或Sandbox效果。
6. 实现Receipt observation准入、producer/evidence adapter固定、safe redaction、usage/evidence验证、唯一权威结论和保守预算核算；late/duplicate/out-of-order/contradictory observation只能形成绑定原Effect的安全audit/reconciliation事实。
7. 实现稳定Kernel-private recovery key/command：绑定scope/effect/attempt/claim epoch/operation/reservation/executor/policy/deadline/source，使用canonical Clock/process-loss/usage evidence、domain-separated fingerprint与确定性pair IDs；未claim只能`not_applied + verified zero + full release`，已claim只能partial/unknown并进入reconciliation，首片禁止再次dispatch。
8. 实现默认拒绝的reconciliation、Run/Task succeeded guard、Effect Projection crash/reopen与Boundary Inventory/Record V2；Recorded replay只读Inventory固定horizon内事实，API不接受executor/writer/settlement authority，Simulated/reexecute稳定拒绝。
9. 覆盖tenant、user presence/value、Workspace、Run、Task、Actor、Effect/attempt、operation/reservation与三类command identity的exact隔离；所有拒绝与事件只持久化安全摘要，外部payload、秘密、路径与原始错误不得进入权威历史。
10. 按SPEC-0008完成Focused、Impacted、Core、SQLite fault injection、FakeClock、并发/model、兼容、replay和安全负向测试；每个Cargo filter先证明非零命中，记录exact命令、环境、计数、Schema/DB identity和质量/成本/延迟观察到`VALIDATION.md`。
11. 提交exact implementation revision，由新的fresh independent Agent使用`code-review`检查Requirement/Spec/RFC/ADR、完整diff与原始验证证据；实现者只整改，Blocker/Major必须由原代码Reviewer复审关闭。
12. 0 open Blocker/Major且完整门禁通过后同步implemented facts，将Requirement依次推进reviewing→verified→done并归档work；此前不得开始REQ-0010。

# Validation

所有下列`pareto-kernel`filter都必须通过`python scripts/assert_cargo_test_filter.py pareto-kernel <filter>`执行；helper必须先断言命中数大于0，不得把Cargo的0 tests成功作为证据。

- Protocol/Schema：`cargo test -p pareto-protocol effect_contract --offline`；`cargo test -p pareto-protocol --all-targets --all-features --offline`；`cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas`；再次生成并以`git diff --exit-code -- schemas`证明生成树byte-identical。
- Focused Effect：filters `effect_runtime::tests::default_deny`、`effect_runtime::tests::intent_before_dispatch`、`effect_runtime::tests::idempotency`、`effect_runtime::tests::dispatch_lease`、`effect_runtime::tests::fake_outcomes`、`effect_runtime::tests::receipt_admission`、`effect_runtime::tests::state_model`、`effect_runtime::tests::partial_success`。
- Recovery/settlement：filters `effect_runtime::tests::crash_recovery`、`effect_runtime::tests::reconciliation`、`effect_runtime::tests::atomic_settlement`、`effect_runtime::tests::cancellation_timeout`、`effect_runtime::tests::late_receipts`。
- Persistence/replay/security：filters `effect_runtime::tests::fold_contract`、`effect_runtime::tests::isolation`、`effect_runtime::tests::projection_recovery`、`effect_runtime::tests::recorded_replay`、`effect_runtime::tests::compatibility`、`effect_runtime::tests::lifecycle_success_guard`；`cargo test -p pareto-kernel --doc --offline`证明外部无法构造authority、lease、Receipt admission或writer transaction。
- Impacted regression：`cargo test -p pareto-kernel event_store --offline`；`cargo test -p pareto-kernel lifecycle:: --offline`；`cargo test -p pareto-kernel projection:: --offline`；`cargo test -p pareto-kernel runtime_control:: --offline`；`cargo test -p pareto-kernel hook_runtime:: --offline`；`cargo test -p pareto-kernel --all-targets --all-features --offline`。
- Scope/compatibility：新增并运行`python scripts/check_req0009_scope.py`，断言SQLite `user_version=2`及v2 DDL/trigger、全部retained SchemaSet和旧Run reducers不变；断言没有真实network/process/sleep、mutable outbox/status表、background scanner、自动redispatch或REQ-0010后续能力。运行`cargo tree --workspace --offline`记录完整依赖树，并以`git diff --exit-code 60cee6ed44d150185bf99ca3095a8ce803bcc0d3 -- Cargo.toml Cargo.lock ':(glob)**/Cargo.toml'`证明相对已接受设计基线没有manifest/lock依赖变化；任何依赖diff触发停止和重审。
- Governance/Core：`python -m unittest discover -s scripts/tests -p "test_*.py"`；`python scripts/check_docs.py`；`cargo fmt --all -- --check`；`cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`；`cargo test --workspace --all-targets --all-features --offline`。
- Hygiene：`git diff --check`；`git status --short`，逐文件分类为预期源代码、生成Schema、测试、工作证据或Reviewer-owned记录，拒绝无关修改。

# Handoff notes

若实施需要public authority/raw SQL、DB v3、mutable outbox/status/receipt表、alternate Event actor、caller-selected/current reader或registry substitution、真实Provider/Tool/Sandbox、外部Worker/RPC/队列、background scanner、自动redispatch、claim后声称确定未执行、跨边界exactly-once承诺或新第三方依赖，立即停止并返回影响分析/SPEC/RFC及独立设计评审。

首个实现commit从Protocol/Schema与retained compatibility开始；在atomic pair、claim/recovery fault tests可运行前，不得声称幂等或结算安全完成。代码完成后必须由新的fresh independent code Reviewer检查；REVIEW-0012只是设计Review，不能替代代码Review。
