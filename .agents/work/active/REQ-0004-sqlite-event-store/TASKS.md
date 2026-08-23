# Requirement Tasks

- [x] TASK-REQ-0004-01: 建立最小 kernel crate、sqlx/Tokio 锁定依赖和原子 v1 migration/append-only DDL。Validation: `cargo test -p pareto-kernel event_store::migration --offline`。
- [x] TASK-REQ-0004-02: 实现私有 admission、无损映射、exact SchemaSet/limits binding、幂等与连续 sequence 事务。Validation: `cargo test -p pareto-kernel event_store::append --offline`。
- [x] TASK-REQ-0004-03: 实现 Stream/Run horizon readers、重启/兼容/隔离/崩溃与并发真实 SQLite 测试。Validation: `cargo test -p pareto-kernel --all-targets --all-features --offline`。
- [x] TASK-REQ-0004-04: 运行 Impacted/Core gates 并记录 VALIDATION。Validation: `cargo test --workspace --all-targets --all-features --offline`。
- [ ] TASK-REQ-0004-05: 独立 Code Review，关闭 Blocker/Major 并 re-review。Validation: `python scripts/check_docs.py`。
- [ ] TASK-REQ-0004-06: 完成全门禁、同步文档与归档。Validation: `git diff --check`; `git status --short`。
