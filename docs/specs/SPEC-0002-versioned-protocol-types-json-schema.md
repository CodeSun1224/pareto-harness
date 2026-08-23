---
id: SPEC-0002
title: 版本化协议类型和 JSON Schema 规范
status: approved
owners: [maintainers]
created: 2026-08-22
updated: 2026-08-22
links: [REQ-0003, EPIC-0002, RFC-0001, RFC-0002, ADR-0001, ADR-0002, ADR-0003, ARCH-0001, ARCH-0002, ARCH-0003, ARCH-0004]
---

# Behavioral contract

协议层是可信内核与所有生产者/消费者之间唯一的公开数据合同。它必须独立于数据库、网络、Provider SDK 和 Runtime 服务；公开 JSON 由版本化 Schema 定义，而不是由 Rust 内存布局偶然决定。输入先完成语法、Schema、身份、边界和交叉字段验证，成功后才可交给未来权威状态写入路径。

RFC-0002 已由维护者接受，独立架构评审经三次 focused re-review 关闭全部 finding，ADR-0003 记录 durable decision。本 Spec 已获批准；实现仍须按 Plan、Tasks、测试追踪和独立代码评审推进。

# Inputs, outputs, states, and failure behavior

## Contract surface

首个协议切片覆盖：强类型 ID 和 digest、Schema 标识/版本、Revision 元数据、`EventEnvelope`、`RunManifest`、`EvidenceRecord`、Schema-set manifest、结构化验证错误。后续 Requirement 可以以版本化扩展增加 payload 和业务类型，但不得绕过基础身份与验证规则。

## Input and output boundaries

- 输入：内存构造值或 UTF-8 JSON 文档，以及 Kernel 生成的 TrustedValidationContext。外部 JSON 先受 raw transport ceiling，解析后与 typed constructor 都按 JCS bytes 接受相同 semantic record/payload limits；禁止从全局默认值猜测版本或上下文。
- 输出：已验证的协议值、确定性的 JSON 表示、与实现同步的 JSON Schema、Schema-set manifest，或不包含秘密/原始不可信 payload 的结构化错误。
- 状态：协议层无外部可变状态，不读取时钟、环境变量、文件、网络、数据库或进程。时间和 actor 均由调用者显式提供。
- 失败：未知/不支持版本、Schema 不匹配、格式或范围错误、摘要不一致、身份域冲突和交叉字段不变量失败均 fail closed；失败值不能被标记为已验证。

## Call path and trust boundary

```text
untrusted JSON / typed constructor
  → syntax and size boundary (caller plus protocol limits)
  → select explicit schema identity
  → structural JSON Schema validation
  → typed deserialization
  → semantic and cross-field validation
  → verified protocol value
  → future Event Store / Manifest Registry / Evidence service
```

授权不由 JSON payload 决定。Kernel 从认证 principal、已 admission SchemaSet、已持久化 RunManifest 或 create-run command 派生 TrustedValidationContext。协议验证 tenant/workspace/run/agent、user presence/value、actor、stream 和 SchemaSetRef exact match；不支持 wildcard/subset/default。通过验证仍不等于授权，Kernel 在写入/效果前另做 capability 与 delegation 检查。

# Impact analysis

