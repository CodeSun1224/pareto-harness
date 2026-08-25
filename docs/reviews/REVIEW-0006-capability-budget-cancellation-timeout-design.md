---
id: REVIEW-0006
title: REQ-0007 Capability、预算、取消与超时独立设计评审
status: changes-requested
owners: [independent-reviewer]
created: 2026-08-25
updated: 2026-08-25
links: [REQ-0007, SPEC-0006, RFC-0006, ADR-0007, REQ-0003, REQ-0004, REQ-0005, REQ-0006, ARCH-0002, ARCH-0003]
independence: independent
reviewed_revision: 05dd7ca7ece0d362aa96a6bb99f6c92e5d8999b2
open_blockers: 0
open_majors: 6
---

# Verdict

要求修改，不批准进入实现。固定提交
`05dd7ca7ece0d362aa96a6bb99f6c92e5d8999b2` 的设计已经正确选择单 Run
control stream、默认拒绝、owner signer、Capability 收窄、同事务多作用域 reserve、事件派生余额、
双时钟边界、终态不可逆、late-result 脱敏审计和 effect-free Recorded replay；但仍有 6 个
open Major。它们都位于后续 Hook、Effect、Provider、Tool、Sandbox、Agent Loop、Task DAG 和
WASM 会直接依赖的可信内核合同，不能由实现者在私有 helper 中自行补语义。

