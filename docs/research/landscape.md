---
id: RES-0001
title: 竞品与前沿研究洞察
status: active
owners: [maintainers]
created: 2026-08-20
updated: 2026-08-27
links: [PRD-0001, RFC-0001, RFC-0007]
---

# 竞品与前沿研究洞察

## 结论先行

Task Graph、Context 管理、模型路由、事件日志、证据图和 Agent 自动优化都已有单项实现或论文原型。当前可挖掘空间不在于给已有名词换名字，而在于把它们收敛为具有可信内核、正交版本身份、证据准入、历史回放、并发控制和渐进晋升的生产闭环。

本文件是综合判断；逐条来源和证据等级见 [证据账本](evidence-register.md)。

## 产品比较

| 对象 | 已展示重点 | 值得借鉴 | Pareto Harness 的不同问题 |
|---|---|---|---|
| OpenAI Codex | Skills、MCP、插件与生命周期 Hooks；稳定产品运行时 | 简洁入口、渐进披露、确定性 Hook | 行为版本如何跨任务评测、晋升和回滚 |
| Claude Code | 权限工具、MCP、Hooks/自动化、子 Agent 与项目指令 | 用户体验、权限配置、Agent 文件生态 | 将质量/成本/延迟和证据纳入统一运行时闭环 |
| DeepSeek Harness/Cordis | 模型、工具、Session、Agent Loop 等高度插件化 | 生命周期组合、Agent Notes、Skills、快照测试 | 插件不能替换治理宪法；外部效果不能靠卸载插件回滚 |
| OpenHands | EventStream、Agent/Runtime 分离、可运行软件任务 | 事件驱动和执行环境 | 正交 Behavior Revision 与受控策略演化 |
| LangGraph | 图执行、Checkpoint、分支和 Human-in-the-loop | Durable execution 与状态图 | 面向 Coding Agent 的证据、成本和行为晋升协议 |
| LiteLLM/RouteLLM | Provider 统一和模型路由 | 成本可观测、路由基线 | 路由与 Task/Context/Evidence 状态联合决策 |

### 关于“万物插件化”

Cordis 式设计的收益是模块组合、测试替换和生命周期统一；代价是依赖关系、初始化顺序、跨插件状态和调试路径变得隐式。“插件卸载可逆”通常只意味着注册项被撤销，不意味着已经执行的文件、网络、数据库或模型语义效果被回滚。

Pareto Harness 因此采用窄腰：策略和适配器可插件化，事件完整性、版本、权限、效果提交、证据准入、晋升和回滚协议不可插件化。内核仍通过明确 trait 保持可测试性，但 trait 不等于允许第三方替换安全语义。

### 关于“Rust 内核”与“全 Rust 产品”

已核验的官方实现表明，原生可信核心和多语言扩展面可以分离。OpenAI Codex 的维护中 CLI/core 以 Rust workspace 和独立二进制为中心，而 TypeScript SDK 通过启动 CLI 与 JSONL 事件接入；Codex 的 Sandbox policy 在 core 中管理，实际 enforcement 仍调用操作系统隔离原语。Claude Code 当前交付平台原生二进制，但其公开资料不足以断言核心实现语言；可以确认的是 Hooks、MCP、Plugins、LSP、可执行文件、Markdown Memory 与 OS Sandbox 构成多种语言无关边界。

上述是产品架构事实，不是质量、成本或延迟排名，也不证明 JSONL、MCP、Hook 或原生二进制天然安全。对 Pareto Harness 的推论是：Rust 应拥有 Event、identity、state、Capability、Budget、Cancellation、Effect/Evidence admission、Replay、Lease/MVCC 与 Promotion 等权威裁决；Provider、Tool、Hook handler、Agent Worker、Memory 检索/排序、评测和候选生成可以多语言，但必须通过版本化、最小授权、可取消、可核算和可审计的窄腰接入。具体 transport 和 Runtime 仍需由真实 Requirement 证明，不能从竞品机制直接照搬。

## 研究簇与洞察

### Task DAG 与 durable execution

工作流图、状态机、Checkpoint 和 DAG 调度已成熟。创新空间是将计划本身建模为 `PlanRevision`，让每次重规划保留父版本、触发证据和对关键路径成本的影响，并将任务验收连接到 Evidence Loop。

### Context DAG

上下文压缩、检索、记忆和分支快照已有大量实现。值得进一步构建的是来源与派生关系显式的 Context DAG：每个投影记录为何入选、依赖谁、何时失效、占多少 Token、支持哪个决策。这样才能评估一次压缩究竟是节省还是破坏证据。

### Model Router

RouteLLM 和网关产品证明跨模型路由可降低成本，但 Coding Agent 的路由状态比单轮 Query 更丰富。路由输入应包括任务阶段、剩余预算、证据缺口、失败史、上下文规模、工具可用性和不确定性；升级/降级决定必须进入事件日志。

### Evidence Loop/Graph

证据图、检索增强验证、程序测试和 LLM-as-judge 已分别存在。项目的重点是把证据变成完成状态机的准入条件：Requirement → Evidence Need → Evidence → Verifier → Verdict，并记录证据版本、适用范围和失效原因。

### Evolution Engine

DSPy、AFlow、Gödel Agent、GEPA、Meta-Harness 和 Continual Harness 表明 Prompt、工作流、Skill 或 Harness 可以通过轨迹和评测自动优化。自动提出候选并不等于生产可演化；仍需解决数据泄漏、过拟合、基准污染、权限、并发候选、成本上限、Canary 和回滚。

## 可形成壁垒的组合

1. `RunManifest` 提供跨模型、工具、Prompt、Skill、上下文和工作区的完整行为指纹。
2. Event Log 是事实源，Task/Context/Evidence 图是可重建投影，减少多套真相。
3. 通过相同任务与环境的配对重放进行行为差异归因。
4. 使用 Pareto dominance 和最低质量底线决定候选，而非可操纵综合分。
5. Evolution Proposal 使用 MVCC 基线，多候选必须显式 rebase 或 merge。
6. 策略只能经 Capability API 产生效果，晋升权属于内核控制面。

## 风险

- Benchmark overfitting：保留隐藏集、时序切分和真实回归集。
- Judge bias：优先确定性测试；LLM Judge 必须版本化并进行一致性校准。
- Replay illusion：外部 API、时间和并发不可完全重放，必须记录边界。
- Optimization tax：演化本身消耗 Token 和时间，全部计入总拥有成本。
- Complexity budget：每个新增抽象必须证明能改善正确性、可解释性或 Pareto 前沿。
