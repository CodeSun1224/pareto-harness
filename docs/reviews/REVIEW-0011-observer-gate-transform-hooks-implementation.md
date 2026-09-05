---
id: REVIEW-0011
title: REQ-0008 Observer、Gate 与 Transform Hook 独立实现评审
status: approved
owners: [independent-reviewer]
created: 2026-08-29
updated: 2026-09-05
links: [REQ-0008, SPEC-0007, RFC-0008, ADR-0009, REVIEW-0010, REQ-0004, REQ-0007]
independence: independent
reviewed_revision: d2594439c95960d2acd18dc7614b00ef55744ea0
open_blockers: 0
open_majors: 0
---

# Verdict

`approved`。本 Reviewer 独立复审 exact remediation revision
`e4877834fb54e3db936677f3b87c5fdf9e1d2d97`，实现基线为
`b1f626ed52ffd7e0e6b4ad9c0cb457c12e8760d7`；`4ea5edc`、`c7483a0`只包含本
Reviewer-owned Review记录。新整改关闭了point full-command mutation和rejected-audit lineage两个Blocker。
重新按accepted SPEC/RFC校准后，pending invocation必须由显式terminal recovery authority结清、不能由point
executor自动重跑；Hook Event只保留Transform安全digest，因此finalized retry返回durable decision/projection和
`proposal: None`是fail-closed合同，不要求持久化敏感proposal或background recovery。最终整改让隔离矩阵走
registry-aware生产准入并补齐task双payload绑定及different-pair reverse-winner；独立targeted、Hook与workspace
门禁全绿。当前0 open Blocker、0 open Major，批准进入后续verification gate。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Blocker | `crates/pareto-kernel/src/event_store/hook_runtime.rs:853-923,1400-1490,1889-1943`; tests `tests.rs:2188-2350,2470-2540` | Kernel-owned execute已连通point start、reserve/terminal pair、cancel/timeout/late、skip/finalization。`point_start_event_id`现在绑定canonical完整`ExecuteHookPointCommand` fingerprint；finalized与start retry都核对该ID，task/time/correlation/deadline mutation稳定conflict且零写、零handler。取消产生唯一`Cancelled` terminal和late/skip。按SPEC-0007:78、RFC-0008:53/65/69/111重新校准：reserve-pending不得由point executor自动release/reexecute，而须显式terminal recovery；Event只保留安全digest，finalized retry返回同一durable decision/projection但不重建敏感Transform payload是合规fail-closed边界，不应要求background scanner、自动继续或持久化完整proposal。 | 独立focused mutation与Kernel-owned tests通过；low-level recovery/timeout terminal、Recorded零执行及projection恢复测试覆盖accepted边界。 | closed |
| F-002 | Blocker | `crates/pareto-kernel/src/event_store/hook_runtime.rs:2730-2800,3178-3230`; tests `tests.rs:2580-2695` | 同一MVCC horizon跨流验证已绑定actual Hook/Control envelope ID；reducer现要求rejection位于当前open point，按terminal decision找到invocation，并绑定point、hook/revision、subject、source、input、registry、redaction与固定reason。七类validly re-sealed mutation均由pure reducer fail closed；Projection/reopen/Recorded复用同一fold路径。 | `hook_runtime::kernel_owned_rejection`独立通过，代码逐字段核对；完整Hook filter中的该测试也通过。 | closed |
| F-003 | Major | pair persistence/query and pair commands in `event_store.rs` and `hook_runtime.rs` | 整改已按`pair_id`+`pair_kind`查询，持久化完整fingerprint、显式双sequence和canonical prepared-event preimage；zero/one/two、same-pair-new-events、cross-kind、two exact retry和mutation均在writer transaction内处理。跨流validator也核对双侧actual envelope identity。 | 独立Hook 39-test与workspace回归通过；pair mutation/corruption测试非零。 | closed |
| F-004 | Major | `crates/pareto-kernel/src/event_store/hook_runtime.rs:1471-2149`; `tests.rs:2296-2330` | Kernel执行循环现在首个Gate deny/invalid/required abstain后设置stop reason，后续Gate/Observer不调用并追加规范skip；Observer结果保持非权威。handler counter直接证明只调用Transform与首Gate。 | `hook_runtime::kernel_owned_gate_short_circuit`和全Hook filter通过。 | closed |
| F-005 | Major | `crates/pareto-protocol/src/hooks.rs:200-310`; Kernel registry/output/transform validation | Registry resolve现验证Manifest exact schema set、input/output/field/protected schema、handler compatibility及resource contract；Kernel构造protected view、递归JSON pointer逐字段验证，reason为闭合enum。Registry canonical key现包含已规范排序的完整`hook_points`向量、phase、priority、ID、revision，消除了旧实现只取首point的歧义，runtime仍按单point闭合phase排序。 | protocol hook contract、nested transform/protected/compatibility/output security filters通过；Schema retained sets与scope checker通过。 | closed |
| F-006 | Major | `crates/pareto-kernel/src/event_store/hook_runtime.rs:143-179,1683-1690,2174-2203`; tests `tests.rs:1450-1628` | Reserve command现在硬绑定Hook key与Control reservation的`task_id`。隔离矩阵通过`append_hook_reserve_pair_with_registry(..., Some(&resolved), ...)`覆盖tenant、user presence/value、workspace、run、scope actor、authenticated actor、task、subject及Hook identity，全部拒绝且双Event零写。registry-free helper受`#[cfg(test)]`限制；生产executor唯一调用传`Some(&resolved)`，未进入生产identity admission。budget concurrency使用不同pair ID/Event IDs/command bytes，在正反两种调用顺序下都exactly one commit、一个operation与一次reserve。此前红测已独立转为targeted 1/1及Hook 39/39。 | 独立targeted、Hook完整filter和workspace full均通过；代码调用图确认生产registry binding。 | closed |

