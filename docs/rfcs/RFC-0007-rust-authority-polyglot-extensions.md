---
id: RFC-0007
title: Rust 权威控制面与多语言扩展边界
status: proposed
owners: [maintainers]
created: 2026-08-27
updated: 2026-08-27
links: [REQ-0001, RFC-0001, ADR-0001, ADR-0002, ARCH-0001, ARCH-0002, ARCH-0004, ROADMAP-0001, BACKLOG-0001, RES-0001, RES-0002]
---

# Summary

G2 至 G5 不以“所有产品代码继续使用 Rust”为目标，而以“所有权威裁决继续由 Rust 可信控制面提交”为稳定边界。Rust 保留事件完整性、版本身份、状态迁移、Capability、Budget、Cancellation、Effect admission/Receipt、Replay admission、Lease/DAG 裁决、Evidence completion gate、MVCC 与 Promote/Rollback；Provider、Tool、Hook handler、Agent Worker、Memory 检索/排序、评测与策略候选可以用最合适的语言实现，但只能通过版本化、最小授权、可取消、可核算、可审计的边界提出请求、执行已授权操作或返回非权威结果。

本 RFC 只接受责任和信任边界，不提前接受某个进程模型、JSONL/JSON-RPC/MCP、队列、gRPC、FFI、容器或远程服务。具体传输、生命周期、Schema、权限与失败语义仍由首次需要该边界的 Requirement 单独批准。

# Motivation and requirements

REQ-0001 要求架构明确可信内核、Runtime、版本化策略和扩展边界。ADR-0001/0002 已决定可信内核使用 Rust 模块化单体起步，但“Rust 可信内核”不应被扩大解释为“G2 至 G5 的 CLI、Provider、Tool、Hook、Memory、Sandbox、Agent Worker、评测与插件都必须用 Rust”。这种扩大解释会把安全责任与语言偏好混为一谈，增加生态适配、研究原型和插件开发成本，也不能替代 OS Sandbox 或协议准入。

经核验的公开事实支持“原生核心 + 多语言边界”而非单语言全栈：OpenAI Codex 的维护中 CLI/core 以 Rust 为主，TypeScript SDK 通过启动 CLI 和 JSONL 事件接入，Python SDK 也打包原生 CLI；Claude Code 当前交付签名原生二进制，同时把 Hook、MCP、Plugin、LSP、Memory 和 OS Sandbox 暴露为语言无关或外部执行边界。该比较是架构机制证据，不是性能排名，也不证明任何具体传输天然安全。

## 讨论前提

1. **Rust 的目标是缩小权威面，不是最大化 Rust 代码量。** 语言不会自动形成安全边界；只有 authority、admission、effect、evidence 和 recovery 的所有权能定义可信边界。
2. **Reference implementation 不等于语言强制。** G2 可先用 Rust 实现 Provider/Tool/Hook reference path，但兼容接口不能要求未来实现链接 Rust ABI 或获得内核内部对象。
3. **非 Rust 组件不是同权插件。** 它们可以 propose、observe、compute 或在授予范围内 execute；不能直接 append 权威 Event、修改 Manifest、签发 Capability、增加 Budget、决定终态、伪造 Receipt/Evidence 或 Promote。
4. **Sandbox enforcement 不只属于 Rust。** Rust 负责策略、授权、预算、deadline 与审计；实际隔离由 Linux/macOS/Windows 原语、容器或 WASI 执行，平台能力差异必须显式。
5. **Memory 必须拆成事实与策略。** 来源、隔离、版本、失效和权威引用属于可信面；embedding、retrieval、rerank、compression 和总结可以演化或由外部 Worker 计算。
6. **传输按风险选择，不按语言预选。** Artifact batch、子进程、MCP、WASI、远程 RPC 与内置 Rust trait 的信任、取消、背压、版本和恢复语义不同，必须由具体 Requirement 证明。
7. **多语言的收益必须计入复杂度。** 只有生态 SDK、研究速度、隔离、故障域或独立扩缩容的证据超过协议、部署、冷启动和调试成本时，才增加新 Runtime。

# Proposed design

## 两个正交平面

