# Pareto Harness

> Kernel-governed runtime for reusable, evidence-verified coding procedures.

Pareto Harness 的目标是把经过证据验证和独立批准的成功任务路径提升为不可变、内容寻址的已验证流程版本，并由可信内核在后续运行中强制节点、依赖、能力、证据、检查点、恢复和补偿规则。模型只能提出动作，不能跳过流程或自行宣告完成。在该质量底线之上，项目分别优化验证质量、Token/费用与端到端延迟的 Pareto 前沿。

可信内核的前七个纵向切片已交付：REQ-0003 提供版本化协议与 Schema；REQ-0004 至 REQ-0006 提供 SQLite Event Store、生命周期、Projection/Snapshot/Recorded replay；REQ-0007 提供默认拒绝 Runtime Control；REQ-0008 提供 Kernel 治理的 Fake Hook；REQ-0009 提供 Manifest-pinned Fake Effect、Intent-before-dispatch、幂等 claim、Receipt admission、原子结算、崩溃恢复、对账及 fixed-horizon Recorded replay。已验证流程、Plan/Task DAG、节点状态机、执行期 Evidence Gate、Provider、Tool、Workspace/Sandbox、Simulated/reexecute 与 CLI 均尚未实现。

## Start here

- [项目文档导航](docs/index.md)
- [项目章程与 PRD](docs/product/project-charter.md)
- [总体架构](docs/architecture/overview.md)
- [竞品与研究洞察](docs/research/landscape.md)
- [路线图](docs/roadmap/roadmap.md)
- [Agent 协作规则](AGENTS.md)

## English summary

Pareto Harness is a coding-agent runtime for promoting independently approved, evidence-backed task paths into immutable verified procedure revisions. A trusted kernel enforces procedure nodes, dependencies, capabilities, evidence, recovery, and effects while versioned strategies remain replaceable. Quality, cost, and latency are evaluated separately.

## Status

`pre-alpha / trusted-kernel protocol, event-store, lifecycle, replay, runtime-control, fake-hook, and fake-effect foundation delivered`. Claims in research documents are evidence-graded; target metrics are not presented as achieved results.

## License

[Apache License 2.0](LICENSE)
