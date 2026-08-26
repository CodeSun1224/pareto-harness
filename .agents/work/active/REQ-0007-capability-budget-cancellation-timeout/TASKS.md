# Requirement Tasks

Current execution: reviewing. Fresh independent implementation review 已在 `1b40e92` 上形成
REVIEW-0007（0 Blocker、9 Major）；FIX-0001 repair candidate 已完成本地全回归，
TASK-REQ-0007-08等待同一独立 Reviewer 对新的 exact revision 做 focused re-review。

- [x] TASK-REQ-0007-00: 修订REQ/SPEC/RFC/ADR以闭合REVIEW-0006六个Major，提交仅含设计修订的exact revision，由同一independent reviewer focused re-review并恢复docs/diff门禁。Validation: REVIEW-0006 approved、0 open Blocker/Major、reviewed revision `a4e3478`；Runtime diff明确排除。

- [x] TASK-REQ-0007-01: 增加Capability/Budget/Clock/Operation/control payload/Projection协议类型、builtin decoders和新SchemaSet，保留旧set byte-identical。Validation: protocol contract commands and schema generation diff in PLAN。
- [x] TASK-REQ-0007-02: 实现crate-private control stream initialization、Manifest-pinned source contract/exact reader、pure fold、reducer/operation-contract registry与full-provenance Projection，不改SQLite v2。Validation: `runtime_control::{recovery,compatibility,schema_manifest_binding,projection}` and Event Store migration regression。
- [x] TASK-REQ-0007-03: 实现lifecycle admission/runtime-aware transition guard、default-deny、root/delegation收窄、revocation/expiry、owner signer/requester payload和safe denial audit。Validation: Capability、`lifecycle_admission`、`lifecycle_reserve_race`及doctest API surface。
- [x] TASK-REQ-0007-04: 实现trusted resource envelope/Kernel meter、checked multi-scope reserve、opaque lease/producer settlement、unknown conservative usage与owner refund，证明低报无效、并发不超卖和幂等。Validation: Budget、`resource_envelope`、`callback_authority`、idempotency filters。
- [x] TASK-REQ-0007-05: 实现三级cancel request authority、probe/ack lease binding、interruptibility、FakeClock deadline、TimeoutKey/确定性recovery ID与显式timeout recovery、terminal race与late audit。Validation: `cancellation_authority`、timeout identity golden、not_due、response-loss、same/different-ID priority及cancel/time/recovery/race/late/model filters；`check_req0007_scope.py`证明no real sleep。
- [x] TASK-REQ-0007-06: 实现Fake Operation最小纵切和read-only Recorded control replay，证明replay零dispatch/append/重复核算。Validation: `runtime_control::recorded_replay` plus event/account/counter assertions。
- [x] TASK-REQ-0007-07: 完成全scope隔离、unknown/old Schema、crash/reopen、row drift和REQ-0003..0006回归；运行全部completion gates并记录evidence。Validation: complete PLAN command list and `VALIDATION.md`。
- [ ] TASK-REQ-0007-08: 由fresh independent Agent执行implementation code-review；实现者修复，reviewer复审关闭全部Blocker/Major。Validation: 使用REVIEW-0007或当时下一个可用ID，independence independent、exact revision、0 open Blocker/Major；不得覆盖设计REVIEW-0006。
- [ ] TASK-REQ-0007-09: 同步implemented facts、final freshness和全门禁，REQ-0007 verified/done并归档；确认REQ-0008未提前实现。Validation: docs/governance/schema/diff/status gates。
