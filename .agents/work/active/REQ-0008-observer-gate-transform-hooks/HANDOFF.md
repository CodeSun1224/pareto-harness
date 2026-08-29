# REQ-0008 Handoff

当前状态：`implementing`，TASK-REQ-0008-01至09完成，正在执行TASK-REQ-0008-10 fresh independent实现Code Review。REQ-0004、REQ-0007前置均done；`.agents/work/active`没有其他Runtime Requirement。

TASK-01证据：新增闭合Hook Protocol、RunManifest/RunCreated v2九角色与config digest、八类Hook Event、Hook Projection/hash view及Gate/Observer/Transform output Schema，最终内容地址SchemaSet为`sha256-3a0c6e67a97675cf6bfcdc1fb9766b30a79ae62e662479d9ae1ef5d7b43ff99d`。历史v1仍严格八角色且禁止Hook config；RunTask Projection分别锁定v1→retained output与v2→Hook-capable output。Protocol 9 unit + 24 contract通过，1个既有性能观察ignored。

TASK-02证据：Hook derived stream使用events表，sequence-1 exact固定source Run/SchemaSet/registry/config；reader按Manifest-pinned source与limits读取，pure fold生成连续history/projection digest，Recorded API仅read/fold。`hook_runtime::{fold_contract,recovery,compatibility,recorded_replay}` 4 passed。

TASK-03至08证据：Manifest-pinned registry、固定phase/稳定排序/lineage、opaque Fake handler、transaction-local Runtime Control admission、atomic reserve/terminal pair、scope/Capability/envelope/budget、Gate/default-deny/Observer隔离/Transform整体回退、FakeClock cancellation/timeout/reopen race和Recorded零执行已形成一个Fake纵切。`cargo test -p pareto-kernel hook_runtime:: --offline` 32 passed；所有PLAN命名测试均在父模块提供exact filter。

设计证据：initial design `8507bae4`经fresh independent REVIEW-0010提出4个Major；整改exact `3aee02adf8815466b02f51de247ae19922efc126`固定cross-kind phase/input lineage、Gate-bearing empty-required unconditional deny、reserve/terminal atomic pair与Transform fixed reject-whole，F-001至F-004由原Reviewer全部关闭，最终approved、0 Blocker/0 Major。ADR-0009及Requirement/Spec/RFC正式接受于`3318cbc6fe8bc8c9717a5a2b4aea1153f0d281d6`，同一Reviewer已完成accepted-doc freshness复核。实现者不得重新解释这些closure。

TASK-09证据在`VALIDATION.md`：29个PLAN filter逐个matched 1并通过；Hook 32、Event Store/Kernel 153 passed/1 ignored、Runtime Control 53、Protocol 9 unit + 24 contract/1 ignored、governance 24、scope/fmt/clippy/workspace/schema双生成均通过。`check_docs.py`仅因既有Review对新实现路径stale而预期失败，必须由TASK-10 fresh implementation Review实质恢复，不得修改历史Review绕过。

下一动作执行TASK-REQ-0008-10：提交exact implementation candidate，并由新的fresh independent Agent只读审查Requirement/Spec/RFC/ADR、完整diff与`VALIDATION.md`，创建REVIEW-0011。任何Blocker/Major由实现者修复、同一Reviewer复审关闭；0/0前不得verified/done或归档。

atomic pair是首要风险：当前REQ-0007 reserve/settlement helper会自行commit，必须重构为crate-private transaction-local admission。reserve和terminal各自固定pair ID/fingerprint、双stream cursor/sequence、两个Event bytes/交叉引用；zero原子写两者、two只exact retry、mutation冲突、one视为损坏，任何validation/insert/commit失败rollback。Hook operation不得走通用single-stream terminal后再补写Hook事实。

测试必须使用Fake Hook和FakeClock，不依赖真实sleep；所有命名Cargo filter先由`assert_cargo_test_filter.py`证明非零。特别保留第二append fault、commit-response-loss、validly-resealed one-sided history、two-writer reverse winner、Observer business-decision字节不变、Transform chain中间失败、跨tenant/user/workspace/run/task/actor隔离、Recorded handler/Event/budget零变化证据。

严格排除：真实shell/Python/TypeScript/HTTP/MCP/WASI Hook Runtime、外部Worker/RPC/队列、Provider、Coding Tool、外部Effect、Sandbox、Agent Loop、Memory、Task DAG、Evidence Graph、WASM/WASI执行、Hook Snapshot、background recovery scanner和并行Hook。若实现需要DB v3、public SQL/authority、第二状态表、alternate Event actor、current reader substitution或新依赖，返回SPEC/RFC并重新设计评审。

实施完成后运行PLAN全部门禁，并使用一个新的fresh independent Agent/session执行Code Review；本次REVIEW-0010只是设计Review，不能替代代码Review。Blocker/Major未由原代码Reviewer复审关闭前，不得verified/done，也不得开始REQ-0009。