# Acceptance trace

| Acceptance | Result | Independent evidence |
|---|---|---|
| AC-01/02/03 | passed | closed注册、kind×point、规范排序、Kernel-owned固定phase纵切已实现；未知组合默认拒绝。 |
| AC-04/05 | passed | Gate即时短路并写skip；Observer不能改business decision。 |
| AC-06/07/08 | passed | recursive Transform/Kernel protected view、narrow lease及完整scope/business identity no-write矩阵通过。 |
| AC-09 | passed | atomic pair zero/two/one、kind/identity/sequence/preimage、双侧actual Event binding已闭合。 |
| AC-10/11/12 | passed | success/failure/cancel/timeout唯一terminal、late audit、pair/point exact retry与full-command mutation均通过。 |
| AC-13 | passed | exact output schema、limit、kind语义、closed reason与安全rejection writer已实现。 |
| AC-14/15 | passed | point/lineage/finalization、双流pair与rejection audit重封均fail closed；pending只由显式terminal recovery，Recorded/reopen复用pure fold。 |
| AC-16 | passed | Recorded projection只读且独立测试证明Event/control facts不变、无handler入口。 |
| AC-17/18 | passed | SQLite v2与历史SchemaSet保留，当前set内容地址化；仅Fake Rust reference handler，无外部transport/ABI依赖。 |
| AC-19 | passed | crate-private live入口提供proposal/result；已提交retry只返回durable decision/projection，未扩大Event敏感payload或下游authority。 |
| AC-20 | passed | isolation与different-pair concurrency targeted、Hook 39-test和workspace回归全部绿色。 |

# Compatibility, permission, and isolation review

- SQLite保持v2，已发布SchemaSet保留byte-identical；当前Manifest/Hook schemas由exact retained reader解析，未增加依赖或第二状态表。
- reserve/terminal pair在单一`BEGIN IMMEDIATE`内进行registry-aware pre/post fold，Hook Projection在同一read transaction与同一MVCC horizon读取Hook和Runtime Control streams。
- Hook handler只收到Kernel构造的bounded request/lease，没有Event transaction、budget/cancellation/terminal authority；Recorded路径没有handler参数。
- 生产executor唯一reserve pair调用使用`Some(&resolved)`的registry-aware fold；registry-free wrapper仅为`#[cfg(test)]` fault/atomicity harness，不构成生产identity admission。

# Regression and test review

本Reviewer在Windows/PowerShell、offline、2026-08-29对 exact `e4877834...` 独立执行：

- `cargo test -p pareto-kernel hook_runtime::isolation --offline`：1 passed，160 filtered。
- `cargo test -p pareto-kernel hook_runtime::budget_concurrency --offline`：1 passed，160 filtered。
- `cargo test -p pareto-kernel hook_runtime:: --offline`：39 passed，0 failed，122 filtered。
- `cargo test --workspace --all-targets --all-features --offline`：Kernel 160 passed/1 ignored；Protocol unit 9 passed、baseline 1 ignored、contract 24 passed；exit 0。all-targets末尾schema generator binary无目标参数时仍打印非门禁`existing content-addressed schema set differs byte-for-byte`，checked-in schema deterministic contract test通过。
- `python scripts/check_req0008_scope.py`：passed；`python -m unittest discover -s scripts/tests -p "test_*.py"`：24 passed。
- `cargo fmt --all -- --check`：passed；`cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`：passed。

F-006的先前红测在exact revision上稳定转绿，且生产调用边界、零写与exactly-one-commit断言均直接覆盖；所有Blocker/Major已关闭。

# Scope and unrelated changes

实现基线至当前exact revision的历史diff含151个文件、8973 insertions/135 deletions，其中包括本Reviewer-owned初审记录和实现者active evidence。产品变化仍集中于Hook协议/schema、Kernel Event Store/Lifecycle/Projection/Runtime Control及测试；Cargo manifests/lock无新增依赖，未实现真实外部Hook runtime、REQ-0009或后续Effect/Provider/Tool/Sandbox能力。除本Review record外，本Reviewer未修改产品代码、测试、Requirement/Spec/RFC/ADR或实现者证据。

# Re-review history

