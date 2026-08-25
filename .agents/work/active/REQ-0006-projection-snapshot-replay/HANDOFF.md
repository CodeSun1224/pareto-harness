# REQ-0006 Handoff

当前阶段：reviewing / independent architecture approved / Runtime vertical slice implemented / non-review gates passed。REQ-0003/0004/0005均done；启动工作区clean，active work无其他Runtime Requirement。首轮独立架构评审固定四份exact hashes并给出changes requested（0 Blocker/6 Major）；author修订prefix trust、digest preimage、reducer resolver/retention、output reader、cross-store comparability和writer epoch后，由同一reviewer focused re-review逐项关闭，并对最终status bytes完成freshness-only确认，最终0 open Blocker/Major。RFC-0005/ADR-0006 accepted、SPEC-0005 approved。

实施进展：`pareto-protocol`已增加10个闭合Projection/Snapshot相关Schema并发布新内容地址SchemaSet `sha256-4ce3872926ce61209fdc5ed48deceeec9703ccfe94ea83be485eb8ef7512ff97`；`pareto-kernel`已实现exact reducer registry、full Projection、rolling history、SQLite v2 writer epoch和immutable Snapshot、prefix-proved assisted recovery、Recorded/Simulated replay gate与full-provenance compare。Projection 27 tests、Event Store/lifecycle/projection 60 tests、Protocol 9 unit + 21 contract、workspace clippy/fmt/test、schema generation、governance与hygiene均通过；`check_docs.py`按预期等待fresh reviewer刷新受影响既有Review，独立代码评审尚未开始。

修订后的最小切片：单Run exact lifecycle events → source-contract exact resolver → retained versioned pure reducer/output reader → store/full-provenance Projection → explicit current-horizon Snapshot → candidate validation → prefix Event exact reread/validation/rolling-chain proof（只跳过prefix reducer fold）→ suffix validation/hash/reduce → candidate-only fallback → Snapshot-free Recorded replay → full-provenance comparison。Projection/Snapshot均为派生值；Event Store和sequence-1 Manifest仍是唯一authority。

关键边界：复用persisted sequence-1 admission和owner-only authority；prefix/suffix每个Event都exact validate并进入闭合rolling chain；unknown/illegal history fail closed；Snapshot绑定store/full scope/stream/cursor/source/reducer/exact output set+limits及所有digests；create用`BEGIN IMMEDIATE`，load用single read horizon；candidate错误fallback，prefix/Event/DB错误不fallback。SQLite v2用writer_epoch列+INSERT trigger拒绝already-open v1 writer，Snapshot UPDATE/DELETE均拒绝。Recorded replay无Effect/append；Simulated在fixture resolver前于Effect dispatch之前拒绝；不实现reexecute。

实施停止条件：需要caller-provided Snapshot/current reader/reducer、public SQL/transaction、mutable authoritative Projection、Event rewrite/DB downgrade、live Effect/Provider/Tool调用、auto background Snapshot、remote/distributed store、generic Projection framework、broader actor authority或不同lifecycle状态语义。发生任一项先更新impact/SPEC/RFC，不得靠代码局部决定。

后续接口：REQ-0007在authority前加Capability/Budget/Cancel且新增事件时显式演进reducer；REQ-0012只消费Manifest中的Workspace identity；REQ-0015只消费带cursor/digest provenance的派生Projection；REQ-0024使用独立Context reducer/schema，不复用RunTask payload为generic JSON。

实施完成后fresh reviewer必须重点检查Replay是否可达真实Effect、reducer determinism、Snapshot protocol bypass、cursor/version/digest completeness、concurrency/crash consistency、isolation、Schema/API/DB compatibility、REQ-0003/4/5 regression、unrelated/dependency changes、rollback及downstream evolvability。Blocker/Major未由独立reviewer关闭并re-review前，REQ-0006不得verified/done。
