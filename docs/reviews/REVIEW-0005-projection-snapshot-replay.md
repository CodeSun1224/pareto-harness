---
id: REVIEW-0005
title: REQ-0006 Projection、Snapshot 与 Replay 独立代码评审
status: approved
owners: [independent-reviewer]
created: 2026-08-25
updated: 2026-09-05
links: [REQ-0006, SPEC-0005, RFC-0005, ADR-0006, REQ-0003, REQ-0004, REQ-0005, REVIEW-0002, REVIEW-0003, REVIEW-0004]
independence: independent
reviewed_revision: 660cfca9e230f1440505c8e3bfd9a07bf17529ab
open_blockers: 0
open_majors: 0
---

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `crates/pareto-kernel/src/event_store/projection.rs`; `projection/test_support.rs` | 初审发现 source/output registry 顺序可改变 reducer/output identity，registration 未绑定 implementation，未来 retained reducer 会隐式执行当前共享函数。修复以纳入版本控制的 lifecycle source-contract key allowlist 固定 exact reducer ref、current output SchemaSet/limits和 `ReducerImplementation`；full/suffix 均按 registration implementation dispatch。未显式映射的 changed binding fail closed；仅增加无关member的source evolution按批准合同保持同一source contract。 | 证明 registry 顺序不改变解析；wrong output/limits、changed binding、missing registration拒绝；第二 implementation 真正走独立dispatch；已发布 `dae028...` 旧 Run 可用 retained reader重建、创建Snapshot并assisted-load，且输出固定current `4ce387...`。 | closed |
| F-002 | Major | `crates/pareto-kernel/src/event_store.rs`; `projection/snapshot.rs` | 初审发现v2 ledger只自证声明DDL，实际 Snapshot table/index 的CHECK、UNIQUE、type和index column/order可漂移；真实历史v1迁移与v2中途rollback证据不足。修复冻结首发v2 ledger checksum，并在open读取 `sqlite_master.sql` 对table/index/trigger作exact identity校验；v2 migration分六个可注入失败的原子步骤。 | 证明含两个真实旧事件的v1→v2保持全部旧row bytes/store identity，旧row epoch=1、v2 writer epoch=2且Projection可读；六个DDL阶段逐一rollback至完整v1后可再次迁移；CHECK/UNIQUE/type/index-order漂移均拒绝；held-open v1 writer拒绝、v2 writer成功并可reopen。 | closed |
| F-003 | Major | `docs/specs/SPEC-0005-projection-snapshot-replay.md`; `crates/pareto-kernel/src/event_store/projection{,/snapshot,/replay}.rs` | 初审的AC→test映射存在0-test authority命令、字段负例和真实并发/migration缺口，corruption helper还会在默认并行测试中跨连接DROP/CREATE trigger而间歇失败。修复新增非零authority、resolver/retention、invalid history、candidate、comparison、isolation、golden、migration和barrier并发矩阵；测试改为单一受控connection的exclusive transaction完成trigger改写/恢复。 | Focused测试逐项命中；Projection默认并行连续3次均35 passed/1 ignored，宽Event Store默认并行连续3次均68 passed/1 ignored；workspace 68 passed/1 ignored、Protocol 9 unit + 21 contract/1 ignored；全部核心、治理、Schema和diff门禁通过。 | closed |
| F-004 | Minor | `crates/pareto-kernel/src/event_store/projection.rs`; `projection/snapshot.rs` | 初审发现稳定错误合同未完全落地。修复已把candidate source/reducer/output mismatch在自记录校验前分类为`Incompatible`，并以matrix固定主要fallback分类；但 `UnsupportedEvent` 和error-form `NotComparable` 仍无构造路径，unknown event仍由权威history validation归为aggregate corruption。所有路径均fail closed且不泄漏其他scope数据，因此保持诊断/演进层Minor，不升级。 | 后续演进时删除不可能类别或使实现按Spec产生它们，并保持unknown/schema与compare分类的稳定负测。 | accepted |
| F-005 | Note | `.agents/work/archived/REQ-0006-projection-snapshot-replay/HANDOFF.md` | closure已把首段更新为done/archived，但后续实施进展段保留“`check_docs.py`等待reviewer关闭F-002/F-003”的旧实施期句子，与最终状态字面矛盾。该句在approved remediation/reviewer-record基线已存在；durable REQ-0006、REVIEW-0005和同文件首段均明确最终批准，且不影响Runtime或门禁结果，故不升级为Blocker/Major。 | 后续仅做文档清理时把该句显式标为历史状态或改为最终`check_docs.py`通过事实；不得改写原始review命令/结果。 | accepted |

