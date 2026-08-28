# REQ-0008 Handoff

当前状态：`planned`，无Runtime实现。REQ-0004、REQ-0007前置均done；`.agents/work/active`没有其他Runtime Requirement。

设计证据：initial design `8507bae4`经fresh independent REVIEW-0010提出4个Major；整改exact `3aee02adf8815466b02f51de247ae19922efc126`固定cross-kind phase/input lineage、Gate-bearing empty-required unconditional deny、reserve/terminal atomic pair与Transform fixed reject-whole，F-001至F-004由原Reviewer全部关闭，最终approved、0 Blocker/0 Major。ADR-0009及Requirement/Spec/RFC正式接受于`3318cbc6fe8bc8c9717a5a2b4aea1153f0d281d6`，同一Reviewer已完成accepted-doc freshness复核。实现者不得重新解释这些closure。

下一动作从TASK-REQ-0008-01开始：先实现Protocol/Schema/Manifest identity和retained compatibility，再建立Hook stream/pure fold；不得先写便捷callback或Fake service绕过持久合同。关键实现边界：Event/顺序/Manifest/authority/budget/cancel/deadline/terminal/Effect/Evidence/replay/final append仍由Rust Kernel拥有；handler只得到opaque bounded lease与非权威view/result接口。

atomic pair是首要风险：当前REQ-0007 reserve/settlement helper会自行commit，必须重构为crate-private transaction-local admission。reserve和terminal各自固定pair ID/fingerprint、双stream cursor/sequence、两个Event bytes/交叉引用；zero原子写两者、two只exact retry、mutation冲突、one视为损坏，任何validation/insert/commit失败rollback。Hook operation不得走通用single-stream terminal后再补写Hook事实。

测试必须使用Fake Hook和FakeClock，不依赖真实sleep；所有命名Cargo filter先由`assert_cargo_test_filter.py`证明非零。特别保留第二append fault、commit-response-loss、validly-resealed one-sided history、two-writer reverse winner、Observer business-decision字节不变、Transform chain中间失败、跨tenant/user/workspace/run/task/actor隔离、Recorded handler/Event/budget零变化证据。

严格排除：真实shell/Python/TypeScript/HTTP/MCP/WASI Hook Runtime、外部Worker/RPC/队列、Provider、Coding Tool、外部Effect、Sandbox、Agent Loop、Memory、Task DAG、Evidence Graph、WASM/WASI执行、Hook Snapshot、background recovery scanner和并行Hook。若实现需要DB v3、public SQL/authority、第二状态表、alternate Event actor、current reader substitution或新依赖，返回SPEC/RFC并重新设计评审。

实施完成后运行PLAN全部门禁，并使用一个新的fresh independent Agent/session执行Code Review；本次REVIEW-0010只是设计Review，不能替代代码Review。Blocker/Major未由原代码Reviewer复审关闭前，不得verified/done，也不得开始REQ-0009。
