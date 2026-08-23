# REQ-0003 Architecture Review

## Review identity

- Subject: REQ-0003, SPEC-0002, RFC-0002 working tree
- Reviewer: primary-agent
- Independence: non-independent design self-review
- Verdict: changes incorporated; RFC remains proposed and Spec remains draft
- Date: 2026-08-22

本记录不是正式独立批准，也不能替代实现后的 REVIEW 或维护者对 RFC/Spec 的接受。

## Constitutional trace

1. Effect path：RFC 将解析/Schema/typed/scope validation 与 Kernel capability/state checks 分离；协议不产生外部效果。Event Store 顺序、幂等和跨记录因果留给 REQ-0004，未虚假下沉到 payload。
2. Kernel bypass：插件不能构造 `Validated<T>`、注册 SchemaSet 或直接写权威状态；JSON Schema 不是 capability token。
3. Version pinning：RunManifest 固定 Task、Behavior、Workspace、Environment、Context、Model、Tool、Kernel 和 SchemaSet；Plan 可显式缺省，不以进程默认补齐。
4. Nondeterminism/replay：调用者显式提供时间；规范化和 digest 确定；recorded replay 使用固定 SchemaSet；外部模型/效果仍由后续 Replay 合同记录。
5. Cancellation/failure/concurrency：验证受版本化资源限制且无后台效果；sequence race、duplicate append 和跨记录 causation 由 Event Store 原子处理；迁移不覆盖原记录。
6. Promotion：本 RFC 不定义候选/晋升；RFC-0001 和 ARCH-0002 的历史重放/Canary 门禁未被改变。
7. Quality/cost/latency：RFC 分别定义合同正确性证据、无 Provider 成本及跨平台延迟基线，不声称优化。

## Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| AR-F-001 | Major | RFC-0002 SchemaSet manifest | manifest 若包含自身 digest 会产生不可解的自引用，跨实现可能采用不同排除规则 | 明确 manifest 内容不含自身 digest，由独立 `SchemaSetRef` 对完整内容计算 | closed in proposal |
| AR-F-002 | Major | RFC-0002 format validation | Draft 2020-12 `format` 可作为 annotation；只依赖 Schema validator 可能接受非法时间/URI/ID | typed/semantic validator 必须强制格式，支持时额外启用 format-assertion，并有负向 fixture | closed in proposal |
| AR-F-003 | Major | RFC-0002 untrusted parsing | 把大小限制留给调用方会允许不同入口绕过 DoS 防线，也无法形成一致测试 | 定义版本化 `ProtocolLimitsV1`，raw bytes 在 parse 前检查，调用方只能收紧 | closed in proposal |
| AR-F-004 | Minor | RFC-0002 ID generation | RFC 只冻结 ID 线格式，不定义唯一性生成算法；REQ-0004 需要可靠 event ID 幂等 | REQ-0004 前选择并测试 ID generator；不把格式验证当唯一性证明 | open |
| AR-F-005 | Note | RFC-0002 compatibility | closed-world Schema 有意不保证 current new writer 被 old reader 接受，可能增加多版本 writer 成本 | 实现保留 target-schema writer fixtures并测量维护成本 | accepted |

## Required independent follow-up

RFC 接受前由独立/新会话 reviewer 复查：JCS 和大整数跨语言 vectors、Schema-set 无自引用、format assertion 双层验证、limits 的实际内存放大、scope confused-deputy 负例，以及 `Validated<T>` 构造边界。AR-F-004 不阻塞本协议 RFC，但必须进入 REQ-0004 影响分析；任何 Blocker/Major 必须由非实现者关闭并复审。