# Verdict

批准。focused independent re-review 固定 exact remediation commit
`1d271549c2607f9c00377bdaa0fa999a131dafe3`；initial implementation为
`5c4f6e7f304c55fb61b6cc7e08d5bbe902b8d82c`，review-record commit为
`a94d756`，本轮逐项审查author remediation diff `a94d756..1d27154`，并对产品实质
`5c4f6e7..1d27154`做交叉核对。F-001、F-002、F-003的required proof均由源码、真实SQLite
fixture和独立执行关闭；最终0 open Blocker、0 open Major。F-004保持accepted Minor，不阻断
REQ-0006进入后续验证，但不能据此宣称错误分类债务已消失。

# Acceptance trace

| Acceptance | Review result | Independent evidence |
|---|---|---|
| AC-01 | 满足 | authority测试非零命中；所有Projection/Snapshot/Replay读取先经Event Store persisted-identity exact reader、row fingerprint与完整scope验证。 |
| AC-02 | 满足 | reducer purity/digest golden通过；显式source-contract allowlist、exact current output/limits与implementation dispatch消除registry顺序和共享当前函数替代。 |
| AC-03 | 满足 | empty/1/N及完整历史重建通过；unknown/major/schema、0/reuse/MAX和非法lifecycle矩阵均fail closed。 |
| AC-04 | 满足 | Snapshot record绑定schema/reducer/source/output/limits、cursor、history、scope/store与projection/snapshot digest；immutability和golden通过。 |
| AC-05 | 满足 | `BEGIN IMMEDIATE`内创建并提交；未提交insert rollback、UPDATE/DELETE禁止、reopen恢复通过。 |
| AC-06 | 满足 | candidate格式/版本/identity/digest/cursor矩阵分类后安全full fallback；权威prefix重读、chain mismatch与prefix corruption不会信任Snapshot。 |
| AC-07 | 满足当前切片 | Recorded replay只有权威read/fold入口且read-only；Simulated明确拒绝，API无Effect/Provider/Tool executor或append capability，counter负测通过。 |
| AC-08 | 满足 | same Run replay digest相等；equal/divergent/not-comparable及各provenance字段matrix通过。 |
| AC-09 | 满足 | Projection target、Snapshot lookup/record与compare覆盖store、tenant、user presence/value、workspace、run、agent、owner actor和stream；逐字段swap负测通过。 |
| AC-10 | 满足 | 两个store+barrier证明read transaction固定append horizon；Snapshot/create append、原子rollback与reopen恢复通过。 |
| AC-11 | 满足 | 冻结v2 checksum与actual sqlite_master identity、含历史v1 migration、六阶段rollback、held-open旧writer/v2 writer、drift和reopen证据完整。 |
| AC-12 | 满足 | 三个旧SchemaSet byte-identical；真实 `dae028...` 旧Run+Snapshot由retained source contract解析，输出固定current set；changed contract/missing reader拒绝。 |
| AC-13 | 满足 | Focused/Impacted/Core、默认并行稳定性、fmt/clippy、governance、docs、Schema generation与diff-check由独立reviewer复跑。 |
| AC-14 | 满足 | diff未引入Capability/Budget、Hook、Effect重执行、Provider/Agent Loop、Memory、Task/Context DAG、distributed Projection或remote Snapshot Store。 |

# Compatibility, permission, and isolation review

