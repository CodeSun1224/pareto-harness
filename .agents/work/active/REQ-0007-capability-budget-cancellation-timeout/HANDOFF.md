# REQ-0007 Handoff

当前阶段：planned，Runtime实现尚未开始。REQ-0005/0006均done，启动工作区clean且无其他active Runtime Requirement。REQ-0007 high-risk Requirement和impact已完成；RFC-0006/ADR-0007 accepted；SPEC-0006 approved；architecture/security self-review 0 open Blocker/Major。下一步从TASK-REQ-0007-01开始，不得重新设计或绕过批准合同。

已冻结边界：单Run derived control stream；first event固定initial grants/budget/clock；control Event envelope signer恒为Manifest owner，真实requester/subject在closed payload exact验证；Capability默认拒绝且child逐项收窄；Run/Task/Actor/operation预算同事务reserve；provider report非权威，unknown全额保守；cancel request/ack/settlement分离；live monotonic、persisted absolute UTC、restart新lease；terminal唯一；late只写redacted digest audit；Projection派生；Recorded replay无Operation executor。

实施必须复用Event Store v2和现有transaction-local append/check，不能增加DB migration或第二权威表。每个control command在一个`BEGIN IMMEDIATE`内load exact lifecycle/control → fold → idempotency → permission/cancel/deadline → budget/state guard → one append。跨scope未授权探测不写目标；同aggregate业务deny可写safe audit。

首片只允许FakeClock/FakeOperation。不得实现Hook、Effect/Receipt、Provider、Coding Tool、Sandbox、Agent Loop、Memory、Task DAG、WASM、background timeout、Control Snapshot或真实外部副作用。若代码需要任一项，先停止并更新impact/SPEC/RFC。
