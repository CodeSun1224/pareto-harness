---
id: REVIEW-0007
title: REQ-0007 Capability、预算、取消与超时独立实现评审
status: changes-requested
owners: [independent-reviewer]
created: 2026-08-26
updated: 2026-08-26
links: [REQ-0007, SPEC-0006, RFC-0006, ADR-0007, REVIEW-0006, FIX-0001, REQ-0003, REQ-0004, REQ-0005, REQ-0006]
independence: independent
reviewed_revision: ab2fbc6d2e979ef12bcffd5df1cfe76b975a9684
open_blockers: 0
open_majors: 5
---

# Verdict

`changes-requested`。本记录由未参与实现、也未参与 REVIEW-0006 设计评审的 fresh independent
Reviewer 维护。首轮固定 revision
`1b40e92be11e73a497ec821118b7cb4e0c1af1ce`，本次 focused re-review 独立检查完整修复差异
`1b40e92..ab2fbc6`、FIX-0001、协议/Schema、Runtime、Projection 和测试，而不采纳实现者的结论。

修复实质关闭了 F-001、F-002、F-004、F-006：trusted retained operation contract、Kernel meter/Fake
Operation、确定性 timeout identity、Capability root/subset/reason，以及 denial/callback/late 的主要幂等路径
已经成立。F-003、F-005、F-007、F-008、F-009 仍为 open Major，因此 REQ-0007 不得进入
`verified` 或 `done`，也不得启动依赖该内核合同的 REQ-0008。

# Findings