- `pareto-protocol`仍不依赖Kernel/sqlx，Cargo manifests/lock和第三方依赖在remediation中未变化；生成Schema与已发布retained set身份保持稳定。
- Projection/Snapshot/Replay入口仍为crate-private；没有raw Snapshot import、caller-selected reducer、Effect callback或绕过Event Store/lifecycle validation的入口。
- comparability、Projection digest、Snapshot query/record/hash view均绑定store、完整scope、stream、cursor、source/output identity、limits、reducer与history root。
- source contract只对批准的Manifest和四个lifecycle event binding建key；无关Schema member evolution不改变合同，任何受消费binding变化都需显式新registration，否则fail closed。
- v2保持首发ledger checksum兼容，同时以实际DDL identity阻止同名/同列数伪装；v1历史row不改写，旧连接受writer epoch trigger拒绝。

# Regression and test review

Reviewer在Windows/PowerShell、2026-08-25、offline依赖条件下独立执行：

- Focused authority、resolver、retained old Run/Snapshot、comparison、candidate/isolation、actual DDL drift、六阶段rollback、held-open writer与v1 migration：全部通过且每个过滤命令命中非零测试。
- `cargo test -p pareto-kernel projection:: --offline` 连续3次：每次35 passed、0 failed、1 ignored。
- `cargo test -p pareto-kernel event_store --offline` 连续3次：每次68 passed、0 failed、1 ignored；另将`event_store::tests::`窄组连续3次，每次15 passed。
- `cargo test --workspace --all-targets --all-features --offline`：Kernel 68 passed/1 ignored；Protocol 9 unit + 21 contract passed/1 observation ignored。
- `cargo fmt --all -- --check`、workspace clippy offline `-D warnings`、18个Python governance tests、`check_docs.py`、Schema generation byte identity、`git diff --check`：通过。
- protocol publisher drift fixture输出的“existing content-addressed schema set differs”是预期负例stderr，测试进程为0。

quality oracle以full fold/Recorded replay相等和projection digest golden固定；本地performance observation仍为无阈值ignored观察。没有Provider/token调用、成本数据或优化声明，因此本评审不批准任何质量、成本或延迟改善结论，也未发现本切片新增相关回归门槛。

# Scope and unrelated changes

remediation产品代码限于Event Store v2 migration、Projection/Snapshot/Replay及其测试辅助代码；另更新REQ-0006 work evidence和SPEC-0005的具体test traceability名称。Spec行为合同、AC、RFC/ADR、Schema、Cargo依赖和公开API均未改变；该文档变化仅把既有AC映射到新增的具体负测，不构成重新设计。未发现无关产品行为。

# Re-review history

