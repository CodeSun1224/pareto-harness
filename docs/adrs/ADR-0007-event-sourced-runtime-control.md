---
id: ADR-0007
title: 采用单 Run 事件溯源 Runtime Control 与双时钟 deadline 边界
status: accepted
owners: [maintainers]
created: 2026-08-25
updated: 2026-08-25
links: [REQ-0007, SPEC-0006, RFC-0006, ADR-0001, ADR-0003, ADR-0004, ADR-0005, ADR-0006, REVIEW-0006]
---

# Context

REQ-0007 首次让Kernel拥有通用Capability、budget ledger、operation cancellation和deadline。grant是否是bearer authority、预算保存为余额还是Event、多个scope怎样原子reserve、cancel request是否等于停止、wall/monotonic time怎样跨重启、late callback是否保留、Recorded replay是否能到达operation都会被Hook、Effect、Provider、Tool、Sandbox、Agent Loop、Task DAG和WASM消费，发布后难以回退。

初始架构专项自审追踪了request→authority→Event→Projection→recovery，但REVIEW-0006对`05dd7ca`的fresh independent review仍发现六个Major：lifecycle准入、取消authority、callback/evidence producer、无callback timeout recovery、Manifest/control SchemaSet binding与执行前resource upper bound未冻结。该评审否决进入实现，本ADR据此补齐决策。特别是Event Store v2现有reader要求envelope actor等于scope owner；本决策不以隐式DB变化突破该边界，也不把同scope身份误当management或settlement authority。

# Decision

接受RFC-0006的修订设计，但其实施准入取决于REVIEW-0006 focused re-review批准。每个Run使用由Run ID派生的单一control stream；sequence 1原子固定initial Capability、与Manifest budget revision绑定的plan、clock contract及Manifest-pinned control source contract。只有Manifest已固定新control-capable SchemaSet/limits且Run为`created`才能初始化；sequence-1、每个control row与reducer source必须exact匹配Manifest，旧set Run不得从current/alternate set后加control。授权、delegation/revocation、reserve/settle/refund、cancellation、terminal与late audit均以版本化Event追加；余额和状态只由exact validated history的retained pure reducer重建，不建立mutable authority table或Control Snapshot。

Capability默认拒绝。root管理authority来自persisted Manifest owner；策略、插件和公开grant只能request。child必须逐项证明subject/Task/resource/operation/time/usage/depth是父的收窄。每次新reserve重新验证完整parent chain、revocation和expiry。control Event envelope由Manifest owner作为Kernel authority signer；真实requester/subject/callback producer在闭合payload中exact验证，delegate不能直接append。

新的reserve只允许Run `running`与exact Task `running`。对已初始化control的Run/Task，进入paused/terminal的lifecycle transition同样在writer transaction中fold control并在pending operation存在时拒绝；reserve与transition无论谁先取得锁，后者都不能制造非法交错。已reserved operation仍能在后续lifecycle state结清，这不合并两套状态机。

预算使用canonical unsigned integer dimensions；不可信requested vector只作proposal。Kernel retained trusted operation contract产生并强制覆盖执行的resource envelope，Fake Operation资源访问由Kernel meter在越界前中止；无法证明上界就不dispatch。Run、Task、Actor和per-operation限制在一个`BEGIN IMMEDIATE`中以trusted vector全量检查并原子reserve。settlement把Kernel-meter verified actual转为gross consume并release差额；Provider自报不是authority，正确producer返回但meter未知时按trusted reserved保守consume。owner refund是引用settlement的幂等追加修正，只降低net，不删除gross或重开operation。

Run/Task/operation cancellation request、ack与operation settlement是不同事实。Run/Task request首片仅Manifest owner可发起；operation request允许owner或persisted exact subject；Gate/plugin只能proposal。probe只读，ack/callback必须来自reserve时批准的producer并携带绑定operation/reservation/process epoch的opaque lease，普通同scope actor或业务ID没有settlement authority。Run/Task request通过ancestor predicate覆盖当前及未来operation，不隐式修改RFC-0004 lifecycle state。cooperative boundary使用probe，uninterruptible boundary只能pending直到返回或timeout。success/failure/cancelled/timed_out互斥不可逆；deadline equality由timeout获胜，deadline前cancel/complete由SQLite序列化的首个合法commit决定。

