---
id: REVIEW-0003
title: REQ-0004 SQLite append-only Event Store 独立代码评审
status: approved
owners: [maintainers]
created: 2026-08-23
updated: 2026-08-25
links: [REQ-0004, SPEC-0003, RFC-0003, ADR-0004]
independence: independent
reviewed_revision: 1748f69d01044a936727b3b5b7659882981b9129
open_blockers: 0
open_majors: 0
---

# Verdict

批准。第二次 focused independent re-review 的精确修复提交 `b7cf277f4232515bebbe15d6a237654336b95271` 有 0 个 open Blocker、0 个 open Major。F-005 已由 trusted authority SchemaSet/limits pins、registry exact resolution、retained set 跨重启读取、wrong-digest/alternate admitted-set substitution 负测，以及 RAII rollback、bounded busy 和跨重启同 event-id retry 证据关闭；本 reviewer 独立复跑 Focused/Core/doc/clippy/diff-check 均通过。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | `crates/pareto-kernel/src/event_store.rs:82-183`; `crates/pareto-kernel/src/lib.rs` | 初审发现 admission 从 envelope/查询自报身份构造。修复新增私有 `KernelAuthority`，append 使用其独立 scope/actor/stream 做协议边界验证，read 只能经 authority + registry 构造；scope/actor/stream mismatch 负测及外部 compile-fail doctest 均通过。当前切片仍不宣称 REQ-0007 完整 capability。 | 增加独立于 envelope/查询请求的可信 principal/target 输入及不可绕过的私有 constructor，负测证明 scope/actor/stream 任一自报差异被拒绝，并以外部 compile-fail/API-surface 测试证明裸 `ValidatedEvent` 或 scope 无读写入口。 | closed |
| F-002 | Major | `crates/pareto-kernel/src/event_store.rs:185-198,415-460,513-530`; `crates/pareto-kernel/src/event_store/tests.rs:377-432` | 初审发现 cursor 未绑定 horizon/keyset position。修复的私有 opaque `Cursor` seal 覆盖 kind、query binding、horizon 和全部三个 position 字段；read 先验证 seal，逐字段篡改、scope binding、并发较小 stream、reopen/VACUUM 测试均通过。authority API 未公开，因此调用者不能重算或构造内部 seal。 | 将全部 cursor state 纳入可验证的 opaque encoding/MAC 或等价不可伪造表示；增加 horizon 与三个 position 字段逐项篡改、跨 scope/kind/schema-set、重启/VACUUM/并发 append 的负测。 | closed |
| F-003 | Major | `crates/pareto-kernel/src/event_store.rs:329-353,464-510,532-555`; `crates/pareto-kernel/src/event_store/tests.rs:707-746` | 初审发现 causation/correlation 与 stored bytes 漂移漏检。修复在 open/read 双向比较 causation/correlation，并在 open 重算 envelope/SchemaSet/limits 三组 fingerprints；逐项离线 drift 的真实文件负测均 fail closed。 | open 与逐行 read 对所有持久化身份列及三个 JSON/fingerprint 对进行完整双向校验；增加 causation、correlation、fingerprint、SchemaSet/limits JSON 与 digest 单独漂移的真实文件负测。 | closed |
| F-004 | Major | `crates/pareto-kernel/src/event_store.rs:208-354`; `crates/pareto-kernel/src/event_store/tests.rs:573-622` | 初审发现 store identity 未固定且 v1 接受 application id 0。修复区分只创建新库的 `open` 与必须携带 expected store id 的 `open_pinned`，v1 严格要求 application id；合法 store 换入、application id 清零和 metadata identity 漂移测试均拒绝。 | open 返回并由可信配置持久绑定 expected store identity（或提供等价不可替换机制），v1 非零 application id 严格匹配；真实文件负测覆盖换入另一合法 store、清零/篡改 application id、identity metadata 漂移和 reopen。 | closed |
| F-005 | Major | `crates/pareto-kernel/src/event_store.rs:82-190`; `crates/pareto-kernel/src/event_store/tests.rs:230-275,675-720,840-909`; `.agents/work/active/REQ-0004-sqlite-event-store/VALIDATION.md` | 第二轮 remediation 将 SchemaSetRef 与 limits pins 移入 private trusted authority；append 拒绝 supplied set/limits 与 pin 不同，read 只能让 retained registry 按 authority exact ref 解析。持久旧 set 在 reopen 后可读；missing/wrong digest 与另一个已 admitted set 替换均 fail closed。RAII transaction drop 保持原值、busy 有界分类、成功提交后跨重启同 event-id retry 返回 `AlreadyCommitted`，共同证明取消/commit-response uncertainty 的规定恢复路径。当前 15 个真实 SQLite tests 与 compile-fail、Core regression 已由 reviewer 独立复跑。 | 按 SPEC-0003 traceability 补齐旧 Schema fixture、persisted-identity-driven registry、missing/wrong/alternate-compatible reader 负测，以及 cancellation/commit-uncertainty 可行的确定性证据；更新 VALIDATION 并重跑 Focused/Impacted/Core/full gates。 | closed |