本评审为 fresh independent design review。Reviewer 未参与 REQ-0007 设计或实现，不采信
`.agents/work/.../ARCHITECTURE-REVIEW.md` 的 self-review 结论，也没有检查或引用提交后的
Runtime 实现声明。所有设计证据均来自 `git show 05dd7ca:<path>`；评审开始时工作树已有的
`crates/pareto-protocol/src/schema.rs` 和 `crates/pareto-protocol/src/types.rs` 未提交修改被明确
排除，未作为正面或负面证据。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `docs/specs/SPEC-0006-capability-budget-cancellation-timeout.md:15,35,49,112-125`; `docs/rfcs/RFC-0006-runtime-control-capability-budget-cancellation.md:94-107` | protected operation 的事务顺序会读取/fold lifecycle，却没有冻结 Run/Task 的状态准入矩阵。匹配 grant、未取消且有预算的请求因此在设计上仍可对 `created/paused/succeeded/failed/cancelled` Run 或 Task reserve 并 dispatch；control cancellation 与 lifecycle terminal 分离不能自动弥补这个绕过。该缺口违反可信内核的状态合法性和终态不可逆边界。 | 在 REQ/SPEC/RFC 中冻结 control 初始化、grant 管理、reserve/dispatch 分别允许的 Run/Task 状态；reserve 必须在同一 `BEGIN IMMEDIATE` 内对 exact lifecycle history 执行该 guard。增加所有 Run/Task 状态对负测，以及 lifecycle terminal/pause transition 与 reserve 的 two-writer 竞争，证明只有唯一合法提交顺序且 terminal 后无 dispatch。 | open |
| F-002 | Major | `docs/specs/SPEC-0006-capability-budget-cancellation-timeout.md:22,26,61-71,129`; `docs/rfcs/RFC-0006-runtime-control-capability-budget-cancellation.md:117-121,163-176` | cancellation request/ack 只要求记录 requester 并“对认证 principal exact 验证”，但没有定义谁可取消 Run、Task 或别人的 operation，也没有说明该权力来自 owner management authority、哪一种 Capability，还是 operation lease。`request_cancellation` 甚至未出现在冻结的 downstream private interface 中。exact scope/actor identity 不是取消 authority；同域低权限 actor 或 Gate 可造成拒绝服务或伪造 acknowledgement。 | 冻结 Run/Task/operation request 与 acknowledgement 的授权判定表、reason code 和 stable request API；若用 Capability，定义 exact resource/operation 与 delegation/revocation/expiry 交互；若 owner-only，明确 downstream Gate 只能 proposal。ack 必须绑定已批准 executor/lease 或 Kernel 事实。增加 owner/subject/issuer/无关同域 actor/跨域、ancestor propagation、撤销/到期及重复/乱序 ack 负测，未授权探测不得写目标 stream。 | open |
| F-003 | Major | `docs/specs/SPEC-0006-capability-budget-cancellation-timeout.md:22,25-27,51-57,73-77`; `docs/rfcs/RFC-0006-runtime-control-capability-budget-cancellation.md:111-113,142-151,165-176` | `SettlementCommand` 和 callback producer 被描述为 exact 验证，`ProtectedOperationRequest` 只说固定一个未定义的 callback contract；设计没有冻结 producer/evidence authority 如何从已批准 operation、lease 或受信 adapter 派生。知道同 aggregate 的 operation/callback ID 的主体可能提交 `unknown` usage，使 operation 终态并全额消费 reserve，或提交 late/rejected audit 污染历史。payload closure、ID 幂等和 scope exact 都不能替代 settlement authority。 | 定义版本化 callback contract：允许 producer/adapter identity、operation/reservation/lease binding、usage-evidence class、callback ID namespace、认证与失效规则；只有 Kernel deterministic meter 或显式受信 adapter 能构造 authoritative settlement admission。覆盖同域错误 actor、正确 actor 错 operation/reservation、stale/revoked lease、伪造 verified evidence、unknown callback、late callback、same-ID mutation和跨域无写入矩阵。 | open |
| F-004 | Major | `docs/specs/SPEC-0006-capability-budget-cancellation-timeout.md:61-71,85-87`; `docs/rfcs/RFC-0006-runtime-control-capability-budget-cancellation.md:123-140,163-176,198-212` | 设计声明没有 background scanner，timeout 只在下一次 poll/callback/recovery command 权威化；但事件/命令合同和 stable interface 都没有 timeout/recovery command，`cancellation_probe` 也未定义为可追加 timed-out settlement 的权威 writer。无 callback 的 hung/uninterruptible operation 或重启后的过期 pending reservation因此可永久保持 reserved，无法确定性形成 `timed_out`、unknown 保守核算和 late-result winner。 | 冻结显式 Kernel timeout/recovery admission：调用主体、确定性 command/event/callback identity、trusted wall/monotonic sample、锁内 refold、timed-out settlement 的 unknown/verified usage规则及无 callback 情形；或者收窄 AC-10/11/12，不再宣称 deadline 会在无重入时终结。测试 FakeClock deadline 前/相等/之后、进程 epoch 更换、wall regression、无 callback uninterruptible、reopen recovery，以及 timeout/cancel/completion 两连接竞态。 | open |
| F-005 | Major | `docs/specs/SPEC-0006-capability-budget-cancellation-timeout.md:13,21-27,137-141`; `docs/rfcs/RFC-0006-runtime-control-capability-budget-cancellation.md:25-40,153-161,230-236`; `docs/architecture/version-and-event-model.md` | 新 control SchemaSet 与 immutable `RunManifest.schema_set_ref` 的 exact 关系没有冻结。初始化 payload未显式固定 control source SchemaSet/limits，而兼容章节一方面发布新增 control bindings 的新 set，另一方面允许既有 Run 没有 control sequence-1。若旧 Manifest 可用新 set 初始化，Run 获得未在 Manifest 固定的行为版本；若必须 exact 等于 Manifest set，则哪些旧/新 Run 可初始化及其错误语义、rollback/retention规则未定义。`RuntimeControlProjectionV1` 的单一 source identity也依赖这个选择。 | 明确并测试唯一规则：首片可选择“仅 Manifest 已固定 control-capable set 的 Run 可初始化，control 每行 set/limits 必须 exact 等于 Manifest”，或通过新的受审 Manifest/aggregate-extension版本合同显式固定独立 control set；不得从 current/compatible set选择。覆盖四个 retained old set、新 set、known alternate/current substitution、row/Manifest mismatch、control stream内 set漂移、unknown major、reader/reducer retention与rollback。 | open |
| F-006 | Major | `docs/specs/SPEC-0006-capability-budget-cancellation-timeout.md:25,43-57,112-125,129-135`; `docs/rfcs/RFC-0006-runtime-control-capability-budget-cancellation.md:76-113,163-176,178-194` | reserve 依据请求中的 resource vector；请求路径又允许 untrusted policy/plugin proposal。设计只在 effect 返回后要求 verified actual `<= reserved`，却没有在 dispatch 前证明 reservation 是 Kernel/adapter 认可的最大用量。低报 request 可先通过 hard limit并执行，随后才得到 `invalid_usage`；此时真实资源已超预算，unknown 只消费低报的 reservation。该接口被声明为 REQ-0009/10/11/13/14/33 的稳定边界，因而不是单纯 Provider 后续细节。 | 为每个可 dispatch operation 固定受信、版本化的 resource envelope/meter contract；Kernel 在 reserve 前把 proposal 规范化为可证明覆盖该执行的上界，或在无法证明时拒绝 dispatch。Capability max、operation limit和全部 Run/Task/Actor accounts必须覆盖该 trusted vector。增加 proposal 低报/漏维度、meter 大于 reserve、unknown observation、partial/cancel/timeout和 malicious adapter 负测，证明 hard limit 在 effect 前而非 settlement 后执行。 | open |
| F-007 | Minor | `docs/specs/SPEC-0006-capability-budget-cancellation-timeout.md:143-166`; `.agents/work/active/REQ-0007-capability-budget-cancellation-timeout/PLAN.md:29-42` | AC 表形式上覆盖 AC-01..20，但 `cargo test <filter>` 即使命中 0 tests 也可成功，`source inspection rejects sleep` 与 DB/clock/scope inspection 没有可复现命令；F-001..F-006 对应的授权、生命周期、timeout和预算上界场景也不存在于现有命名矩阵。当前计划不能单独证明 AC 追踪非零且完整。 | 设计修订后更新 AC→test matrix，给每个新增规则具体测试名和层级；Validation记录每个 filter 的非零 test count，静态检查使用可复现命令，two-pool/model/reopen测试明确接受集合与禁止结果。 | accepted |
| F-008 | Note | `.agents/work/active/REQ-0007-capability-budget-cancellation-timeout/TASKS.md:12` | 固定提交把后续独立代码评审编号预写为 `REVIEW-0006`；本正式独立设计评审按当时 next available ID 已占用该编号。编号不影响产品合同，但后续不得覆盖本记录。 | 实施后的 fresh independent code review 使用下一个可用 ID（预期 `REVIEW-0007`），并在 Plan/Tasks 正常修订阶段更新引用；保留本评审历史。 | accepted |

