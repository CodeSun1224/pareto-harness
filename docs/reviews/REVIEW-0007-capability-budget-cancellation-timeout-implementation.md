---
id: REVIEW-0007
title: REQ-0007 Capability、预算、取消与超时独立实现评审
status: approved
owners: [independent-reviewer]
created: 2026-08-26
updated: 2026-08-27
links: [REQ-0007, SPEC-0006, RFC-0006, ADR-0007, REVIEW-0006, FIX-0001, REQ-0003, REQ-0004, REQ-0005, REQ-0006]
independence: independent
reviewed_revision: 80249cc5c73575a3f92027f843cc657536905b9e
open_blockers: 0
open_majors: 0
---

# Verdict

`approved`。本记录由未参与实现、也未参与 REVIEW-0006 设计评审的 fresh independent Reviewer
维护。第四次 focused re-review 固定 exact candidate
`80249cc5c73575a3f92027f843cc657536905b9e`；实质 Runtime/Protocol/Schema repair 为
`cda43cd4f6c4c5a918259bb51e4739cc42e243a1`，其后仅有 Handoff/Validation 交接修订。Reviewer
独立检查完整 `97bca8b..80249cc` diff、批准合同、FIX-0001、协议/Schema、Runtime、Projection、
测试和原始门禁，不采纳实现者的 finding closure 声明。

本轮实质关闭最后的 F-007：`CallbackAuthorityV1` 必填持久化 decision monotonic sample，live
callback/ack/late path 都从实际 Clock decision sample 构造 durable authority；versioned pure fold
拒绝 decision wall/monotonic 早于 lease、非-timeout decision monotonic 到达或越过 deadline，且
verified meter evidence process epoch 必须等于 callback authority epoch。两类 validly re-sealed
`lease-after-settlement` 与 `meter-epoch-mismatch` 历史在 Projection、Recorded replay 和 close/reopen
三条入口均 fail closed。F-001 至 F-009 无回退；最终 0 open Blocker、0 open Major。

# Findings

