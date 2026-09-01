---
id: FIX-0002
title: Effect authority、pair integrity 与 recovery closure
status: fixed
owners: [runtime-kernel]
created: 2026-08-31
updated: 2026-09-01
links: [REQ-0009, SPEC-0008, RFC-0009, ADR-0010, REVIEW-0013]
---

# Impact

REQ-0009 初始实现候选 `6cad604ffe5ec2126f9745bf22ece713f2c0ce85` 的既有测试为绿，但
REVIEW-0013 发现 dispatch、Receipt、recovery、reconciliation、双 stream pair、Task success
guard、Projection 与历史 reducer identity 共 9 个 Major。候选尚未进入 verified/done，也没有真实
Provider、Tool、文件、进程或网络 Effect，因此缺陷未逃逸到外部效果；影响限于未发布的 REQ-0009
实现候选及其验证证据。

# Reproduction and evidence

- exact claim retry 返回可执行 lease；Intent 后 cancellation/deadline 仍可 claim。
- wrong Receipt producer 会向目标 stream 写 rejection；authenticated malformed Receipt 只写 audit，
  reservation 保持 open。
- recovery 的 current epoch digest 未绑定 canonical Clock；terminal 后新 Event IDs 返回冲突。
- reconciliation 只检查自报 revision 与排序 EventId，terminal EventId 可自证关闭。
- 删除 pair 任一 counterpart 后，另一 stream 的 projection 仍可读取。
- Task `succeeded` 未检查 Task-scoped Effect；Effect Projection 丢失 recovery/budget/evidence 字段。
- v3 source 注册后，原 v2 lifecycle golden 被直接改写。

完整源码位置、影响和 required proof 见 REVIEW-0013 F-001..F-009。

# Root cause

首个违反的不变量是“Kernel authority 必须由 exact、连续且可重算的持久事实派生”。实现把 crate-private
值误当作完整 authority closure：claim 没有在 writer lock 内重折叠所有 mutable guard；executor、epoch
和 reconciliation producer/evidence 缺少 Manifest/Clock/Event 派生绑定；pair seal 只在写入时计算而
读取时不验证 counterpart；Projection 只保留摘要；兼容测试跟随 current schema 漂移。测试随后固化了
这些简化行为，未覆盖跨边界故障后的唯一终态和旧 identity 不变性。

# Repair

- claim writer 内重验 lifecycle、Task、reservation、cancellation 与 deadline；exact retry不再签发lease。
- Fake implementation compatibility digest 与 descriptor 固定；Kernel 私有 orchestration 完成
  claim→invoke→Receipt/terminal，所有 Fake fault 均关闭 operation 或进入 reconciliation。
- wrong producer/adapter no-write；authenticated malformed/oversized/非规范 Receipt 转为 conservative
  unknown settlement并保留 rejection audit；Clock、result bytes、usage排序和Schema均验证。
- recovery current epoch 从 canonical Clock派生；same-ID exact/mutation优先，different-ID terminal为no-op。
- reconciliation source逐Event验证存在性、类型、scope、effect/attempt、pinned producer/adapter与Receipt；
  evidence fingerprint由source payload digests、external key与policy重算。
- Effect/Control读取双向验证counterpart、完整pair、可重算prepared digests与pair fingerprint。
- Task success接入exact Task Effect guard；Projection扩展为无损recovery/budget/pair/Receipt/evidence视图。
- v2 golden固定保留的Hook SchemaSet，v3使用独立golden；旧SchemaSet不改写。
- REVIEW-0013首轮整改复审后，对剩余F-002..F-005移除executor trait自报identity，改为
  Kernel固定实现digest解析sealed concrete Fake executor；`CrashAfterReturn`停在claim后并由reopen
  recovery关闭。authenticated-invalid Receipt的双stream terminal pair与rejection audit改为同一
  三Event事务。recovery改由module-private KernelRecoveryClock签发用途受限authority。reconciliation
  registry固定producer/adapter/implementation，resolution只来自sealed admitted query observation。
- 第二轮复审关闭F-002/F-003/F-004并发现F-005残余：claim后recovery产生的Unknown没有Receipt
  identity，而query observation admission错误地只接受Receipt-backed lineage。修复将source严格分为
  完整Receipt-backed与Kernel `effect-recovery-after-claim`两类，并新增crash/reopen recovery Unknown由
  Manifest-pinned sealed query producer关闭、且external executor不重入的端到端回归。

# Regression proof

新增/强化 `claim_revalidates_cancellation_and_deadline_under_writer_lock`、`dispatch_lease`、
`fake_outcomes`、`authenticated_invalid_receipt_settles_unknown_and_is_audited`、`crash_recovery`、
`reconciliation`、`pair_counterpart_loss_fails_effect_and_control_reads_closed`、
`projection_reopens_losslessly_for_unclaimed_and_partial_effects`、`lifecycle_success_guard`、
`digest_golden` 与 `effect_v3_digest_golden`。最终命令、计数和 exact commit 将写入 REQ-0009
VALIDATION，并由 REVIEW-0013 同一 independent reviewer 复审。

# Compatibility and rollback

修复不改 SQLite v2 DDL/trigger、不增加依赖或第二权威 mutable 表。Effect Projection/Receipt Schema 的
未发布 v3 合同演进为新的内容地址 SchemaSet；`ed548…` 及全部更早集合保持 byte-identical。回滚仅可
整体回退 REQ-0009 未发布实现与新 SchemaSet，不能部分关闭 claim/pair/evidence 校验或把 unknown 降格。