# Constitutional effect trace

| Effect path | Design evidence at `05dd7ca` | Independent result |
|---|---|---|
| Capability issue/delegate/revoke | authenticated principal → persisted owner signer → parent-chain/subset check → `capability-issued/revoked` → pure fold → Projection/reopen | root/default-deny、child收窄、撤销/到期主链成立；cancellation/settlement等管理权未被这条链完整覆盖，见 F-002/F-003。 |
| Protected operation allow | request → persisted lifecycle/control → Capability → cancellation/deadline → atomic reserve Event → Fake boundary → settlement Event → Projection/recovery | 事件与预算事务边界合理；缺 lifecycle state guard和可信resource upper bound，故 operation boundary可在非法状态或低报预算后到达，见 F-001/F-006。 |
| Protected operation deny | integrity/isolation → decision → same-aggregate safe denial Event，cross-scope no append → audit counters → replay | 默认拒绝和跨域不写目标原则明确；management/callback主体的业务授权类别未闭合，见 F-002/F-003。 |
| Cancellation | requester → `cancellation-requested` → ancestor predicate → probe/ack/cancelled settlement → Projection/reopen | request/ack/settlement与lifecycle state分离是正确选择；requester和ack authority未定义，见 F-002。 |
| Deadline/timeout | injected Clock → absolute UTC + live monotonic lease → timed-out settlement → late audit → restart fold | 双时钟持久化边界正确；无权威 timeout/recovery writer路径，trace在 timed-out Event 前中断，见 F-004。 |
| Budget settlement/refund | callback/evidence → checked consume/release或unknown全额 → settlement Event；owner refund → correction Event → fold/projection | gross/refund/net equations和owner refund边界合理；callback admission及执行前用量上界未闭合，见 F-003/F-006。 |
| Late/duplicate/out-of-order | callback identity → AlreadyApplied/conflict或safe digest audit → counters only → replay | terminal后no-mutation原则明确；producer admission仍依赖未定义callback contract，见 F-003。 |
| Recorded replay | exact source control history → retained reader/reducer → Projection/digest；无executor/append authority | read/fold-only类型隔离满足replay零执行/零重复核算的设计要求；仍需实现证据证明counter/event/budget不变。 |