活进程使用注入Clock产生的monotonic deadline；Event只持久化absolute canonical UTC deadline和wall observation。重启丢弃旧process tick，以current trusted wall判断过期并建立新monotonic lease；不持久化或跨process比较monotonic值。首片只有FakeClock且无background scanner，但提供只有Kernel recovery authority可构造的显式timeout/recovery command。reserve固定含scope、operation/reservation、deadline、policies及source/operation contract的`TimeoutKeyV1`；每次command以该key、canonical Clock sample和冻结usage-evidence fingerprint的domain-separated canonical SHA-256确定性派生command/event ID及fingerprint。`not_due`不消费ID；due event提交响应丢失时exact bytes重试返回原结果；same-ID mutation在terminal检查前冲突，不同ID晚到只返回既有terminal且无append/accounting。该入口锁内refold，到期后即使无callback也追加唯一timed-out settlement，并按verified partial或unknown全额规则结清reservation；reopen从Projection枚举pending后调用，不自动release/reexecute。

终态后只有仍能证明原producer/lease binding的exact callback retry或新late callback可进入目标aggregate；exact retry幂等，same ID mutation冲突，新late/out-of-order callback只追加脱敏digest audit，不改变operation、budget、cancellation、lifecycle或Effect。未授权producer不写目标。RuntimeControl Projection绑定store/full scope/stream/cursor/Manifest source/reducer/history；Recorded control replay只有read/reducer依赖，不接受Operation/Effect executor、timeout authority，不append或重复核算。

# Alternatives

- 继续owner-only或让Tool/Hook自判权限：产生默认允许和绕过点，拒绝。
- bearer Capability token或可编程ABAC首发：分别把bytes变authority或过早引入解释器/termination安全面，拒绝。
- mutable balance/state表、每scope独立stream或Provider事后自报扣费：分别形成双权威、跨stream原子复杂度或超卖/低报，拒绝。
- cancel request立即标终态、只用wall clock或持久化monotonic tick：分别虚假宣称停止、受时钟跳变影响或跨重启不可比较，拒绝。
- 丢弃late result或让Recorded replay走live service的dry-run：分别丢审计和留下重复效果入口，拒绝。

# Consequences

获得不可自授予/提权的默认拒绝、可审计grant chain、lifecycle非运行态无新dispatch、执行前可信资源上界、多scope防超卖预算、闭合cancel/callback/recovery authority、request/ack分离的取消、确定性terminal race、无callback显式timeout recovery、Manifest-pinned版本恢复和effect-free replay。代价是每个control及runtime-aware lifecycle command在SQLite单writer下完整fold lifecycle/control histories，多维canonical Event与operation contract增加存储和CPU，pending reservation在crash后需要显式reconciliation，unknown usage会保守高估，且首片没有background timeout、Control Snapshot、真实Provider/Tool或跨进程cancel transport。

新增control-capable SchemaSet保留既有sets；只有新Manifest exact固定该set的Run可初始化control，四个旧set Run行为不变。RunTask reducer仍只绑定四类lifecycle event，existing Projection/Snapshot identity不变。SQLite保持v2及现有DDL/trigger/checksum。首个control Event持久化后，代码rollback必须保留control reader/reducer、Schema与trusted operation contract；不能删Event、静默release pending reservation、把unknown usage改零、重释deadline或让late callback覆盖终态。

# Revisit triggers

- 需要多signer EventEnvelope actor、service principal、global admin或跨Run delegation。
- 单Run control fold/SQLite writer延迟达到可测瓶颈，需要Control Snapshot、account sharding或远程协调。
- 需要background timeout、跨进程cancel transport、真实Effect/Receipt reconciliation、Provider billing或自动refund。
- 需要resource glob/path/network selector、ABAC policy、signed bearer credentials或WASM capability handles。
- 必须改变budget equations、unknown usage policy、terminal precedence、clock persistence、late-result redaction、source reducer或replay effect boundary。

触发后必须新Requirement/RFC/ADR、forward migration、old grant/budget/event/reducer compatibility、权限/隔离/竞态/rollback负测及独立架构评审；不得原位重释已持久化history。
