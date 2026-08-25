---
id: REQ-0006
title: Projection、Snapshot 与 Replay
status: done
owners: [maintainers]
created: 2026-08-24
updated: 2026-08-25
links: [EPIC-0002, REQ-0003, REQ-0004, REQ-0005, SPEC-0005, RFC-0005, ADR-0003, ADR-0004, ADR-0005, ADR-0006, REVIEW-0005]
risk: high
work: .agents/work/archived/REQ-0006-projection-snapshot-replay
---

# Context and user

可信内核、运行检查、后续 Capability、Workspace、Memory 与 Context DAG 需要一个可验证的当前 Run/Task 视图和诚实的本地重放路径。REQ-0003 已交付版本化协议与 exact SchemaSet admission，REQ-0004 已交付 Kernel 私有 SQLite append-only Event Store，REQ-0005 已交付完整 Manifest 首事件、Run/Task lifecycle pure fold 与同事务状态迁移。本需求只在这些已完成前置条件之上建立派生视图。

# Problem

当前状态只能由 lifecycle 命令路径内部每次完整折叠事件获得，没有版本化 Projection 输出、可丢弃 Snapshot、Snapshot 后增量恢复或独立 Recorded replay/摘要比较入口。若消费者直接读表、自建 reducer、信任未绑定游标的缓存，或把 replay 当成重新执行，就可能绕过协议验证、重释历史、跨隔离域读取或重复真实副作用。

# Desired outcome

提供单进程、SQLite、本地确定性的最小纵向切片：从单个 Run 的权威 lifecycle 事件构建版本化 Run/Task Projection；创建与事件游标、Schema、Reducer 和完整性摘要绑定的 Snapshot；从可信 Snapshot 应用后续事件；Snapshot 不可信时安全回退完整历史；Recorded replay 从完整历史重建 Projection 并比较确定性摘要，全程不执行真实外部副作用。

# Acceptance criteria

- AC-01：Projection 的权威输入只能是由 Kernel 私有 reader 从 Event Store 读取、按持久化 exact SchemaSet/Protocol Limits 重新验证的连续 lifecycle Event；Snapshot-assisted load 也必须重新读取、exact validate 并重算 `[1..snapshot cursor]` 的 history chain，Snapshot 不能自证此前缀。调用者、payload、Projection 或 Snapshot 不能自报已验证事件或替代 reader。
- AC-02：版本化 reducer 是无时钟、随机数、环境、网络、文件或可变全局状态的纯函数；其不可变 descriptor 固定 accepted event bindings、Manifest admission、状态/parent guards、排序、output SchemaSet/limits 和 digest 合同。Kernel 只能按 persisted source contract exact resolve 并保留已引用 reducer；对同一有序输入产生逐字节相同的 canonical Projection 与摘要，missing/wrong/current substitution 及其他错误以稳定类别 fail closed。
- AC-03：从 sequence 1 的完整权威历史可重建完整 Manifest、Run state、按 Task ID 确定排序的 Task/parent/state 和末端事件游标；gap、reuse、非法序号、错误 Schema/identity、未知 event type/version 或非法历史均 fail closed，不跳过、不默认解释。
- AC-04：Snapshot 只能由 Kernel 在已验证当前 Projection 上按显式请求创建；首版不设自动阈值。版本化 Snapshot 格式固定完整隔离 scope、Event Store identity、lifecycle stream、末端 sequence/event ID、source SchemaSet/limits、reducer、exact output/snapshot SchemaSet 与 Protocol Limits、可增量 history-chain state、Projection digest 与 Snapshot 完整性摘要；所有 digest 使用闭合 versioned hash-view 和 checked-in golden vectors。
- AC-05：Snapshot 创建与并发 append 在 SQLite 事务上串行化并原子提交；崩溃只产生“无新 Snapshot”或“完整新 Snapshot”。Snapshot 不能修改事件、Manifest 或 lifecycle state，也不能成为第二权威事实。
- AC-06：Projection 读取固定一个一致的事件 horizon；使用 Snapshot 前必须从 Event Store 重新读取、exact validate `[1..snapshot cursor]` 并得到与 Snapshot 相同的 versioned history-chain state，随后 Snapshot 才可作为 reducer seed，并只应用 `(snapshot cursor,horizon]` 的已验证连续事件。candidate 缺失、失效、损坏、摘要错误、游标不匹配或 output Schema/Reducer 不兼容时安全回退完整历史并报告 disposition；prefix/Event Store 损坏必须 fail closed 而非降级为 cache miss。
- AC-07：Recorded replay 是只读的完整历史 Projection 重建，不使用 Snapshot，不追加事件、不调用 Effect/Provider/Tool，也不覆盖 source Run。Simulated replay 只允许固定 fixture revision 且永不获得真实 Effect 执行入口；最小切片在 fixture resolver 尚未实现时必须在任何 Effect dispatch 前稳定拒绝。
- AC-08：只有 Kernel 结果 provenance 中的 store identity、完整 tenant/user/workspace/run/owner actor/stream、source SchemaSet/limits、cursor、history-chain state、reducer identity 及 exact output SchemaSet/limits 都相同时结果才可比较；这些 provenance 字段进入 Projection digest。同一 Run/同一 store 的正常完整 Projection 与 Recorded replay 必须得到相同 canonical digest；跨库 clone 或任一字段不同时返回安全 `not_comparable/unauthorized` 而不泄漏或修改权威状态。
- AC-09：Run、tenant、user presence/value、Workspace、owner Agent/actor、lifecycle stream 与 Event Store identity 全部 exact 绑定；跨 Run/Workspace/Actor、换库、复用 Snapshot/cursor/digest 或 payload shadowing 均不能读取、恢复或比较目标 Projection。首版沿用 REQ-0005 owner-only authority，不宣称 REQ-0007 通用权限。
- AC-10：并发 Projection 读取获得明确 point-in-time 结果；Snapshot 创建与同 Run append 不产生游标/内容撕裂。关闭/重开数据库后可读取可信 Snapshot 并增量恢复；事件或 Snapshot 事务中断后恢复仍以 Event Store 为事实源。Snapshot-assisted 路径只节省 prefix reducer fold，不得声称省去 prefix Event 读取或验证。
- AC-11：SQLite v1→v2 migration 原子增加 Snapshot 存储，并为 `events` 增加不改写历史 Event JSON/sequence/SchemaSet/Manifest 的 writer-epoch 列与 v2 insert trigger；v2 writer 显式写 epoch，迁移前已打开的 v1 writer 因默认旧 epoch 而 fail closed。失败 migration 回滚，未知更新版本拒绝；代码回滚保留 v2 reader/trigger、旧 Schema/reducer 和全部历史，Snapshot 可忽略重建。
- AC-12：新增 Projection/Snapshot 协议 Schema 和新内容地址 SchemaSet 时保留全部已发布 set；source Event 与 output/snapshot 记录分别固定 exact retained SchemaSet/limits。Kernel 保留所有被历史 Run/Snapshot 引用的 reducer descriptor/implementation；Snapshot/Reducer 不兼容只使 candidate 失效，missing source reader/reducer 或 current substitution 不得静默迁移/重释历史。
- AC-13：REQ-0003 protocol/golden/compatibility/isolation/replay-manifest、REQ-0004 Event Store migration/append/read/idempotency/concurrency/recovery 与 REQ-0005 lifecycle 状态机/Manifest/fold/隔离测试全部回归通过；依赖方向保持 protocol 不依赖 Kernel/SQLite。
- AC-14：交付范围仅为单进程 SQLite、本地 Run/Task Projection、Snapshot、Snapshot 后增量恢复、Recorded replay 和摘要比较；不得提前实现完整 Capability/Budget、Hook、外部 Effect 重执行、Provider/Agent Loop、Memory、Task/Context DAG、分布式 Projection 或远程 Snapshot Store。

