---
id: REVIEW-0007
title: REQ-0007 Capability、预算、取消与超时独立实现评审
status: changes-requested
owners: [independent-reviewer]
created: 2026-08-26
updated: 2026-08-27
links: [REQ-0007, SPEC-0006, RFC-0006, ADR-0007, REVIEW-0006, FIX-0001, REQ-0003, REQ-0004, REQ-0005, REQ-0006]
independence: independent
reviewed_revision: 26b63ca2abb99bf3d6216d395994d006c1b3e2b5
open_blockers: 0
open_majors: 3
---

# Verdict

`changes-requested`。本记录由未参与实现、也未参与 REVIEW-0006 设计评审的 fresh independent
Reviewer 维护。第二次 focused re-review 固定 exact candidate
`26b63ca2abb99bf3d6216d395994d006c1b3e2b5`，Runtime repair 为其父提交
`ea751c83d26d45ed91aa3cfbc1b2cd2c316e334e`；Reviewer 独立检查完整
`ab2fbc6..26b63ca` diff、批准合同、FIX-0001、协议/Schema、Runtime、Projection、测试和原始门禁，
不采纳实现者的 finding closure 声明。

本轮实质关闭 F-003 和 F-008：未到期 reopen operation 可只读建立新 monotonic lease，
callback/settlement/late Event 与 Projection 也持久保留 producer/reservation/lease/process-epoch authority。
F-005、F-007、F-009 仍为 open Major：rebind fact 被错误地当作“执行边界已停止”的取消确认事实；pure
fold 仍未重验 settlement 的 cancellation/deadline/namespace 胜负语义；新增 `model_sequences` 也不是批准的
bounded command/concurrency model。REQ-0007 不得进入 `verified`/`done`，也不得启动 REQ-0008。

# Findings