- 2026-08-29：fresh independent implementation review exact `dfeee45f286c4dce4bdd950bf98d30cbc4b00fb8` against parent `b1f626ed52ffd7e0e6b4ad9c0cb457c12e8760d7`：2 Blocker、4 Major open。
- 2026-08-29：检查中间remediation `498597b5e43ccd628f53cd4aca259d6dbae9e791`，确认Kernel纵切、pair identity、short-circuit与schema/Transform大幅修复，同时独立发现cancel、actual Hook envelope binding、完整retry/recovery缺口；该candidate被后续exact revision取代，未作为最终reviewed revision。
- 2026-08-29：同一independent Reviewer复审 exact `4817ef31a95d00592840be50207ff59baf67f694`。关闭F-003/F-004/F-005；F-001/F-002保持Blocker，F-006保持Major。当前2 Blocker、1 Major，`changes-requested`。后续修复仍须由本Reviewer以新的exact revision和原始证据复审关闭。
- 2026-08-29：复审 exact `8cf926a64c0e611aa6b357b0bfb93fb5bf9d6601`。按accepted recovery/safe-digest边界关闭F-001；逐字段代码与七类重封负例关闭F-002。独立完整Hook filter稳定暴露`isolation`红测，F-006保持Major。当前0 Blocker、1 Major，`changes-requested`。
- 2026-08-29：最终复审 exact `e4877834fb54e3db936677f3b87c5fdf9e1d2d97`。task双payload绑定、registry-aware完整隔离矩阵和different-pair reverse-winner均经独立测试与调用图确认；关闭F-006。当前0 Blocker、0 Major，`approved`。
- 2026-08-29：closure freshness re-review exact `84ce5a705edb20f268898938be4579f4946d5e4f`。相对reviewer-approved runtime与其Review commit仅同步README/index/Epic/architecture/REQ-0008 done及active→archived证据；`crates/`、`schemas/`、Cargo和scripts零差异。归档证据准确引用exact `e4877834`、F-001至F-006 closed与0/0，且明确REQ-0009/真实外部Hook Runtime未实现。原findings/verdict不变，REVIEW-0011保持approved、0/0。
- 2026-08-30：focused REQ-0009 design freshness re-review exact `aba3a33703e681c542fd58b32f3d0ae41cff369d`。候选仅新增/修订REQ-0009 proposed/draft设计，Hook实现、Protocol/Schema、DB、Cargo、scripts与REQ-0008 accepted合同零变化；未来Effect不得把Hook handler/decision变成executor、producer或authority。REQ-0009未接受/实现，REVIEW-0012仍changes-requested。原findings/verdict不变，REVIEW-0011保持approved、0/0。
- 2026-08-30：final REQ-0009 design freshness re-review exact `021b353d0efc923ef8739e3cb97d88f586c4fe06`。最小设计修订未改Hook实现、Protocol/Schema、DB、Cargo、scripts或REQ-0008 accepted合同；REQ-0009仍changes-requested、未接受/实现。原findings/verdict不变，REVIEW-0011保持approved、0/0。
- 2026-08-30：one-line REQ-0009 freshness exact `b7acbd82824d8410d432117c89be1bd56c8ce05c`。仅proposed RFC一行变化，Hook实现/Schema/DB/Cargo/scripts零变化；REQ-0009尚未接受/实现。原verdict不变，REVIEW-0011保持approved、0/0。
- 2026-08-30：REQ-0009 design-acceptance closure freshness exact `60cee6ed44d150185bf99ca3095a8ce803bcc0d3`。仅接受已评审设计、创建ADR-0010并同步共享文档；Hook实现/Schema/DB/Cargo/scripts与REQ-0008合同零变化，REQ-0009未实现。原verdict不变，REVIEW-0011保持approved、0/0。
- 2026-08-30：REQ-0009 focused planning freshness exact `46772c7fbb30e82f0e8fd4fb50915e8414acaa65`。仅新增忠实planning并补齐状态/依赖门禁；Hook实现、Schema、DB、Cargo、scripts与REQ-0008合同零变化，未实现。原verdict不变，REVIEW-0011保持approved、0/0。
- 2026-09-01：REQ-0009 closure freshness exact `62bc44e250587594912f7ef16b431be6b1c12103`。仅同步独立批准后的done/archive事实，无Hook runtime变化；原verdict/findings不变。
- 2026-09-05：Verified Procedure 路线 freshness re-review exact `660cfca9e230f1440505c8e3bfd9a07bf17529ab`。candidate对Hook runtime、Protocol、Schema、DB、Cargo、tests及reserve/terminal pair实现零差异；新Procedure路线把Hook/adapter继续限制为proposal/observation，不能直写Event/Evidence/terminal或绕过Node/Capability。REQ-0008实现批准合同无回退，REVIEW-0011保持approved、0/0。
- 2026-09-05：路线接受 closure freshness exact `6df161ff5d5fc150cfa09f48ae54b7501cababcb`。closure对Hook Runtime、Protocol、Schema、DB、Cargo、tests及atomic pair零差异；ADR和accepted状态未新增Hook/adapter authority或实现声明。REQ-0008实现批准无回退，REVIEW-0011保持approved、0/0。
