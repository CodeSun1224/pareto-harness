# Pareto Harness

> Evidence-governed, cost-aware runtime for coding agents.

Pareto Harness 的目标是在结果质量、Token/费用与端到端延迟之间持续寻找 Pareto 最优的 Agent 行为。项目采用“稳定机制内核 + 版本化策略”的架构：内核负责事件、版本、权限、重放和晋升安全，规划、上下文、路由、重试和评测策略可以独立实验与演化。

项目目前处于设计基线阶段。这里不会用空模块冒充实现；当前交付物是可审查的产品定义、研究证据、架构契约、评测协议和 AI Coding 协作规范。

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

`pre-alpha / architecture baseline`. Claims in research documents are evidence-graded; target metrics are not presented as achieved results.

## License

[Apache License 2.0](LICENSE)
