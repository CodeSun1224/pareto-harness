---
id: REVIEW-0007
title: REQ-0007 Capability、预算、取消与超时独立实现评审
status: changes-requested
owners: [independent-reviewer]
created: 2026-08-26
updated: 2026-08-27
links: [REQ-0007, SPEC-0006, RFC-0006, ADR-0007, REVIEW-0006, FIX-0001, REQ-0003, REQ-0004, REQ-0005, REQ-0006]
independence: independent
reviewed_revision: 97bca8b7b34ceadd5ab4f8ad01f49e10b3377adb
open_blockers: 0
open_majors: 1
---

# Verdict

`changes-requested`。本记录由未参与实现、也未参与 REVIEW-0006 设计评审的 fresh independent
Reviewer 维护。第三次 focused re-review 固定 exact candidate
`97bca8b7b34ceadd5ab4f8ad01f49e10b3377adb`，Runtime repair 为其父提交
`693299cb1dbe5fa5c75729445bcd8f1054389731`；Reviewer 独立检查完整
`26b63ca..97bca8b` diff、批准合同、FIX-0001、协议/Schema、Runtime、Projection、测试和原始门禁，
不采纳实现者的 finding closure 声明。

本轮实质关闭 F-005 和 F-009：rebind 已变为零 append 的 lease 换发，取消确认只保留 producer
executor-return authority；23 组有界 command graph、2 组真实 SQLite writer race 和 late exact retry
逐步验证唯一终态、合法 winner、预算与 replay。F-007 仍为 open Major：settlement pure fold 已补齐
namespace/cancel/outcome/deadline/equation，但仍未共享 live path 的 callback 时序及 meter/lease epoch 绑定，
重新封印的非法历史仍可成为权威。REQ-0007 不得进入 `verified`/`done`，也不得启动 REQ-0008。

# Findings

| ID | Severity | Location | Latest independent disposition | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `runtime_control.rs:35-49,299-437,839-986,1990-2035,2754-2865`; tests `873-971` | Exact retained registry、Kernel meter/Fake Operation、超界前停止、unknown保守核算和Recorded replay零dispatch继续通过；final retained contract已前移到exact set `c3e2fda5…`。 | 已满足。 | closed |
| F-002 | Major | `runtime_control.rs:277-298,1717-1871,2071-2141`; tests `638-725,1157-1190` | Frozen timeout key/sample/evidence/fingerprint、批准domain、golden、not-due、verified partial、same-ID mutation、response-loss exact retry和different-ID terminal no-op继续成立。Schema identity变化后golden按完整preimage确定性前移。 | 已满足。 | closed |
| F-003 | Major | `runtime_control.rs:988-1039,3432-3567`; tests `1053-1136` | `rebind_operation_lease`从persisted absolute deadline和注入Clock建立当前epoch monotonic lease；拒绝initial/stale epoch、wall regression、terminal和到期operation，不append、不延长deadline、不dispatch。reopen测试证明旧lease拒绝、新lease可probe/callback、replay不执行；到期只能走timeout recovery。 | 已满足。取消ack是否可据此宣称已停止是独立F-005语义。 | closed |
| F-004 | Major | `runtime_control.rs:2915-3109`; tests `1008-1051` | owner root、same-scope initial subject、resource/Task/operation/time/usage/depth subset、parent revoke/expiry和稳定reason继续成立；新增not-before/expiry exact边界测试通过。 | 已满足。 | closed |
| F-005 | Major | `runtime_control.rs:988-1039,1429-1524`; tests `1053-1136,1204-1230` | `CancellationRecoveryFact`、`acknowledge_cancellation_recovery`与`kernel_recovery` authority已删除；rebind只建立当前process lease且零append。producer ack仍绑定exact operation/reservation/producer/epoch；uninterruptible早期rebind不确认，只有executor-return producer ack或absolute deadline timeout terminal可证明边界停止。focused及reopen/replay测试通过。 | 已满足；后续不得把lease/rebind重新解释成stop/ack事实。 | closed |
| F-006 | Major | `runtime_control.rs:733-800,1041-1185,1281-1379,1534-1656`; tests `719-757,1225-1260,1574-1601` | denial/request digest锁定、callback exact/mutation/new-late优先级、ack payload identity与safe rejected audit继续通过；reopen/projection保留late callback authority且预算不变。 | 已满足。 | closed |
| F-007 | Major | `runtime_control.rs:2268-2358,2636-2654,3324-3450`; tests `1861-1936` | third repair已让pure fold重验callback namespace、effective cancellation/outcome、deadline equality和`lease_monotonic + (deadline_wall - lease_wall)`，六类validly re-sealed非法winner在Projection/reopen/Recorded replay均fail closed。但live `verify_lease`还要求settlement Clock的wall/monotonic不早于lease，live `verify_meter_snapshot`又把meter epoch绑定到同一decision Clock；pure `validate_callback_authority`不接收`settled_wall`，故不拒绝`lease_wall > settled_at`，`validate_persisted_meter_evidence`也只要求非空epoch，未与callback authority epoch相等。攻击者可把lease wall移到settlement之后并按正确wall/monotonic方程重新封印，或拼接另一process epoch的validly sealed meter evidence；Schema、digest与现有fold检查均可通过，Projection/reopen/Recorded replay会接受live path不可能提交的权威terminal。 | 在versioned pure reducer中重验`settled_wall >= lease_wall`及对应monotonic not-before关系，并在verified callback settlement中要求meter evidence process epoch等于callback authority/decision epoch；加入validly re-sealed future-lease-wall与cross-epoch-meter fixtures，三条恢复路径均fail closed。 | open |
| F-008 | Major | `pareto-protocol/src/runtime_control.rs:554-631,719-742,843-910`; `runtime_control.rs:1120-1149,1554-1594,2359-2479,2615-2660,3731-3823` | `CallbackAuthorityV1`持久保存reservation、producer、process epoch、lease wall/monotonic/deadline和完整lease fingerprint；settlement/ack/late payload、Projection/hash view携带该identity，late audits按序恢复。Live admission和pure fold都逐项绑定scope/operation/reservation/producer，unknown settlement也不再丢失authority。final content-addressed set为`c3e2fda5…`，既有五个published retained set仍完整。剩余semantic validation归F-007。 | 已满足；后续Provider/Tool不得获得lease constructor或改写authority。 | closed |
| F-009 | Major | tests `1043-1319,1861-1936`; `scripts/check_req0007_scope.py` | `model_sequences`现枚举23组request/ack/complete/cancel/timeout duplicate、order与triple command graph，并执行complete-vs-timeout及cancelled-callback-vs-timeout两组真实SQLite concurrent writer race；每步检查terminal不可改写、winner合法、reserved归零及`limit = available + reserved + gross - refunded`，再检查late exact retry零新增Event和Recorded replay等价。Reviewer独立运行53个focused与全workspace；新模型还发现并覆盖late event-ID exact retry，修复分支按既有terminal event ID选择`late-result-observed` payload，未见重复audit/核算回归。F-007残余负例仍需补，但bounded model本身的required proof已满足。 | 已满足。 | closed |

