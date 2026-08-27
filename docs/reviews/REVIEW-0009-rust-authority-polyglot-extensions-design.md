---
id: REVIEW-0009
title: Rust 权威控制面与多语言扩展边界独立架构评审
status: approved
owners: [independent-reviewer]
created: 2026-08-27
updated: 2026-08-27
links: [REQ-0001, SPEC-0001, RFC-0007, RFC-0001, ADR-0001, ADR-0002, ARCH-0001, ARCH-0002, ARCH-0004, ROADMAP-0001, BACKLOG-0001, RES-0001, RES-0002]
independence: independent
reviewed_revision: b42ccdc3216f518ff60303cec20da92b78d190a1
open_blockers: 0
open_majors: 0
---

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `docs/research/evidence-register.md:33-43`; `docs/research/landscape.md:38-40`; `docs/rfcs/RFC-0007-rust-authority-polyglot-extensions.md:21` | 初审发现 Codex E-017 的三行 README 无法支持复合声明，Python SDK 缺证据，且事实与 inference 混写。`1748f69` 将 E-017..E-022 拆成 Cargo workspace、core README、release binary、TypeScript SDK、Python packaging 与 sandbox 六个原子官方来源，并把 SDK/authority、binary/internal trust 等不支持范围显式排除；E-023..E-027 的 Claude delivery、Hook、Plugin、Memory 与 Sandbox 声明也逐项保持在官方页面范围内。RES-0001/RFC-0007 现以“已核验事实”和“项目推论”分段，明确竞品事实不证明本RFC authority模型、具体transport安全或Q/C/L收益。 | Fresh Reviewer 逐项打开 E-017..E-027：Cargo workspace确列`core/cli/exec/tui`；core README明确business logic/Rust UI及SandboxPolicy/平台enforcement；repo release明确平台binary；TS README明确spawn CLI+JSONL；Python pyproject固定`openai-codex-cli-bin`依赖；Claude官方页分别支持native binary、五类Hook、Plugin组件/`bin/`、Markdown per-repo Memory、permission-before-tool与OS/proxy sandbox。完整`6892a8f..1748f69`只修复证据与fact/inference措辞，七项设计前提无变化。 | closed |

# Verdict

`approved` for exact proposed-design remediation
`1748f69d01044a936727b3b5b7659882981b9129`（parent
`6892a8faae25db9b1f1471f12847bf76c5359b82`）。本评审由未参与设计修订的 fresh independent
Reviewer 执行；不依赖实现者或其他 Reviewer 的结论。F-001 required proof 已由原子官方来源、明确的
fact/inference 分界及受影响综合措辞关闭；最终 0 open Blocker、0 open Major。

设计的权威边界本身未发现 Blocker/Major：Rust 用于缩小 authority surface 而非追求 Rust LOC；
reference implementation 不构成语言强制；非 Rust 组件只能 propose、observe、compute 或执行已授权的
bounded operation；Sandbox policy/admission 与 OS/container/WASI enforcement 分离；Memory
provenance/admission 与 retrieval/rerank/compression 分离；transport 留给具体 Requirement；新增 Runtime
需要以可复现收益覆盖协议、部署、冷启动和调试复杂度。

Focused accepted-doc review 进一步确认 exact `b42ccdc3216f518ff60303cec20da92b78d190a1`（baseline reviewer
commit `2c80b89e6d0a5f5f1f6bca3dc1677bd14a8ca2e5`）只执行已经批准的接受步骤：RFC-0007 改为 accepted、
建立 ADR-0008，并同步 ARCH-0001/0004、ROADMAP-0001、BACKLOG-0001、RES-0001/0002 和导航。七项前提无回退，
Requirement 顺序与 prerequisite 保持，且没有选择 transport 或授权任何具体外部 Runtime。

# Acceptance trace