| ID | Severity | Location | Latest independent disposition | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `runtime_control.rs:35-49,299-437,839-986,1990-2035,2754-2865`; tests `873-971` | Exact retained registry、Kernel meter/Fake Operation、超界前停止、unknown保守核算和Recorded replay零dispatch继续通过；final retained contract已前移到exact set `c3e2fda5…`。 | 已满足。 | closed |
| F-002 | Major | `runtime_control.rs:277-298,1717-1871,2071-2141`; tests `638-725,1157-1190` | Frozen timeout key/sample/evidence/fingerprint、批准domain、golden、not-due、verified partial、same-ID mutation、response-loss exact retry和different-ID terminal no-op继续成立。Schema identity变化后golden按完整preimage确定性前移。 | 已满足。 | closed |
| F-003 | Major | `runtime_control.rs:988-1039,3432-3567`; tests `1053-1136` | `rebind_operation_lease`从persisted absolute deadline和注入Clock建立当前epoch monotonic lease；拒绝initial/stale epoch、wall regression、terminal和到期operation，不append、不延长deadline、不dispatch。reopen测试证明旧lease拒绝、新lease可probe/callback、replay不执行；到期只能走timeout recovery。 | 已满足。取消ack是否可据此宣称已停止是独立F-005语义。 | closed |
| F-004 | Major | `runtime_control.rs:2915-3109`; tests `1008-1051` | owner root、same-scope initial subject、resource/Task/operation/time/usage/depth subset、parent revoke/expiry和稳定reason继续成立；新增not-before/expiry exact边界测试通过。 | 已满足。 | closed |
| F-005 | Major | `runtime_control.rs:988-1039,1429-1524`; tests `1053-1136,1204-1230` | producer lease ack仍绑定exact operation/reservation/producer/epoch；但新的`CancellationRecoveryFact`仅由“换一个process epoch并建立新lease”产生，未观察Fake/executor返回、cooperative stop或timeout terminal。`acknowledge_cancellation_recovery`仍允许在absolute deadline前用该fact追加`kernel_recovery` ack；测试在`00:00:21`、deadline `00:01:00`时把它断言为成功。rebind是Clock/lease事实，不是AC-10要求的“执行边界已停止”事实；对uninterruptible operation会再次把request误表述为confirmed stop。该新事实也未由批准RFC/ADR定义。 | recovery ack只能由受信执行边界真实返回/停止的closed fact，或到deadline的Kernel timeout/recovery command形成；单纯rebind不得确认。增加uninterruptible和cooperative的early-rebind-ack拒绝、executor-return ack、timeout terminal/ack、并发旧callback矩阵；如要引入新的reclaim语义，先回到RFC/ADR独立设计批准。 | open |
| F-006 | Major | `runtime_control.rs:733-800,1041-1185,1281-1379,1534-1656`; tests `719-757,1225-1260,1574-1601` | denial/request digest锁定、callback exact/mutation/new-late优先级、ack payload identity与safe rejected audit继续通过；reopen/projection保留late callback authority且预算不变。 | 已满足。 | closed |
| F-007 | Major | `runtime_control.rs:2203-2678`，尤其`:2359-2479`; tests `1330-1461` | reservation fold现在重验grant usage上界、prior cancellation、adapter/timeout contract、lifecycle cursor/state、warnings和deadline，关闭上一轮forged reservation证据。但`operation-settled` fold仍未共享live admission：它不检查callback ID属于persisted namespace，不检查`cancelled`必须已有effective cancellation，不拒绝cancellation已生效后的`succeeded/failed`，也不以wall deadline拒绝deadline前`timed_out`或deadline时/后的callback settlement。`callback_authority`的lease monotonic deadline也未复算`lease_monotonic + (absolute wall - lease wall)`。因此Schema-valid、向量/lease hash自洽的非法terminal history仍可被Projection/reopen/replay接受；新增测试只篡改epoch但不重算lease fingerprint，没有覆盖这些语义。AC-12/14的唯一合法winner仍可回退。 | pure versioned reducer与live settlement共用cancel/deadline/namespace/lease-equation invariant；加入validly re-fingerprinted wrong-namespace authority、cancelled-without-request、success-after-cancel、success-at/after-deadline、timeout-before-deadline fixtures，Projection/reopen/replay均fail closed。 | open |
| F-008 | Major | `pareto-protocol/src/runtime_control.rs:554-631,719-742,843-910`; `runtime_control.rs:1120-1149,1554-1594,2359-2479,2615-2660,3731-3823` | `CallbackAuthorityV1`持久保存reservation、producer、process epoch、lease wall/monotonic/deadline和完整lease fingerprint；settlement/ack/late payload、Projection/hash view携带该identity，late audits按序恢复。Live admission和pure fold都逐项绑定scope/operation/reservation/producer，unknown settlement也不再丢失authority。final content-addressed set为`c3e2fda5…`，既有五个published retained set仍完整。剩余semantic validation归F-007。 | 已满足；后续Provider/Tool不得获得lease constructor或改写authority。 | closed |
| F-009 | Major | tests `430-456,660-698,871-934,1053-1136,1377-1461`; `scripts/check_req0007_scope.py` | expiry/not-before、真实deadline、restart rebind、forged reservation和durable authority负测已补；52个focused测试均绿。但`model_sequences`仅对complete/cancel/timeout各执行一条固定happy path再写一次late callback，没有枚举命令顺序、非法转换、cancel/complete/timeout提交竞争或bounded state graph，不能证明AC-12/19要求的“模型化并发状态序列”。它也没有暴露F-005 early recovery ack和F-007 forged settlement负例，反而把early rebind recovery ack断言为正确。 | 建立真正bounded command model：枚举request/ack/complete/cancel/timeout/duplicate/late顺序与可观察writer winner，逐步断言唯一terminal、合法outcome、budget守恒、无重复effect/replay；并补F-005/F-007 required negatives。 | open |

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
| AC-08 | passed | live及durable producer/lease/meter identity与callback幂等通过，见F-008。 |
| AC-09 | blocked | request/probe/producer ack成立；rebind不能证明停止却可写recovery ack，见F-005。 |
| AC-10 | blocked | uninterruptible只能pending的批准语义被early rebind recovery ack破坏，见F-005。 |
| AC-11 | passed | FakeClock、absolute/monotonic、restart rebind和deterministic timeout recovery通过，见F-003。 |
| AC-12 | blocked | live SQLite race基础存在；pure fold winner语义和bounded model仍缺，见F-007/F-009。 |
| AC-13 | passed | authorized late/duplicate/mutation保持终态与预算隔离，authority进入Projection。 |
| AC-14 | blocked | reservation validator已闭合，但非法settlement history仍可fold，见F-007。 |
| AC-15 | passed | complete Projection、reopen/rebind与Recorded replay零执行/零核算通过。 |
| AC-16 | passed | tenant/user presence-value/workspace/run/actor/business-ID隔离及Event Store回归通过。 |
| AC-17 | passed | final set deterministic；既有五个published sets完整；DB v2不变；old reader测试通过。 |
| AC-18 | blocked | private stable接口基本齐全；recovery ack语义及settlement pure validator仍不稳定，见F-005/F-007。 |
| AC-19 | blocked | 52个测试通过，但批准的bounded model与关键负例缺失，见F-009。 |
| AC-20 | passed | workspace Kernel 120/1 ignored、Protocol 9+23/1 ignored；无依赖、DB migration或后续框架。 |