- 2026-08-25：fresh independent review of exact `5c4f6e7f304c55fb61b6cc7e08d5bbe902b8d82c` against baseline `bb395ad78f762b53d5f486c742194dd8d551dc61`。结论0 Blocker、3 Major、1 accepted Minor，changes requested。
- 2026-08-25：focused independent re-review of exact `1d271549c2607f9c00377bdaa0fa999a131dafe3`，review-record `a94d756`，author remediation diff `a94d756..1d27154`。独立逐项核对F-001/F-002/F-003 required proof，复查F-004分类，复跑focused、默认并行3×、workspace/Core/治理/Schema门禁；F-001/F-002/F-003 closed，F-004保持accepted Minor，最终approved、0 Blocker、0 Major。
- 2026-08-25：final freshness-only independent confirmation of exact closure `907eee7295a7c3e7c2fa408a035c52d684f52fb4`，以approved remediation `1d271549c2607f9c00377bdaa0fa999a131dafe3`和review-record commit `14b5438`为基线。完整`14b5438..907eee7` diff只含REQ-0006 done/navigation/architecture implemented facts、validation/handoff/plan/tasks和active→archived move；`crates/`、Schema、Cargo、治理代码和既有Review finding bytes均无变化。实现事实与REVIEW-0005批准范围一致，REQ-0007/Effect/Provider/Memory/DAG仍明确未实现；F-001/F-002/F-003保持closed，F-004保持accepted Minor，归档Handoff的旧实施期句子记录为F-005 accepted Note；最终仍approved、0 Blocker、0 Major，freshness前移至exact `907eee7`。
- 2026-08-25：substantive freshness confirmation of exact candidate `cfa7a06c3588a6ad975a9511140d0984f5eb1b8f`。完整`907eee7..cfa7a06`无Projection/Snapshot/Replay Runtime、Schema、DB、Cargo或API实现变化；REQ-0007设计使用独立control stream/reducer/Projection，明确RunTask reducer仍只消费四类lifecycle binding、既有Snapshot identity/digest不变、Recorded control replay无executor/append/recovery writer。新control-capable set和RuntimeControl Projection尚未生成/实现且受REVIEW-0006 open Major阻塞。REVIEW-0005既有F-001..F-003保持closed，F-004/F-005保持accepted，approved与0 open Blocker/Major不变，freshness前移至exact`cfa7a06`。
- 2026-08-25：substantive freshness confirmation of exact candidate `a4e34785908207e622365250ae1466b85b4baecb`。`cfa7a06..a4e3478`无Projection/Snapshot/Replay Runtime、Schema、DB、Cargo或API实现变化，只冻结未来timeout recovery identity及reopen从pending Projection显式调用、既有terminal no-op的合同；Recorded replay仍无executor/append/recovery authority，RunTask reducer与Snapshot identity/digest不变。REVIEW-0005既有findings状态、approved与0 open Blocker/Major不变，freshness前移至exact`a4e3478`。
- 2026-08-26：substantive freshness confirmation of exact implementation candidate `1b40e92be11e73a497ec821118b7cb4e0c1af1ce`。RunTask Projection/Snapshot/Replay产品reducer、snapshot DDL/reader和replay effect boundary未改；测试fixture对新control-capable source set显式解析retained output set `4ce387...`，并更新与source provenance相关的golden。Reviewer逐行检查该适配，独立复跑35个Projection/Snapshot/Replay tests与全workspace，四个retained set、old reader、v2 migration、recorded/simulated no-effect均通过；无control writer进入REQ-0006 replay API。REVIEW-0007对新RuntimeControl Projection完整性保持open Major，不改变REQ-0006既有批准。REVIEW-0005保持approved、0 open Blocker/Major，freshness前移至exact`1b40e92`。
- 2026-08-26：substantive freshness confirmation of exact remediation candidate `ab2fbc6d2e979ef12bcffd5df1cfe76b975a9684`。RunTask reducer、Snapshot DDL/reader和Recorded replay effect boundary仍未修改；golden仅随最终control-capable Manifest source SchemaSet canonical identity更新。Reviewer复跑全workspace，既有Projection/Snapshot/Replay、old retained reader、migration、recorded/simulated no-effect均通过；RuntimeControl replay仍是只读Projection调用且Fake Operation dispatch counter为0。REVIEW-0007对新control pure fold和authority identity保留open Major，不改变REQ-0006已批准output/replay合同。REVIEW-0005保持approved、0 open Blocker/Major，freshness前移至exact`ab2fbc6`。
- 2026-08-27：substantive freshness confirmation exact `26b63ca2abb99bf3d6216d395994d006c1b3e2b5`。RunTask reducer、Snapshot DDL/reader与Recorded replay effect boundary未改；golden只随final Manifest source SchemaSet identity更新。RuntimeControl Projection新增late authority字段但replay仍只调用read/fold、无writer/executor；Fake Operation replay counter继续为0。Reviewer复跑Projection/Snapshot/Replay、old reader、migration和全workspace；均通过。新control settlement fold finding由REVIEW-0007阻塞，不改变REQ-0006既有批准。REVIEW-0005保持approved、0 open Blocker/Major，freshness前移至exact`26b63ca`。
- 2026-08-27：substantive freshness confirmation exact `97bca8b7b34ceadd5ab4f8ad01f49e10b3377adb`。`26b63ca..97bca8b`未修改RunTask reducer、Snapshot DDL/reader、golden、retained output reader或Recorded replay effect boundary；RuntimeControl replay仍只read/fold且model逐例验证零重复核算，Fake Operation replay零dispatch。Reviewer复跑完整Projection/Snapshot/Replay、old reader、migration及workspace；全部通过。REVIEW-0007 F-007阻塞新control非法authority history，不改变REQ-0006既有pure reducer、snapshot/reopen和recorded no-effect批准合同。REVIEW-0005保持approved、0 open Blocker/Major，freshness前移至exact`97bca8b`。
- 2026-08-27：substantive freshness confirmation exact `80249cc5c73575a3f92027f843cc657536905b9e`。RunTask reducer、Snapshot DDL/reader、retained output `4ce387…`和Recorded replay effect boundary未改；六个golden仅因Manifest source SchemaSet从未发布`c3e2fda5…`前移到`a95c824d…`而更新。RuntimeControl两类validly re-sealed非法history在Projection、Recorded replay和close/reopen均fail closed；normal replay仍零dispatch/append/accounting。完整Event Store/Projection/Snapshot/Replay及workspace通过，REQ-0006合同无回退。REVIEW-0005保持approved、0 open Blocker/Major，freshness前移至exact`80249cc`。
- 2026-08-27：substantive freshness confirmation exact `87be5391c40fdaa5b423c921747e7c941f7e2d42`。`f18f410..87be539`未修改RunTask/RuntimeControl reducer、Projection/Snapshot/Replay代码、Schema、golden、retained output reader或effect boundary；仅同步implemented facts和归档work。Recorded replay仍无writer/executor，REQ-0008未实现。REVIEW-0007 F-010只涉及归档Validation格式，不改变REQ-0006批准。REVIEW-0005保持approved、0 open Blocker/Major，freshness前移至exact`87be539`。
- 2026-08-27：substantive freshness confirmation exact `53338a836f646cdcefb6858ce07b0b0e8e12b11e`。`828f9aa..53338a8`只重组归档Validation历史叙述，对Projection/Snapshot/Replay、Schema、golden、reader和effect boundary零差异；Recorded replay和REQ-0006合同不变。REVIEW-0005保持approved、0 open Blocker/Major，freshness前移至exact`53338a8`。
- 2026-08-27：substantive freshness confirmation exact `8bb885bda678f5f785706e9eb335f472b5244974`。`53338a8..8bb885b`对Projection/Snapshot/Replay、Schema、golden、reader和effect boundary零差异；`ARCH-0004`继续要求版本化artifact、摘要和provenance，并禁止离线输出直接提交权威状态。Recorded replay与REQ-0006合同不变。REVIEW-0005保持approved、0 open Blocker/Major。
- 2026-08-27：substantive freshness confirmation exact `1748f69d01044a936727b3b5b7659882981b9129`。`8bb885b..1748f69`对Projection/Snapshot/Replay、Schema、golden、reader、DB和effect boundary零差异；RFC-0007要求Recorded replay不执行、Simulated/Reexecute显式经过Effect gate，并禁止外部cache/index成为第二事实源。REQ-0006合同不变，REVIEW-0005保持approved、0 open Blocker/Major。
- 2026-08-27：substantive freshness confirmation exact `b42ccdc3216f518ff60303cec20da92b78d190a1`。`2c80b89..b42ccdc`对Projection/Snapshot/Replay、Schema、golden、reader、DB和effect boundary零差异；accepted ADR保留Rust Replay admission，且真实边界仍须单独冻结Replay与Effect合同。REVIEW-0005保持approved、0 open Blocker/Major。
- 2026-08-28：substantive freshness confirmation exact `8507bae4ad979232e69ba282ee9c97ee71e3520e`。`754798d..8507bae`对现有RunTask/RuntimeControl Projection、Snapshot、Recorded replay、Schema、golden、reader和DB零差异；候选仅提出未来Hook Projection且继续要求Recorded零handler/writer/accounting、旧reducers不变。REVIEW-0010因新Hook point/atomic pair等4个Major阻塞其批准，不改变REQ-0006既有pure fold和no-effect replay合同。REVIEW-0005保持approved、0 open Blocker/Major。
- 2026-08-28：substantive freshness confirmation exact `3aee02adf8815466b02f51de247ae19922efc126`。`43f3a5b..3aee02a`对现有Projection/Snapshot/Replay代码、Schema、golden、reader和DB零差异；未来Hook Projection新增phase/lineage/pair fail-closed设计，Recorded仍零handler/writer/accounting。REVIEW-0010批准不替代实现测试，REQ-0006既有pure fold/no-effect合同无回退，REVIEW-0005保持approved、0 open Blocker/Major。
- 2026-08-28：substantive freshness confirmation exact `3318cbc6fe8bc8c9717a5a2b4aea1153f0d281d6`。`ea9633c..3318cbc`对Projection/Snapshot/Replay、Schema、golden、reader和DB零差异；ADR-0009忠实保留Recorded只读零handler/writer/accounting及旧reader语义。REQ-0006无回退，REVIEW-0005保持approved、0 open Blocker/Major。
- 2026-08-28：substantive freshness confirmation exact `e3d8d805b46fb4e1e25b23bc53bead71de730853`。`5546d1f..e3d8d80`对Projection/Snapshot/Replay、Schema、golden、reader和DB零差异；PLAN把Hook pure fold/recovery与Recorded零handler/append/accounting逐字段不变列为未开始任务和命名非零测试，且禁止writer/recovery authority。REQ-0006无回退，REVIEW-0005保持approved、0 open Blocker/Major。
- 2026-08-29：substantive freshness confirmation exact `84ce5a705edb20f268898938be4579f4946d5e4f`。新增Hook Projection由exact Event range pure fold并与Control同一MVCC horizon核对；Recorded入口类型上无handler/writer/timeout authority，测试证明零执行、零append、零核算。既有RunTask Projection/Snapshot/Replay合同与DDL未改，workspace回归通过；closure仅文档归档。REVIEW-0005保持approved、0/0。
- 2026-08-30：focused REQ-0009 design freshness re-review exact `aba3a33703e681c542fd58b32f3d0ae41cff369d`。候选仅提出未来Effect Projection、V2 inventory与fixed-horizon Recorded合同，并显式保留现有Projection/Snapshot/Replay、Inventory V1、Schema/reader与SQLite bytes；`crates/`和`schemas/`零变化。REQ-0009仍未接受/实现，REVIEW-0012 F-004保持open。REQ-0006合同无回退，REVIEW-0005保持approved、0/0。
- 2026-08-30：final REQ-0009 design freshness re-review exact `021b353d0efc923ef8739e3cb97d88f586c4fe06`。最小timeout文字修订不改Projection/Snapshot/Replay代码、Inventory、Schema/reader或SQLite；REQ-0009仍proposed/draft、F-004 open且未实现。REQ-0006无回退，REVIEW-0005保持approved、0/0。
- 2026-08-30：one-line REQ-0009 freshness exact `b7acbd82824d8410d432117c89be1bd56c8ce05c`。只改proposed recovery accounting；Projection/Snapshot/Replay/Inventory/Schema零变化，REQ-0009未实现。REQ-0006无回退，REVIEW-0005保持approved、0/0。
- 2026-08-30：REQ-0009 design-acceptance closure freshness exact `60cee6ed44d150185bf99ca3095a8ce803bcc0d3`。仅接受fixed-horizon Effect设计、创建ADR-0010并同步共享文档；现有Projection/Snapshot/Replay/Inventory/Schema及REQ-0006合同零变化，未声称实现。REVIEW-0005保持approved、0/0。
- 2026-08-30：REQ-0009 focused planning freshness exact `46772c7fbb30e82f0e8fd4fb50915e8414acaa65`。仅规划已批准fixed-horizon Projection/Inventory/Recorded proof；现有Projection/Snapshot/Replay/Inventory/Schema及REQ-0006合同零变化，未实现。REVIEW-0005保持approved、0/0。
- 2026-09-01：REQ-0009 closure freshness exact `62bc44e250587594912f7ef16b431be6b1c12103`。仅同步独立批准后的done/archive事实，无Projection/Snapshot/Replay runtime变化；原verdict/findings不变。
- 2026-09-05：Verified Procedure 路线 freshness re-review exact `660cfca9e230f1440505c8e3bfd9a07bf17529ab`。candidate对Projection/Snapshot/Recorded replay runtime、Schema、golden、reader和DB零差异；未来procedure-capable replay继续要求fixed source horizon、Recorded零Provider/Tool/Workspace执行/写入/核算，并把reexecute/simulated定义为有lineage的新Run。REQ-0006 pure fold与no-effect replay合同无回退，REVIEW-0005保持approved、0/0。
