# REQ-0009 Handoff

当前状态：`reviewing / TASK-REQ-0009-11`。REQ-0004、REQ-0007、REQ-0008前置均done；REQ-0009设计与规划已独立批准。实现、安全/隔离/redaction/兼容负测、scope守卫、Focused/Impacted/Core/Full适用门禁与VALIDATION证据已完成；当前必须固定实现候选并交给新的fresh independent code reviewer，不能用REVIEW-0012设计评审替代。

fresh independent REVIEW-0012最终批准设计exact `b7acbd82824d8410d432117c89be1bd56c8ce05c`及接受闭环exact `60cee6ed44d150185bf99ca3095a8ce803bcc0d3`，F-001至F-004全部closed，0 Blocker/0 Major。四项关键closure是：Recorded replay固定Inventory source horizon；Inventory/Record V2无损区分partial/unknown；executor内容地址identity贯穿全部边界；claim/crash recovery command及未claim/已claim结论闭合。

现有基线：SQLite v2不变；REQ-0008最终Hook-capable SchemaSet `sha256-0efc2e…`保持retained。TASK-01已发布Manifest v3、Effect/Executor/Event/Projection与Boundary Inventory/Record V2闭合Schema；TASK-03按既有Spec补齐`effect_pair` kind及双方prepared-bytes digest后将重新生成最终content-addressed set，`sha256-161a…`仅为未提交中间候选，不得作为最终identity。全部旧Manifest、Inventory、SchemaSet、reader/reducer及历史字节继续保留。

开始任何行为编辑前，先把Requirement从`planned`推进为`implementing`并同步TASKS/HANDOFF。第一实施步是Protocol/Schema：闭合Effect/executor descriptor/Intent/claim/Receipt/conclusion/reconciliation/Projection/Inventory V2类型并生成内容地址SchemaSet。随后才实现Effect stream、Runtime Control双事件原子pair、幂等Intent、dispatch claim、Fake executor、Receipt admission、恢复/reconciliation及Recorded replay。

最高风险不变量：Intent必须先于dispatch；same key只允许exact retry；dispatch lease绑定exact executor；未claim recovery只能`not_applied + verified zero + full release`；claim后只能partial/unknown + reconciliation且不redispatch；Effect结论与operation terminal/settlement必须原子一致；Recorded replay固定horizon且零executor/writer/settlement authority。

严格排除：真实文件/进程/network/Provider/Tool/Sandbox效果、外部Worker/RPC/队列、DB v3、mutable outbox/status/receipt表、alternate Event actor、background scanner、自动redispatch、跨边界exactly-once承诺、caller-selected reader/registry和新第三方依赖。触发任一项必须返回SPEC/RFC与独立设计复审。

实施测试全部使用Fake executor与FakeClock，不依赖真实sleep。每个命名Cargo filter必须经`assert_cargo_test_filter.py`证明非零。实现完成后必须启动一个新的fresh independent Agent使用`code-review`评审代码与原始证据；不能用REVIEW-0012替代实现评审。
