# REQ-0007 Handoff

当前阶段：reviewing。REVIEW-0007 已完成首次 focused re-review：F-001/F-002/F-004/F-006
closed，F-003/F-005/F-007/F-008/F-009 仍为 open Major。FIX-0001 second repair
candidate 已通过实现者完整门禁；下一步固定 exact revision 后交由同一 independent reviewer
再次 focused re-review。Blocker/Major只能由 reviewer 关闭，未关闭前不得 verified/done。

候选修订边界：单Run derived control stream；Manifest必须先exact固定control-capable SchemaSet，first event固定initial grants/budget/clock/source contract且所有row不漂移；control Event envelope signer恒为Manifest owner；Capability默认拒绝且child逐项收窄；lifecycle全状态准入及pending-operation transition guard；trusted operation contract产生并由Kernel meter强制resource envelope；Run/Task/Actor/operation预算同事务reserve；opaque lease绑定callback/ack producer，provider report非权威，unknown仅由authorized producer触发；Run/Task owner-only与operation owner/subject cancellation authority；TimeoutKey+Clock sample+冻结evidence确定性派生recovery event ID/fingerprint，not_due不消费、exact response-loss retry、same-ID mutation优先、different-ID terminal no-op；显式Kernel timeout recovery；live monotonic、persisted absolute UTC、restart新lease；terminal唯一；late只对authorized producer写redacted digest audit；Projection派生；Recorded replay无Operation executor/recovery writer。

独立复审批准后的实施必须复用Event Store v2和现有transaction-local append/check，不能增加DB migration或第二权威表。每个control command在一个`BEGIN IMMEDIATE`内load exact lifecycle/control → fold → lifecycle/idempotency → permission/producer/cancel/deadline → trusted envelope/budget/state guard → one append；runtime-aware pause/terminal transition也在同一writer guard pending operation。跨scope或同域未授权cancel/callback探测不写目标；同aggregate已授权业务deny可写safe audit。

首片只允许FakeClock/FakeOperation。实现未增加Cargo依赖、未改SQLite v2/DDL、不使用真实sleep/network/process；RunTask Projection对新source set继续使用REQ-0006固定的`4ce387…` output set，历史SchemaSet均保留。second repair candidate 当前已通过52个Runtime focused tests、Protocol 9+23（1 ignored）、全仓Kernel 120（1 ignored）、governance 21、fmt、clippy、scope、两次稳定schema generation和diff check；最终source set为`c3e2fda5…`。`check_docs.py`须在exact candidate独立复审后由reviewer实质恢复freshness，实施者不得自行前移。不得实现Hook、Effect/Receipt、Provider、Coding Tool、Sandbox、Agent Loop、Memory、Task DAG、WASM、background timeout scanner、Control Snapshot或真实外部副作用。
