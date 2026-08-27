---
id: REVIEW-0009
title: Rust 权威控制面与多语言扩展边界独立架构评审
status: changes-requested
owners: [independent-reviewer]
created: 2026-08-27
updated: 2026-08-27
links: [REQ-0001, SPEC-0001, RFC-0007, RFC-0001, ADR-0001, ADR-0002, ARCH-0001, ARCH-0002, ARCH-0004, ROADMAP-0001, BACKLOG-0001, RES-0001, RES-0002]
independence: independent
reviewed_revision: c4e23ee546ebf6bc4c4decbd9d1a18de99949ad1
open_blockers: 0
open_majors: 1
---

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `docs/research/evidence-register.md:33-34`; `docs/research/landscape.md:38`; `docs/rfcs/RFC-0007-rust-authority-polyglot-extensions.md:21,151` | RFC 的设计验收要求官方来源全部进入 RES-0002，并把事实与项目推论分开，但 E-017 的唯一链接 `codex-rs/README.md` 当前只有 3 行：标题与 Codex CLI 文档链接。它不能证明该行声称的 Rust 实现、独立二进制、`core` 业务逻辑或 CLI/TUI/exec crate 分解。RFC 又声称 Python SDK 打包原生 CLI，却没有对应证据行；E-018 的“不是第二套 Agent 内核”也把由“SDK 启动 CLI”推导出的解释写成来源直接支持的事实。该复合证据随后被 RES-0001 与 RFC-0007 用作“原生核心 + 多语言边界”的已核验事实，因此 REQ-0001 的直接来源门禁和 RFC-0007 自身的 evidence acceptance 尚未满足。架构推论可能合理，但当前证据账本不能复算其所声明的支持范围。 | 将复合声明拆成原子事实；为 Rust/二进制、`codex-core` 职责、workspace/crate 结构和 Python SDK 分别引用能直接支持该项的当前官方文档或源码，并记录核验日期、等级、成熟度。把“包装层而非第二内核”及跨产品架构结论显式标为 inference，或只保留来源可直接证明的事实。同步修正 RES-0001/RFC-0007 中受影响的复合措辞后，对新的 exact commit 做独立 focused re-review。 | open |

# Verdict

`changes-requested` for exact proposed-design commit
`c4e23ee546ebf6bc4c4decbd9d1a18de99949ad1`（parent
`2441826337e7bf76aa946ea40ac5b40ae6aadd4b`）。本评审由未参与设计修订的 fresh independent
Reviewer 执行；不依赖其他 Reviewer 的结论。最终 0 open Blocker、1 open Major。

设计的权威边界本身未发现 Blocker/Major：Rust 用于缩小 authority surface 而非追求 Rust LOC；
reference implementation 不构成语言强制；非 Rust 组件只能 propose、observe、compute 或执行已授权的
bounded operation；Sandbox policy/admission 与 OS/container/WASI enforcement 分离；Memory
provenance/admission 与 retrieval/rerank/compression 分离；transport 留给具体 Requirement；新增 Runtime
需要以可复现收益覆盖协议、部署、冷启动和调试复杂度。F-001 阻塞的是该设计自定的证据准入与可复算性，
不是对上述推论作反向结论。

按照独立评审门禁，本 Reviewer 未修改 RFC、ARCH、Roadmap、Backlog 或 Research；整改与接受决定属于后续
实施者/维护者任务。REVIEW-0001..0008 也未在存在 open Major 时前移 freshness。

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
| 官方 Codex/Claude 证据准确且范围充分 | failed | Claude 条目谨慎区分原生交付与未知内部语言；Codex E-017、Python SDK 与 E-018 inference 的 source-to-claim 对应不足，形成 F-001。 |

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

本提交是 proposed RFC、研究证据与导航的文档变更，没有 Runtime、Cargo、Schema、DB、API、依赖或测试实现。
设计评审所需回归是 governance、document、whitespace 与 exact diff 检查。Reviewer 在本 Review 落盘后执行：

- `python -m unittest discover -s scripts/tests -p "test_*.py"`：21 passed。
- `git diff --check`：passed。
- `git status --short`：只有未跟踪的 reviewer-owned `REVIEW-0009`。
- `python scripts/check_docs.py`：failed only because REVIEW-0001..0007 仍固定 `8bb885b`，并正确报告本候选的
  `docs/index.md`、RES-0001、RES-0002 与 RFC-0007 为 substantive stale paths；没有报告 REVIEW-0009
  metadata、finding count、link 或格式错误。

docs gate 应保持失败，直到 F-001 整改经新的 exact-commit re-review，且既有 approved REVIEW-0001..0008 对
substantive RFC/research 增量完成 freshness disposition。该失败不能通过本轮前移旧 Review 或把不充分来源改写为
已批准来规避。

# Scope and unrelated changes

精确 diff `2441826337e7bf76aa946ea40ac5b40ae6aadd4b..c4e23ee546ebf6bc4c4decbd9d1a18de99949ad1`
只修改 `docs/index.md`、RES-0001、RES-0002 并新增 RFC-0007，共 178 insertions、4 deletions。没有
Runtime、Schema、数据库、Cargo、CI、治理规则或依赖变化；未发现无关产品实现。

# Re-review conditions

F-001 必须由实现者/维护者在新提交中整改。Focused re-review 应逐项打开每个新增官方来源，核对来源实际内容
与原子声明、事实/inference 分类、核验日期和传播到 RES-0001/RFC-0007 的措辞；然后重新检查七项讨论前提、
G2–G5 非授权边界、REQ-0008..0033 影响、permissions/isolation/effect/budget/cancel/late/replay/evidence/
promotion、alternatives、failure/rollback 与 quality/cost/latency。只有 0 open Blocker/Major 后才能批准 RFC、
建立 ADR 或同步 accepted architecture/Roadmap。

# Re-review history

- 2026-08-27：fresh independent architecture/design review of exact
  `c4e23ee546ebf6bc4c4decbd9d1a18de99949ad1` against parent
  `2441826337e7bf76aa946ea40ac5b40ae6aadd4b`；0 Blocker、1 Major，`changes-requested`。