| ID | Severity | Location | Latest independent disposition | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `runtime_control.rs:35-49,299-437,839-986,1990-2035,2754-2865`; tests `873-971` | Exact retained registry、Kernel meter/Fake Operation、超界前停止、unknown保守核算和Recorded replay零dispatch继续通过；final retained contract已前移到exact set `a95c824d…`。 | 已满足。 | closed |
| F-002 | Major | `runtime_control.rs:277-298,1717-1871,2071-2141`; tests `638-725,1157-1190` | Frozen timeout key/sample/evidence/fingerprint、批准domain、golden、not-due、verified partial、same-ID mutation、response-loss exact retry和different-ID terminal no-op继续成立。Schema identity变化后golden按完整preimage确定性前移。 | 已满足。 | closed |
| F-003 | Major | `runtime_control.rs:988-1039,3432-3567`; tests `1053-1136` | `rebind_operation_lease`从persisted absolute deadline和注入Clock建立当前epoch monotonic lease；拒绝initial/stale epoch、wall regression、terminal和到期operation，不append、不延长deadline、不dispatch。reopen测试证明旧lease拒绝、新lease可probe/callback、replay不执行；到期只能走timeout recovery。 | 已满足。取消ack是否可据此宣称已停止是独立F-005语义。 | closed |
| F-004 | Major | `runtime_control.rs:2915-3109`; tests `1008-1051` | owner root、same-scope initial subject、resource/Task/operation/time/usage/depth subset、parent revoke/expiry和稳定reason继续成立；新增not-before/expiry exact边界测试通过。 | 已满足。 | closed |
| F-005 | Major | `runtime_control.rs:988-1039,1429-1524`; tests `1053-1136,1204-1230` | `CancellationRecoveryFact`、`acknowledge_cancellation_recovery`与`kernel_recovery` authority已删除；rebind只建立当前process lease且零append。producer ack仍绑定exact operation/reservation/producer/epoch；uninterruptible早期rebind不确认，只有executor-return producer ack或absolute deadline timeout terminal可证明边界停止。focused及reopen/replay测试通过。 | 已满足；后续不得把lease/rebind重新解释成stop/ack事实。 | closed |
| F-006 | Major | `runtime_control.rs:733-800,1041-1185,1281-1379,1534-1656`; tests `719-757,1225-1260,1574-1601` | denial/request digest锁定、callback exact/mutation/new-late优先级、ack payload identity与safe rejected audit继续通过；reopen/projection保留late callback authority且预算不变。 | 已满足。 | closed |
| F-007 | Major | `runtime_control.rs:2349-2370,3387-3453`; protocol `runtime_control.rs:560-576`; tests `1866-1963` | `CallbackAuthorityV1`新增必填`decision_monotonic_millis`；live settlement/ack/late durable authority均写入实际decision sample。pure `validate_callback_authority`现重验decision wall/monotonic not-before；非-timeout settlement要求decision monotonic严格早于deadline；meter evidence要求process epoch等于callback authority epoch。validly re-sealed `lease-after-settlement`和`meter-epoch-mismatch` fixtures穿透Schema与seal后，Projection、Recorded replay和close/reopen都返回`AggregateCorrupt`。 | Reviewer独立检查diff与三入口helper，运行53个focused、121个Event Store impacted及workspace full全部通过。 | closed |
| F-008 | Major | `pareto-protocol/src/runtime_control.rs:554-631,719-742,843-910`; `runtime_control.rs:1120-1149,1554-1594,2359-2479,2615-2660,3731-3823` | `CallbackAuthorityV1`持久保存reservation、producer、process epoch、lease wall/monotonic/deadline、decision monotonic和完整lease fingerprint；settlement/ack/late payload、Projection/hash view携带该identity。final content-addressed set为`a95c824d…`；未发布`c3e2fda5…`被替换，既有五个published retained set仍完整。 | 已满足；后续Provider/Tool不得获得lease constructor或改写authority。 | closed |
| F-009 | Major | tests `1043-1319,1861-1963`; `scripts/check_req0007_scope.py` | `model_sequences`枚举23组request/ack/complete/cancel/timeout duplicate、order与triple command graph，并执行complete-vs-timeout及cancelled-callback-vs-timeout两组真实SQLite concurrent writer race；每步检查terminal不可改写、winner合法、reserved归零及预算方程，再检查late exact retry零新增Event和Recorded replay等价。第四轮另加入F-007两类validly re-sealed三入口负例；Reviewer独立运行53个focused与全workspace，未见重复audit/核算或模型回退。 | 已满足。 | closed |

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
| AC-08 | passed | live与pure fold均绑定callback authority、decision sample和meter process epoch；provider observation仍不成authority。 |
| AC-09 | passed | request/probe/producer executor-return ack成立；rebind零append且不能自行确认。 |
| AC-10 | passed | cooperative/uninterruptible早期rebind都不确认；后者仅executor return或deadline timeout终止。 |
| AC-11 | passed | FakeClock、absolute/monotonic、restart rebind和deterministic timeout recovery通过，见F-003。 |
| AC-12 | passed | 23组bounded graph与2组SQLite race成立；pure fold拒绝lease晚于settlement及非-timeout monotonic越界。 |
| AC-13 | passed | authorized late/duplicate/mutation保持终态与预算隔离，authority进入Projection。 |
| AC-14 | passed | Schema-valid、validly re-sealed future lease与cross-epoch meter历史均由exact versioned fold fail closed。 |
| AC-15 | passed | Projection/reopen/Recorded replay正常路径等价且零执行/零重复核算；两类非法authority历史三入口均拒绝。 |
| AC-16 | passed | tenant/user presence-value/workspace/run/actor/business-ID隔离及Event Store回归通过。 |
| AC-17 | passed | final set deterministic；既有五个published sets完整；DB v2不变；old reader测试通过。 |
| AC-18 | passed | private稳定接口、rebind/ack和callback durable authority共享同一decision/lease/epoch合同。 |
| AC-19 | passed | 53个focused、23组graph、2组SQLite race及F-007两类三入口重放负例全部通过。 |
| AC-20 | passed | workspace Kernel 121/1 ignored、Protocol 9+23/1 ignored；无依赖、DB migration或后续框架。 |

