---
id: REVIEW-0007
title: REQ-0007 Capability、预算、取消与超时独立实现评审
status: changes-requested
owners: [independent-reviewer]
created: 2026-08-26
updated: 2026-08-26
links: [REQ-0007, SPEC-0006, RFC-0006, ADR-0007, REVIEW-0006, REQ-0003, REQ-0004, REQ-0005, REQ-0006]
independence: independent
reviewed_revision: 1b40e92be11e73a497ec821118b7cb4e0c1af1ce
open_blockers: 0
open_majors: 9
---

# Verdict

`changes-requested`。本次是 fresh independent implementation code review；Reviewer 未参与
REQ-0007 实现，也不是 REVIEW-0006 的设计 Reviewer。评审固定 exact revision
`1b40e92be11e73a497ec821118b7cb4e0c1af1ce`，检查完整实现差异
`6de3598..1b40e92`，独立读取 Requirement、Spec、RFC、ADR、独立设计批准、active work、源码、
Schema 与测试，并复跑 focused 和 workspace 门禁。

候选能通过现有 32 个 Runtime Control 测试与全仓 100 Kernel + 9 Protocol unit + 23 Protocol
contract 测试，但这些绿灯没有证明批准合同。发现 9 个 open Major：trusted operation/meter authority、
timeout command identity、process-epoch/Clock、Capability 语义、cancellation admission/API、幂等与 late
result、权威 fold、持久协议/Projection，以及 AC-19 测试充分性。REQ-0007 不得进入 verified/done；
实现者不得自行关闭这些 finding，修复后必须由本独立 Reviewer 在新的 exact revision 上 focused re-review。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `crates/pareto-protocol/src/runtime_control.rs:250-268,488-496`; `crates/pareto-kernel/src/event_store/runtime_control.rs:538-655,1641-1651,1961-1990`; `runtime_control/tests.rs:131-175,262-272,343-350,414-440` | Trusted operation contract 不是由 Manifest-pinned retained Kernel registry 解析，而是初始化 command payload 自带，Kernel 只检查非空/重复 kind-operation；调用者因此可自报 envelope、meter 和 producer revision。`SettlementCommand`又直接携带 `evidence_class`与`metered_usage`，持有 lease 的 producer 可以把任意向量声明成 `KernelMeterVerified`，甚至空向量，Kernel 没有不可伪造的 meter snapshot/handle。候选没有 Fake Operation 执行边界或“下一单位超界前停止”的 Kernel meter；`resource_envelope`测试只断言 Event 中保存了4。meter 超界还留下 pending reservation，而未按批准合同全额保守消费并记录 violation。插件/adapter可低报权威用量，AC-05/06/08/18 的预算与 confused-deputy 边界未成立。 | 增加按 exact source set/adapter revision 注册的 retained contract registry；初始化只能引用而不能定义 trusted contract。verified usage 只能由 crate-private Kernel meter snapshot 构造，producer report 始终 observation；meter violation按合同终结并保守核算。实现真实 Fake Operation + mediated meter vertical slice，负测低报、漏维度、空/伪造 verified、无contract、越界前停止、unknown触发权限与 replay dispatch counter。 | open |
| F-002 | Major | `runtime_control.rs:233-238,1059-1145,1218-1223,2261-2272`; `runtime_control/tests.rs:500-534` | Timeout recovery 没有实现批准的 durable deterministic command。`TimeoutRecoveryCommand`不保存 persisted `TimeoutKeyV1`、decision Clock sample、冻结的 verified/unknown evidence 或 command fingerprint；`recover_timeout`在每次调用时重新采样Clock。摘要域实际是 `pareto-runtime-control-v1\0timeout-recovery-command\0`，不是冻结的 `pareto.runtime-timeout-recovery.command.v1` canonical preimage，event ID 为`event_timeout-<hex>`而不是合同规定的`event_<hex>`。实现永远按 unknown 全额消费，不支持冻结的 verified partial。same-ID mutation无法由API表达；改变evidence/sample会形成different ID并在terminal后直接no-op，现有测试反而把这个合同违例断言成正确。commit-response-loss跨Clock变化的exact bytes retry没有实现。AC-11/12及REVIEW-0006 F-004 required proof回退。 | 用闭合 command 固定完整 key/sample/evidence/fingerprint/event ID，逐字段核对persisted operation；按批准domain和event ID提供golden。覆盖not_due不消费、verified partial/unknown、response loss exact command bytes、same-ID mutation在terminal检查前冲突、different-ID terminal no-op、reopen新sample以及并发唯一terminal。 | open |
| F-003 | Major | `runtime_control.rs:664-744,1993-2060,2275-2341`; `runtime_control/tests.rs:16-38,381-440,516-522` | Opaque lease 没有形成 live process-epoch/Clock 边界。`verify_lease`只重算无秘密hash且不检查当前 Clock epoch；当sample epoch与lease不同，settlement反而回退wall判断并可接受重启前的stale lease。Clock只检查字符串换算，不与reserve/先前可信sample比较回退；日期解析接受如不存在的月日；settlement Event 的`settled_at_utc`来自producer command而不是本次可信Clock。proposal callback namespace也未绑定trusted contract并未在callback验证。stale/revoked producer、wall regression、跨epoch completion和authoritative event time均可偏离AC-08/11/12。 | 将 producer registration/lease epoch 与当前 live authority exact绑定，重启后旧lease必须拒绝并由显式recovery重建；Clock canonical round-trip及non-regression fail closed；persisted decision time只能来自trusted sample；callback namespace/producer来自retained contract并逐项验证。增加epoch、wall regression、invalid calendar、spoofed occurred/settled time和namespace负测。 | open |
| F-004 | Major | `runtime_control.rs:330-449,1592-1639,1691-1806`; `runtime_control/tests.rs:275-322` | Capability 实现没有兑现批准的模型。`issue_capability`总要求parent，因此owner无法在初始化后签发root grant；初始化又强制initial subject必须owner，排除了设计允许的same-scope受托subject。delegation要求resource完全相等，禁止kind-only → exact ID收窄；authorization同样要求完全相等，使kind-only grant不能授权exact资源。operations/constraints/time没有完整canonical/sorted/unique校验，invalid UTC依赖字典序。所有missing/revoked/expired/constraint mismatch最终都压成`default_deny`，没有AC-04要求的稳定结构化reason表。`capability_table`测试只测两个整数map的`vector_lte`，没有Capability判定表。 | 实现owner root issuance和完整subset算法（Task、resource ID、operations、time、usage、delegation flag/depth），每次authorize重验full parent chain；产生`capability_missing/inactive/revoked/...`稳定安全reason。增加逐字段widen、kind-only narrowing、same-scope initial subject、root/child authority、invalid time/sort/duplicate、revocation cascade/expiry边界测试。 | open |
| F-005 | Major | `runtime_control.rs:747-891,1901-1959`; `runtime_control/tests.rs:443-498` | Cancellation admission与稳定API不完整。owner对Task cancel只通过Run management state检查，从不验证Task存在或Task非终态，因此可给不存在/已终态/未来创建的Task追加有效cancel并覆盖后续operation。没有只读cancellation probe入口；ack不拒绝已terminal operation，也没有Kernel recovery authority ack路径；`control-message-rejected`虽有Schema/fold分支却没有任何command写入。测试仅覆盖一个unauthorized Task actor和单一路径，未覆盖owner/subject/issuer/同域无关actor/跨域、Run/Task/operation全矩阵、future operation propagation、重复/乱序ack。AC-09/10/13/18未满足。 | Task cancel必须exact加载existing nonterminal Task；实现lease-only只读probe、producer/recovery ack authority、terminal/duplicate/out-of-order稳定处理及safe rejected audit，跨域/未授权保持no-write。补齐三级target、全principal、ancestor/future propagation、cooperative/uninterruptible和terminal ack矩阵。 | open |
| F-006 | Major | `runtime_control.rs:451-520,707-710,868-879,893-967`; `runtime_control/tests.rs:548-579` | 业务幂等不满足exact retry。拒绝事件没有建立request/command identity索引；同一exact proposal先因无grant或预算被拒后，grant/余额变化即可用原command转为reservation。late callback只把callback ID映射到event ID，不保存canonical callback digest；相同callback ID换新event ID并改变payload会直接返回旧`AlreadyCommitted`，而非`idempotency_conflict`。cancellation ack同样按`(cancellation,operation)`返回旧结果而不比较payload。终态callback不是统一settlement入口：caller必须另选`observe_late_result`，正常settlement晚到只返回`TerminalConflict`，无法落实批准的duplicate/mutation/late优先级。AC-08/12/13的重试、乱序和迟到隔离未成立。 | 持久化并fold command/callback canonical fingerprint；所有首次denied/allowed结果均锁定exact command identity。统一callback admission按same ID exact→AlreadyApplied、same ID mutation→conflict、new ID after terminal→authorized redacted late audit处理；ack/refund/recovery采用相同优先级。测试跨状态变化重试、different event ID/same callback ID mutation、response loss与无预算/状态副作用。 | open |
| F-007 | Major | `runtime_control.rs:1311-1589,1592-1651`; `runtime_control/tests.rs:620-637` | Recorded/reopen的pure fold只做Schema downcast和局部算术，没有重验权威语义。`capability-issued`不重跑delegation subset；revoke可引用不存在grant；reserved event不核对Capability decision、trusted contract、TimeoutKey、allocation scope/amount/hard limit；settlement不核对accounted/released等式和evidence authority；cancel/ack不核对request authority、target、reservation或terminal顺序。初始化不校验account scope/operation-limit唯一性、source reducer allowlist或contract registry。于是JSON-Schema-valid但非法的历史可被Projection/Replay接受，而AC-14明确要求illegal delegation/budget/cancel/row identity fail closed。`compatibility`测试只传空registry，没有任一非法历史负例。 | 把全部持久不变量实现为版本固定的pure reducer/validator，live append和reopen/replay共用；source reducer、operation contract与output reader必须exact retained allowlist。加入Schema-valid非法grant、allocation/equation、duplicate limit/account、timeout key、cancel/ack、settlement、unknown event/major与reader substitution corruption fixtures。 | open |
| F-008 | Major | `crates/pareto-protocol/src/runtime_control.rs:250-308,638-749`; `runtime_control.rs:2165-2258` | 已发布的v1协议和Projection缺少批准合同中的关键持久身份。sequence-1 payload没有RFC/SPEC冻结的lifecycle cursor；`TrustedOperationContractV1`缺exact source SchemaSet、adapter revision、required dimensions、envelope/meter policy与callback namespace；source contract缺accepted bindings/history/reducer implementation/output-reader identity。Projection没有history digest、初始化budget/clock/operation-contract identities，也只给cancellation count/boolean，不保留cancellation requester/reason/time/target/ack等可恢复状态。`runtime_control_projection_contract`仅用字符串contains检查少数字段，无法证明AC-14/15/17/18的版本化恢复与兼容。该SchemaSet一旦作为v1发布会固化不完整、难逆的跨需求接口。 | 在发布前补全或按显式新版本设计持久identity；Projection应完整、排序地表达初始化合同、cancellation/ack/deadline与history provenance，并用独立hash view golden绑定。证明旧四set byte-identical、新set exact reader/reducer、unknown old/new major fail closed、RunTask Projection/Snapshot output identity不回退。 | open |
| F-009 | Major | `runtime_control/tests.rs:275-669`; `scripts/assert_cargo_test_filter.py`; `scripts/check_req0007_scope.py` | AC-19 traceability是名义覆盖而非风险证明：`capability_table`/`usage_authority`/`deadline`是几行纯函数或恒真断言；lifecycle与terminal“race”均为顺序执行；`model_sequences`只是三整数恒等式；isolation只换workspace；compatibility只用空registry；recorded replay没有Fake Operation/counter，甚至候选没有Fake Operation执行实现。没有全lifecycle状态表、two-pool reverse winner、tenant/user presence-value/run/task/actor/ID矩阵、root/resource delegation表、producer/evidence/epoch矩阵、timeout ID golden/response loss/same-ID mutation、非法history、四旧set、crash pending recovery或bounded command model。non-zero helper只能证明测试名存在。scope helper把current文件与`git show HEAD`比较，因此在已提交候选上永远不能发现候选修改DB常量或Cargo依赖。32/32绿不能作为AC-01..20完成证据。 | 用风险表驱动的独立场景替换占位测试；每个filter仍非零且断言其命名风险。scope helper比较批准baseline或冻结golden而不是HEAD自身。补齐SPEC-0006 test matrix全部负例、真实并发、close/reopen、FakeOperation/FakeClock、Recorded replay counter和old retained contracts，再跑Focused→Impacted→Core。 | open |

