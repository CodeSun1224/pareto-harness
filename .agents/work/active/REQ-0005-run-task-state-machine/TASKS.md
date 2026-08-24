# Requirement Tasks

Current execution: `TASK-REQ-0005-07` second remediation/re-review in progress;
TASK-REQ-0005-06 has the final review-freshness gate pending. REVIEW-0004 independently
re-reviewed exact revision `38f0beb710fdd05ffd7b7047db6e3cb7cb7a2f79`, closed F-001/F-002/F-004,
and retained F-003 as the sole open Major. The second remediation makes fold perform exact
SchemaSet RunManifest semantic admission; only the independent reviewer may close F-003.

- [x] TASK-REQ-0005-01: 增加版本化 Task ID、Run/Task state 与 lifecycle payload/event bindings，发布新内容地址 SchemaSet并证明旧set不变。Validation: `cargo test -p pareto-protocol lifecycle_manifest_contract --offline`; schema generation diff。
- [x] TASK-REQ-0005-02: 增加 crate-private persisted Manifest bootstrap reader 和 transaction-local append/check primitive，不改变 SQLite `user_version` 或公开authority surface。Validation: `cargo test -p pareto-kernel lifecycle::manifest --offline`; `cargo test -p pareto-kernel --doc --offline`。
- [x] TASK-REQ-0005-03: 实现 pure fold、create/established authority、Run/Task create、完整合法迁移和parent/Run/terminal guards。Validation: `cargo test -p pareto-kernel lifecycle::state_machine --offline`; `cargo test -p pareto-kernel lifecycle::hierarchy --offline`。
- [x] TASK-REQ-0005-04: 在同一 `BEGIN IMMEDIATE` 中实现幂等、expected sequence/state、fold和单事件append，覆盖same-ID mutation、两pool竞争、rollback和commit-response uncertainty。Validation: `cargo test -p pareto-kernel lifecycle::idempotency --offline`; `cargo test -p pareto-kernel lifecycle::concurrency --offline`; `cargo test -p pareto-kernel lifecycle::transaction --offline`。
- [x] TASK-REQ-0005-05: 完成bounded模型序列、终态/迟到、Manifest不可变、隔离、崩溃重开、旧reader和Projection/Replay fold合同测试。Validation: `cargo test -p pareto-kernel lifecycle::model_sequences --offline`; `cargo test -p pareto-kernel lifecycle::terminal_and_late --offline`; `cargo test -p pareto-kernel lifecycle::isolation --offline`; `cargo test -p pareto-kernel lifecycle::recovery --offline`; `cargo test -p pareto-kernel lifecycle::compatibility --offline`; `cargo test -p pareto-kernel lifecycle::fold_contract --offline`。
- [ ] TASK-REQ-0005-06: 执行Focused、Impacted、Core和completion gates，记录实际命令/环境/结果及quality/cost/latency观察。Validation: `cargo test --workspace --all-targets --all-features --offline`; repository completion commands in PLAN。Runtime/static/schema gates passed; final `check_docs.py` rerun awaits independent REVIEW-0004 freshness.
- [ ] TASK-REQ-0005-07: 由独立Agent/新会话执行code-review，关闭并复审全部Blocker/Major。首轮为 0/4；`38f0beb` focused re-review 后为 0/1，F-001/F-002/F-004 closed，F-003 standalone fold semantic admission remediation 已实现并通过定向测试，等待下一 exact revision re-review。Validation: future REVIEW-0004 approved，`independence: independent`，0 open Blocker/Major。
- [ ] TASK-REQ-0005-08: 同步implemented facts、完成全门禁、将REQ-0005 verified/done并归档work；完成前不得启动REQ-0006 implementation。Validation: `python scripts/check_docs.py`; `git diff --check`; `git status --short`。