| 平面 | 稳定责任 | 语言策略 |
|---|---|---|
| Authority/control plane | identity、Event、state transition、Capability、Budget、Cancellation、Effect、Evidence admission、Replay、Lease/MVCC、Promotion | Rust；实现可重构但责任不可委托给扩展 |
| Extension/compute plane | Provider 调用、Tool 算法、Hook handler、Agent work、Memory 检索、评测、Router/Context 候选、UI/SDK | 语言不限；只能消费版本化输入并返回 proposal、observation、artifact、metric 或受约束 execution result |

## 组件责任矩阵

| 能力 | Rust 权威控制面必须拥有 | 可多语言实现 |
|---|---|---|
| Hook | 事件顺序、Observer/Gate/Transform 准入、timeout、Budget、结果 Schema、Effect 隔离和审计 | shell/Python/TypeScript/HTTP/MCP/WASI handler；具体类型由 REQ-0008 及后续演进决定 |
| Provider | principal、Manifest pin、请求/重试 identity、Budget reserve、usage evidence admission、Cancellation 和 Receipt | OpenAI-compatible、Anthropic、本地模型 adapter；首个 reference adapter 可为 Rust |
| Coding Tool | Capability、Workspace/path scope、Effect Intent/Receipt、deadline、late-result 和 replay policy | Search、LSP、formatter、测试或领域工具实现 |
| Memory | source/provenance、tenant/user/workspace/run isolation、revision、expiry/invalidation、authoritative reference admission | embedding、retrieval、rerank、compression、summarization 和离线索引构建 |
| Sandbox | policy compile、Capability、Budget、deadline、secret/network/path decision、audit | OS primitive、container、remote sandbox 或 WASI enforcement adapter |
| Agent Loop | Run/Task lifecycle、Budget、Cancellation、Effect/Evidence completion admission | planner、context、router、retry/repair、evaluator 与不同语言 Agent Worker |
| Multi-Agent | DAG/Plan Revision、Lease、Heartbeat、dedupe、workspace ownership、merge/final validation | 本地/远程 Agent Worker，可包含 Pareto、Codex、Claude Code 或专用实现 |
| Plugin | registration admission、version pin、Capability、resource limit、output revalidation | WASI Guest、MCP/LSP server、Skill、Hook、可执行工具 |
| Evolution | Proposal lifecycle、baseline MVCC、evaluation admission、Canary、Promote/Rollback | candidate generation、统计、消融、搜索和离线历史评测 |

## 分阶段路线

| 阶段 | Rust 权威控制面 | 扩展与计算面 | 不提前接受 |
|---|---|---|---|
| G1 | Kernel、protocol admission、Event Store、Lifecycle、Manifest、Replay、Capability/Budget、Hook/Effect contract | Fake Model/Tool 与确定性测试夹具 | 真实外部 adapter、通用 Worker 或插件执行 Runtime |
| G2 | Kernel、单 Agent lifecycle、Effect/Evidence gate、Workspace/Sandbox policy、CLI command admission | Rust reference Provider/Tool/Hook；在具体 Requirement 证明后允许受控外部 adapter | 因 SDK 方便而给予 DB/Event/Capability 构造权；通用远程服务 |
| G3 | Plan/DAG、Lease/Heartbeat、Cancellation/Budget propagation、Workspace ownership、merge/final validation | 隔离的本地或远程 Agent Worker，语言和产品不限 | Worker 共享权威数据库、共享可写 Workspace 或自行判定 Task 完成 |
| G4 | Context/Evidence provenance、Router admission、版本与质量底线 | Python 可承担离线分析；有 accepted Requirement 和可复现收益时可成为受控 retrieval/evaluation Worker | Python 成为第二 Runtime authority；未版本化输入输出；无上限后台进程 |
| G5 | Behavior/Proposal/MVCC/Canary/Promote/Rollback | Python research/evaluation 与多语言 WASI Guest | Guest 自授权限、自报提升、绕过历史/隐藏集或直接切换 production pointer |

G2/G3 的“Rust 主链”因此改写为“Rust 权威控制面”。是否增加 TypeScript/Python/其他进程，不由阶段名称决定，而由对应 Requirement 的直接依赖、威胁模型、质量/成本/延迟证据和回滚方案决定。

# Interfaces, data flow, and invariants

## 抽象数据流