# Acceptance trace

| Acceptance | Review result | Evidence and gap |
|---|---|---|
| AC-01 | 设计满足 | Capability闭合字段、subject/scope/resource/operation/time/delegation/parent及“payload不等于authority”已冻结。 |
| AC-02 | 部分满足 | 默认拒绝、persisted admission和private authority成立；非法lifecycle state仍可进入operation path，见F-001。 |
| AC-03 | 设计满足 | root owner、delegate subset、full-chain revalidation、revocation/expiry及FakeClock目标均明确。 |
| AC-04 | 部分满足 | same-aggregate safe denial与cross-scope no-write明确；取消和callback authority缺口使“已认证业务拒绝”边界不完整，见F-002/F-003。 |
| AC-05 | 部分满足 | dimension、canonical unsigned、Run/Task/Actor/operation账本和hard/soft equations明确；requested vector不是可信上界，见F-006。 |
| AC-06 | 部分满足 | 单 `BEGIN IMMEDIATE` 多账户全有或全无与防超卖成立；缺lifecycle state guard和pre-dispatch trusted resource bound，见F-001/F-006。 |
| AC-07 | 部分满足 | consume/release/refund、gross/net及unknown保守规则明确；timeout无callback的settlement路径和actual超reserve前置防护未定义，见F-004/F-006。 |
| AC-08 | 部分满足 | observation非权威和ID冲突语义明确；谁能形成callback/evidence admission、如何阻止低报后执行未定义，见F-003/F-006。 |
| AC-09 | 未满足 | request/ack/settlement事实已分离，但三级取消的授权主体与稳定request API未冻结，见F-002。 |
| AC-10 | 部分满足 | cooperative/uninterruptible语义和互斥终态目标明确；无回调/无重入时timeout无法权威化，见F-004。 |
| AC-11 | 部分满足 | absolute UTC、process-local monotonic和restart新lease正确；缺失timeout/recovery command及其authority/event/accounting，见F-004。 |
| AC-12 | 部分满足 | deadline equality和SQLite commit-order规则明确；lifecycle terminal与reserve竞态、无callback timeout竞态未进入模型，见F-001/F-004。 |
| AC-13 | 部分满足 | exact duplicate/mutation/late digest no-mutation明确；callback producer authority未定义，见F-003。 |
| AC-14 | 设计满足 | 单control stream、sequence-1、exact validated full fold、无第二权威表及corruption fail-closed已冻结。 |
| AC-15 | 设计满足 | full-provenance Projection、完整历史Recorded replay与no executor/append/account mutation边界明确。 |
| AC-16 | 部分满足 | scope和业务ID exact isolation矩阵完整；同域低权限cancel/callback混用仍无授权判定，见F-002/F-003。 |
| AC-17 | 未满足 | retained sets、DB v2不变和unknown major fail-closed方向正确；Manifest与control source SchemaSet的exact关系未冻结，见F-005。 |
| AC-18 | 未满足 | downstream模块不得取得authority/raw transaction的原则正确；cancel、callback、timeout和trusted resource envelope接口尚不足以成为稳定消费边界，见F-002/F-003/F-004/F-006。 |
| AC-19 | 未满足 | 场景列表广，但缺口语义没有对应测试且非零命中/静态检查不可复算，见F-001..F-007。 |
| AC-20 | 仅为计划 | 设计声明保持protocol依赖方向、DB v2、REQ-0003..0006回归和无真实Provider/Tool；提交尚无实现，本评审不把计划命令当通过证据。 |

# Compatibility, permission, and isolation review