# Quality, cost, and latency guardrails

- 质量：完整历史 fold 是正确性 oracle；Snapshot-assisted 结果必须与同一 horizon 的完整 fold 摘要一致。任何无法证明的 Snapshot 都回退而不是进入权威状态。
- Token/费用：不调用真实模型、Provider 或外部 Tool；该维度不宣称优化。Snapshot 的本地存储字节和测试运行成本分别记录。
- 延迟：记录无 Snapshot 完整 fold、Snapshot 创建、Snapshot-assisted 增量恢复和 Recorded replay 在不同事件数下的本机观察；首版不虚构收益阈值，SQLite busy 等待沿用有限上界。

# Non-goals

- 不实现 Capability、Budget、通用 delegation、外部取消/超时传播或 Effect Intent/Receipt；这些属于 REQ-0007/0009。
- 不重新执行 Provider、Tool、网络、文件或进程副作用；`reexecute` 仍是后续独立能力。
- 不实现 fixture repository、完整 simulated executor、Provider/Agent Loop、Memory、Task DAG、Context DAG、CLI 或远程服务。
- 不把 Projection/Snapshot 变为权威状态，不建立可被插件写入的通用 Projection API，不引入分布式一致性或远程 Snapshot Store。
- 不改变 REQ-0005 lifecycle 状态集合、合法边、Manifest 首事件、owner authority 或单事件 append 语义。

# Risks and open questions

Snapshot prefix 信任根、四类 digest preimage、数据库 writer epoch、reducer resolver/retention、output Schema admission、Recorded/Simulated replay 语义及后续 Projection 扩展点都是跨需求且难以回退的高风险合同。RFC-0005 与 ADR-0006 冻结首片选择；实现若需要省略 prefix exact validation、改变 digest/epoch/resolver、绕过 exact reader、改变 lifecycle 语义、引入真实 Effect dispatch、允许外部 Snapshot import 或弱化隔离，必须退回 Spec/RFC 门禁。
