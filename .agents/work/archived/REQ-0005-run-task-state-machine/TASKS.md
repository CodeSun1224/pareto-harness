# Requirement Tasks

Current execution: complete. Independent REVIEW-0004 approved exact revision
`675e3f8fe6888c1d01fec14dda8e0f9164bb8a1b` with 0 open Blocker/Major; F-001 through
F-004 are independently closed. Runtime/static/schema completion gates passed, durable facts
are synchronized, and this work record is archived. REQ-0006 implementation has not started.

- [x] TASK-REQ-0005-01: 增加版本化 Task ID、Run/Task state 与 lifecycle payload/event bindings，发布新内容地址 SchemaSet并证明旧set不变。Validation: `cargo test -p pareto-protocol lifecycle_manifest_contract --offline`; schema generation diff。
- [x] TASK-REQ-0005-02: 增加 crate-private persisted Manifest bootstrap reader 和 transaction-local append/check primitive，不改变 SQLite `user_version` 或公开authority surface。Validation: `cargo test -p pareto-kernel lifecycle::manifest --offline`; `cargo test -p pareto-kernel --doc --offline`。
- [x] TASK-REQ-0005-03: 实现 pure fold、create/established authority、Run/Task create、完整合法迁移和parent/Run/terminal guards。Validation: `cargo test -p pareto-kernel lifecycle::state_machine --offline`; `cargo test -p pareto-kernel lifecycle::hierarchy --offline`。
- [x] TASK-REQ-0005-04: 在同一 `BEGIN IMMEDIATE` 中实现幂等、expected sequence/state、fold和单事件append，覆盖same-ID mutation、两pool竞争、rollback和commit-response uncertainty。Validation: `cargo test -p pareto-kernel lifecycle::idempotency --offline`; `cargo test -p pareto-kernel lifecycle::concurrency --offline`; `cargo test -p pareto-kernel lifecycle::transaction --offline`。
- [x] TASK-REQ-0005-05: 完成bounded模型序列、终态/迟到、Manifest不可变、隔离、崩溃重开、旧reader和Projection/Replay fold合同测试。Validation: `cargo test -p pareto-kernel lifecycle::model_sequences --offline`; `cargo test -p pareto-kernel lifecycle::terminal_and_late --offline`; `cargo test -p pareto-kernel lifecycle::isolation --offline`; `cargo test -p pareto-kernel lifecycle::recovery --offline`; `cargo test -p pareto-kernel lifecycle::compatibility --offline`; `cargo test -p pareto-kernel lifecycle::fold_contract --offline`。
- [x] TASK-REQ-0005-06: 执行Focused、Impacted、Core和completion gates，记录实际命令/环境/结果及quality/cost/latency观察。Validation: workspace Kernel 33 + Protocol 9/19，governance 18，fmt/clippy/schema/diff gates passed；document freshness由最终独立 review refresh 证明。
- [x] TASK-REQ-0005-07: 由独立Agent/新会话执行code-review，关闭并复审全部Blocker/Major。首轮 `d0011d0` 为 0/4；`38f0beb` 后为 0/1；`675e3f8` 后 F-003 closed，REVIEW-0004 approved、`independence: independent`、0 open Blocker/Major。
- [x] TASK-REQ-0005-08: 同步implemented facts、完成全门禁、将REQ-0005 verified/done并归档work；REQ-0006 implementation 未启动。Validation: README/index/EPIC/ARCH 同步；closure revision 后由 independent freshness review 复跑 `python scripts/check_docs.py`；`git diff --check`; `git status --short`。
