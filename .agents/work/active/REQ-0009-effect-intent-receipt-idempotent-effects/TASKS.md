# Requirement Tasks

Current execution: planned / TASK-REQ-0009-01。设计exact `b7acbd82824d8410d432117c89be1bd56c8ce05c`及接受闭环exact `60cee6ed44d150185bf99ca3095a8ce803bcc0d3`已由fresh independent REVIEW-0012批准，0 open Blocker/Major；产品实现尚未开始。

- [x] TASK-REQ-0009-00: 完成Requirement、直接/间接影响分析、SPEC、RFC、AC→测试矩阵、fresh independent设计Review、finding整改复审、ADR接受和设计freshness门禁。Validation: REVIEW-0012 `approved`、independent、0 open Blocker/Major；设计接受commit `60cee6e`，Review提交`1a49817`。
- [ ] TASK-REQ-0009-01: 新增闭合Effect Protocol、Manifest v3 registry/executor role、Effect Event/Projection与Boundary Inventory/Record V2 Schema，保留全部旧set与旧Run解释。Validation: `effect_contract`、Protocol全套、Schema双生成稳定和retained byte identity。
- [ ] TASK-REQ-0009-02: 实现crate-private Effect stream、exact reader、continuous fold、Projection与显式cursor/history digest读取，不改SQLite v2或增加第二权威状态表。Validation: `fold_contract/projection_recovery/compatibility`。
- [ ] TASK-REQ-0009-03: 实现Runtime Control与Effect reserve/Intent及terminal/conclusion原子pair，覆盖fault、one-sided corruption、mutation和response-loss retry。Validation: `intent_before_dispatch/atomic_settlement`及SQLite fault injection。
- [ ] TASK-REQ-0009-04: 实现Kernel admission、幂等identity和exact retry/conflict语义。Validation: `default_deny/idempotency/isolation`。
- [ ] TASK-REQ-0009-05: 实现dispatch claim、bounded lease、exact executor descriptor与确定性Fake executor outcomes。Validation: `dispatch_lease/fake_outcomes`。
- [ ] TASK-REQ-0009-06: 实现Receipt observation admission、state model、safe evidence/redaction、late/duplicate处理与保守预算核算。Validation: `receipt_admission/state_model/partial_success/late_receipts`。
- [ ] TASK-REQ-0009-07: 实现crash/cancel/timeout recovery command、稳定优先级、未claim零usage释放、已claim partial/unknown与reconciliation。Validation: `crash_recovery/cancellation_timeout/reconciliation`。
- [ ] TASK-REQ-0009-08: 实现success guard、Inventory/Record V2 fixed horizon与Recorded replay零执行/写入/核算。Validation: `lifecycle_success_guard/recorded_replay`。
- [ ] TASK-REQ-0009-09: 完成完整隔离、权限、redaction、兼容与scope负向测试，确认无真实外部I/O、自动redispatch或新依赖。Validation: `isolation/compatibility`和`python scripts/check_req0009_scope.py`。
- [ ] TASK-REQ-0009-10: 跑完Focused/Impacted/Core/Full适用门禁、Schema双生成与完整仓库门禁，并写入`VALIDATION.md`。Validation: PLAN全部命令有exact结果与非零filter计数。
- [ ] TASK-REQ-0009-11: 由新的fresh independent Agent执行实现code review；实现者整改，原Reviewer复审关闭全部Blocker/Major。Validation: 新Review为`approved`、independent、0 open Blocker/Major且固定exact implementation revision。
- [ ] TASK-REQ-0009-12: 同步implemented facts与最终freshness，Requirement reviewing→verified→done并归档active work。Validation: 完整门禁最终复跑、Review 0/0、工作区仅含预期提交。