# Acceptance trace

| Acceptance | Review result | Evidence |
|---|---|---|
| AC-01 | 满足当前 v1 合同 | 新库、pinned reopen、newer version、trigger drift、失败 migration rollback、application id 清零及 store swap 均有真实文件证据；首版无已发布旧 DB migration，不虚构历史升级。 |
| AC-02 | 满足 | authority 与 envelope 身份独立，API compile-fail；envelope/SchemaSetRef/limits 保存并重算 fingerprints，显式身份双向重验。 |
| AC-03 | 部分满足 | 固定 UPDATE/DELETE trigger 真实执行且 drift 被 open 拒绝；尚无 migration bypass/drop 的完整负测。 |
| AC-04 | 满足核心合同 | `BEGIN IMMEDIATE`、完整 non-NULL user key、sequence UNIQUE；两个独立 store/pool 竞争产生一个 commit、一个稳定 SequenceConflict，最终一行。user-present 隔离由 authority scope matrix 覆盖。 |
| AC-05 | 部分满足 | exact envelope/schema/limits fingerprints 支持 retry/conflict；测试只改变 correlation，未覆盖 scope/run/stream/sequence 与同 sequence 异 event 的完整矩阵。 |
| AC-06 | 满足 | 事务 RAII drop rollback、causation failure、fresh connection visibility、bounded busy 与 stable I/O 分类有真实证据；跨重启 identical event-id retry 返回 `AlreadyCommitted`，证明 commit response 不确定时的规定恢复路径。 |
| AC-07 | 满足 | 私有 authority reader、稳定排序、ordinal horizon 与 sealed opaque cursor 已覆盖篡改、scope mixing、页间 append、reopen/VACUUM。 |
| AC-08 | 满足主要合同 | tenant/user/workspace/run/agent/stream authority mismatch、payload shadow、causation rollback，以及所有持久 JSON identity/fingerprint drift 已覆盖。 |
| AC-09 | 满足主要 E2E | tempfile 真实 SQLite 覆盖 create/append/read/fresh connection/reopen/continue、幂等、sequence、两个 store 竞争、失败 rollback、busy 和 VACUUM。 |
| AC-10 | 满足 | authority 固定 exact SchemaSetRef/limits，retained registry 按 pin 解析；持久旧 set 跨重启可读，missing/wrong digest 与 alternate admitted set 替换均拒绝，stored row 再次要求 exact JSON/fingerprint 相等。DB migration 未改写 Event JSON。 |
| AC-11 | 满足现有原始记录 | VALIDATION 记录 protocol 9 unit + 17 contract、workspace tests 均通过；commit 依赖方向仍是 kernel -> protocol，protocol 未新增 SQLite/runtime/network 依赖。 |

# Compatibility, permission, and isolation review

- 依赖方向正确，`pareto-protocol` 未反向依赖 Kernel/SQLite；DB schema 与 Event Schema 数据列分离。
- 所有 Event Store 类型当前 module-private，外部 compile-fail 通过；私有 `KernelAuthority` 已把 authority target 与 envelope/query claim 分离，F-001 关闭。
- optional user 采用 `(user_present,user_id)` 非 NULL 规范编码并进入 sequence/query key，避免 SQLite NULL-distinct 绕过。
- Run/Stream SQL 均绑定 tenant、user presence/value、workspace、run、agent；payload 没有参与这些 query predicates。
- persisted reader identity 由 trusted authority pin 固定，retained registry 仅 exact resolve；读取再与行内 persisted SchemaSet/limits identity 双向比较。missing、wrong digest 与 alternate admitted set substitution 均有负测。
- causation append 要求同完整 scope/run/stream 已存在，失败在事务内 rollback；causation/correlation 及所有 stored byte fingerprints 漂移现已 fail closed。

