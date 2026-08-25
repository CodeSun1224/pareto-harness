# REQ-0007 Handoff

当前阶段：independent design remediation/re-review，Runtime实现暂停。REQ-0005/0006均done，启动工作区clean且无其他active Runtime Requirement。REVIEW-0006对设计提交`05dd7ca`给出`changes-requested`，0 Blocker、6 open Major；下一步是完成TASK-REQ-0007-00、提交仅含设计修订的exact revision并由同一reviewer复审。0 open Blocker/Major前不得开始TASK-REQ-0007-01。

候选修订边界：单Run derived control stream；Manifest必须先exact固定control-capable SchemaSet，first event固定initial grants/budget/clock/source contract且所有row不漂移；control Event envelope signer恒为Manifest owner；Capability默认拒绝且child逐项收窄；lifecycle全状态准入及pending-operation transition guard；trusted operation contract产生并由Kernel meter强制resource envelope；Run/Task/Actor/operation预算同事务reserve；opaque lease绑定callback/ack producer，provider report非权威，unknown仅由authorized producer触发；Run/Task owner-only与operation owner/subject cancellation authority；显式Kernel timeout recovery；live monotonic、persisted absolute UTC、restart新lease；terminal唯一；late只对authorized producer写redacted digest audit；Projection派生；Recorded replay无Operation executor/recovery writer。

独立复审批准后的实施必须复用Event Store v2和现有transaction-local append/check，不能增加DB migration或第二权威表。每个control command在一个`BEGIN IMMEDIATE`内load exact lifecycle/control → fold → lifecycle/idempotency → permission/producer/cancel/deadline → trusted envelope/budget/state guard → one append；runtime-aware pause/terminal transition也在同一writer guard pending operation。跨scope或同域未授权cancel/callback探测不写目标；同aggregate已授权业务deny可写safe audit。

首片只允许FakeClock/FakeOperation。不得实现Hook、Effect/Receipt、Provider、Coding Tool、Sandbox、Agent Loop、Memory、Task DAG、WASM、background timeout scanner、Control Snapshot或真实外部副作用；显式Kernel timeout/recovery command是唯一无callback timeout writer。若代码需要任一扩张，先停止并更新impact/SPEC/RFC并重新独立设计评审。
