---
id: REVIEW-0011
title: REQ-0008 Observer、Gate 与 Transform Hook 独立实现评审
status: changes-requested
owners: [independent-reviewer]
created: 2026-08-29
updated: 2026-08-29
links: [REQ-0008, SPEC-0007, RFC-0008, ADR-0009, REVIEW-0010, REQ-0004, REQ-0007]
independence: independent
reviewed_revision: 4817ef31a95d00592840be50207ff59baf67f694
open_blockers: 2
open_majors: 1
---

# Verdict

`changes-requested`。本 Reviewer 独立复审 exact remediation revision
`4817ef31a95d00592840be50207ff59baf67f694`，实现基线为
`b1f626ed52ffd7e0e6b4ad9c0cb457c12e8760d7`；中间的 `4ea5edc` 只包含本
Reviewer-owned 初审记录。整改已建立 Kernel-owned execution vertical path，关闭 atomic pair identity、
Gate short-circuit、exact schema/Transform contract 等四项 Major，并补上 cancellation terminal 与实际 Hook
envelope cross-reference。然而 point command 的 exact/mutation 与中途恢复合同仍未闭合，reducer 仍接受未绑定
lineage 的 validly re-sealed rejection audit。当前 2 open Blocker、1 open Major，不批准进入 `verified/done`。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Blocker | `crates/pareto-kernel/src/event_store/hook_runtime.rs:853-904,1424-1470,1876-1930,2129-2149`; tests `tests.rs:2163-2294,2398-2470` | Kernel-owned execute、point start、reserve/terminal pair、cancel/timeout/late、skip/finalization现已连通，取消分支也产生唯一`Cancelled` terminal。但新增 finalized/start retry是字段子集比较而非canonical full-command identity：`point_id_for`和shortcut均未绑定`task_id`、`occurred_at`、`correlation_id`、`absolute_deadline_utc`，所以这些字段变更仍返回already-committed或继续执行，而不是mutation conflict。start recovery还要求existing start恰为aggregate流尾；crash发生在任一reserve或terminal后、point未finalize时会报conflict，不能恢复剩余组合。finalized response-loss返回`proposal: None`，Transform后的可消费proposal无法从已提交结果恢复。违反SPEC的完整command bytes、same-ID exact/mutation、pending不重跑与AC-10/12/15/19。 | 以持久canonical point command identity/fingerprint验证exact/mutation；所有命令字段mutation必须no-write conflict。通过start-only、reserve-pending、已terminal后未下一invocation、finalize response-loss的close/reopen测试证明不重跑已完成handler、不重复核算并返回与首次调用等价的可消费proposal/result；pending handler只走显式terminal recovery authority。 | open |
| F-002 | Blocker | `crates/pareto-kernel/src/event_store/hook_runtime.rs:2708-2778,3157-3176`; protocol `crates/pareto-protocol/src/hooks.rs:666-686`; tests `tests.rs:2632-2745` | Registry-aware reducer、point/phase/lineage/finalization与同一MVCC horizon跨流验证已大幅补齐；`2722-2729`也将pair payload的`hook_event_id`绑定到实际Hook envelope。但`hook-message-rejected` fold仍只检查registry revision和固定reason，完全不绑定open point/invocation、point/hook/revision/source cursor/input digest/redaction revision/decision ID。攻击者可修改这些字段并重新生成合法envelope fingerprint，Projection、reopen和Recorded仍会接纳并增加rejected count。这是初审要求的“陌生rejected/错误lineage”缺口，破坏AC-14/15的exact Event range重算与fail-closed。新增resealed测试只覆盖point/final digest/pair及counterpart ID，没有覆盖rejection。 | 让rejection携带并由reducer验证稳定invocation/attempt或等价闭合身份，逐字段绑定当前point、registration、source、input与redaction；在Projection、close/reopen和Recorded入口加入validly re-sealed陌生decision、错point/hook/revision/source/input/redaction负例，全部fail closed且无写入。 | open |
| F-003 | Major | pair persistence/query and pair commands in `event_store.rs` and `hook_runtime.rs` | 整改已按`pair_id`+`pair_kind`查询，持久化完整fingerprint、显式双sequence和canonical prepared-event preimage；zero/one/two、same-pair-new-events、cross-kind、two exact retry和mutation均在writer transaction内处理。跨流validator也核对双侧actual envelope identity。 | 独立Hook 39-test与workspace回归通过；pair mutation/corruption测试非零。 | closed |
| F-004 | Major | `crates/pareto-kernel/src/event_store/hook_runtime.rs:1471-2149`; `tests.rs:2296-2330` | Kernel执行循环现在首个Gate deny/invalid/required abstain后设置stop reason，后续Gate/Observer不调用并追加规范skip；Observer结果保持非权威。handler counter直接证明只调用Transform与首Gate。 | `hook_runtime::kernel_owned_gate_short_circuit`和全Hook filter通过。 | closed |
| F-005 | Major | `crates/pareto-protocol/src/hooks.rs:200-310`; Kernel registry/output/transform validation | Registry resolve现验证Manifest exact schema set、input/output/field/protected schema、handler compatibility及resource contract；Kernel构造protected view、递归JSON pointer逐字段验证，reason为闭合enum。Registry canonical key现包含已规范排序的完整`hook_points`向量、phase、priority、ID、revision，消除了旧实现只取首point的歧义，runtime仍按单point闭合phase排序。 | protocol hook contract、nested transform/protected/compatibility/output security filters通过；Schema retained sets与scope checker通过。 | closed |
| F-006 | Major | `crates/pareto-kernel/src/event_store/hook_runtime/tests.rs`; `.agents/work/active/REQ-0008-observer-gate-transform-hooks/VALIDATION.md` | 测试现覆盖生产纵切、Gate短路、timeout late audit、取消、invalid-kind rejection、Recorded no-write、point-start retry和双侧counterpart mutation，较初审显著改善。但仍没有F-001的full-command mutation、中途point recovery、response-loss Transform proposal，也没有F-002 rejection reseal矩阵；Hook isolation仍未直接覆盖AC-20列出的完整presence/value与业务ID矩阵，Hook budget concurrency仍不是different-pair reverse winner。Validation的“完整恢复/全隔离/bounded concurrency”结论因此仍超出直接断言。 | 补齐F-001/F-002所列行为测试，并让Hook自身的完整隔离矩阵、different-pair reverse-winner与bounded phase/command模型直接非零命中；Validation只记录实际场景和计数。 | open |