- 可信内核不可绕过的总体方向成立：control Event envelope恒为persisted Manifest owner，delegate/requester只在闭合payload中表达；公开Schema值、Projection和Replay均不取得append或dispatch authority。
- tenant、user presence/value、workspace、run、owner、subject、Task、stream和强类型业务ID的exact隔离设计充分；但“同一隔离域”只解决身份，不决定cancel、ack或settlement权限，F-002/F-003必须单独闭合。
- 单Run control stream与SQLite单writer适合首片原子reserve和terminal winner；lifecycle stream与control stream虽在同库，但必须明确在一个writer transaction内执行lifecycle状态guard，不能只读取Manifest/Task存在性。
- Control Projection/Snapshot/Replay没有第二权威表，Recorded replay也没有Operation/Effect executor，满足ARCH-0002的事件完整性与重放诚实方向。
- SQLite `user_version=2`、现有Snapshot DDL/trigger和四个retained SchemaSet不应改变；F-005要求先决定control source set怎样由immutable Manifest固定，否则兼容与rollback无法验证。
- quality、cost、latency仍被分开描述，且没有优化声明；但 hard budget质量门禁必须在dispatch前成立，不能用settlement后的`invalid_usage`代替，见F-006。

# Regression and test review

本轮是固定提交上的设计评审，不是实现或代码评审。未运行Runtime、Cargo、Schema生成或性能测试，也不接受
`.agents/work/.../VALIDATION.md` 的已有baseline作为REQ-0007行为证据。设计修订与后续实现至少需要：

- Focused：新增F-001..F-006所列状态、authority、callback、timeout、SchemaSet和resource-envelope表/模型测试；每个filter记录非零命中。
- Impacted：REQ-0005 lifecycle terminal/concurrency、REQ-0006 reducer/source identity/Recorded replay、Event Store transaction/idempotency/isolation与所有retained SchemaSet回归。
- Core：跨tenant/user presence-value/workspace/run/actor/Task全矩阵、同域低权限主体、two-pool race、real SQLite close/reopen、unknown/current substitution、replay零dispatch/append/accounting。
- Compatibility：明确Manifest/control source set规则后，证明四个旧set byte-identical、旧Run的受支持/拒绝行为、新set重复生成、DB v2 actual DDL/trigger bytes和RunTask reducer identity不变。
- 静态与治理：`python scripts/check_docs.py`、completion gates、可复现no-real-sleep/no-Provider/Tool/Effect/API/dependency检查及`git diff --check`。

本Review记录创建后独立执行的文档校验：

- `python scripts/check_docs.py`：failed。checker 报告 `REVIEW-0001` 至 `REVIEW-0005` 的
  reviewed revision stale；报告的 substantive paths 同时包含固定提交新增的REQ-0007
  Requirement/Spec/RFC/ADR和active work文件，以及评审开始前已有的两处未提交protocol代码。
  本Reviewer未修改旧Review、checker或被评审设计来掩盖该失败；复审批准前必须按项目规则恢复门禁，
  且不得把本次失败描述为通过。
- `git diff --check`：passed；该命令覆盖整个工作树，因此结果包含但不采信评审开始前两处用户Runtime改动的语义。

# Scope and unrelated changes

评审对象只包括固定提交中的REQ-0007 Requirement/Spec/RFC/ADR、指定active work记录、ARCH-0002/0003、
REQ/SPEC/RFC/ADR-0005/0006及其Event Store/协议accepted boundary。没有修改任何被评审设计文件、
Runtime代码、Schema、Cargo文件或用户已有改动。本轮唯一新增文件是本Review记录。

# Re-review conditions

同一 independent reviewer 可对仅含设计修订的精确commit做focused re-review，但F-001..F-006只能在
REQ/SPEC/RFC/ADR形成一致、可测试的durable contract后关闭；仅修改Plan、Tasks、self-review或实现代码不足以关闭。
复审必须固定revision、逐项核对required proof、重做AC-01..20 trace、恢复`check_docs.py`并确认0 open Blocker/Major后才可把本记录改为
`approved`。设计获批后，实施仍需由fresh independent reviewer以新的Review ID执行exact implementation code review，
本设计评审不能替代该门禁。

# Re-review history

- 2026-08-25：fresh independent design review of exact
  `05dd7ca7ece0d362aa96a6bb99f6c92e5d8999b2`；排除提交后的未提交Runtime改动。结论0 Blocker、
  6 Major、1 accepted Minor、1 accepted Note，changes requested。
