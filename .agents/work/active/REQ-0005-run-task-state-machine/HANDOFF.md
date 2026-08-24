# REQ-0005 Handoff

当前阶段：reviewing，TASK-REQ-0005-01 至 05 已完成；TASK-REQ-0005-06 的 Runtime/static/schema 门禁通过，`check_docs.py` 仅因本 substantive diff 尚无独立 REVIEW-0004 而按预期阻止 freshness。正在执行 TASK-REQ-0005-07 独立 code-review。REQ-0003/0004 均为 done，既有独立 Review 均 approved 且 0 open Blocker/Major；用户已有的 REQ-0005 SDD/工作记录已保留。

已冻结设计：完整Manifest作为derived lifecycle stream的sequence-1 `RunCreated` payload；状态只由单流事件pure fold；先由persisted exact reader建立owner-only authority，再在同一 `BEGIN IMMEDIATE` 内revalidate binding → validate/fold aggregate → idempotency → expected sequence/state/guards → one append；不得在authority/fold前查询全局event ID或让retry掩盖损坏历史。terminal不可逆且无隐式Task cascade。新SchemaSet必须保留旧set；SQLite `user_version`保持1。

已实现事实：新 set `sha256-dae028a86b31c5ab341240a0768e5166ac36cd4104bfa7e8c759230add368a71` 发布且旧 set 无 diff；Event Store append 复用 transaction-local check/insert；lifecycle 在同一 `BEGIN IMMEDIATE` 内用 sequence-1 持久 identity exact resolve reader、owner authorize、全流 validate/fold、幂等、expected/guard 和单事件 append；状态只来自 pure fold，无 DDL/`user_version`/公开 SQL API 变化。30 个 kernel 测试及 kernel clippy 已通过。下一步执行 PLAN 全部命名/全仓门禁并记录证据，然后交独立 Agent code-review；在独立批准前不得标记 verified/done。