# Acceptance trace

| Acceptance | Result | Independent evidence |
|---|---|---|
| AC-01 | passed | closed Capability/Schema/version identities继续成立。 |
| AC-02 | passed | live default deny、retained envelope与forged reservation fold均成立。 |
| AC-03 | passed | child subset/full parent chain/revoke/expiry成立。 |
| AC-04 | passed | stable denial reasons和full-scope no-write矩阵通过。 |
| AC-05 | passed | multi-scope budget、hard/soft、trusted dimensions和decimal u64冻结。 |
| AC-06 | passed | single-writer multi-account atomic reserve与reverse winner无超卖。 |
| AC-07 | passed | verified/unknown/release/refund/meter violation核算通过。 |
| AC-08 | blocked | live identity及callback幂等成立，但pure fold未绑定meter/lease process epoch，见F-007。 |
| AC-09 | passed | request/probe/producer executor-return ack成立；rebind零append且不能自行确认。 |
| AC-10 | passed | cooperative/uninterruptible早期rebind都不确认；后者仅executor return或deadline timeout终止。 |
| AC-11 | passed | FakeClock、absolute/monotonic、restart rebind和deterministic timeout recovery通过，见F-003。 |
| AC-12 | blocked | 23组bounded graph与2组SQLite race成立；pure fold仍接受lease晚于settlement的非法winner，见F-007。 |
| AC-13 | passed | authorized late/duplicate/mutation保持终态与预算隔离，authority进入Projection。 |
| AC-14 | blocked | winner/namespace/deadline负例已补，但future lease/cross-epoch meter历史仍可fold，见F-007。 |
| AC-15 | blocked | 正常Projection/reopen/Recorded replay零执行零核算成立；非法authority历史仍未fail closed，见F-007。 |
| AC-16 | passed | tenant/user presence-value/workspace/run/actor/business-ID隔离及Event Store回归通过。 |
| AC-17 | passed | final set deterministic；既有五个published sets完整；DB v2不变；old reader测试通过。 |
| AC-18 | blocked | private稳定接口与rebind/ack语义成立；settlement pure authority validator仍不完整，见F-007。 |
| AC-19 | blocked | 53个focused、23组graph与2组writer race通过，但F-007两类关键重放负例仍缺。 |
| AC-20 | passed | workspace Kernel 121/1 ignored、Protocol 9+23/1 ignored；无依赖、DB migration或后续框架。 |