| Dimension | Finding | Evidence / response |
|---|---|---|
| Direct | 新增协议类型、JSON Schema、Schema-set manifest、golden/negative fixtures 和协议测试；实现阶段将首次建立 Rust workspace 的最小 `protocol` 纵切 | `ARCH-0001` 将 `protocol` 定义为独立模块；`ARCH-0003` 列出最小公共类型；当前 `rg --files` 无 Runtime/Cargo/Schema 文件，因此无现存代码可修改 |
| Indirect | REQ-0004 Event Store、REQ-0005 Run Manifest、REQ-0006 Replay、REQ-0007 Capability、REQ-0009 Effect、REQ-0010 Provider、REQ-0011 Tools、REQ-0020 Agent Message、REQ-0026 Evidence、REQ-0028 Behavior Revision 都会消费或扩展合同 | `docs/roadmap/requirement-backlog.md` 的依赖顺序；`ARCH-0001` 运行/演化数据流；后续实现不得复制基础 ID、digest、version 或 envelope 类型 |
| Call/permission | 当前无调用方；预期调用链为外部 JSON/构造器 → Schema → typed/semantic validation → 授权 → 状态写入。解析本身不能授予 capability | `ARCH-0002` 最小能力不变量；`RFC-0001` 要求策略只经 capability 接口；授权由未来 Kernel 调用方承担，协议错误不得泄漏秘密 |
| Data isolation | Run、Stream、Workspace、actor/Agent、用户/租户隔离键若缺失、默认或被 payload 覆盖会跨边界混用 | RFC-0002 要求 Event/Manifest/Evidence 的 tenant/workspace/run/agent profile 必需，user presence/value exact match；负向 fixture 覆盖 omit、actor delegation、Run/Stream swap、payload shadowing 和 wrong SchemaSet |
| API/schema | 首次公开合同会成为持久化和跨语言兼容基线；字段命名、required/default、unknown fields、数字/时间、enum、null 与扩展点都难以回退 | `ARCH-0003` 声明公开数据携带 Schema 版本；`ADR-0002` 选择 Serde + versioned JSON Schema。发布前以 golden schema 和 compatibility fixtures 冻结 |
| Persistence/replay | REQ-0003 不写数据库，但它定义未来 Event Log、Manifest、Evidence 的可持久化字节语义；非确定序列化或隐式默认会破坏 digest、重放和旧数据读取 | `ARCH-0002` 事件完整性/重放诚实；`RFC-0001` 已发布 Revision 不原位修改。测试固定 canonicalization/digest vectors 和旧新版本 fixture |
| Concurrency | 协议值应不可变且验证无共享可变状态；并发生产者仍可能重复 event ID、争用 sequence，这些原子性由 REQ-0004 处理 | 本需求只验证局部格式与明显边界，不能声称提供 Event Store 幂等/顺序保证；并发生成 Schema 必须确定且无工作目录竞态 |
| Security | 风险包括反序列化资源耗尽、递归/超大 payload、混淆类型或摘要域、错误回显秘密、unknown-field smuggling、confused deputy 与 TOCTOU | 协议层设结构/大小边界并域分离摘要；权限在未来状态写入前复核。fuzz/property 可作为实现计划候选，但不能替代确定性负例 |
| Performance | 强类型与验证增加 CPU、内存和二进制依赖；Schema 生成增加 PR 时间，Schema 文件增加存储 | `ARCH-0004` 依赖原则；优先复用 Serde 生态并记录新增依赖维护/许可证/体积。先建立基线，不声明未测量优化 |
| Operational/dependency | 首个 Rust workspace 会引入 toolchain、lockfile、跨平台 CI 和 Schema 漂移管理 | `ADR-0002` 已接受 Rust stable；Plan 要求锁定依赖、离线可重复测试和 Windows/Linux/macOS 门禁，不引入数据库或 Provider SDK |
| Documentation | `docs/index.md`、EPIC-0002、backlog 和 ARCH-0003 需要链接正式合同；批准后的字段语义应回写架构文档而非只留在 Work 记录 | 本轮先增加 Requirement/Spec 导航；实现/批准阶段同步 RFC/ADR 和架构合同 |
| Rollback | Spec 批准前可直接修改/撤回文档；公开 Schema 一经被事件持久化便不能靠删除或原位编辑回退 | 发布后保留旧 Schema 与 reader fixtures，以新增版本演进；实现尚未开始，因此当前回滚不涉及 Runtime 数据迁移 |

## Direct and indirect dependency graph

```text
REQ-0003 protocol identities + schemas
  ├─ REQ-0004 Event Store ─ REQ-0006 Projection/Replay
  ├─ REQ-0005 Run Manifest ─ REQ-0007 Capability/Budget
  ├─ REQ-0009 Effect Intent/Receipt
  ├─ REQ-0010/0011 Provider and Tool boundaries
  ├─ REQ-0020 Agent messages / REQ-0026 Evidence graph
  └─ REQ-0028 Behavior revisions and controlled evolution
```

## Regression scope derived from impact

