---
id: REVIEW-0008
title: 技术选型分阶段语言边界独立架构评审
status: approved
owners: [independent-reviewer]
created: 2026-08-27
updated: 2026-08-27
links: [REQ-0001, SPEC-0001, RFC-0001, ADR-0001, ADR-0002, ARCH-0002, ARCH-0004, ROADMAP-0001]
independence: independent
reviewed_revision: b42ccdc3216f518ff60303cec20da92b78d190a1
open_blockers: 0
open_majors: 0
---

# Verdict

`approved` for exact commit `8bb885bda678f5f785706e9eb335f472b5244974`（parent
`f49ef286796eb2ccd509d610bae40dd658cc306a`）。本评审由未参与该文档修订的 fresh independent
Reviewer 执行。变更仅澄清 `ARCH-0004` 的分阶段语言与进程边界；没有修改 Runtime、协议、Schema、
数据库、依赖、测试或治理规则。0 open Blocker，0 open Major。

本 lightweight 澄清没有专属产品 Spec；`SPEC-0001` 链接用于约束独立 Review 与 freshness 门禁，
不把它重释为语言或传输技术合同。产品依据仍为 REQ-0001、RFC-0001、ADR-0001/0002 与 ROADMAP-0001。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Note | `docs/architecture/technology-selection.md:25-66` | G4 的离线 Python、G5 的 WASI 与 v0.2 的 Python Worker 分层明确；具体 Worker 传输、生命周期、权限与部署协议未被本文提前接受。 | 后续正式 Worker 必须另建 Requirement/RFC/ADR 和独立评审；G4 工具保持 artifact-only、非权威、不可回调 Runtime。 | accepted |

# Acceptance trace

| Review concern | Result | Independent evidence |
|---|---|---|
| G1–G5 与 Roadmap 一致 | passed | G1–G3 保持 Rust 主链；G4 对应 Context/Evidence/Router 与消融，仅允许离线分析；G5 对应 REQ-0028..REQ-0033，并把在线不可信策略限定到 WASI。 |
| v0.2 Python Worker 未被提前实现或接受 | passed | Worker、TypeScript SDK 与 Web 控制台不进入 G1–G5 关键路径；Worker 触发条件及 Requirement/RFC/ADR 门禁显式保留。 |
| 不形成未经接受的传输决策 | passed | G4 明确不冻结常驻进程、双向 Worker、队列、JSONL 或 gRPC；v0.2 仅列出待评审候选，没有选择协议。现有 Event API 的 JSONL export 未被重释为 Worker transport。 |
| 可信内核边界准确 | passed | Event Store、版本、状态、Capability、Budget、Cancellation、Replay、Effect、MVCC 与 Promote/Rollback 均保留在 Rust 可信内核；非 Rust 扩展只能提交请求或候选。 |
| 版本、证据和重放边界准确 | passed | 离线输入要求 Schema、摘要和 provenance；输出为非权威 artifact/metric/candidate，不能直接提交权威状态或覆盖 revision。 |
| 不实 implemented claim | passed | 内容按阶段描述技术基线与未来边界，没有声称 G4/G5、Python Worker 或 WASI 已交付；当前实现事实未被修改。 |

# Compatibility, permission, and isolation review

- 变更与 `ADR-0001` 的稳定可信内核、`ADR-0002` 的 Rust 模块化单体及“核心语义稳定后增加
  Python Worker/WASM”的决定一致。
- 默认拒绝没有因语言改变而放宽：WASI Guest 默认无文件、网络、进程和秘密访问；未来 Worker
  不获得 Event Store、Manifest、Capability、Budget、终态、Effect 或 Promotion 写权限。
- G4 离线工具不得直接访问权威数据库，也不能回调 Runtime；其分析结果必须经内核验证和授权后才能
  影响后续权威行为。这避免形成 confused-deputy 或自我授予路径。
- 本次没有公共 Schema、数据库布局、事件版本、传输机制或依赖变化，因此无需迁移或回滚实现；文档
  回滚可恢复 parent 的较粗粒度表述。

# Regression and test review

本次是 lightweight、doc-only architecture clarification。Reviewer 独立运行：

- `python scripts/check_docs.py`
- `git diff --check`
- `git status --short`

Review/freshness 文档落盘后再次执行，结果记录在本次 reviewer commit 的交接报告中。Runtime 测试不因
该单文档澄清而成为必要证明；变更未触及产品代码或依赖。

# Scope and unrelated changes

`f49ef28..8bb885b` 只修改 `docs/architecture/technology-selection.md`，新增 34 行、删除 3 行。
没有无关产品、工作证据、Schema、Cargo、CI 或治理修改。Reviewer-owned 增量只包含本 Review 与
REVIEW-0001..0007 的 freshness 记录。

# Re-review history

- 2026-08-27：fresh independent architecture review exact
  `8bb885bda678f5f785706e9eb335f472b5244974`；0 Blocker、0 Major，`approved`。
- 2026-08-27：focused substantive freshness re-review exact
  `1748f69d01044a936727b3b5b7659882981b9129`。新增proposed RFC-0007没有修改accepted ARCH-0004；它与本Review
  批准的G4 artifact-only、G5 WASI、v0.2 Worker门禁一致，并进一步要求每个真实跨语言边界由具体Requirement
  冻结transport、authority、failure、rollback和quality/cost/latency证据。E-017..E-027经官方来源原子化复核，
  fact/inference边界明确。REVIEW-0008保持approved、0 open Blocker/Major。
- 2026-08-27：focused accepted-doc re-review exact `b42ccdc3216f518ff60303cec20da92b78d190a1` against `2c80b89`。RFC acceptance、ADR-0008及ARCH-0004正式采用“Rust authority + extension compute”边界；Requirement顺序/prerequisite和默认Rust reference path不变。G2–G5均受具体accepted Requirement、可复现收益和transport neutrality总门禁约束，未扩大成已启用Runtime。REVIEW-0008保持approved、0 open Blocker/Major。