| Review concern | Result | Independent evidence |
|---|---|---|
| Rust 缩小权威面而非最大化 LOC | passed | RFC 将 Event、identity、state、Capability、Budget、Cancellation、Effect/Evidence admission、Replay、Lease/MVCC 和 Promotion 固定在 Rust control plane；策略/adapter 语言不构成 authority。 |
| Reference implementation 不是语言强制 | passed | G2 保留 Rust reference Provider/Tool/Hook，但未来外部 adapter 必须由具体 Requirement 证明，不能获得 Rust ABI、DB 或 authority constructor。 |
| 非 Rust 不形成第二 authority | passed | 抽象数据流要求 Kernel 从 principal、Manifest、lifecycle、Capability 与 Budget 重建准入；扩展只返回 proposal、observation、artifact、metric 或 bounded result。 |
| Sandbox policy 与 enforcement 分离 | passed | Rust 拥有 policy compile/admission/audit；OS、container、remote sandbox 或 WASI 执行隔离，平台差异和无法证明隔离时的拒绝均明确。 |
| Memory 事实与策略分离 | passed | provenance、scope、revision、expiry/invalidation 和 authoritative reference admission 属于可信面；embedding/retrieval/rerank/compression/summarization 属于可演化计算面。 |
| Transport 未被提前冻结 | passed | RFC 明确不接受 JSONL/JSON-RPC/MCP/queue/gRPC/FFI/container/remote service，首个真实调用 Requirement 才能冻结 wire contract；MCP/WASI 仅作为待选择边界示例。 |
| 多语言收益覆盖复杂度 | passed at design level | RFC 要求对应 Requirement 提供直接依赖、威胁模型、quality/cost/latency 证据和 rollback；没有证据时保持进程内 Rust reference path。本提交未声称已测得收益。 |
| 官方 Codex/Claude 证据准确且范围充分 | passed | E-017..E-027 均由当前官方源码/文档支持其原子事实；Python SDK packaging已有固定依赖证据；不受支持的trust/security结论被排除或标为项目推论。 |

# Compatibility, permission, and isolation review

- `ADR-0001` 的可信内核职责与 `ADR-0002` 的 Rust 模块化单体起点保持有效；RFC-0007 只拟收窄“必须
  Rust”的范围，并没有修改已发布 Runtime、Schema、SQLite 或 Manifest。若接受，仍需新 ADR 记录该新解释。
- RFC 没有把 G2–G5 阶段表变成外部 Runtime 的充分授权。G2 外部 adapter、G3 local/remote Worker、G4
  retrieval/evaluation Worker、G5 WASI Guest 均受对应 accepted Requirement、威胁模型、版本/权限/失败合同和
  独立评审约束；remote extension 还明确要求单独 Requirement。因此没有发现提前授权 G2–G5 的 Major。
- Hook 的顺序/Gate/Transform admission，Provider 的 principal/request identity/usage admission，Tool 的
  Workspace/path/Effect contract，Sandbox 的 secret/network/path decision，Agent/Multi-Agent 的 lifecycle、
  Lease、dedupe、workspace ownership 与 final validation，Plugin 的 registration/capability/output revalidation，
  Evolution 的 Proposal/MVCC/Canary/Promote/Rollback 均留在 Rust authority plane。
- REQ-0007 的默认拒绝、trusted resource envelope、opaque bounded authority、producer/evidence admission、
  cancellation/deadline、late result、exact scope、Recorded replay zero execution 与 no second authority 合同没有
  被重释。未来 REQ-0008..0033 必须消费这些 request/result 语义，不能取得 Event transaction、Capability/lease
  constructor、Budget mutable handle、replay dispatcher 或 promotion authority。
- Memory index、Worker cache、Hook state、MCP/LSP server 和 remote scheduler 均被明确禁止成为事实源；导入
  Memory/Evidence/Plan 前要求类型、provenance、scope 与 policy admission，覆盖 prompt/tool-output injection 和
  confused-deputy 风险。

# Regression and test review

