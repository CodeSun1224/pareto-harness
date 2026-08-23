# REQ-0004 Delivery Plan

1. 建立最小 `pareto-kernel::event_store` 纵切及锁定依赖，实现 v1 SQLite open/migration/append-only DDL。
2. 实现 crate-private read/write admission、EventEnvelope/SchemaSet/limits 无损映射、幂等和连续 sequence 事务。
3. 实现 stable Stream/Run keyset readers、显式 append ordinal horizon、重启恢复与协议重验证。
4. 完成 Focused、Impacted、Core 测试，记录命令与结果；再执行独立 Code Review 并关闭 Blocker/Major。
5. 运行全部 completion gates，同步 durable docs，将 Requirement verified/done 并归档 work directory。

## Concrete validation commands

- Focused: `cargo test -p pareto-kernel --all-targets --all-features --offline`
- Impacted: `cargo test -p pareto-protocol --all-targets --all-features --offline`
- Core: `cargo test --workspace --all-targets --all-features --offline`
- Static: `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features --offline -- -D warnings`
- Governance: `python -m unittest discover -s scripts/tests -p "test_*.py"`; `python scripts/check_docs.py`
- Schema identity: `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas`; then `git diff --check` and verify `schemas/` unchanged.

不调用真实 Provider；质量、费用与延迟分别记录。实施若发现 approved Spec 未覆盖的权限、migration、cursor 或协议变化，立即停止实现并退回设计门禁。
