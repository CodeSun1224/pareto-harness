---
id: RES-0002
title: 研究证据账本
status: active
owners: [maintainers]
created: 2026-08-20
updated: 2026-08-27
links: [RES-0001, RFC-0007]
---

# 研究证据账本

等级：A = 官方代码/文档或原始论文；B = 作者工件或可复现实验；C = 可信二手资料；D = 待核验线索。成熟度与证据等级分开记录。

| ID | 对象与来源 | 核验 | 等级 | 成熟度 | 受支持声明 |
|---|---|---:|:---:|---|---|
| E-001 | [DeepSeek Harness architecture](https://github.com/deepseek-ai/deepseek-harness/blob/master/docs/architecture.md) | 2026-08-19 | A | 开源实现 | 官方架构将 model、tool registry、session log、agent loop 等描述为插件。 |
| E-002 | [DeepSeek Harness AGENTS](https://github.com/deepseek-ai/deepseek-harness/blob/master/AGENTS.md) | 2026-08-19 | A | 工程实践 | 使用 AGENTS、skills、agent notes、postmortems、snapshot 和文档门禁组织 AI Coding。 |
| E-003 | [DeepSeek Harness Agent Notes](https://github.com/deepseek-ai/deepseek-harness/tree/master/.agents/notes) | 2026-08-19 | A | 工程实践 | 非平凡变更记录问题、方案、备选、验收、风险或决策后果。 |
| E-004 | [OpenAI Codex plugins](https://developers.openai.com/codex/plugins) | 2026-08-19 | A | 产品能力 | Codex 插件将 Skills、MCP 等能力打包复用。 |
| E-005 | [OpenAI Codex hooks](https://developers.openai.com/codex/hooks) | 2026-08-19 | A | 产品能力 | Hooks 在工具调用、压缩和停止等生命周期点运行确定性脚本。 |
| E-006 | [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code/cli-usage) | 2026-08-19 | A | 产品能力 | CLI 暴露模型、权限、工具、会话恢复、最大轮次和结构化输出控制。 |
| E-007 | [Claude Code LLM gateway](https://docs.anthropic.com/en/docs/claude-code/llm-gateway) | 2026-08-19 | A | 产品集成 | 官方文档描述通过第三方网关进行用量、成本、路由、回退和审计。 |
| E-008 | [LangGraph durable execution](https://docs.langchain.com/oss/python/langgraph/durable-execution) | 2026-08-20 | A | 开源实现 | Checkpoint 与 durable execution 支持暂停、恢复和重放图执行。 |
| E-009 | [OpenHands events](https://docs.openhands.dev/sdk/arch/events) | 2026-08-20 | A | 开源实现 | SDK 使用不可变、类型化的 append-only event log 驱动 Agent 执行和状态管理。 |
| E-010 | [RouteLLM](https://github.com/lm-sys/RouteLLM) | 2026-08-20 | A | 开源研究实现 | 使用路由器在强弱模型之间选择以优化质量与成本。 |
| E-011 | [DSPy](https://dspy.ai/) | 2026-08-20 | A | 开源实现 | 将 LM 程序参数化并基于指标优化，而非只手写 Prompt。 |
| E-012 | [AFlow paper](https://arxiv.org/abs/2410.10762) | 2026-08-20 | A | 论文/代码原型 | 通过搜索优化 Agent 工作流；结果依赖任务、模型和评测设置。 |
| E-013 | [Gödel Agent paper](https://arxiv.org/abs/2410.04444) | 2026-08-20 | A | 论文原型 | 探索 Agent 修改自身逻辑并以任务表现接受修改。 |
| E-014 | [Meta-Harness](https://openreview.net/pdf?id=WxCOyYbmbT) | 2026-08-19 | A | 论文/研究实现 | 从完整轨迹搜索 Harness，并维护多目标候选前沿。 |
| E-015 | [Continual Harness](https://github.com/sethkarten/continual-harness) | 2026-08-19 | B | 新近研究实现 | 探索固定模型上的 Harness 在线适应；收益具有模型能力依赖。 |
| E-016 | [Model Context Protocol](https://modelcontextprotocol.io/specification/) | 2026-08-20 | A | 标准/生态 | MCP 标准化 Host、Client、Server 之间的上下文与工具连接，不规定完整 Agent 内核。 |
| E-017 | [OpenAI Codex Rust workspace](https://github.com/openai/codex/blob/main/codex-rs/Cargo.toml) | 2026-08-27 | A | 开源产品实现 | 官方 Cargo workspace 列出 `core`、`cli`、`exec`、`tui` 等 Rust crate；该来源只证明 workspace 成员及其 Rust 构建身份。 |
| E-018 | [OpenAI Codex core](https://github.com/openai/codex/blob/main/codex-rs/core/README.md) | 2026-08-27 | A | 开源产品实现 | `codex-core` README 明确称其实现 Codex 的业务逻辑，并供以 Rust 实现的 UI 使用。 |
| E-019 | [OpenAI Codex repository](https://github.com/openai/codex) | 2026-08-27 | A | 开源产品交付 | 官方仓库为支持的平台发布独立 Codex CLI 二进制；该事实不单独证明内部信任边界。 |
| E-020 | [OpenAI Codex TypeScript SDK](https://github.com/openai/codex/blob/main/sdk/typescript/README.md) | 2026-08-27 | A | 开源产品 SDK | TypeScript SDK 启动 `@openai/codex` CLI，并通过 stdin/stdout JSONL 事件交互。 |
| E-021 | [OpenAI Codex Python SDK packaging](https://github.com/openai/codex/blob/main/sdk/python/pyproject.toml) | 2026-08-27 | A | 开源产品 SDK | Python SDK 声明对固定版本 `openai-codex-cli-bin` Runtime 包的依赖；该来源不证明 SDK 与 CLI 之间的安全边界。 |
| E-022 | [OpenAI Codex core sandbox support](https://github.com/openai/codex/blob/main/codex-rs/core/README.md) | 2026-08-27 | A | 开源产品实现 | `codex-core` README 描述 `SandboxPolicy`，并列出 macOS Seatbelt 与 Linux Landlock/bubblewrap 等平台 enforcement 机制。 |
| E-023 | [Claude Code advanced setup](https://code.claude.com/docs/en/setup) | 2026-08-27 | A | 产品能力 | 推荐安装和 npm 包交付同一平台原生二进制；公开资料不足以据此断言内部核心实现语言。 |
| E-024 | [Claude Code Hooks reference](https://code.claude.com/docs/en/hooks) | 2026-08-27 | A | 产品扩展能力 | Hook handler 可为 shell command、HTTP、MCP、prompt 或 agent；生命周期扩展面不要求与主程序同语言。 |
| E-025 | [Claude Code Plugins](https://code.claude.com/docs/en/plugins) | 2026-08-27 | A | 产品扩展能力 | Plugin 可打包 Skills、Agents、Hooks、MCP/LSP server、monitor 和可执行文件，形成多种进程与语言边界。 |
| E-026 | [Claude Code Memory](https://code.claude.com/docs/en/memory) | 2026-08-27 | A | 产品能力 | 项目指令与 auto memory 使用可审计 Markdown 文件并按项目隔离。由此推断 Memory 表示/策略可以与核心 Runtime 语言解耦；来源不证明其权威准入模型。 |
| E-027 | [Claude Code Sandboxing](https://code.claude.com/docs/en/sandboxing) | 2026-08-27 | A | 产品安全能力 | 权限在工具执行前判定，Bash 子进程再由 Seatbelt/bubblewrap 与网络代理做 OS 级限制；官方同时公开平台和网络过滤限制。 |

## 证据缺口

- 对 CMV、VeriGraph、EviGraph、GEPA 和 Context DAG 相关实现进行逐论文复核并补充代码可用性。
- 以相同版本的 Codex、Claude Code、DeepSeek Harness、OpenHands 和 LangGraph 执行统一任务集；当前产品比较属于架构能力比较，不是性能排名。
- 核验 Pareto Harness 名称在代码托管、包管理、域名和商标层面的可用性。
- 对 Codex/Claude Code 的公开比较只支持“原生核心与多语言扩展面可分离”的架构事实；Claude Code 未公开完整核心源码，不能从安装载体推断其内部语言或与 Pareto Harness 的事件权威模型等价。

## 维护规则

时间敏感结论每季度或重大版本发布后重新核验。量化结果必须同时记录任务集、模型、预算、样本数、统计方法和失败样本；无法复现时不得写成普遍事实。