候选、整改与最终接受只有 RFC/ADR、架构/路线/研究/导航和 Review 文档，没有 Runtime、Cargo、Schema、DB、
API、依赖或测试实现。Focused re-review 完成 E-017..E-027 source-to-claim 检查；accepted-doc re-review 又核对
七项前提、Requirement 顺序、transport neutrality 及 REQ-0007..0033 authority/security 传播。更新
REVIEW-0001..0009 后，Reviewer 独立执行：

- `python -m unittest discover -s scripts/tests -p "test_*.py"`：21 passed。
- `python scripts/check_docs.py`：passed，176 Markdown files / 55 formal IDs。
- `git diff --check`：passed。
- `git status --short`：只有九份 reviewer-owned `docs/reviews/REVIEW-0001..0009` 修改。

# Scope and unrelated changes

初始精确 diff `2441826337e7bf76aa946ea40ac5b40ae6aadd4b..c4e23ee546ebf6bc4c4decbd9d1a18de99949ad1`
只修改 `docs/index.md`、RES-0001、RES-0002 并新增 RFC-0007。Focused remediation
`6892a8faae25db9b1f1471f12847bf76c5359b82..1748f69d01044a936727b3b5b7659882981b9129`
只修改 RES-0001、RES-0002 与 RFC-0007 的证据/措辞，共14 insertions、11 deletions。没有 Runtime、Schema、
数据库、Cargo、CI、治理规则或依赖变化；未发现无关产品实现。

Accepted-doc diff `2c80b89e6d0a5f5f1f6bca3dc1677bd14a8ca2e5..b42ccdc3216f518ff60303cec20da92b78d190a1`
新增 ADR-0008 并修改 RFC-0007、ARCH-0001/0004、ROADMAP-0001、BACKLOG-0001、RES-0001/0002 与导航，
共83 insertions、28 deletions。Requirement 表32行及 prerequisite 未重排；没有 Runtime、Schema、数据库、
Cargo、CI、治理规则或依赖变化。

# Re-review conditions

设计与 accepted-doc 传播评审门禁均满足。未来若来源变化、事实/推论重新混写、提前冻结transport或授权外部Runtime，必须在对应 exact commit
重新评审 permissions/isolation/effect/budget/cancel/late/replay/evidence/promotion、rollback 与quality/cost/latency。

# Re-review history

- 2026-08-27：fresh independent architecture/design review of exact
  `c4e23ee546ebf6bc4c4decbd9d1a18de99949ad1` against parent
  `2441826337e7bf76aa946ea40ac5b40ae6aadd4b`；0 Blocker、1 Major，`changes-requested`。
- 2026-08-27：focused independent re-review of exact
  `1748f69d01044a936727b3b5b7659882981b9129` against reviewer commit
  `6892a8faae25db9b1f1471f12847bf76c5359b82`。逐项打开E-017..E-027当前官方来源，确认Codex workspace/core/
  binary/TS/Python/sandbox和Claude delivery/Hook/Plugin/Memory/Sandbox原子声明、限制与fact/inference传播；七项前提、
  G2–G5非授权边界及REQ-0008..0033 authority合同无回退。F-001 closed；0 Blocker、0 Major，`approved`。
- 2026-08-27：focused independent accepted-doc review of exact
  `b42ccdc3216f518ff60303cec20da92b78d190a1` against approved reviewer baseline
  `2c80b89e6d0a5f5f1f6bca3dc1677bd14a8ca2e5`。逐项核对 RFC accepted 状态、ADR-0008、ARCH-0001/0004、
  ROADMAP/BACKLOG、RES/index：七项前提忠实传播；G2–G5 仅表达可由后续 Requirement 批准的计算面，不构成外部
  Runtime 授权；Requirement 顺序/prerequisite 不变；未预选 JSONL/MCP/WASI/RPC；REQ-0007 及 REQ-0008..0033
  的 authority、isolation、Effect、Budget、Cancellation、late、Replay、Evidence 与 Promotion 边界无削弱。
  0 Blocker、0 Major，`approved`。
