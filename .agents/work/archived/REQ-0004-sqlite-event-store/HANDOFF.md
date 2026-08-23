# REQ-0004 Handoff

当前阶段：done。RFC-0003/ADR-0004 已接受、SPEC-0003 已批准，独立 REVIEW-0003 最终 0 Blocker / 0 Major。最小 `pareto-kernel::event_store` 纵切、SQLite v1 migration/append-only DDL、可信 authority admission、事务 append、Stream/Run horizon reader、retained Schema reader 与真实文件测试均已完成。

实现保持读写入口 crate-private，读取重新执行 exact SchemaSet/limits 协议验证；open 校验 application/store identity、migration checksum、固定 triggers、quick_check 与 envelope/index 漂移。未实现完整 capability、状态机、Projection 或 Replay executor。
