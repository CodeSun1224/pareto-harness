---
id: REVIEW-0011
title: REQ-0008 Observer、Gate 与 Transform Hook 独立实现评审
status: changes-requested
owners: [independent-reviewer]
created: 2026-08-29
updated: 2026-08-29
links: [REQ-0008, SPEC-0007, RFC-0008, ADR-0009, REVIEW-0010, REQ-0004, REQ-0007]
independence: independent
reviewed_revision: dfeee45f286c4dce4bdd950bf98d30cbc4b00fb8
open_blockers: 2
open_majors: 4
---

# Verdict

`changes-requested`。本次 fresh independent implementation review 只审查 exact
`dfeee45f286c4dce4bdd950bf98d30cbc4b00fb8` 相对父提交
`b1f626ed52ffd7e0e6b4ad9c0cb457c12e8760d7` 的实现。候选提供了 Hook 协议类型、内容地址
SchemaSet、Runtime Control transaction-local admission 和双 Event append 基础，但尚未形成 SPEC-0007
要求的 Kernel-owned Hook point 执行纵切；Hook Projection 的 pure fold 也未验证 point、lineage、pair 和
finalization 的完整状态机。当前 2 open Blocker、4 open Major，不批准进入 `verified/done`。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Blocker | `crates/pareto-kernel/src/event_store/hook_runtime.rs:209-256,317-467,530-534,683-1094`; tests `tests.rs:430-575,1630-1730` | 没有生产 Kernel 编排入口把 Manifest-pinned registry、point start、逐 invocation reserve、bounded handler、输出重验、terminal pair、skip 和 point finalization 串成一次调用。`ResolvedHookRegistry`、`evaluate_point`、`FakeHookHandler` 与 pair APIs 是互不连接的私有片段；测试先在另一数据库用普通 Runtime Control operation 生成 payload，再手工拼 pair，且直接调用 handler。八类 Hook Event 中只有 initialize/reserve/terminal 有 writer，point-start/skip/finalized/late/rejected 只有 fold 分支。这违反 Desired outcome、SPEC call path 及 AC-02/03/07/09-16/18/19；不存在可由后续受信调用方使用且不可绕过的 Hook 决策路径。 | 增加单一 crate-private Kernel execute-point vertical path，从已持久 Manifest/lifecycle/control/hook history解析 exact registry/handler，写 point-start，逐次原子 reserve→调用→terminal，按固定 phase/lineage 产生 skip/finalization，并支持 cancel/timeout/recovery/late。Focused 和 E2E 测试必须只通过该入口证明 Transform→Gate→Observer、before-commit 和 after-only points，而不是手工构造已准入 payload。 | open |
| F-002 | Blocker | `crates/pareto-kernel/src/event_store/hook_runtime.rs:1121-1242`; protocol payloads `crates/pareto-protocol/src/hooks.rs:411-598` | `fold_hook_events` 除连续 sequence/Schema 外，只维护 reserve→terminal 的最小 map。它无条件接受任意 point-start、skip、late/rejected；finalized 只检查 `point_id` 不重复；不验证 point/phase/ordered invocation、input/predecessor/final digest、source cursor、required Gate component decisions、skip 原因、terminal kind/result、pair fingerprint/counterpart Event、accounted usage或对应 control reservation/settlement。Schema-valid 且重新封存的伪造 finalization、陌生 skip/late、Observer 携带 Gate decision、错误 pair binding 都可被 Projection/Recorded replay当成历史。这直接破坏 AC-14/15、Recorded replay 与“exact Event range 重算决定”的质量门禁。 | 实现版本化、连续、闭合的 Hook state-machine reducer，并在一致 horizon 下交叉验证 Runtime Control pair/budget facts；加入 validly re-sealed 负例，逐项证明缺 first event/gap/非法顺序、未知 invocation、错误 phase/lineage/source、双 terminal、kind/result不符、pair/budget mismatch和伪造 finalization在 Projection、reopen、Recorded 三入口 fail closed。 | open |
| F-003 | Major | `crates/pareto-kernel/src/event_store.rs:314-341`; `crates/pareto-kernel/src/event_store/hook_runtime.rs:52-68,544-644,748-992`; `crates/pareto-protocol/src/hooks.rs:391-409` | atomic pair 的 existing/mutation 判定只按两个 proposed Event ID调用单 Event idempotency lookup。持久层不按 `pair_id` 查找，`HookPairBindingV1`也没有合同要求的 `pair_kind`；因此同一 pair ID 换一组 Event ID/command bytes，在新 expected cursors 下会被当成 zero-existing，而不是 pair mutation。命令也未闭合携带两份 prepared Event bytes/显式 next sequence。现有 mutation 测试只改 correlation 而复用 Event ID，未覆盖该路径。违反 AC-09/12 的 pair identity、zero/one/two 优先级及 cross-kind 防重放。 | 持久化并在领域准入前按 pair ID+kind 搜索/验证完整 fingerprint、双 Event identity/sequence/bytes/cross-ref；同 pair ID不同 Event IDs、reserve/terminal cross-kind reuse 均须无写入地报 mutation。补 exact two retry、same-pair-new-events、cross-kind、one-sided reseal、response-loss 和 reverse-winner测试。 | open |
| F-004 | Major | `crates/pareto-kernel/src/event_store/hook_runtime.rs:341-457`; tests `tests.rs:1157-1187` | Gate deny/required abstain/invalid 后循环不会 break，Observer仍会被纳入 `observer_results`，其他 Gate 也继续处理；`evaluate_point`接收的是所有 handler 输出预先收集成的 map，更不可能证明未调用 short-circuited handler，也从不产生 skip Event。Observer fail-closed 同样只改 status，不阻止后续 dispatch。违反 AC-03/04/05 的 first-deny short circuit、后续 phase不调用和可审计 skip。现有 `gate_composition` 只断言最终枚举。 | 通过 F-001 的执行入口在 writer-serialized state 上即时组合；用 handler counters + Hook Event 顺序证明首个 deny/invalid/required abstain 后未调用剩余 Gate/Observer、产生规范 skip/finalization，且 Observer failure policy不改既定 business decision。 | open |
| F-005 | Major | `crates/pareto-protocol/src/hooks.rs:90-194,235-250`; `crates/pareto-kernel/src/event_store/hook_runtime.rs:209-255,469-528` | 注册/输出验证没有落实 exact Schema/handler合同：resolve 不验证 input/output/transform field/protected SchemaRef属于 Manifest set或匹配类型，不解析 exact handler compatibility/resource contract；`input_schema_ref`与`TransformContractV1.field_schema_ref`从未使用。Transform mask实现只比较顶层 key，因此已批准的 `/metadata/label` 不能按字段Schema执行；protected view由 handler随结果回传后仅与原值比较，而非由 Kernel从候选重新构造。另有所有 reason code 使用开放 `String`，而 RFC要求版本化闭合枚举、unknown reason拒绝。违反 AC-01/06/13/17。 | 在 registry初始化时验证全部 exact Schema/contract/handler identity；Kernel自行构造 protected view，按版本化 pointer mask和 field schema逐字段验证输入/每一步输出；reason使用闭合版本类型。加入 missing/current/compatible schema、nested mask、unknown-field smuggling、forged protected view、unknown reason和missing handler负例。 | open |
| F-006 | Major | `crates/pareto-kernel/src/event_store/hook_runtime/tests.rs:939-986,1369-1436,1483-1625,1630-1730,1760-1903`; `.agents/work/active/REQ-0008-observer-gate-transform-hooks/VALIDATION.md` | AC-20证据显著超出实际断言：`isolation`只改 actor，未覆盖tenant/user presence-value/workspace/run/task/subject/Hook IDs；“budget concurrency”是同一命令exact retry，不是different pair reverse winner；cancel/deadline没有before/equal/after probe或真实 hung/uninterruptible state；model只跑一条 reserve/retry/terminal/retry序列；late测试期望零audit；recovery只重开initial或手工timeout；Recorded的`recorded_counter`从未绑定任何 handler，live path也手工调用。point/finalization事件和 forged-history矩阵完全缺失。绿色filter因而不能证明 AC-08/11/12/14-16/20，Validation中的“全隔离矩阵、bounded model、hung recovery、Recorded零handler”等结论不可采纳。 | 修复F-001至F-005后，按SPEC test traceability建立真实SQLite、FakeClock、validly re-sealed、跨域矩阵及bounded command/phase/pair model；每个命名filter保持非零，并让断言直接覆盖声明的行为。更新原始Validation为真实计数与场景，不以静态token checker代替行为证据。 | open |