```text
untrusted/semi-trusted extension
  → versioned request/proposal + claimed scope
  → Rust Kernel reconstructs principal/Manifest/lifecycle/capability/budget
  → deny, or reserve + issue opaque bounded execution authority
  → extension computes/executes inside the selected isolation boundary
  → observation/result + evidence reference
  → Rust Kernel verifies authority, identity, deadline, usage, Schema and terminal winner
  → append authoritative Event / Receipt / Evidence verdict, or isolated rejection/late audit
```

任何跨语言接口都必须由对应 Requirement 冻结以下合同；本 RFC 不定义字段或 wire format：

- protocol major/minor、SchemaSet、limits、canonical identity 和 compatibility；
- authenticated principal、tenant/user presence-value、Workspace/Run/Task/Actor scope；
- Capability request 与 opaque execution authority，扩展不能构造 authority；
- Budget upper envelope、reserve/consume/release、usage evidence producer；
- cancellation/deadline、进程退出、不可中断边界、timeout recovery 与 late result；
- idempotency、retry、duplicate/out-of-order、partial success 和 effect reconciliation；
- input/output/artifact digest、provenance、sensitivity、retention 和日志脱敏；
- replay mode：Recorded 不执行，Simulated/Reexecute 必须显式且受 Effect gate；
- backpressure、output/token/CPU/memory/process/network/path 上限；
- crash/restart/upgrade/rollback、旧协议保留与 unknown version fail-closed。

## 信任等级

1. **Kernel-private Rust**：可持有事务内 authority；不得通过 public ABI 或序列化泄露。
2. **Trusted in-process strategy**：只持有版本化 strategy interface；仍不能直接写权威状态或绕过 Kernel admission。
3. **Isolated local extension**：通过受控进程、MCP 或 WASI 等边界；只有 opaque bounded authority 和最小 OS 访问。
4. **Remote extension/service**：除上述约束外，还需认证、租约、网络失败、重放攻击、数据驻留与远程 attestation/evidence 设计；没有单独 Requirement 不启用。
5. **Offline analysis**：只读导出 artifact；输出默认非权威，导入必须重新验证并记录来源。

# Impact analysis

| Surface | Direct impact | Indirect impact and required follow-up |
|---|---|---|
| ARCH-0004 / ROADMAP-0001 | 从“阶段主语言”改为“Rust 权威控制面 + 扩展计算面” | 不改变 Requirement 顺序；澄清 G2–G5 可多语言但需逐 Requirement 准入 |
| ADR-0001/0002 | 保留 Rust Kernel 与模块化单体决定 | 新 ADR 只解释其适用范围，不撤销或重写历史决定 |
| REQ-0008/0009 | Hook/Effect authority 必须先于外部 handler | 不在本 RFC 选择 shell、HTTP、MCP、WASI 或进程协议 |
| REQ-0010/0011/0014 | Provider、Tool、Agent Loop 可有多语言 adapter/worker | Spec 必须覆盖 producer authority、取消、usage、late result、prompt/tool output injection |
| REQ-0013 | Rust 负责 policy/admission，OS/容器负责 enforcement | 各平台安全声明和 fail-open/fail-closed 必须单独测试 |
| REQ-0015 | Memory provenance/admission 与 retrieval policy 分离 | 外部索引不得成为第二事实源；跨 worktree/tenant/user 注入负例 |
| REQ-0018..0022 | Scheduler/Lease/merge authority 保持 Rust | Worker transport、crash/reclaim、duplicate Effect、Workspace isolation 由各 Spec 冻结 |
| REQ-0023..0027 | Python 可用于离线分析或经批准的 Worker | 每项优化必须保留配对基线和质量/成本/延迟证据，不能因语言改变降低门禁 |
| REQ-0028..0033 | Evolution authority 保持 Rust，WASI Guest 多语言 | Candidate/Guest 自报结果不权威；Capability/resource/escape negative tests 必须存在 |
| Protocol/Schema/API/DB | 当前无变化 | 首个真实跨语言 Requirement 才能发布 wire Schema；不得暴露 SQLite layout 或 Rust ABI |
| Dependency/deployment | 当前无变化 | 新 Runtime/SDK/Wasmtime/MCP 依赖需单独记录供应链、体积、启动、升级和回滚证据 |

# Failure modes and security