# Acceptance trace

| Acceptance | Result | Independent evidence |
|---|---|---|
| AC-01/02/03 | passed | closed注册、kind×point、规范排序、Kernel-owned固定phase纵切已实现；未知组合默认拒绝。 |
| AC-04/05 | passed | Gate即时短路并写skip；Observer不能改business decision。 |
| AC-06/07/08 | partial | recursive Transform/Kernel protected view、narrow lease存在；Hook自身完整隔离矩阵仍受F-006阻塞。 |
| AC-09 | passed | atomic pair zero/two/one、kind/identity/sequence/preimage、双侧actual Event binding已闭合。 |
| AC-10/11/12 | partial | success/failure/cancel/timeout pair与late audit通过；point full-command retry和response-loss仍受F-001阻塞。 |
| AC-13 | passed | exact output schema、limit、kind语义、closed reason与安全rejection writer已实现。 |
| AC-14/15 | failed | 主state machine已闭合，但rejection audit仍可被合法重封伪造；中途point恢复和完整retry未实现。见F-001/F-002。 |
| AC-16 | passed | Recorded projection只读且独立测试证明Event/control facts不变、无handler入口。 |
| AC-17/18 | passed | SQLite v2与历史SchemaSet保留，当前set内容地址化；仅Fake Rust reference handler，无外部transport/ABI依赖。 |
| AC-19 | partial | crate-private proposal/result入口存在，但finalized response-loss不给Transform proposal。见F-001。 |
| AC-20 | failed | 39个Hook tests和workspace回归绿色，但缺F-001/F-002及完整隔离/不同pair并发证明。见F-006。 |