# Acceptance trace

| Acceptance | Review result | Independent evidence |
|---|---|---|
| AC-01 | partial / blocked | 闭合ID和基础grant Schema存在；F-004/F-008显示签发、resource narrowing、canonical约束与持久identity不完整。 |
| AC-02 | partial / blocked | reserve路径在Kernel私有事务中且默认无匹配grant时deny；F-001/F-004/F-007显示trusted contract可由初始化payload定义、fold不重验，不能证明不可绕过。 |
| AC-03 | blocked | parent chain/revocation有基础实现，但root issuance、kind-only narrowing、初始subject及完整表缺失，见F-004。 |
| AC-04 | blocked | denial event结构化，但reason被压成`default_deny`且全隔离/安全audit矩阵未测，见F-004/F-009。 |
| AC-05 | blocked | u64 decimal、多scope account和hard/soft字段存在；trusted envelope/meter authority未成立，见F-001/F-008。 |
| AC-06 | partial / blocked | `BEGIN IMMEDIATE`和多account reserve能防当前单维fixture超卖；无受信执行上界、全有或全无多维负例及真正reverse race，见F-001/F-009。 |
| AC-07 | partial / blocked | verified/unknown/refund算术happy path存在；verified可由producer伪造、meter violation和全outcome矩阵不符，见F-001/F-007。 |
| AC-08 | blocked | lease/producer revision基础检查存在；usage authority与command/callback idempotency可绕过，见F-001/F-003/F-006。 |
| AC-09 | blocked | 三类target和owner/operation subject基础规则存在；Task admission、probe/ack/recovery及authority矩阵缺失，见F-005。 |
| AC-10 | blocked | outcome enum互斥且cancel可阻止success；无probe/执行边界，uninterruptible只测一个error，见F-001/F-005。 |
| AC-11 | blocked | absolute deadline与FakeClock存在；frozen timeout command、golden、partial evidence、response loss、epoch/regression不符合批准合同，见F-002/F-003。 |
| AC-12 | blocked | SQLite单writer和terminal只写一次基础成立；timeout优先级/ID mutation及真正并发模型未证明，见F-002/F-006/F-009。 |
| AC-13 | blocked | authorized late event不改account happy path通过；same callback mutation、统一late admission与rejected audit缺失，见F-005/F-006。 |
| AC-14 | blocked | state由单control stream fold；semantic corruption未重验，见F-007。 |
| AC-15 | blocked | projection/read-only method和digest存在；完整cancellation/deadline/history provenance与真实Recorded replay counter缺失，见F-008/F-009。 |
| AC-16 | partial / blocked | Event Store exact scope reader复用且workspace swap no-write通过；其余scope/actor/Task/business-ID矩阵未覆盖，见F-009。 |
| AC-17 | partial / blocked | 新内容地址set与四个旧set均保留，DB v2未改；source/reducer/output identity与old/unknown兼容测试不足，见F-007/F-008/F-009。 |
| AC-18 | blocked | 接口均crate-private；缺probe、retained registry、Fake operation/meter与批准timeout command，见F-001/F-002/F-005/F-008。 |
| AC-19 | blocked | helper报告命中且32个测试通过，但关键风险矩阵未实现，见F-009。 |
| AC-20 | regression passed, requirement blocked | 独立workspace回归通过且无依赖增长/DB migration；这不关闭REQ-0007自身9个Major。 |

