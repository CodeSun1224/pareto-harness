---
title: REQ-0008 Observer、Gate 与 Transform Hook 骨架交付计划
status: active
owner: runtime-kernel
updated: 2026-08-28
links: [REQ-0008, SPEC-0007, RFC-0008, ADR-0009, RFC-0007, ADR-0008, REQ-0004, REQ-0007, REVIEW-0010]
---

# Goal and acceptance

按REQ-0008 AC-01至AC-20交付最小纵向切片：创建Run/Task → Manifest exact固定Hook-capable SchemaSet与registry revision/config → 注册进程内Rust Fake Observer、Fake Gate、Fake Transform → Kernel按固定phase和稳定phase-local顺序执行 → 重建bounded authority并以atomic reserve pair预留预算 → Transform只处理允许的非权威proposal → Gate allow/deny/default-deny → Observer只读且不改business decision → 以atomic terminal pair结算并记录结构化Hook决定 → 支持FakeClock取消/deadline/timeout → 拒绝迟到、重复、乱序、越权和单边pair历史 → Event Store/Projection恢复Hook决定 → Recorded replay零handler执行、零Event追加、零重复核算。

# Current state

REQ-0004和REQ-0007均done，REQ-0007最终control SchemaSet为`sha256-a95c824d3a47dbc891f884921811859dc2d132e1e39f6f781e833ea9b306a217`，SQLite为v2。REQ-0008 Requirement/影响分析/SPEC-0007/RFC-0008已完成；fresh independent REVIEW-0010先对`8507bae4`提出4个Major，再独立批准整改exact `3aee02adf8815466b02f51de247ae19922efc126`，F-001至F-004全部closed、0 open Blocker/Major。RFC/Spec/Requirement和ADR-0009由accepted-doc commit `3318cbc6fe8bc8c9717a5a2b4aea1153f0d281d6`正式接受，并由同一Reviewer完成freshness复审。

当前没有Hook产品代码、Schema、测试、依赖或外部Runtime；`.agents/work/active`此前只有`.gitkeep`。本计划创建后REQ-0008仅为`planned`，不得表示implementing、verified或done。

# Plan

1. 在`pareto-protocol`新增闭合Hook kind/point/phase、registry/config、invocation key/input lineage、pair identity、result/decision/rejection、point finalization与Projection类型；发布新内容地址Hook-capable SchemaSet和RunManifest新major/role，保留全部旧set字节与旧Run解释。
2. 在`pareto-kernel::event_store`增加crate-private Hook stream初始化、exact retained reader、continuous pure fold、full-provenance Projection和Recorded replay读取入口；不改SQLite v2 DDL/trigger、不增加Hook可写表或public SQL。
3. 实现Manifest-pinned registry解析、kind×point矩阵、固定`Transform -> Gate -> Observer`/`Gate -> Observer`phase、phase-local稳定排序、逐invocation input lineage、point decision/finalization和规范skip；Rust Fake handler接口不暴露ABI、DB或Kernel对象。
4. 把REQ-0007现有自提交reserve/settlement准入重构为不扩权的transaction-local private helper；实现`HookReservePairCommandV1`与`HookTerminalPairCommandV1`的双cursor/sequence/Event/fingerprint、zero/two exact、mutation、one-existing corruption、双insert/commit rollback和response-loss重试；通用single-stream terminal入口拒绝Hook binding。
5. 实现principal/tenant/user/workspace/run/task/owner/subject/actor/hook/invocation/attempt/decision exact重建与隔离，opaque bounded lease、Capability撤销/过期/收窄、trusted resource envelope、全账户原子预算和并发防超卖。
6. 实现Transform allow mask与protected hash view、Gate deny-first/required explicit allow/empty-required default deny、Observer business-decision隔离和分离execution status；所有输出在Kernel重验limits/Schema/scope/identity/producer/lease，日志与拒绝只保存安全字段。
7. 实现FakeClock cancellation probe、absolute/monotonic deadline、completion/cancel/timeout terminal pair竞态、hung recovery、late/duplicate/retry/out-of-order安全审计；crash/reopen只恢复pending并显式reconcile，不自动release/reexecute。
8. 组合最小Fake纵切并证明Event Store/Projection恢复；Recorded replay API不接受handler/writer/recovery authority，前后Fake counter、Event数、pair数和全部budget账户逐字段不变；Simulated/Reexecute在dispatch前稳定拒绝。
9. 按SPEC-0007 test traceability完成Focused、Impacted、Core、真实SQLite fault injection、FakeClock、bounded command/phase/pair/concurrency model、兼容与隔离测试；每个Cargo filter先由helper证明非零命中，记录exact环境、命令、计数、Schema/DB identity及质量/成本/延迟观察到`VALIDATION.md`。
10. 提交exact implementation revision，由新的fresh independent Agent使用`code-review`检查Requirement/Spec/RFC/ADR、完整diff和原始证据。实现者只修复；Blocker/Major必须由原Reviewer复审关闭。
11. 0 open Blocker/Major且AGENTS完整门禁通过后才同步implemented facts，将REQ依次推进reviewing→verified→done并归档work；此前不得开始REQ-0009。

