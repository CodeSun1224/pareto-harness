# Requirement Tasks

Current execution: implementing / TASK-REQ-0008-10。设计 exact `3aee02adf8815466b02f51de247ae19922efc126` 已由 fresh independent REVIEW-0010批准，F-001至F-004全部closed，0 open Blocker/Major；ADR-0009和accepted docs exact `3318cbc6fe8bc8c9717a5a2b4aea1153f0d281d6` 已完成独立freshness复核。

- [x] TASK-REQ-0008-00: 完成Requirement、直接/间接影响分析、SPEC、RFC、AC→测试矩阵、fresh independent设计Review、finding整改复审、ADR接受和active work门禁。Validation: REVIEW-0010 `approved`、0 open Blocker/Major，reviewed revision `3318cbc6fe8bc8c9717a5a2b4aea1153f0d281d6`；产品/Schema/Cargo/script diff为零。
- [x] TASK-REQ-0008-01: 新增闭合Hook Protocol类型、Manifest registry role/new major、Hook Event/Projection/handler-output Schema和内容地址SchemaSet，保留全部旧set与旧Run解释。Validation: Protocol 9 unit + 24 contract（1 observation ignored）；current `sha256-3a0c6e67a97675cf6bfcdc1fb9766b30a79ae62e662479d9ae1ef5d7b43ff99d`，retained sets未改。
- [x] TASK-REQ-0008-02: 实现crate-private Hook derived stream、sequence-1 source contract、exact reader、pure fold、Projection与read-only Recorded replay入口，不改SQLite v2或增加第二状态表。Validation: `hook_runtime::{fold_contract,recovery,compatibility,recorded_replay}` 4 passed；Recorded前后Event计数不变，Live/Reexecute入口拒绝。
- [x] TASK-REQ-0008-03: 实现Manifest-pinned registry、kind×point矩阵、固定phase、稳定phase-local顺序、input lineage、point decision/finalization和Rust Fake handler boundary。Validation: `hook_runtime::{kind_point_table,phase_order_lineage,ordering}`及bounded Fake handler通过。
- [x] TASK-REQ-0008-04: 实现transaction-local Runtime Control admission与reserve/terminal atomic pair command，覆盖zero/two exact、mutation、one-existing corruption、第一/第二insert及commit fault rollback、response-loss和single-stream terminal拒绝。Validation: 五个命名filter通过。
- [x] TASK-REQ-0008-05: 实现bounded authority、全scope隔离、Capability撤销/收窄、trusted envelope、Run/Task/Actor/operation预算及并发防重复预留。Validation: `authority/isolation/budget_reserve/budget_concurrency/settlement`通过。
- [x] TASK-REQ-0008-06: 实现Observer非权威失败语义、Gate组合/default deny、Transform pipeline/mask/protected fields及不可信输出limits/Schema。Validation: 七个命名filter通过。
- [x] TASK-REQ-0008-07: 实现FakeClock cancel/deadline/timeout authority、reopen recovery、terminal race与late/duplicate/retry安全审计。Validation: 五个命名filter通过，无真实sleep。
- [x] TASK-REQ-0008-08: 完成Fake Run/Task→Manifest pin→Fake Gate→pair settlement→Projection recovery纵切，并证明Recorded零handler/append/核算及unsupported mode拒绝。Validation: `recorded_replay/unsupported_modes`通过。
- [x] TASK-REQ-0008-09: 完成全部Focused/Impacted/Core、真实SQLite fault/concurrency、兼容/隔离/安全/model测试，新增scope checker并运行AGENTS完整门禁，记录`VALIDATION.md`。Validation: 29个命名filter均matched 1并通过；Hook 32、Kernel 153/1 ignored、Protocol 9+24/1 ignored、Python 24通过；Schema双生成稳定、DB v2/retained/scope通过；`check_docs.py`仅按设计等待fresh实现Review恢复freshness。
- [x] TASK-REQ-0008-10: fresh independent REVIEW-0011初审2 Blocker/4 Major，经同一Reviewer多轮复审全部关闭。Validation: REVIEW-0011 `approved`、independence independent、reviewed revision `e4877834fb54e3db936677f3b87c5fdf9e1d2d97`、0 open Blocker/Major。
- [x] TASK-REQ-0008-11: 同步implemented facts与最终freshness，REQ-0008 reviewing→verified→done并归档active work；确认REQ-0009尚未开始。Validation: 完整门禁最终复跑、Review 0/0、工作区只含预期提交。
