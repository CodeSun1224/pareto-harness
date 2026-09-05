---
id: EPIC-0005
title: 上下文、证据与路由增强
status: proposed
owners: [maintainers]
created: 2026-08-22
updated: 2026-09-05
links: [PRD-0001, REQ-0034, RFC-0013, BENCH-0001, BACKLOG-0001]
---

# Outcome

在已验证流程执行基线上分别验证自适应 Plan、Context、完整 Evidence Graph 和 Model Router 对质量、Token/费用和延迟的真实贡献。

# Planned requirements

REQ-0023 至 REQ-0027：自适应 Task DAG、Context DAG、Context Cache/GC、完整 Evidence Graph、成本感知 Router。REQ-0026 扩展 provenance 与复杂 evaluator，但不能替代 G2 已交付的最小 Evidence Gate。

# Exit criteria

每项优化都有独立消融和回归证据；只有满足质量底线并改善至少一个目标维度的候选进入默认 Behavior。