# Constitutional effect trace

| Path | Second re-review result |
|---|---|
| request → capability → envelope → reserve | live与pure reservation path闭合。 |
| lease → Fake Operation → meter → settlement | live chain与durable authority闭合；settlement pure winner validation仍缺（F-007）。 |
| cancel → probe → ack → terminal | producer return路径成立；rebind fact被误作stop confirmation（F-005）。 |
| deadline → rebind/timeout → recovery | restart monotonic rebind和到期timeout成立；只读rebind不append/dispatch。 |
| callback → duplicate/late → Projection | authority、redacted late audit、idempotency和预算隔离成立。 |
| Event → fold → reopen/Recorded replay | reservation/authority provenance改善；非法terminal Event仍可能成为权威（F-007）。 |

# Compatibility, scope, and regression

- `ab2fbc6..26b63ca`未修改SQLite `user_version=2`、ledger、writer epoch、Event Store DDL/trigger，
  未增加Cargo依赖，也未实现Hook、Provider、真实Tool/Effect、Sandbox、Agent Loop、Task DAG或WASM。
- 未批准中间set `195669…`被最终set `c3e2fda5…`替换；仓库现有五个此前published retained sets
  `68535…/7adfe…/dae028…/4ce387…/a1f960…`仍完整。Protocol retained-reader、content-address和生成测试通过。
- RunTask Projection/Snapshot golden随Manifest source SchemaSet identity变化；Reviewer复跑完整Projection/Snapshot/
  Replay、migration、old reader和Recorded no-effect回归，未发现REQ-0003..0006合同回退。
- scope helper继续比较批准baseline `6de3598`；DB常量、Cargo manifests、real I/O/no-sleep边界通过。

# Independent validation

Reviewer在Windows/PowerShell、offline、2026-08-27独立执行：

- `cargo test -p pareto-kernel runtime_control --all-features --offline --no-fail-fast`：52 passed。
- `cargo test -p pareto-protocol --all-features --offline`：9 unit + 23 contract passed；1 observation ignored；
  compile-fail doctest passed。publisher drift stderr来自预期负例，命令exit 0。
- `python -m unittest discover -s scripts/tests -p "test_*.py"`：21 passed。
- `python scripts/check_req0007_scope.py`：passed。
- `cargo test --workspace --all-targets --all-features --offline`：Kernel 120 passed/1 ignored；Protocol
  9 + 23 passed/1 ignored；命令exit 0。
- `cargo fmt --all -- --check`、workspace clippy offline `-D warnings`、Schema generation、
  `git diff --check`：passed；generator后Schema无diff。
- `python scripts/check_docs.py`在本Review及REVIEW-0001..0005 substantive freshness更新后passed：
  172 Markdown / 51 formal IDs。测试绿不能关闭F-005/F-007/F-009的合同缺口。

# Re-review conditions

实现者不得自行关闭open Major。下一候选必须以批准合同而不是重命名fact修复F-005；让live settlement与pure
fold共享cancel/deadline/namespace/lease equation；并实现真正bounded command/concurrency state model。
focused re-review须固定新的exact revision逐项检查F-005/F-007/F-009；任一仍open时保持`changes-requested`。

# Re-review history

- 2026-08-26：fresh independent implementation review exact `1b40e92be11e73a497ec821118b7cb4e0c1af1ce`；
  0 Blocker、9 Major，`changes-requested`。
- 2026-08-26：focused independent re-review exact `ab2fbc6d2e979ef12bcffd5df1cfe76b975a9684`；
  F-001/F-002/F-004/F-006 closed，F-003/F-005/F-007/F-008/F-009 open；0 Blocker、5 Major。
- 2026-08-27：second focused independent re-review exact
  `26b63ca2abb99bf3d6216d395994d006c1b3e2b5` and full `ab2fbc6..26b63ca` diff；
  F-003/F-008 closed，F-005/F-007/F-009 open；0 Blocker、3 Major，仍为`changes-requested`。