- **语言被误当作 trust**：Rust adapter 也必须经过 authority admission；Python/TypeScript 也不能因本地运行获得额外权限。
- **第二权威源**：Worker cache、vector index、Hook state、MCP server 或远程 scheduler 不得直接决定 authoritative balance、terminal、Evidence verdict 或 promotion。
- **confused deputy**：Kernel 从 authenticated principal 和 persisted Manifest 重建 scope，不信任扩展自报 Workspace/Run/Task/user。
- **extension compromise**：默认最小 capability、无秘密、有限网络/path/process、deadline、预算和脱敏输出；无法证明隔离时拒绝高风险操作。
- **process crash / hung callback**：reservation、lease、deadline 和 recovery identity 由 Kernel 持久化；响应丢失、duplicate 和 late result 不得重复 Effect 或核算。
- **protocol drift**：unknown major/Schema/limits fail closed；旧 Run 按 retained exact reader 解释，不做 current substitution。
- **prompt/tool output injection**：外部文本是 observation，不是 authority 或 instruction；导入 Memory/Evidence/Plan 前经过类型、来源、scope 和策略准入。
- **sandbox mismatch**：文档分别声明 OS enforcement；不把“Rust broker 存在”描述为所有平台同等强隔离。
- **operational sprawl**：没有可复现收益和明确故障域时保持进程内 Rust reference implementation，避免无证据拆服务。

# Alternatives considered

1. **G2 至 G5 全部 Rust。** 信任面和部署简单，但会把 Provider SDK、LSP/MCP、研究/统计、UI 和插件生态成本强加给 Kernel 语言；拒绝作为长期强制，保留为默认 reference path。
2. **G2 起以 TypeScript 或 Python 为主 Runtime。** 生态和原型速度高，但会复制或外移已建立的 Event/Capability/Budget/Replay authority；拒绝。
3. **所有组件同权插件化。** 组合灵活，但 Hook、Tool、Memory、Scheduler 或 Candidate 可替换安全语义并形成第二事实源；拒绝。
4. **立即建立通用子进程 JSONL 或远程 gRPC Worker。** 可快速多语言，但在首个真实调用方前无法正确冻结认证、取消、usage、backpressure、升级和恢复；拒绝提前选型。
5. **保持 ARCH-0004 当前按阶段“主语言”措辞。** 简单，但容易把 Rust authority 误解成所有实现必须 Rust，也无法指导每个扩展的 trust level；拒绝。

# Compatibility, migration, and rollback

本次只改变设计与路线图，不修改代码、Cargo、API、Schema、SQLite、Manifest 或已发布 Run。ADR-0001/0002 继续有效：Rust 模块化单体仍是默认部署和 reference implementation。新决定只收窄“必须 Rust”的范围到权威控制面，并允许后续 Requirement 在证据成立时增加多语言扩展。

接受后同步 ARCH-0001/0004、ROADMAP-0001、BACKLOG-0001 和 docs index；不重新排序 Requirement，也不把未来能力写成 implemented fact。回滚可恢复旧路线措辞，不影响 Runtime bytes。任何已发布跨语言协议的回滚必须由其自身 ADR 执行，本 RFC 不授予删除兼容 reader 或强制迁移的权力。

# Evaluation and acceptance

本设计阶段的可复算门禁：

- 官方来源均进入 RES-0002，注明核验日期、等级、成熟度和受支持声明；综合结论区分事实与项目推论。
- 独立架构 Reviewer 固定 exact commit，核对讨论前提、责任矩阵、G2–G5 Roadmap、所有受影响 Requirement、权限/隔离/effect/replay/evidence/promotion 边界及 alternatives。
- 0 open Blocker/Major 后才接受 RFC、建立 ADR 并更新 accepted architecture/Roadmap。
- `python -m unittest discover -s scripts/tests -p "test_*.py"`
- `python scripts/check_docs.py`
- `git diff --check`
- `git status --short`

未来每个跨语言实现 Requirement 至少规划：positive contract、unknown/old version、default deny、scope isolation、self-escalation、budget envelope、cancellation/timeout、crash/restart、retry/duplicate/late、output injection、sensitive logging、Recorded replay zero execution、sandbox escape、dependency/license，以及 quality/cost/latency 对照。没有这些证据，本文不能作为启用外部 Runtime 的充分授权。
