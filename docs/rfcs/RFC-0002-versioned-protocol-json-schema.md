---
id: RFC-0002
title: 版本化协议、规范化 JSON 与 Schema 兼容合同
status: accepted
owners: [maintainers]
created: 2026-08-22
updated: 2026-08-22
links: [REQ-0003, SPEC-0002, RFC-0001, ADR-0001, ADR-0002, ADR-0003, ARCH-0002, ARCH-0003, ARCH-0004]
---

# Summary

为可信内核建立语言无关的首个公共协议基线：JSON Schema Draft 2020-12 描述闭合数据合同；所有持久化顶层记录显式携带 Schema 与隔离作用域；RFC 8785 JCS 产生规范化 JSON；SHA-256 在类型和 Schema 域内生成 digest；reader 只接受显式注册的版本，writer 可按目标版本生成数据，不做隐式迁移。

本 RFC 只冻结消除实现歧义所需的线协议与兼容规则，不实现 Event Store、授权、Replay 引擎或业务状态机。核验日期为 2026-08-22；规范依据为 [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785.html)、[JSON Schema Draft 2020-12](https://json-schema.org/draft/2020-12) 和 [RFC 3339](https://www.rfc-editor.org/rfc/rfc3339.html)。

# Motivation and requirements

ARCH-0003 已给出 `EventEnvelope`、`RunManifest` 和 `EvidenceRecord` 的字段方向，但没有回答同一值如何跨平台获得相同字节、Schema 怎样被固定、旧数据怎样读取、未知字段怎样处理，以及 payload 为什么不能改变授权或隔离上下文。REQ-0004 之后的持久化与 Replay 一旦写入数据，这些选择就难以回退。

RFC 必须满足 REQ-0003 的 AC-01 至 AC-10，并保持以下宪法边界：协议层验证“形状、身份和局部不变量”，Kernel 验证 capability、状态迁移、Event Store 顺序/幂等和跨记录关系；策略、插件和 payload 都不能宣布自己已获授权或已被验证。

# Proposed design

## 1. Schema dialect, identity, and manifest

- 使用 JSON Schema Draft 2020-12；每个发布 Schema 都声明 `$schema` 和不可变 `$id`。
- `$id` 使用仓库内稳定 URN：`urn:pareto-harness:schema:<type>:<major>.<minor>`。`type` 为小写 kebab-case；版本为无前导零十进制整数。
- 每个顶层记录包含 `schema_ref`：`type`、`major`、`minor`、`schema_digest`。不能从文件名、Rust 类型或“当前版本”推断。
- `SchemaSetManifest` 使用固定 Schema ID `urn:pareto-harness:schema:schema-set-manifest:1.0`，按 `$id` 排序列出完整 `SchemaRef`，并包含按 event type/version 排序的 `EventTypeBinding`。每个 binding 唯一映射到完整 payload SchemaRef 和 typed event variant；重复 key、重复 `$id`、同 `$id` 不同 digest或缺失成员均拒绝。manifest 不含自身 digest；`SchemaSetRef` 包含 manifest SchemaRef 和对完整 manifest 内容计算的 `schema-set` digest。Run Manifest 固定该 SchemaSetRef。
- Schema、manifest 和 golden fixtures 纳入版本控制。生成器必须在不同顺序、重复运行和支持平台上逐字节一致。

Schema `major` 表示不兼容合同，`minor` 表示在声明兼容方向内可演进的同一合同。补丁级修订不进入线协议：已发布 Schema 字节发生任何变化都必须产生新 minor 或 major，避免“同版本不同 Schema”。

## 2. Closed-world JSON rules

- 顶层及所有权限、身份、版本、隔离和权威状态对象使用 `unevaluatedProperties: false`；实现中的 typed deserializer 同样拒绝未知字段。
- 所有语义必需字段均为 `required`。可选值以字段缺省表达；除某字段 Schema 明确列出 `null` 外，禁止 `null`。
- `default` 只可作为文档 annotation，不参与持久化输入补齐；首个版本不生成带 `default` 的权威字段。
- 禁止重复 object key、非有限数字及无效 Unicode。字符串不做 Unicode normalization；摘要保持输入字符串 code point 序列，与 JCS 一致。
- bool、受限小整数可使用 JSON 原生值。可能超过跨语言安全整数范围的 sequence、计数、字节数、Token 数和货币最小单位使用规范十进制字符串：`0` 或 `[1-9][0-9]*`，无符号、空格和前导零。
- 首个权威协议不使用 JSON 浮点数。未来比率或统计量必须单独版本化为十进制定点字符串或明确的特殊类型。
- 时间使用 UTC RFC 3339 字符串，固定为 `YYYY-MM-DDTHH:mm:ss.SSSZ`；调用者提供时间，协议层不读取系统时钟。
- JSON Schema 的 `format` 可能只产生 annotation，因此格式正确性由 typed/semantic validator 强制；Schema validator 若支持 Draft 2020-12 format-assertion vocabulary 也必须启用，但不能成为唯一防线。

## 3. Canonicalization and digest

规范化采用 RFC 8785 JCS 的 UTF-8 输出。输入必须先通过上述 I-JSON 子集与类型验证；不接受“先 hash、后验证”。数组顺序有语义，不排序。

digest 文本表示为 `sha256:<64 lowercase hex>`。`LP(x)` 定义为 `u64be(len(x_bytes)) || x_bytes`；标签、domain、ID 和 digest 文本均先编码为 UTF-8，长度按 byte 计。preimage 为：

```text
LP("pareto-harness-digest-v1")
LP(domain)
LP(canonical_schema_ref)
LP(jcs_utf8)
```

`canonical_schema_ref` 是 `{type, major, minor, schema_digest}` 的 JCS UTF-8。Schema 自摘要为避免循环而单独使用 `LP("pareto-harness-digest-v1") || LP("schema") || LP($id) || LP(schema_jcs_utf8)`；其他对象绑定完整 SchemaRef。domain 至少区分 `schema-set`、`revision:<kind>`、`event-payload` 和 `artifact-manifest`。

每种 Revision 发布独立 `RevisionHashView` Schema。`content_digest` 只覆盖行为内容、不含 revision metadata；`revision_id` 覆盖 `{logical_id, revision_kind, parent_revision, schema_ref, content_digest, creator_actor, source, created_at}`，仅排除 `revision_id`。metadata 变化生成不同 revision ID。二进制 artifact 不走 JCS：`ArtifactManifest` 绑定 kind、media type、byte length 和 `sha256(raw_bytes)`，Evidence 引用其 `artifact-manifest` 域 digest。

## 4. Public types

所有 ID 是不可互换的新类型，线表示为带种类前缀的小写 ASCII 字符串。首版共同限制为 1 至 128 bytes、正则 `[a-z][a-z0-9-]*_[a-z0-9][a-z0-9-]*`；具体类型固定前缀，如 `run_`、`event_`、`stream_`、`workspace_`、`agent_`。生成算法不属于本 RFC，但比较按完整字节串，禁止大小写折叠。

`IsolationScope` 必含 `tenant_id`，并按记录 profile 要求 `user_id`、`workspace_id`、`run_id`、`agent_id`。EventEnvelope、RunManifest、EvidenceRecord 均必含 tenant/workspace/run/agent；user 的存在性和值必须与认证上下文 exact match。单租户仍写 `tenant_local`，不得用缺字段或默认值表达。payload 不能定义或覆盖 scope。

Kernel 从认证 principal、已持久化 RunManifest、目标 stream 和 append command 派生不可伪造的 `TrustedValidationContext`：完整 scope、actor、target stream、run、SchemaSetRef 和可选 delegation grant。比较是字段集合和值 exact match，不支持 wildcard、subset 或缺字段补齐。actor delegation 必须由 Kernel capability grant 授权并记录，不能由 envelope 自报。

`RevisionMetadata` 包含逻辑 ID、revision ID、revision kind、可选 parent revision、Schema ref、content digest、creator actor、source 和 created_at。parent 若存在必须与当前 revision kind 相同；跨记录 parent 存在性由 Registry 检查。

`EventEnvelope` 包含自身 Schema ref、IsolationScope、event/stream/run ID、sequence 十进制字符串、可选 causation ID、correlation ID、event type/version、occurred_at、actor、payload Schema ref、payload digest 和 payload。sequence 必须大于零。Kernel exact-match TrustedValidationContext，并从固定 SchemaSet 的 EventTypeRegistry 查找 type/version，要求 payload SchemaRef 完整相等且 payload 验证为对应 typed variant。Event Store 再验证 event ID 幂等、stream sequence 原子递增、causation scope 和授权。

`RunManifest` 包含自身 Schema ref 与 IsolationScope，并固定 task、behavior、workspace、environment、context graph、model snapshot、tool set、kernel、SchemaSetRef、不可变 budget revision/snapshot 和 `protocol_limits_ref`；plan revision 可缺省但不能为 null。反序列化不得用当前模型、工具、预算、limits、环境或 Schema 默认值补齐。

`execution_mode` 是带 discriminator 的联合：`live` 禁止 source Run；`recorded_replay` 和 `reexecute` 必须有不等于派生 Run 的 `derived_from_run_id`；`simulated` 必须固定非空 fixture revisions，并以 `simulation_origin: standalone | derived` 区分，只有 derived 必须带 source。任何声称从既有 Run 派生的模式都必须建立谱系，standalone simulation 不称为 replay。

RunManifest 只固定不可变 `BoundaryRecordingPolicyRef`：允许的 boundary kind、必须产生的 intent/request/receipt/artifact 事件、脱敏与保留规则以及取消/迟到结果策略；不预知动态 request ID 或执行结果。live 执行中，Kernel 按 Event Log 追加 request/intent/receipt/failure/artifact 事实。Run 进入终止态后，投影器从固定事件范围生成不可变 `BoundaryInventoryRevision`，逐项固定 boundary kind、input event/request identity、处理结果、receipt/artifact digest 或“无 receipt”的终止原因，并绑定 source Run、最后 event sequence、SchemaSetRef 和 recording policy；它不能回填或修改原 RunManifest。

`recorded_replay` 与 `reexecute` 的派生 RunManifest 在启动前固定 source Run 和已 finalized 的 BoundaryInventoryRevision；simulated 固定 fixture revisions。完全确定性 source 的空 inventory 必须由终止后的 inventory revision 证明，不能由启动 Manifest 自报。Intent 无 Receipt、取消、partial effect 和迟到 Receipt 都作为事件事实进入 finalization；终止 sequence 之后的迟到结果不能改变已 finalized inventory，只能产生关联的 late-result 审计事件和新的 reconciliation revision。Kernel 核验 source tenant/workspace、SchemaSet compatibility、policy 和 inventory digest；派生 Run 永不覆盖源 Run。

`EvidenceRecord` 包含自身 Schema ref 与 IsolationScope、requirement ID、claim、evidence type、producer/verifier/subject revision、artifact digest、结构化 verdict、scope、freshness、limitations 和 observed_at。verdict 首版为 `passed | failed | inconclusive | invalidated`；自然语言只能进入 claim/limitations，不能形成其他通过状态。

`ValidationError` 默认只含稳定 code、JSON Pointer、合同 ID 和安全摘要。`ProtocolLimitsV1`：外部 JSON transport 在 parse 前最多 1 MiB raw UTF-8 bytes；解析后或 typed constructor 都以 JCS bytes 测量，整记录最多 1 MiB、payload 最多 768 KiB；字符串按解码后 UTF-8 bytes 最多 256 KiB；根 value depth=1，每进入 array/object 子 value 加一，最大 64；object 在 duplicate-key 判定前按 lexical member、array 按 element 计，最大 16,384；错误按 JSON Pointer UTF-8 byte lexicographic 再按 code 排序，截取 32。pretty JSON 可能先触发 transport ceiling，而同一逻辑 typed/minified value 以 JCS semantic ceiling 判定，这是明确的入口资源差异。Kernel 只能收紧，但实际 limits profile/ref 必须固定进 RunManifest。vectors 覆盖 typed/minified/pretty、escape、duplicate key、N/N+1 和错误排序。

## 5. Compatibility model

兼容性始终声明方向和角色，禁止只写“兼容”：

- `old-writer → new-reader`：同 major 的新 reader 必须读取所有旧 writer golden fixtures。
- `new-writer(target=old) → old-reader`：新 writer 只有在显式选择旧 Schema 且生成逐 Schema 合法数据时才兼容。
- `new-writer(current) → old-reader`：不保证；闭合 Schema 会安全拒绝未知字段。
- major 不同：默认不兼容；只允许由显式、纯函数、版本化迁移器生成新对象，原记录不覆盖。

同 major 候选变化只限保守 checker 能证明的白名单安全变换，例如新增非 required 且无 default 的字段或扩大既有数值/长度接受集合；仍需 consumer fixtures。删除/重命名、required 变化、收窄合法值、改变格式/单位/default/null/unknown-field、canonicalization/digest 或隔离/权限语义必须升级 major。枚举增加默认升级 major，除非从首版就有版本化 `unknown` 表达且证明 exhaustive consumer 安全。

Schema compatibility checker 是强制门禁，只对白名单变换作保守证明；遇到无法证明的 `$ref` graph、`oneOf`/`anyOf`/`not` 等变化必须 fail closed，要求 major bump 或专项 RFC 扩展证明规则。fixtures 是额外必要证据而非充分证明。mutation tests 覆盖 range/pattern/length 收窄、required/null/enum、nested `$ref` 和组合关键字。

## 6. Validation and authorization API boundary

协议 API 是纯函数式边界：

```text
reject_raw_size(bytes, trusted_context.protocol_limits_ref)
  → parse_with_exact_limits(bytes)
  → load_exact_schema_set(trusted_context.schema_set_ref)
  → verify_manifest_digest_unique_ids_and_members
  → select_member_schema(schema_ref)
  → validate_schema(value)
  → deserialize_typed(value)
  → validate_semantics_and_exact_context(value, trusted_context)
  → Validated<T>
```

`TrustedValidationContext` 只由 Kernel 派生。SchemaSet 必须按 RunManifest 的 exact SchemaSetRef 查找，构造时重算 manifest/member digest、拒绝重复 `$id`，envelope 和 payload SchemaRef 必须是成员。验证期间不访问网络或磁盘。`Validated<T>` 仍不代表授权；Kernel 随后基于认证身份、delegation 和 capability 检查效果或状态变更。

初始化不能自信任待验证数据，因此 Kernel 明确分离三条入口：

1. `admit_schema_set(command, expected_ref, bytes)`：Kernel build 固定 bootstrap trust root，包括 Draft 2020-12 metaschema digest、SchemaSetManifest SchemaRef/digest 和允许的 digest 算法。只有持有 `manage_schema_sets` capability 的认证 principal 可调用。bootstrap parser 按 expected_ref 重算 manifest/member digest、验证 `$id` 唯一、所有 member 对固定 metaschema 合法、EventTypeBinding 只引用成员；不得从网络取 metaschema或相信 manifest 自报 digest。
2. `create_run(command, admitted_schema_set, budget, limits)`：context 从已授权 command、principal、已 admission 的 exact SchemaSet、budget/limits 和目标 workspace 派生；先验证并持久化 RunManifest，再产生 established-run context。未 admission SchemaSet 不能创建 Run。
3. `validate_established_run(bytes, persisted_manifest_context)`：只使用已持久化 Manifest 派生的 TrustedValidationContext，执行前述 exact validation。

bootstrap root 随 kernel version 固定并进入 RunManifest；变更 root 属于 Kernel major 兼容变化。负例覆盖首个 SchemaSet、首个 Run、自签恶意 Schema、expected digest 不符、未 admission set 和错误 capability。

策略/插件只能提交未受信提议；不能构造 `Validated<T>`、注册 Schema、改变 SchemaSet 或直接写权威状态。Rust 实现通过私有字段/构造器表达此边界，JSON Schema 仅是跨语言合同，不是 capability token。

# Interfaces, data flow, and invariants

```text
untrusted producer / plugin / adapter
  → bounded bytes + explicit SchemaRef
  → exact RunManifest SchemaSet lookup and membership proof
  → structural + typed + semantic + TrustedValidationContext validation
  → Validated<T>
  → Kernel capability/state-transition check
  → REQ-0004 Event Store append / registry write
  → event-backed projections and evidence
  → recorded replay selects the pinned SchemaSet
```

责任边界：protocol 拥有线类型、Schema、规范化、digest 和局部验证；Kernel 拥有 capability、状态、预算、取消、顺序、幂等和跨记录完整性；Runtime 服务只能请求；策略/插件只能产生提议；adapter 负责外部格式转换但不能跳过 protocol/Kernel。

关键不变量：

1. 每个持久化顶层对象显式固定 Schema 与 IsolationScope。
2. 先验证再规范化/摘要；摘要绑定 domain、完整 SchemaRef 和 canonical bytes。
3. SchemaSet 不可变并由 RunManifest exact 固定；EventTypeRegistry 是集合成员，Replay 不从“当前 Schema”猜测。
4. payload、Schema annotation 和反序列化 default 均不能授予权限或改变外层 scope。
5. 验证无外部效果；时钟、文件、网络、进程、秘密和可变全局注册表均不在协议路径。
6. 解析/Schema 成功不等于授权、Event Store 追加成功或 Evidence 准入成功。

# Failure modes and security

| Failure | Required behavior and recovery |
|---|---|
| 未知 Schema/digest 不匹配 | fail closed；返回稳定错误；不得网络拉取 Schema；运维显式部署支持集后重试 |
| duplicate key、非法 Unicode、oversize/deep JSON | 解析前执行 raw-byte ceiling，解析器执行深度/collection/string 限制；不产生 digest 或部分 `Validated<T>` |
| unknown field/null/default smuggling | 闭合 Schema 与 typed validator 双重拒绝；payload 不能覆盖 envelope/scope |
| digest/type confusion | UTF-8 length-prefix + domain + 完整 SchemaRef；RevisionHashView 和 raw artifact golden tests |
| scope/confused deputy | exact 对照 Kernel 派生 TrustedValidationContext；actor delegation 单独授权；失败不写权威状态 |
| event type/payload confusion | 只接受 pinned EventTypeRegistry 的 type/version → payload SchemaRef typed binding |
| wrong/stale SchemaSet | exact 对照 RunManifest SchemaSetRef，重算 manifest/member digest 并验证 membership |
| Schema generator nondeterminism/concurrent invocation | 在临时输出生成后逐字节比较并原子发布；生成失败保留已发布 Schema，不写半成品 |
| sequence/duplicate event race | protocol 只做格式检查；REQ-0004 Event Store 事务性判定，不在此层虚假保证 |
| cancellation/timeout/budget | RunManifest 固定 budget 与 limits ref；protocol 按精确定义单位限额，Kernel 传播取消，本 RFC 不创建后台任务 |
| partial migration | 迁移输出先完整验证并写为新记录；原记录不可覆盖；批次 checkpoint/恢复由消费 Requirement 定义 |
| stale/removed Schema | 已被持久化记录或 RunManifest 引用的 Schema 永不原位修改或物理删除；可停止新写入但保留 reader |
| error disclosure | 默认错误仅含 code/path/contract/safe digest；详细诊断必须在受控日志策略中另行授权 |

# Alternatives considered

1. **Serde 默认 JSON + 结构体派生 Schema**：实现最少，但字段顺序、default、unknown fields 和 Rust 布局容易成为偶然合同，也无法独立证明跨语言兼容；拒绝。
2. **开放对象并忽略未知字段**：新 writer 更容易被旧 reader 接受，但权限/隔离字段可能被旧实现静默忽略，形成 downgrade 和 smuggling 风险；对权威合同拒绝。未来非权威 extension bag 必须显式命名、版本化且不能承载权限语义。
3. **Protobuf/gRPC 首发**：成熟的字段演进规则和高效编码有吸引力，但偏离 ADR-0002，增加工具链且不解决 JSON/MCP 边界；当前拒绝，达到 ARCH-0004 重审条件时再评估。
4. **自定义排序 JSON + hash**：可避开 JCS 数字约束，但产生项目私有跨语言算法和维护负担；拒绝。采用 JCS，并通过禁用浮点及大整数字符串化缩小风险。
5. **内容寻址 Schema，不设 major/minor**：能精确标识字节，却不能表达消费者支持窗口和迁移政策；仅 digest 不足。采用语义版本 + digest 双身份。
6. **状态 quo：等 Event Store 时再决定**：短期无文档成本，但会把不可逆线协议混入 REQ-0004 实现并阻塞 Replay；拒绝。

# Compatibility, migration, and rollback

首发无历史 Runtime 数据，因此迁移入口为空；这不是未来兼容的证据。首版发布后，旧 Schema、manifest、golden fixtures 和 reader 必须保留。minor/major 判定、writer target 和迁移器均进入测试矩阵；迁移产生带新 Schema 身份的新记录并保留来源关系，不覆盖 Event Log。

实现未发布前可通过 Git revert 回退 RFC/Schema。发布后回滚 writer 默认版本不删除已写新版本：先停止新写入，恢复支持旧目标的 writer，reader 保持多版本读取；若新版本已有记录，则使用受审迁移/投影路径或保持双读，不能伪装成字节级回退。

本 RFC 接受后创建 ADR，记录 JCS、闭合 Schema、显式作用域和有方向兼容模型。改变 canonicalization、digest preimage、Schema 身份、unknown-field 或隔离语义属于 major 架构变化，需要新 Requirement/RFC/ADR、迁移、负向测试和独立架构评审。

# Evaluation and acceptance

- 质量：官方 JCS vectors、完整 SchemaRef/cross-domain/Revision/artifact vectors、Schema golden、保守 compatibility mutation tests、malicious/limit/scope/actor/stream/event-binding/SchemaSet fixtures，以及各 replay mode lineage fixture 必须通过。
- Token/费用：无真实模型/Provider 调用；分别记录为不适用，不宣称成本优化。
- 延迟：在 Windows/Linux/macOS 记录 parse、validate、canonicalize、digest 和 Schema generation 基线；每 PR 运行确定性测试。首版只建立可复现基线，不设置无证据性能收益阈值。
- 权限/隔离：逐字段 omit/mismatch、actor delegation、stream/run swap、payload shadowing 和 same-run/different-SchemaSet 均 fail closed；验证路径无网络、文件、进程、秘密或时钟访问。
- Replay/回滚：RunManifest 固定 SchemaSet、budget、limits 和 recording policy；live 动态 boundary/Intent 无 Receipt/partial/cancel/late Receipt 通过事件记录，终止后 finalization 生成 BoundaryInventoryRevision；recorded_replay/reexecute 精确固定 source inventory，standalone/derived simulation、自引用/跨 scope 谱系均有正负向 fixture。

批准门禁：架构评审逐项检查七项宪法问题，所有 Blocker/Major 关闭后由维护者将 RFC 标为 `accepted`，创建对应 ADR，并将 SPEC-0002 标为 `approved`；此前禁止 Runtime 实现。