# Regression and test review

- 更新 VALIDATION 记录 Focused 15 tests、doc compile-fail、Impacted、Core、governance、clippy 与 diff-check；其中 Core 行的 kernel 14 计数是第二轮 remediation 前的陈旧记录，本 reviewer 已在精确提交独立复跑并确认 kernel 15 与全 workspace 通过。
- 本 reviewer 第二轮独立复跑 `cargo test -p pareto-kernel --all-targets --all-features --offline`（15/15）、`cargo test -p pareto-kernel --doc --offline`（1/1）、kernel clippy `-D warnings`、`cargo test --workspace --all-targets --all-features --offline`（kernel 15、protocol 9+17，observation 1 ignored）与修复范围 diff-check，全部通过。
- crate 测试继续使用真实临时 SQLite 文件；remediation 增加 authority/isolation、migration rollback、identity swap、cursor field tamper、两个 pool 竞争、transaction drop/busy/fresh visibility 和 stored-byte drift。
- 本 review 不以 closure 声明替代测试断言；第二轮源码断言与 reviewer 独立执行已补齐 retained old set、wrong/alternate registry、authority-pinned reader，以及 RAII/commit-uncertainty retry 证据，F-005 关闭。
- 锁定的 `sqlx 0.8.6`、Tokio、SHA-256 与 tempfile 属于 approved SPEC/RFC 已识别依赖；未发现 protocol dependency growth 或网络/provider 调用。

# Scope and unrelated changes

精确提交的产品代码集中于新增 `pareto-kernel::event_store`、workspace dependency/lockfile 和 REQ-0004 设计/工作记录；未发现 Runtime、状态机、Projection、Replay executor、CLI 或通用 SQL escape hatch。`Cargo.lock` 大幅增长与 sqlx/Tokio 传递依赖一致。提交同时包含实施前设计记录，但均属于 REQ-0004 范围；未发现明显无关产品行为修改。

发布前 rollback 可 revert 新 crate/workspace member；若数据库已被使用，必须保留 v1 reader/migration 且不得降 `user_version` 或改写历史。本 review 已批准精确提交，后续仍需完成仓库 completion gates、durable docs 同步与归档。

# Re-review history