| ID | Severity | Location | Focused re-review evidence and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `runtime_control.rs:29-48,280-418,815-906,1768-1812,2484-2575`; `runtime_control/tests.rs:793-891` | sequence-1 现在只保存 exact retained contract ref；Kernel 按 exact final SchemaSet `sha256:19566903…`解析固定 adapter/envelope/meter/producer/callback namespace。`KernelMeterSnapshot`字段与构造器不向producer开放，Fake Operation 在下一单位超界前停止，violation按unknown全额保守核算，Recorded replay dispatch counter为0。伪造snapshot与未注册contract负测通过。 | 已满足；后续 Provider/Tool 不得获得 meter snapshot constructor 或覆盖 retained registry。 | closed |
| F-002 | Major | `runtime_control.rs:258-279,1597-1748,1849-1919`; `runtime_control/tests.rs:558-627,974-1007` | timeout command 已冻结 persisted `TimeoutKeyV1`、canonical Clock sample、verified/unknown evidence、fingerprint和`event_<hex>`；使用批准domain，golden为`event_0c88…be91`。integrity在terminal检查前执行；not-due零写入、verified partial、unknown、same-command response-loss retry、same-ID mutation及different-ID terminal no-op均有行为证据。 | 已满足。 | closed |
| F-003 | Major | `runtime_control.rs:1597-1637,3130-3181`; `runtime_control/tests.rs:910-926` | stale process-epoch lease、wall regression、invalid calendar和callback namespace现已fail closed；但重启后未到absolute deadline时没有批准合同要求的“以current wall剩余时间建立新monotonic deadline/lease”入口。`make_lease`只在首次reserve调用；reopen测试只证明旧lease拒绝，然后直接等到deadline做timeout recovery。deadline前的`prepare_timeout_recovery`只会生成最终返回`NotDue`的command，不能返回新`OperationLease`，operation因此无法在新epoch继续probe或合法callback。AC-11/RFC-0006的restart边界未闭合。 | 增加显式Kernel recovery/rebind入口：exact pending operation、current trusted wall、new process epoch → 新monotonic lease；不reexecute、不append、不延长absolute deadline。测试重启前deadline重建、旧lease拒绝、新lease probe/callback、wall regression及已到期只能timeout。 | open |
| F-004 | Major | `runtime_control.rs:2634-2827`; `runtime_control/tests.rs:928-972` | owner可签发root；初始化grant可给same-scope subject；kind-only resource可收窄exact ID；Task/resource/operation/time/usage/depth按subset收窄，parent chain在授权时检查revocation/validity。grant shape要求canonical UTC、sorted unique operations和canonical usage；denial使用稳定`missing/revoked/not_yet_valid/expired/...`reason。root/narrow/widen/revocation行为通过。 | 行为修复满足；expiry边界测试缺口归入F-009。 | closed |
| F-005 | Major | `runtime_control.rs:1088-1412`; `runtime_control/tests.rs:1009-1040` | Task cancellation现在拒绝missing/terminal Task；probe只读且绑定live lease；producer ack检查operation/reservation/producer/epoch，unknown/out-of-order有safe rejected audit。但`acknowledge_cancellation_recovery`仅检查Manifest owner和pending record，没有opaque recovery authority、deadline/timeout settlement、executor返回或“操作已停止”事实。新增测试反而在`00:00:13`、absolute deadline `00:01:00`前用不同epoch的普通`CancellationAckCommand`直接写`kernel_recovery` ack。Kernel owner可对仍在运行、尤其uninterruptible的operation提前声称确认取消，违反request与ack分离及AC-09/10。 | recovery ack必须由不可伪造的Kernel recovery fact/handle构造，并证明操作已回到Kernel或已由timeout recovery终结；deadline前仅有owner身份不得确认。补充cooperative producer ack、uninterruptible pending、owner early recovery拒绝、timeout/reopen recovery ack和重复/乱序矩阵。 | open |
| F-006 | Major | `runtime_control.rs:713-780,956-1086,1184-1256,1414-1535`; `runtime_control/tests.rs:629-661,1042-1072,1293-1320` | 首次denial按operation/request digest持久锁定，状态变化后exact proposal仍返回原denial。callback保存canonical fingerprint：exact retry返回原结果、same callback mutation冲突、新callback在terminal后走统一redacted late audit且预算不变。ack比较完整payload，unknown/out-of-order消息的exact retry与mutation优先级有证据。 | 已满足。 | closed |
| F-007 | Major | `runtime_control.rs:2006-2374`，尤其`:2126-2190`; `runtime_control/tests.rs:1122-1181` | reducer已重验delegation、account拓扑、allocation方程、settlement evidence和ack存在性，但`operation-reserved` fold仍未重演完整authorize/cancellation语义：它不检查trusted envelope `<= grant.constraints.max_operation_usage`，也不调用`cancellation_applies`。因此可在已有Task/Run cancellation之后，或用max usage小于retained envelope的grant，追加Schema-valid、allocation自洽的forged reservation并被reopen/Projection接受。reserved payload也未保存adapter revision/lifecycle admission cursor，fold无法复证live path的exact adapter与reserve时lifecycle事实。现有非法history测试只覆盖widened grant、伪造settlement和不存在request的ack，未覆盖首轮required proof中的forged reservation/cancel-before-reserve。AC-02/14仍可在recovery路径回退。 | live reserve与pure versioned validator共享完整authorization、trusted contract、cancellation及lifecycle admission不变量；必要身份进入Event。加入低usage grant forged reserve、cancel-before-reserve、wrong adapter/contract/deadline/lifecycle cursor等Schema-valid非法历史fixtures，reopen/projection/replay均`AggregateCorrupt`且零副作用。 | open |
| F-008 | Major | `crates/pareto-protocol/src/runtime_control.rs:535-581,657-681,707-846`; `runtime_control.rs:2192-2294` | source/contract/history/reader、完整reservation/settlement、cancellation/ack及Projection provenance已补全，新内容地址SchemaSet和四旧set兼容测试通过。但批准的settlement Event应固定producer/lease binding；`OperationSettledPayloadV1`没有presented producer revision、lease/process-epoch identity（unknown evidence时也没有meter epoch）。`LateResultObservedPayloadV1`同样没有producer/reservation/lease binding。fold只能相信live writer曾做过检查，无法从durable Event审计或复证callback/late authority，且当前v1一旦发布会成为REQ-0009/0010难逆的接口。 | 在首发Schema中持久化或以明确、可验证的closed identity表达settlement/late的approved producer、reservation及lease/process-epoch authority；Projection/hash view携带同一事实，pure fold校验exact reservation contract。增加unknown settlement和late authority reopen/replay负测。 | open |
| F-009 | Major | `runtime_control/tests.rs:328-377,582-588,770-790,910-926,1122-1181`; `scripts/check_req0007_scope.py` | scope helper已改为固定批准baseline，新增真实Fake Operation、two-writer、full scope、timeout、Projection和部分非法history场景；50个focused测试均绿。但AC-19仍未达到批准矩阵：`revocation_and_expiry`只测撤销、没有expiry/not-before边界；`model_sequences`仍只是三整数恒等式而非bounded command/terminal/cancel model；`deadline`仍为两次样本比较；没有restart-before-deadline lease重建、owner early recovery-ack拒绝、cancel-before-reserve/forged reservation、settlement/late durable authority fixture。测试还把F-005的错误early recovery ack断言为成功。 | 补齐上述命名风险，确保每个AC filter非零且真正触发目标路径；bounded model应枚举合法command序列和提交顺序并断言最多一个terminal、budget守恒、late/replay零执行。 | open |

