# Requirement Tasks

Current execution: planned. REQ/SPEC/RFC/ADR design gates are approved; Runtime implementation has not started.

- [ ] TASK-REQ-0007-01: 增加Capability/Budget/Clock/Operation/control payload/Projection协议类型、builtin decoders和新SchemaSet，保留旧set byte-identical。Validation: protocol contract commands and schema generation diff in PLAN。
- [ ] TASK-REQ-0007-02: 实现crate-private control stream initialization、persisted exact reader、pure fold、reducer registry与full-provenance Projection，不改SQLite v2。Validation: `runtime_control::{recovery,compatibility,projection}` and Event Store migration regression。
- [ ] TASK-REQ-0007-03: 实现default-deny、root/delegation收窄、revocation/expiry、owner signer/requester payload和safe denial audit。Validation: all Capability named commands plus doctest API surface。
- [ ] TASK-REQ-0007-04: 实现checked multi-scope reserve、settlement/release、unknown conservative usage与owner refund，证明并发不超卖和幂等。Validation: all Budget/idempotency named commands。
- [ ] TASK-REQ-0007-05: 实现三级cancel request/ack、interruptibility、FakeClock deadline、terminal race与late/duplicate/out-of-order audit。Validation: cancel/time/race/late/model commands；no real sleep。
- [ ] TASK-REQ-0007-06: 实现Fake Operation最小纵切和read-only Recorded control replay，证明replay零dispatch/append/重复核算。Validation: `runtime_control::recorded_replay` plus event/account/counter assertions。
- [ ] TASK-REQ-0007-07: 完成全scope隔离、unknown/old Schema、crash/reopen、row drift和REQ-0003..0006回归；运行全部completion gates并记录evidence。Validation: complete PLAN command list and `VALIDATION.md`。
- [ ] TASK-REQ-0007-08: 由fresh independent Agent执行code-review；实现者修复，reviewer复审关闭全部Blocker/Major。Validation: REVIEW-0006 approved、independence independent、exact revision、0 open Blocker/Major。
- [ ] TASK-REQ-0007-09: 同步implemented facts、final freshness和全门禁，REQ-0007 verified/done并归档；确认REQ-0008未提前实现。Validation: docs/governance/schema/diff/status gates。
