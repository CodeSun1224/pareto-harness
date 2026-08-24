# REQ-0005 Handoff

当前阶段：done。独立 REVIEW-0004 已批准 exact `675e3f8fe6888c1d01fec14dda8e0f9164bb8a1b`，F-001/F-002/F-003/F-004 全部由 reviewer closed，最终 0 Blocker / 0 Major；F-005 accepted Minor 的陈旧 revision 表述已在本 handoff 同步。实现、remediation、全仓回归、独立复审、durable docs 同步与工作归档均完成，REQ-0006 未开始。

已冻结设计：完整Manifest作为derived lifecycle stream的sequence-1 `RunCreated` payload；状态只由单流事件pure fold；先由persisted exact reader建立owner-only authority，再在同一 `BEGIN IMMEDIATE` 内revalidate binding → validate/fold aggregate → idempotency → expected sequence/state/guards → one append；不得在authority/fold前查询全局event ID或让retry掩盖损坏历史。terminal不可逆且无隐式Task cascade。新SchemaSet必须保留旧set；SQLite `user_version`保持1。

已实现事实：新 set `sha256-dae028a86b31c5ab341240a0768e5166ac36cd4104bfa7e8c759230add368a71` 发布且旧 set 无 diff；Event Store append 复用 transaction-local check/insert；lifecycle 在同一 `BEGIN IMMEDIATE` 内用 sequence-1 持久 identity exact resolve reader、owner authorize、全流 validate/fold、幂等、expected/guard 和单事件 append；状态只来自 pure fold，无 DDL/`user_version`/公开 SQL API 变化。ValidatedEvent 现保留 admission 使用的 exact SchemaSet/limits identity，pure fold 逐事件检查 tenant/user/workspace/run/agent/actor/derived stream 与首 Manifest binding。三个 established 命令对 `i64::MAX`、负数和零均稳定返回结构化冲突且不 panic/append；Task stale sequence 先于 Task lookup。Event Store legacy fixture 以 checked-in `sha256-68535b...` old set 为 parent，重开后 `user_version=1` 并拒绝 current reader替代。