# Compatibility, permission, and isolation review

- `EventStore`、authority constructors和Runtime Control entry points继续crate-private；control envelope actor固定Manifest owner，
  没有新增public raw-SQL或外部writer。这是正确基础，但F-001显示“private”本身不能替代retained contract和meter authority。
- Event Store scope query仍绑定tenant、user presence/value、workspace、run、owner actor与derived stream；独立workspace
  swap测试无写入。测试没有覆盖其余隔离维度和业务ID shadow，不能把单一负例推广为AC-16完整证明。
- `BEGIN IMMEDIATE` reserve和lifecycle pending guard在同一SQLite writer上，当前单维并发fixture没有超卖；
  lifecycle test只有“reserve先、transition后”的顺序，不包含transition先或two-pool barrier。
- SQLite constants、Cargo manifests/lock相对`6de3598`无差异；新set`a1f960...`为唯一新增set，四个retained set
  目录保持tracked。RunTask生产reducer代码未修改；测试显式使用retained `4ce387...` output set并通过全部旧回归。
- 没有Hook、Provider、真实Tool、Sandbox、Agent Loop、Task DAG、WASM或网络/进程依赖；scope没有越界。

# Constitutional effect trace

| Path | Observed implementation | Result |
|---|---|---|
| request → capability | persisted grant map与default deny | root/narrow/reason/fold缺口，F-004/F-007 |
| capability → trusted envelope → reserve | payload-carried contract → multi-account event | contract/meter可自报，F-001 |
| lease → Fake operation → meter → settlement | 没有Fake operation；producer command自带evidence/metered vector | authority chain断裂，F-001/F-003 |
| cancel → probe/ack → terminal | request/ack events存在；无probe，Task准入错误 | F-005 |
| deadline → recovery identity → timeout settlement | 调用时采样并派生非合同ID；unknown-only | F-002/F-003 |
| callback → duplicate/late | settlement与late是两个caller-selected入口 | mutation/late优先级不成立，F-006 |
| event → reopen/projection/replay | exact row reader + current fold | semantic corruption和完整provenance缺失，F-007/F-008 |

