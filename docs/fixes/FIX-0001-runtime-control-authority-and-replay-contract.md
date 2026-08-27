---
id: FIX-0001
title: REQ-0007 Runtime Control 权威边界与回放合同缺口
status: fixed
owners: [runtime-kernel]
created: 2026-08-26
updated: 2026-08-27
links: [REQ-0007, SPEC-0006, RFC-0006, ADR-0007, REVIEW-0006, REVIEW-0007]
---

# Impact

REQ-0007 的首个实现候选 `1b40e92be11e73a497ec821118b7cb4e0c1af1ce`
通过了当时的测试，但未完整实现独立批准的可信 Operation、计量、超时恢复、
Capability、取消、幂等、持久化与 Replay 合同。独立实现评审 REVIEW-0007 因此
记录 0 Blocker、9 open Major，并禁止该 Requirement 进入 `verified` 或 `done`。

影响仅限尚未发布的 REQ-0007 候选 SchemaSet 和 crate-private Runtime Control API；
四个既有 retained SchemaSet、Event Store DB v2、REQ-0003 至 REQ-0006 的公开合同
没有迁移。不得以兼容旧 REQ-0007 候选为由保留不安全语义。

# Reproduction and evidence

固定复现 revision 为 `1b40e92be11e73a497ec821118b7cb4e0c1af1ce`，完整证据、
位置和 required proof 见 REVIEW-0007 F-001 至 F-009。最小违例序列包括：

1. 初始化 payload 自带 operation contract，随后 producer 可把自报向量标为
   `KernelMeterVerified` 并少计预算。
2. reserve 后改变 Clock sample 重试 timeout recovery，会产生不同 command identity，
   无法证明 response-loss exact retry。
3. 进程 epoch 改变后，旧 lease 仍可 settlement。
4. kind-only resource grant 不能收窄到 exact ID，且 inactive 原因被压成
   `default_deny`。
5. owner 可对不存在或终态 Task 写入 cancellation，且 operation 无只读 probe。
6. 同一 callback ID 改 payload 可被误判为已提交；denied proposal 在状态变化后可转为 allowed。
7. Schema-valid 但语义非法的 grant、allocation、settlement 或 cancel history 可被 fold 接受。
8. Projection 缺 initialization、history、完整 cancellation/ack 与 operation contract identity。
9. 现有 32 个 focused 测试含占位断言，未覆盖批准风险矩阵；scope helper 把 HEAD 与自身比较。

# Root cause

根因是首个实现把“crate-private”误当成了完整 authority boundary：初始化 command、
producer settlement command 和 JSON-Schema-valid history 仍被赋予了定义可信事实的能力。
同时，live path 与 pure fold 分别实现了部分校验，导致 reopen/replay 不能复证 live append
时应成立的全部不变量。测试按 API happy path 拆分，而不是从批准 Spec 的 authority、竞态、
恢复和兼容矩阵反向构造，因此绿灯未暴露这些缺口。

# Repair

修复以 REVIEW-0007 的九项 required proof 为边界：

- 用 exact retained registry 解析 operation contract；producer observation 与 crate-private
  Kernel meter snapshot 分离，并加入 mediated Fake Operation。
- 持久化闭合 deterministic timeout command，固定 key、Clock sample、evidence、fingerprint
  和 event ID；强制 live process epoch、Clock canonical/non-regression。
- 完成 Capability root/subset/reason、Task cancellation admission、probe/ack/recovery authority，
  以及 denied/callback/ack/refund/recovery 的 canonical command identity。
- live append 与 reopen/replay 共用版本固定 pure validator；补全 source、contract、history、
  cancellation 与 projection provenance identity，并发布新的内容寻址 SchemaSet。
- 用真实负例、并发、close/reopen、旧 SchemaSet 和 bounded model 测试替换占位测试；
  scope helper 对固定批准 baseline 检查。

首次 focused re-review 关闭 F-001/F-002/F-004/F-006 后，second repair 继续：

- 为未到期 reopen operation 提供只读 lease rebind，拒绝旧 process epoch、墙钟回退、
  deadline 延长与重复 dispatch；取消恢复确认必须携带 Kernel-sealed recovery fact。
- 在 reservation 中持久化 exact lifecycle checkpoint、adapter、timeout policy 与初始 epoch，
  在 settlement/late 中持久化 producer/reservation/lease/current epoch authority。
- pure fold 按历史 cursor 重建 lifecycle，并重验 grant usage 上界、取消、adapter、timeout、
  callback 与 late-result authority；Projection/hash 同步保留隔离后的 late audit provenance。
- 补充 exact not-before/expiry、deadline 边界、restart rebind、early recovery rejection、
  forged history 和真实 bounded complete/cancel/timeout state-sequence tests。

# Regression proof

每项 F-001 至 F-009 必须至少有一个在首个候选上失败、修复后通过的命名测试；
最终证据记录在 `.agents/work/active/REQ-0007-capability-budget-cancellation-timeout/VALIDATION.md`。
除 Focused、Impacted、Core 外，必须运行 AGENTS.md 全部门禁，并由 REVIEW-0007 的独立
Reviewer 在新 exact revision 上逐项关闭所有 Major。

# Compatibility and rollback

修复不修改 Event Store DB v2，不增加依赖，不触及 Hook、Provider、Tool、Sandbox、Agent Loop、
Task DAG 或 WASM。四个既有 SchemaSet 必须 byte-identical；修复过程中生成但从未发布的
中间 REQ-0007 SchemaSet 不进入仓库，新的 Manifest 只能固定修复后的 retained set。

回滚单位为整个 REQ-0007 Runtime Control 模块、新 SchemaSet、测试与关联 lifecycle guard；
回滚不会迁移或删除既有数据库。若修复候选仍有 open Blocker/Major，则保持 Requirement 为
`reviewing`，不得部分启用该能力。