- 2026-08-23：fresh-agent 独立评审精确提交 `9aab8614a7083958659f5969f2caec38b7c597cb`；0 Blocker、5 Major，结论 changes requested。
- 2026-08-23：focused independent re-review 精确提交 `a7c73b317c57813a2a1d29b35ef09a2773c6e17b` 及 `9aab861..a7c73b3` 修复 diff；独立复跑 14 kernel tests、1 compile-fail doctest、kernel clippy 与 diff-check。F-001/F-002/F-003/F-004 closed；F-005 保持 open。最终 0 Blocker、1 Major，仍 changes requested。
- 2026-08-24：第二次 focused independent re-review 精确提交 `b7cf277f4232515bebbe15d6a237654336b95271` 及 `a7c73b3..b7cf277` 修复 diff；独立复跑 15 kernel tests、1 compile-fail doctest、kernel clippy、全 workspace tests 与 diff-check。F-005 closed；最终 0 Blocker、0 Major，批准。
- 2026-08-24：final lifecycle/archive freshness re-review 精确提交 `5ef949dd084b1e6ae82015f4c66adb8281aebf65`。`b7cf277..5ef949d` 仅将 REQ-0004 标为 done、同步 README/index/ARCH/EPIC/Requirement 实现事实、补全 Validation/Handoff/Tasks，并将 work 从 active 归档；未修改 `crates/`、Cargo manifests/lock、tests、schemas、scripts、skills 或 governance behavior。正式 REVIEW-0003 disposition 只作路径/生命周期事实同步，finding 状态与 0 Blocker/0 Major 未被实现者改写。批准与零 open findings 保持，freshness 前移至 exact `5ef949d`。
- 2026-08-24：final freshness-only re-review精确提交`b5850b76325bbc31825303215224d60c931e27c6`。自`5ef949d`起的REQ-0005 lifecycle消费者及Event Store helper变化已由独立REVIEW-0004审查，真实SQLite Event Store 33 tests通过，并证明v1 DDL/`user_version`、append-only authority、exact retained reader、事务边界及private raw-SQL surface不退化。exact closure diff `675e3f8..b5850b7`只同步durable docs/status并归档work，未修改`crates/`、Cargo manifests/lock、tests、schemas或依赖。REVIEW-0003保持approved、0 open Blocker/Major，freshness前移至exact `b5850b7`。
- 2026-08-25：substantive freshness re-review精确提交`1d271549c2607f9c00377bdaa0fa999a131dafe3`。REQ-0006把Event Store从v1原子迁移到v2 writer epoch和immutable Snapshot table；初审因actual DDL identity、真实历史迁移/rollback与并行稳定性证据不足而保持本Review stale。focused remediation现冻结首发v2 ledger checksum，对实际`sqlite_master` table/index/trigger SQL作exact验证，并以含两个事件的v1 fixture证明全部旧row bytes/store ID保留、六个DDL阶段失败完整rollback、held-open v1 writer拒绝、v2 writer成功及Projection/reopen恢复；CHECK/UNIQUE/type/index-order drift均fail closed。Reviewer将宽Event Store默认并行69-test selection连续运行3次（每次68 passed/1 ignored），另将15个store core tests连续运行3次，并运行workspace Core；全部通过。append-only authority、exact reader、cursor、隔离和事务合同未放宽；REVIEW-0003保持approved、0 open Blocker/Major，freshness前移至exact`1d27154`。
- 2026-08-25：exact closure `907eee7295a7c3e7c2fa408a035c52d684f52fb4` freshness-only re-review。`14b5438..907eee7`无Event Store/SQLite/Runtime/Schema/Cargo变化，只把已独立批准的v2 Projection/Snapshot事实同步到README、architecture、Epic、Requirement和归档证据；没有修改v1/v2 DDL、migration ledger、writer epoch、authority、cursor、隔离、事务或rollback合同。REVIEW-0003保持approved、0 open Blocker/Major，freshness前移至exact`907eee7`。
- 2026-08-25：exact candidate `cfa7a06c3588a6ad975a9511140d0984f5eb1b8f` substantive freshness re-review。完整`907eee7..cfa7a06`无`crates/`、SQLite、Schema、Cargo或API实现变化；REQ-0007仅设计复用v2 `events`和同一`BEGIN IMMEDIATE` writer，明确`user_version=2`、ledger checksum、writer epoch及Snapshot DDL/trigger不变，未增加raw SQL/authority表或第二事实源。实际Event Store合同和已批准证据未被修改；新command实现仍未开始且受REVIEW-0006阻塞。REVIEW-0003保持approved、0 open Blocker/Major，freshness前移至exact`cfa7a06`。
- 2026-08-25：exact candidate `a4e34785908207e622365250ae1466b85b4baecb` substantive freshness re-review。`cfa7a06..a4e3478`无`crates/`、SQLite、Schema、Cargo或API实现变化，只细化未来timeout recovery command在既有`BEGIN IMMEDIATE` writer内的identity/idempotency/terminal/due优先级。未增加权威表、raw SQL、第二事实源或修改v2 DDL/ledger/writer epoch/Snapshot；Event Store已批准事实不变。REVIEW-0003保持approved、0 open Blocker/Major，freshness前移至exact`a4e3478`。
- 2026-08-26：exact implementation candidate `1b40e92be11e73a497ec821118b7cb4e0c1af1ce` substantive freshness re-review。`event_store.rs`仅新增private runtime-control module；DB v2、DDL/index/trigger、migration ledger、writer epoch、Event Store authority/reader/cursor实现对比`6de3598`未改。Runtime复用events表和transaction-local private helpers，没有raw-SQL/public transaction escape hatch或第二权威表。Reviewer独立复跑全workspace，含Event Store migration/append/authority/isolation/concurrency/recovery回归全部通过。REQ-0007新语义由REVIEW-0007保持changes-requested，不改变REQ-0004已批准存储事实。REVIEW-0003保持approved、0 open Blocker/Major，freshness前移至exact`1b40e92`。
- 2026-08-26：exact remediation candidate `ab2fbc6d2e979ef12bcffd5df1cfe76b975a9684` substantive freshness re-review。独立逐项检查`1b40e92..ab2fbc6`：仍只复用v2 `events`与同一`BEGIN IMMEDIATE` writer；`user_version`、ledger checksum、writer epoch、table/index/trigger、Snapshot DDL和raw SQL可见性没有变化，也未增加第二权威状态表。workspace 118 passed/1 ignored，Event Store authority/isolation/migration/reopen回归全部通过。REQ-0007 pure-fold残留由REVIEW-0007 F-007阻塞，但不放宽REQ-0004既有row validation、append-only、transaction与DB兼容合同。REVIEW-0003保持approved、0 open Blocker/Major，freshness前移至exact`ab2fbc6`。
- 2026-08-27：exact second-repair candidate `26b63ca2abb99bf3d6216d395994d006c1b3e2b5` substantive freshness re-review。`ab2fbc6..26b63ca`继续只复用v2 `events`/single writer；DB version、ledger、writer epoch、table/index/trigger、Snapshot DDL和raw SQL authority无变化，无第二状态表或依赖增长。lifecycle checkpoints由validated Event pure fold派生而非持久表。workspace Kernel 120/1 ignored及Event Store migration/authority/isolation/reopen回归通过；REQ-0007 settlement fold残留单独由REVIEW-0007阻塞。REVIEW-0003保持approved、0 open Blocker/Major，freshness前移至exact`26b63ca`。
- 2026-08-27：exact third-repair candidate `97bca8b7b34ceadd5ab4f8ad01f49e10b3377adb` substantive freshness re-review。`26b63ca..97bca8b`只修改private Runtime Control reducer/commands/tests及文档；DB v2、ledger、writer epoch、table/index/trigger、Snapshot DDL、raw SQL authority和single `BEGIN IMMEDIATE` writer不变，无第二状态表、migration或依赖增长。两组真实SQLite writer race、Event Store authority/isolation/migration/reopen及workspace Kernel 121/1 ignored全部通过。REQ-0007 pure settlement authority残留由REVIEW-0007 F-007阻塞，不放宽REQ-0004 append-only、事务、row validation或恢复合同。REVIEW-0003保持approved、0 open Blocker/Major，freshness前移至exact`97bca8b`。
- 2026-08-27：exact fourth-repair candidate `80249cc5c73575a3f92027f843cc657536905b9e` substantive freshness re-review。`97bca8b..80249cc`只收紧private Runtime Control durable callback authority/pure fold并更新Schema/测试；DB v2、ledger、writer epoch、table/index/trigger、Snapshot DDL、raw SQL authority和single writer未改，无第二状态表、migration或依赖增长。Event Store 121 passed/1 ignored、full workspace及close/reopen非法历史负例通过，不放宽REQ-0004合同。REVIEW-0003保持approved、0 open Blocker/Major，freshness前移至exact`80249cc`。
- 2026-08-27：exact closure candidate `87be5391c40fdaa5b423c921747e7c941f7e2d42` substantive freshness re-review。`f18f410..87be539`对`crates/`、SQLite、Schema、Cargo、API和权限实现零差异，仅同步REQ-0007 done/archive事实；DB v2、ledger、writer epoch、DDL/trigger、single writer及append-only authority完全未触及。REVIEW-0007 F-010只阻塞归档Validation格式，不放宽REQ-0004合同。REVIEW-0003保持approved、0 open Blocker/Major，freshness前移至exact`87be539`。
- 2026-08-27：exact F-010 remediation `53338a836f646cdcefb6858ce07b0b0e8e12b11e` substantive freshness re-review。`828f9aa..53338a8`只有归档Validation历史证据结构变化，对Event Store/SQLite/Schema/Cargo/API/权限零差异；DB v2和append-only合同不变。REVIEW-0003保持approved、0 open Blocker/Major，freshness前移至exact`53338a8`。
- 2026-08-27：exact architecture clarification `8bb885bda678f5f785706e9eb335f472b5244974` substantive freshness re-review。`53338a8..8bb885b`对Event Store、SQLite、Schema、Cargo、API和权限实现零差异；新文档边界明确离线Python不得直接访问权威数据库，未来Worker同样不得依赖内核数据库布局。REQ-0004 append-only与事务authority不变。REVIEW-0003保持approved、0 open Blocker/Major。
- 2026-08-27：exact polyglot design remediation `1748f69d01044a936727b3b5b7659882981b9129` substantive freshness re-review。`8bb885b..1748f69`对Event Store、SQLite v2、DDL/trigger、writer epoch、Schema、Cargo、API和权限实现零差异；RFC-0007明确扩展不得共享权威数据库、直接append Event或取得transaction authority。REQ-0004 append-only、isolation与single-writer合同不变，REVIEW-0003保持approved、0 open Blocker/Major。