# Regression and test review

Reviewer独立执行（Windows PowerShell，offline，2026-08-26）：

- `cargo test -p pareto-kernel runtime_control --offline --no-fail-fast`：32 passed。
- `cargo test --workspace --all-targets --all-features --offline`：Kernel 100 passed/1 ignored；Protocol
  unit 9 passed；contract 23 passed；performance observation 1 ignored。Schema publisher drift stderr来自预期负例，
  总命令exit 0。
- `python -m unittest discover -s scripts/tests -p "test_*.py"`：20 passed。
- `cargo fmt --all -- --check`、workspace clippy offline `-D warnings`、`python scripts/check_req0007_scope.py`、
  `git diff --check`：passed。scope helper本身的HEAD-self-comparison缺陷记录为F-009，不能因命令绿而忽略。
- `cargo run -p pareto-protocol --bin generate_schemas --offline -- schemas`：passed，worktree不变；目录为四个retained
  set加新`sha256-a1f960...`。
- 初次`python scripts/check_docs.py`：仅REVIEW-0001..0005 freshness stale；Reviewer在逐项核对既有合同与上述
  全仓回归后，单独前移这些旧Review并记录substantive evidence。REQ-0007的9个Major不被旧Review freshness掩盖。

测试命令成功只能证明已写断言通过。F-009列出的空洞测试使当前VALIDATION中“AC-01..20已完成”的结论不成立；
现有green suite不得用于将本Requirement标为verified。

# Scope and unrelated changes

完整`6de3598..1b40e92`包含协议类型/Schema、新Kernel private module、lifecycle pending guard、Projection测试适配、
辅助脚本和REQ-0007 work/status更新；未发现第三方依赖增长、DB migration、真实外部effect或REQ-0008+实现。
新增约2400行单模块与约670行测试均属请求范围，但缺少approved contract中的执行/meter/recovery层，而不是出现
无关框架。`1b40e92`相对implementation commit`9f979f0`只更新review handoff/status，不改变产品行为。

# Re-review conditions

实现者应按F-001..F-009逐项提供修复commit和新增原始证据，不得用重命名filter、扩大reason文字或更新VALIDATION
代替行为修复。focused re-review至少重看新的协议/SchemaSet identity、Runtime diff、非法history fixtures、Fake operation
vertical slice、timeout golden/idempotency、full cancellation/isolation表、two-pool races和完整completion gates。任何
Blocker/Major保持open时，REQ-0007不得verified/done，也不得启动REQ-0008。

# Re-review history

- 2026-08-26：fresh independent implementation review of exact
  `1b40e92be11e73a497ec821118b7cb4e0c1af1ce`。0 Blocker、9 Major；`changes-requested`。