# Acceptance trace

| Acceptance | Result | Independent evidence |
|---|---|---|
| AC-01 | failed | 协议类型存在，但 exact schema/handler/resource解析缺失，registry多point排序只取`hook_points[0]`。见F-001/F-005。 |
| AC-02 | failed | kind×point表存在；没有 Kernel point执行入口或默认拒绝的权威调用边界。 |
| AC-03 | failed | 排序/lineage为独立纯函数，未持久执行；无point-start/finalization writer。见F-001/F-004。 |
| AC-04 | failed | 最终deny计算存在，但不短路调用、不记录skip。见F-004。 |
| AC-05 | failed | isolated evaluator保持Observer business decision，但没有执行级阻断/审计纵切。 |
| AC-06 | failed | 顶层mask部分实现；field schema、nested mask、Kernel-derived protected view缺失。 |
| AC-07 | failed | opaque测试结构存在，但没有从认证事实重建并发放 invocation authority 的执行路径。 |
| AC-08 | failed | pair target含scope检查；完整隔离矩阵、业务ID和unauthorized no-probe/no-write未证明。 |
| AC-09 | failed | 双insert同事务基础存在；pair ID/kind mutation合同和实际Hook trusted-envelope构造缺失。 |
| AC-10 | partial | transaction-local settlement复用REQ-0007核算；仅手工payload测试，未由Hook invocation执行路径证明。 |
| AC-11 | failed | 复用timeout key但无Hook probe/hung handler执行与deadline边界测试。 |
| AC-12 | failed | terminal pair竞态有基础；无完整Hook point唯一terminal、late audit与编排路径。 |
| AC-13 | failed | typed output大小检查存在；closed reasons、pre-decode路径和安全拒绝writer缺失。 |
| AC-14 | failed | Event Schema存在；fold不验证point/lineage/finalization/cross-stream事实。见F-002。 |
| AC-15 | failed | 可重开简单projection；非法历史不fail closed且无真实pending orchestration recovery。 |
| AC-16 | failed | Recorded API只读，但只重建不完整Projection；测试counter未绑定handler。 |
| AC-17 | partial | 新内容地址set、retained sets、RunManifest v2和SQLite v2保留通过；exact Hook reader语义仍受F-002/F-005阻塞。 |
| AC-18 | partial | 未引入外部transport/Rust ABI；Fake handler尚未形成reference execution implementation。 |
| AC-19 | failed | 私有类型未形成下游可消费且由Kernel治理的proposal/decision接入点。 |
| AC-20 | failed | 32个filter非零且绿，但场景/断言不足且绕过缺失生产路径。见F-006。 |