# Constitutional effect trace

| Path | Fourth re-review result |
|---|---|
| request → capability → envelope → reserve | live与pure reservation path闭合。 |
| lease → Fake Operation → meter → settlement | live与pure fold共同绑定decision wall/monotonic not-before、deadline和meter/callback epoch。 |
| cancel → probe → ack → terminal | producer executor-return路径成立；rebind零append，uninterruptible只由return/timeout终止。 |
| deadline → rebind/timeout → recovery | restart monotonic rebind和到期timeout成立；只读rebind不append/dispatch。 |
| callback → duplicate/late → Projection | authority、redacted late audit、event-ID exact retry、idempotency和预算隔离成立。 |
| Event → fold → reopen/Recorded replay | future-lease/cross-epoch-meter validly re-sealed terminal在三入口均fail closed；正常replay零dispatch/append/accounting。 |

# Compatibility, scope, and regression

- `97bca8b..80249cc`未修改SQLite `user_version=2`、ledger、writer epoch、Event Store DDL/trigger，
  未增加Cargo依赖，也未实现Hook、Provider、真实Tool/Effect、Sandbox、Agent Loop、Task DAG或WASM。
- final control-capable set为`a95c824d…`；未发布candidate `c3e2fda5…`被替换，仓库现有五个此前
  published retained sets `68535…/7adfe…/dae028…/4ce387…/a1f960…`仍完整且未改写。Protocol
  retained-reader、content-address、closed required field和生成测试通过。
- RunTask Projection/Snapshot六个golden只随Manifest source SchemaSet identity变化，retained output set
  `4ce387…`、reducer实现与history语义未变；Reviewer复跑完整Projection/Snapshot/Replay、migration、
  old reader和Recorded no-effect回归，未发现REQ-0003..0006合同回退。
- scope helper继续比较批准baseline `6de3598`；DB常量、Cargo manifests、real I/O/no-sleep边界通过。

# Independent validation

Reviewer在Windows/PowerShell、offline、2026-08-27独立执行 exact `80249cc5…`：

- `cargo test -p pareto-kernel event_store::runtime_control --all-features --offline`：53 passed。
- `cargo test -p pareto-protocol --all-targets --all-features --offline`：9 unit + 23 contract passed；
  1 observation ignored；publisher drift stderr来自预期负例，命令exit 0。
- `python -m unittest discover -s scripts/tests -p "test_*.py"`：21 passed。
- `python scripts/check_req0007_scope.py`：passed。
- `cargo test --workspace --all-targets --all-features --offline`：Kernel 121 passed/1 ignored；Protocol
  9 + 23 passed/1 ignored；命令exit 0。
- `cargo test -p pareto-kernel event_store --all-features --offline`：121 passed、1 observation ignored。
- `cargo fmt --all -- --check`、workspace clippy offline `-D warnings`、Schema generation、
  `git diff --check`：passed；generator后Schema无diff。
- `python scripts/check_docs.py`在本Review及REVIEW-0001..0005 substantive freshness更新后passed：
  172 Markdown / 51 formal IDs。

# Re-review conditions

独立实现评审门禁已满足：0 open Blocker、0 open Major。REQ-0007仍须由维护者按Plan执行最终
implemented-facts同步、freshness/全门禁复跑及`reviewing → verified → done`生命周期收尾；本Review不自行
改变Requirement状态，也不授权提前实现REQ-0008。后续若改变callback authority、meter evidence epoch、
deadline winner或retained SchemaSet解释，必须以新Requirement/Schema和独立评审处理，不能原位放宽pure fold。

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
- 2026-08-27：fourth focused independent re-review exact
  `80249cc5c73575a3f92027f843cc657536905b9e`，Runtime/Protocol/Schema repair exact
  `cda43cd4f6c4c5a918259bb51e4739cc42e243a1`，完整检查`97bca8b..80249cc`。F-007由durable
  decision monotonic、pure not-before/deadline和meter/callback epoch binding及两类三入口validly re-sealed
  负例关闭；F-001..F-009无回退。最终0 Blocker、0 Major，`approved`。