# Compatibility, permission, and isolation review

- SQLite保持v2，已发布SchemaSet保留byte-identical；当前Manifest/Hook schemas由exact retained reader解析，未增加依赖或第二状态表。
- reserve/terminal pair在单一`BEGIN IMMEDIATE`内进行registry-aware pre/post fold，Hook Projection在同一read transaction与同一MVCC horizon读取Hook和Runtime Control streams。
- Hook handler只收到Kernel构造的bounded request/lease，没有Event transaction、budget/cancellation/terminal authority；Recorded路径没有handler参数。
- authority与scope边界方向正确，但Hook-specific全隔离负向证据仍不足，计入F-006。

# Regression and test review

本Reviewer在Windows/PowerShell、offline、2026-08-29对 exact `4817ef31...` 独立执行：

- `cargo test -p pareto-kernel hook_runtime::kernel_owned_ --offline`：6 passed，0 failed，155 filtered。
- `cargo test -p pareto-kernel hook_runtime::resealed_history_rejection --offline`：1 passed，0 failed。
- `cargo test -p pareto-kernel hook_runtime:: --offline`：39 passed，0 failed，122 filtered。
- `cargo test -p pareto-protocol hook_contract --offline`：1 matched/passed。
- `cargo test --workspace --all-targets --all-features --offline`：Kernel 160 passed/1 ignored；Protocol unit 9 passed、baseline 1 ignored、contract 24 passed。命令最终exit 0；all-targets末尾的schema generator binary在无目标参数时打印`existing content-addressed schema set differs byte-for-byte`，不作为schema稳定证据。
- `python scripts/check_req0008_scope.py`：passed；`python -m unittest discover -s scripts/tests -p "test_*.py"`：24 passed。
- `cargo fmt --all -- --check`：passed；`cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`：passed。

这些绿色结果证明已修子项，但现有测试明确把finalized retry的`proposal: None`作为期望，且只造start-only recovery；没有覆盖上述mutation、中途恢复和rejected-audit reseal，所以不能关闭剩余finding。

# Scope and unrelated changes

实现基线至当前exact revision的历史diff含151个文件、8973 insertions/135 deletions，其中包括本Reviewer-owned初审记录和实现者active evidence。产品变化仍集中于Hook协议/schema、Kernel Event Store/Lifecycle/Projection/Runtime Control及测试；Cargo manifests/lock无新增依赖，未实现真实外部Hook runtime、REQ-0009或后续Effect/Provider/Tool/Sandbox能力。除本Review record外，本Reviewer未修改产品代码、测试、Requirement/Spec/RFC/ADR或实现者证据。

# Re-review history

- 2026-08-29：fresh independent implementation review exact `dfeee45f286c4dce4bdd950bf98d30cbc4b00fb8` against parent `b1f626ed52ffd7e0e6b4ad9c0cb457c12e8760d7`：2 Blocker、4 Major open。
- 2026-08-29：检查中间remediation `498597b5e43ccd628f53cd4aca259d6dbae9e791`，确认Kernel纵切、pair identity、short-circuit与schema/Transform大幅修复，同时独立发现cancel、actual Hook envelope binding、完整retry/recovery缺口；该candidate被后续exact revision取代，未作为最终reviewed revision。
- 2026-08-29：同一independent Reviewer复审 exact `4817ef31a95d00592840be50207ff59baf67f694`。关闭F-003/F-004/F-005；F-001/F-002保持Blocker，F-006保持Major。当前2 Blocker、1 Major，`changes-requested`。后续修复仍须由本Reviewer以新的exact revision和原始证据复审关闭。