# Compatibility, permission, and isolation review

- SQLite仍为v2，既有DDL/trigger常量与已发布SchemaSet保持byte-identical；未增加依赖或第二状态表。
- RunManifest/RunCreated使用新major并为旧set保留v1 reader，方向正确；RunTask/Runtime Control回归未见显式旧major替换。
- Hook authority、permission和isolation目前只在手工pair API做target/control scope比较；没有认证request→Manifest registry→handler lease的完整路径，因此不能认定trusted-kernel boundary已闭合。
- Hook Projection只读取Hook stream，未按SPEC在一致horizon交叉验证control pair/budget；这使Recorded/reopen可接受单流内部Schema-valid但领域伪造的历史。

# Regression and test review

独立在Windows/PowerShell、offline、2026-08-29执行：

- `cargo test -p pareto-kernel hook_runtime:: --offline`：32 passed，0 failed；122 filtered。
- `cargo test -p pareto-protocol hook_contract --offline`：1 matched/passed；其他target的0-test输出未计为证据。
- `python scripts/check_req0008_scope.py`：passed；该脚本只证明DB/依赖/retained/scope静态边界，不证明Hook行为。
- `python -m unittest discover -s scripts/tests -p "test_*.py"`：24 passed。
- `cargo fmt --all -- --check`：passed。
- `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`：passed。
- `python scripts/check_docs.py`：failed；只报告REVIEW-0001至0007对本实现substantive paths stale，符合
  TASK-10尚未批准的状态。本Reviewer不前移历史Review来掩盖当前2 Blocker/4 Major。
- `git diff --check`：passed；`git status --short`仅显示本Reviewer-owned REVIEW-0011。
- 首个组合命令外层120秒timeout发生在前四项完成之后；后续门禁已拆分复跑，不把timeout本身记为测试失败。

绿色结果与F-001至F-006并不矛盾：现有tests直接访问同模块私有构件并手工制造已准入事实，未经过缺失的生产编排器，也未构造F-002/F-003所述validly re-sealed历史。

# Scope and unrelated changes

exact diff为85个文件、5218 insertions/126 deletions。变更集中在Hook协议/Schema、Kernel Event Store/Lifecycle/
Projection/Runtime Control、测试、scope checker、Requirement状态与active evidence；Cargo manifests和`Cargo.lock`
无变化，未发现真实shell/Python/TypeScript/HTTP/MCP/WASI runtime、Provider/Tool/Effect/Sandbox/Loop/
Memory/DAG/Evidence实现。生成SchemaSet为`sha256-3a0c6e67a97675cf6bfcdc1fb9766b30a79ae62e662479d9ae1ef5d7b43ff99d`；
历史set未被修改。除本Reviewer-owned REVIEW-0011外，本评审未修改产品、测试或实现者证据。

# Re-review history

- 2026-08-29：fresh independent implementation review exact
  `dfeee45f286c4dce4bdd950bf98d30cbc4b00fb8` against parent
  `b1f626ed52ffd7e0e6b4ad9c0cb457c12e8760d7`。结论2 Blocker、4 Major，全部open，
  `changes-requested`。后续修复必须由本Reviewer检查新的exact diff和原始证据后关闭；不得以实现者closure声明替代复审。