# Acceptance trace

| Acceptance | Re-review result | Independent evidence |
|---|---|---|
| AC-01 | passed | closed grant ID/Schema、root/child、scope/resource/operation/constraints/version identity存在；见F-004。 |
| AC-02 | blocked | live path真实default deny且无raw bypass，但pure fold仍接受部分forged reservation，见F-007。 |
| AC-03 | passed | child只能subset、full parent revocation/validity链重验；见F-004。 |
| AC-04 | passed | default deny与稳定结构化reason成立；全scope no-write矩阵通过。 |
| AC-05 | passed | retained trusted envelope、多scope account、hard/soft和decimal u64均冻结；见F-001。 |
| AC-06 | passed | `BEGIN IMMEDIATE`内多account reserve、reverse winner及防超卖测试通过。 |
| AC-07 | passed | verified partial、unknown full consume、release/refund上限与meter violation保守核算通过。 |
| AC-08 | partial / blocked | live producer/lease/meter与callback幂等成立；durable settlement authority identity仍缺，见F-008。 |
| AC-09 | blocked | request admission/propagation/probe改进；recovery ack可无停止事实提前确认，见F-005。 |
| AC-10 | blocked | terminal outcome互斥且producer ack存在；uninterruptible/recovery confirmation语义不成立，见F-005。 |
| AC-11 | blocked | timeout command/FakeClock/due规则成立；restart-before-deadline新monotonic lease缺失，见F-003。 |
| AC-12 | partial / blocked | SQLite terminal winner与timeout/callback race通过；批准的bounded command model仍缺，见F-009。 |
| AC-13 | partial / blocked | duplicate/mutation/late不改预算；late Event不能持久复证producer/lease，见F-008。 |
| AC-14 | blocked | reducer显著加强，但forged reservation/cancel-before-reserve仍可通过，见F-007。 |
| AC-15 | partial / blocked | complete Projection、reopen和Recorded replay零dispatch通过；pending reopen不能重建live lease，见F-003。 |
| AC-16 | passed | tenant、user presence/value、workspace、run、owner actor和business-ID shadow no-write矩阵通过；Event Store隔离回归全绿。 |
| AC-17 | partial / blocked | final set deterministic且四旧set完整，DB v2不变；首发settlement/late authority Schema仍不完整，见F-008。 |
| AC-18 | blocked | private retained reserve/meter/cancel/timeout/projection接口存在；rebind/recovery-ack/durable callback identity仍缺，见F-003/F-005/F-008。 |
| AC-19 | blocked | 50个focused测试通过但批准矩阵仍有实质空洞，见F-009。 |
| AC-20 | passed | workspace 118 passed/1 ignored，Protocol 9+23 passed/1 ignored；无依赖、DB migration或后续框架扩张。 |

