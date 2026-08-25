# Pareto Harness

> Evidence-governed, cost-aware runtime for coding agents.

Pareto Harness 的目标是在结果质量、Token/费用与端到端延迟之间持续寻找 Pareto 最优的 Agent 行为。项目采用“稳定机制内核 + 版本化策略”的架构：内核负责事件、版本、权限、重放和晋升安全，规划、上下文、路由、重试和评测策略可以独立实验与演化。

可信内核的前四个纵向切片已交付：REQ-0003 提供版本化协议类型、JSON Schema 和合同测试；REQ-0004 提供 Kernel 私有、真实 SQLite 文件支持的 append-only Event Store；REQ-0005 提供完整 Run Manifest 首事件、Run/Task 状态机和同事务 fold-and-append；REQ-0006 提供版本化 Run/Task Projection、带游标与摘要的同库 immutable Snapshot、prefix-proved 增量恢复、完整历史 Recorded replay 和 provenance comparison。所有 Replay 入口均不执行真实外部副作用；Simulated fixture resolver、Effect reexecute 和 CLI 仍未实现。

## Start here

- [项目文档导航](docs/index.md)
- [项目章程与 PRD](docs/product/project-charter.md)
- [总体架构](docs/architecture/overview.md)
- [竞品与研究洞察](docs/research/landscape.md)
- [路线图](docs/roadmap/roadmap.md)
- [Agent 协作规则](AGENTS.md)

## English summary

Pareto Harness is an independent coding-agent runtime designed to maximize verified outcome quality while minimizing token cost and end-to-end latency. It keeps event integrity, version identity, permissions, replay, and promotion safety in a trusted kernel while allowing fast-moving policies to be versioned, evaluated, canaried, and rolled back.

## Status

`pre-alpha / protocol, SQLite event-store, lifecycle, and local projection/replay foundation delivered`. Claims in research documents are evidence-graded; target metrics are not presented as achieved results.

## License

[Apache License 2.0](LICENSE)