# Constitutional effect trace

| Path | Third re-review result |
|---|---|
| request → capability → envelope → reserve | live与pure reservation path闭合。 |
| lease → Fake Operation → meter → settlement | live chain闭合；pure fold未绑定settlement/lease时序及meter/lease epoch（F-007）。 |
| cancel → probe → ack → terminal | producer executor-return路径成立；rebind零append，uninterruptible只由return/timeout终止。 |
| deadline → rebind/timeout → recovery | restart monotonic rebind和到期timeout成立；只读rebind不append/dispatch。 |
| callback → duplicate/late → Projection | authority、redacted late audit、event-ID exact retry、idempotency和预算隔离成立。 |
| Event → fold → reopen/Recorded replay | winner语义改善；future-lease/cross-epoch-meter terminal仍可能成为权威（F-007）。 |

# Compatibility, scope, and regression

- `26b63ca..97bca8b`未修改SQLite `user_version=2`、ledger、writer epoch、Event Store DDL/trigger，
  未增加Cargo依赖，也未实现Hook、Provider、真实Tool/Effect、Sandbox、Agent Loop、Task DAG或WASM。
- 本轮未修改protocol/Schema；final set仍为`c3e2fda5…`，仓库现有五个此前published retained sets
  `68535…/7adfe…/dae028…/4ce387…/a1f960…`仍完整。Protocol retained-reader、content-address和生成测试通过。
- RunTask Projection/Snapshot golden随Manifest source SchemaSet identity变化；Reviewer复跑完整Projection/Snapshot/
  Replay、migration、old reader和Recorded no-effect回归，未发现REQ-0003..0006合同回退。
- scope helper继续比较批准baseline `6de3598`；DB常量、Cargo manifests、real I/O/no-sleep边界通过。

# Independent validation

Reviewer在Windows/PowerShell、offline、2026-08-27独立执行：

- `cargo test -p pareto-kernel event_store::runtime_control --all-features --offline`：53 passed。
- `cargo test -p pareto-protocol --all-targets --all-features --offline`：9 unit + 23 contract passed；
  1 observation ignored；publisher drift stderr来自预期负例，命令exit 0。
- `python -m unittest discover -s scripts/tests -p "test_*.py"`：21 passed。
- `python scripts/check_req0007_scope.py`：passed。
- `cargo test --workspace --all-targets --all-features --offline`：Kernel 121 passed/1 ignored；Protocol
  9 + 23 passed/1 ignored；命令exit 0。
- `cargo fmt --all -- --check`、workspace clippy offline `-D warnings`、Schema generation、
  `git diff --check`：passed；generator后Schema无diff。
- `python scripts/check_docs.py`在本Review及REVIEW-0001..0005 substantive freshness更新后passed：
  172 Markdown / 51 formal IDs。测试绿不能关闭F-007的authority/replay合同缺口。

# Re-review conditions

实现者不得自行关闭open Major。下一候选必须让live settlement与pure fold共享callback not-before lease
chronology，并绑定verified meter evidence与callback authority的process epoch；补充validly re-sealed
future-lease-wall与cross-epoch-meter历史，Projection/reopen/Recorded replay均须fail closed。focused re-review
须固定新的exact revision检查F-007；其仍open时保持`changes-requested`。

# Re-review history

- 2026-08-26：fresh independent implementation review exact `1b40e92be11e73a497ec821118b7cb4e0c1af1ce`；
  0 Blocker、9 Major，`changes-requested`。
- 2026-08-26：focused independent re-review exact `ab2fbc6d2e979ef12bcffd5df1cfe76b975a9684`；
  F-001/F-002/F-004/F-006 closed，F-003/F-005/F-007/F-008/F-009 open；0 Blocker、5 Major。
- 2026-08-27：second focused independent re-review exact
  `26b63ca2abb99bf3d6216d395994d006c1b3e2b5` and full `ab2fbc6..26b63ca` diff；
  F-003/F-008 closed，F-005/F-007/F-009 open；0 Blocker、3 Major，仍为`changes-requested`。
- 2026-08-27：third focused independent re-review exact
  `97bca8b7b34ceadd5ab4f8ad01f49e10b3377adb`，Runtime repair exact
  `693299cb1dbe5fa5c75729445bcd8f1054389731`，完整检查`26b63ca..97bca8b`；F-005/F-009 closed，
  F-007因settlement/lease chronology及meter/lease epoch仍open；0 Blocker、1 Major，仍为`changes-requested`。