- Focused：类型构造、Serde round-trip、Schema golden、manifest、规范化/digest vectors、错误代码和全部负向边界。
- Impacted：旧/新版本 reader compatibility、Schema consumer fixture、协议包依赖方向检查，以及 REQ-0004/0005 的最小编译契约（这些消费者存在后启用）。
- Core：事件身份、隔离键、权限不被 payload 授予、recorded/simulated replay fixture、跨平台一致 Schema 和全仓库治理门禁。
- Full：首个纵向 Runtime、真实持久化迁移和跨语言 SDK 尚不存在，本需求阶段记录为不适用；不得据此声称端到端兼容。

# Compatibility and migration

实现必须同时保存人可审查 Schema 与机器可读 Schema-set manifest。Reader 只能接受明确声明支持的版本；writer 只产生当前明确版本。兼容变更至少要由“旧 writer → 新 reader” fixture 证明；不兼容变更使用新主 Schema 身份并保留旧 reader/fixture。禁止原位修改已发布 Schema、靠默认值重释已持久化字段，或把数据库迁移与事件 Schema 迁移混为一体。

RFC-0002 与 ADR-0003 已冻结以下合同：RFC 8785 JCS 与完整 SchemaRef 域分离；闭合 Schema；保守兼容 checker；Kernel 派生 TrustedValidationContext；pinned SchemaSet/EventTypeRegistry；Run 固定 budget/limits/recording policy；执行后 BoundaryInventoryRevision；条件化 ReplayLineage；显式 scope。

# Test traceability

| Acceptance | Scope/layer | Scenario | Planned evidence |
|---|---|---|---|
| AC-01 | Focused static/unit | 公共类型可构造、强类型 ID 不可误用、JSON 不暴露 Rust 内部表示 | `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; protocol unit tests |
| AC-02 | Focused contract/golden | 连续两次生成 Schema/manifest 与版本控制文件逐字节一致 | Schema generation test plus `git diff --exit-code -- schemas/` |
| AC-03 | Focused negative/component | 缺字段、unknown、非法格式/enum/range/version/oversize fixture 均返回稳定错误类别 | named negative-fixture test target |
| AC-04 | Focused golden/property | 等价输入摘要一致；类型或 Schema 域不同则身份不同；canonical vectors 跨平台一致 | digest/canonicalization golden tests on Windows/Linux/macOS |
| AC-05 | Impacted compatibility | 旧 writer fixture 可被允许的新 reader 读取；breaking fixture 被兼容检查拒绝 | compatibility matrix test and checked-in fixtures |
| AC-06 | Core security/isolation | exact tenant/user/workspace/run/agent/actor/stream、EventTypeBinding、payload Schema/digest 冲突被拒绝 | omit/mismatch/delegation/swap/shadowing/event-binding negative tests |
| AC-07 | Core contract/replay | Manifest 固定版本/budget/limits/SchemaSet/recording policy；live 边界由事件记录并在终止后生成不可变 inventory revision；recorded_replay/reexecute 精确固定 source inventory | dynamic boundary, intent-without-receipt, cancellation/late receipt, empty finalization and replay exact-reference fixtures |
| AC-08 | Focused/Core | Evidence 字段完整，非法 verdict/freshness/空谱系失败，自信文本不能成为 verdict | evidence contract and negative tests |
| AC-09 | Core security/isolation | bootstrap admission、create-run 与 established-run 分离；exact context、错误 capability、自签/未 admission SchemaSet 和 payload shadowing fail closed | trust-root/capability/context malicious fixtures and boundary inspection |
| AC-10 | Impacted/Core | 协议包不依赖 Runtime/DB/Provider，全部跨平台和仓库门禁通过 | dependency-direction test, CI matrix, completion commands in Plan/VALIDATION |

# Approval and implementation entry

批准后重跑影响分析确认：仓库仍无 Cargo workspace、Runtime、Schema 或真实调用方；直接影响保持为新协议纵切，间接消费者仍为 REQ-0004/0005/0006/0007/0009/0010/0011/0020/0026/0028。权限、隔离、持久化、Replay、并发和回归范围未缩小；独立评审新增的 bootstrap、event binding、budget/limits、lineage、digest、compatibility 和 boundary finalization 风险均已进入本 Spec 与测试矩阵。若实现前工作树出现新调用方、CI 或依赖，必须再次更新影响矩阵。