# Constitutional effect trace

| Path | Focused re-review result |
|---|---|
| request → capability → retained envelope → reserve | live path闭合；reopen pure fold仍缺完整reservation authorization/cancellation复证（F-007）。 |
| lease → Fake Operation → Kernel meter → settlement | live trusted chain闭合；durable settlement未固定presented producer/lease authority（F-008）。 |
| cancel → probe → ack → terminal | producer live-lease路径成立；owner可无recovery fact提前ack（F-005）。 |
| deadline → deterministic command → timeout | identity、evidence和terminal优先级成立；reopen未到期不能建立新monotonic lease（F-003）。 |
| callback → duplicate/mutation/late | live幂等和预算隔离成立；late durable authority identity缺失（F-008）。 |
| Event → Projection/reopen/Recorded replay | Projection完整度和零执行显著改善；非法reservation仍可能被fold接受（F-007）。 |

# Compatibility, isolation, scope, and regression

- 完整`1b40e92..ab2fbc6`未修改SQLite `user_version=2`、ledger、writer epoch、Snapshot DDL，未增加
  Cargo依赖，也没有Hook、Provider、真实Tool/Effect、Sandbox、Agent Loop、Task DAG或WASM实现。
- final content-addressed set为`sha256:19566903f801e66b5a4367ff173b9ff1982232456f9b432fd075db4e4639b1f9`；
  生成器、retained-set completeness和old-writer exact-reader测试通过。四个既有published set未被替代。
- RunTask Projection/Snapshot golden改变来自Manifest source SchemaSet identity；Reviewer复跑其完整回归确认既有reducer、
  Snapshot、Recorded replay和DB兼容行为未回退。
- scope helper现在比较批准baseline `6de3598`，修复了首轮HEAD-self-comparison缺陷；DB常量和Cargo manifest差异检查通过。

# Independent validation

Reviewer在Windows/PowerShell、offline、2026-08-26独立执行：

- `cargo test -p pareto-kernel runtime_control --offline --no-fail-fast`：50 passed。
- `cargo test -p pareto-protocol --all-features --offline`：9 unit + 23 contract passed；1 observation ignored；
  compile-fail doctest passed。stderr中的existing-set drift来自预期负例，命令exit 0。
- `python -m unittest discover -s scripts/tests -p "test_*.py"`：21 passed。
- `python scripts/check_req0007_scope.py`：passed。
- `cargo test --workspace --all-targets --all-features --offline`：Kernel 118 passed/1 ignored；Protocol
  9 + 23 passed/1 ignored；命令exit 0。
- `cargo fmt --all -- --check`、workspace clippy offline `-D warnings`、Schema generation、
  `python scripts/check_docs.py`、`git diff --check`：全部passed；generator后`schemas/`无diff，
  文档检查为172 Markdown / 51 formal IDs。

测试成功只证明现有断言成立；F-003/F-005/F-007/F-008指出的代码合同与F-009的缺失场景不因绿灯关闭。

# Re-review conditions

实现者不得自行关闭open Major。下一候选至少必须提供：restart-before-deadline lease rebind；有真实recovery
fact的cancellation ack；forged reservation/cancel-before-reserve pure-fold fixtures；settlement/late durable
producer/lease identity；expiry和bounded command model。focused re-review须固定新的exact revision并逐项检查
F-003/F-005/F-007/F-008/F-009；任一仍open时保持`changes-requested`。

# Re-review history

- 2026-08-26：fresh independent implementation review of exact
  `1b40e92be11e73a497ec821118b7cb4e0c1af1ce`；0 Blocker、9 Major，`changes-requested`。
- 2026-08-26：focused independent re-review of exact
  `ab2fbc6d2e979ef12bcffd5df1cfe76b975a9684` and full `1b40e92..ab2fbc6` remediation diff。
  F-001/F-002/F-004/F-006 closed；F-003/F-005/F-007/F-008/F-009保持open。最终0 Blocker、5 Major，
  仍为`changes-requested`。