# Validation

所有下列`pareto-kernel`filter都必须通过`python scripts/assert_cargo_test_filter.py pareto-kernel <filter>`执行；helper先list并断言命中数大于0，再运行filter并把count/result记录到`VALIDATION.md`，不得把Cargo的0 tests成功作为证据。

- Protocol/Schema：`cargo test -p pareto-protocol hook_contract --offline`；`cargo test -p pareto-protocol --all-targets --all-features --offline`；`cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas`；再次生成并以`git diff --exit-code -- schemas`证明已提交生成树byte-identical。
- Kind/phase/ordering：filters `hook_runtime::kind_point_table`、`hook_runtime::phase_order_lineage`、`hook_runtime::ordering`、`hook_runtime::gate_composition`、`hook_runtime::default_deny`、`hook_runtime::failure_policy`、`hook_runtime::observer_non_authority`、`hook_runtime::transform_chain_failure`。
- Transform/security/authority：filters `hook_runtime::transform_protected_fields`、`hook_runtime::authority`、`hook_runtime::isolation`、`hook_runtime::output_security`；`cargo test -p pareto-kernel --doc --offline`证明外部无法构造authority/lease/Event transaction。
- Pair/budget/accounting：filters `hook_runtime::reserve_pair_atomicity`、`hook_runtime::pair_fault_injection`、`hook_runtime::budget_reserve`、`hook_runtime::budget_concurrency`、`hook_runtime::settlement`、`hook_runtime::idempotency`、`hook_runtime::terminal_pair_atomicity`。
- Cancel/time/race/model：filters `hook_runtime::cancellation_deadline`、`hook_runtime::terminal_race`、`hook_runtime::model_sequences`、`hook_runtime::late_and_duplicate`；全部时间测试使用FakeClock且无真实sleep。
- Persistence/recovery/replay：filters `hook_runtime::fold_contract`、`hook_runtime::recovery`、`hook_runtime::pair_corruption`、`hook_runtime::compatibility`、`hook_runtime::recorded_replay`、`hook_runtime::unsupported_modes`。
- Impacted regression：`cargo test -p pareto-kernel event_store --offline`；`cargo test -p pareto-kernel lifecycle:: --offline`；`cargo test -p pareto-kernel projection:: --offline`；`cargo test -p pareto-kernel runtime_control:: --offline`；`cargo test -p pareto-kernel --all-targets --all-features --offline`。
- Scope/compatibility：新增并运行`python scripts/check_req0008_scope.py`，断言SQLite `user_version=2`及v2 DDL/trigger、全部retained SchemaSet、旧Run reducers、Recorded no-writer API不变；断言无shell/Python/TypeScript/HTTP/MCP/WASI runtime、Provider/Tool/Effect/Sandbox/Loop/Memory/DAG/Evidence实现、真实sleep/network/process和不必要依赖。
- Governance/Core：`python -m unittest discover -s scripts/tests -p "test_*.py"`；`python scripts/check_docs.py`；`cargo fmt --all -- --check`；`cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`；`cargo test --workspace --all-targets --all-features --offline`。
- Hygiene：`git diff --check`；`git status --short`，逐文件分类为预期源代码、生成Schema、测试、工作证据或Reviewer-owned记录，并拒绝无关修改。

# Handoff notes

若实施需要public authority/raw SQL、DB v3、outbox/decision table、alternate Event actor、多个Hook事实源、补偿单边pair、handler持有writer transaction、并行Hook、background scanner/writer、自动crash release/reexecute、caller-selected reader/registry/current substitution、真实Effect/Provider/Tool/transport、Rust ABI、外部进程/network或新第三方依赖，立即停止并返回影响分析/SPEC/RFC，不得把它作为局部实现选择。

首个实现commit从Protocol/Schema和retained compatibility开始；在Hook pair fault tests可运行前，不得声称预算/terminal原子性完成。代码完成后必须由新的fresh independent code Reviewer检查，不能复用实现者或把本次设计Review当代码Review。
