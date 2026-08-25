# Requirement Tasks

Current execution: done. Fresh independent code review of exact `5c4f6e7` returned 3 Major；author remediation `1d27154` was independently re-reviewed and all three findings were closed。REVIEW-0005 is approved with 0 Blocker / 0 Major；completion gates, durable fact sync and archive are complete。

- [x] TASK-REQ-0006-01: 增加versioned reducer descriptor/key/ref、Cursor、RunTask Projection/Snapshot及history/hash-view类型，冻结rolling chain和全部digest golden，发布exact output SchemaSet并证明旧set不变。Validation: protocol contract/digest golden commands；schema generation diff。
- [x] TASK-REQ-0006-02: 将lifecycle fold提取为exact versioned deterministic reducer，建立source→reducer/output registry与retention，生成store/full-provenance Projection，拒绝unknown/schema/sequence/illegal history和current substitution。Validation: reducer/resolution/full/invalid/compatibility named commands。
- [x] TASK-REQ-0006-03: 实现SQLite v1→v2 atomic migration、events writer epoch trigger和UPDATE/DELETE-protected snapshot store，不改历史Event JSON/authority。Validation: migration/already-open-v1-writer/compatibility named commands。
- [x] TASK-REQ-0006-04: 实现point-in-time full Projection、transactional Snapshot创建、exact output reader/candidate校验、prefix revalidation/history-chain proof、suffix incremental和candidate-only fallback。Validation: snapshot creation/atomicity/incremental/output-reader/prefix/fallback/no-snapshot commands。
- [x] TASK-REQ-0006-05: 实现read-only Recorded replay、exact comparability/digest result与Simulated pre-effect rejection；证明0 real Effect/append。Validation: all four `replay::` named commands in PLAN plus API/dependency inspection。
- [x] TASK-REQ-0006-06: 完成Run/Workspace/Actor/tenant/user/store/source/output隔离、cross-store comparability、append/read/snapshot concurrency、crash/reopen及old source/output/reducer negative matrix。Validation: isolation/cross-store/concurrency/recovery/compatibility named commands in PLAN。
- [x] TASK-REQ-0006-07: 执行Focused、Impacted、Core及全部completion gates，记录exact环境/命令/结果、Schema/DB identity和quality/cost/latency观察。Validation: complete PLAN command list and `VALIDATION.md`。
- [x] TASK-REQ-0006-08: 由fresh independent reviewer执行code-review；修复并由reviewer复审关闭全部Blocker/Major。Validation: REVIEW-0005 approved, independence independent, exact reviewed revision `1d271549c2607f9c00377bdaa0fa999a131dafe3`, 0 open Blocker/Major。
- [x] TASK-REQ-0006-09: 同步implemented facts、完成全门禁、REQ-0006 verified/done并归档work；确认REQ-0007未提前实现。Validation: docs/governance/schema/diff/status gates。
