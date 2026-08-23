# REQ-0003 Independent Architecture Review

## Review identity

- Subject: REQ-0003, SPEC-0002, RFC-0002 untracked working-tree documents
- Reviewer: fresh sub-agent `/root/req0003_arch_review`
- Independence: independent; reviewer did not inherit implementation discussion and did not read implementer self-review
- Verdict: approved for architecture finding closure after three focused re-reviews; RFC acceptance and Spec approval remain maintainer actions
- Initial result: 0 Blocker, 7 Major, 1 Minor
- Date: 2026-08-22

## Findings

| ID | Severity | Finding and impact | Required proof | Status |
|---|---|---|---|---|
| AR-REQ0003-01 | Major | EventEnvelope 未绑定 exact tenant/user/workspace/run/actor/stream 可信上下文，存在冒充和 confused-deputy 风险 | 定义 Kernel 派生的 TrustedValidationContext、逐类型 scope profile 和负例 | closed by reviewer |
| AR-REQ0003-02 | Major | event_type 未与受信 payload Schema 建立不可变映射 | SchemaSet/Kernel 固定 EventTypeRegistry、typed variant 和错配负例 | closed by reviewer |
| AR-REQ0003-03 | Major | 验证用 SchemaSet 未强制等于 RunManifest 固定集合 | 冻结 SchemaSetRef，校验成员 digest/$id 唯一，并按 Manifest exact lookup | closed by reviewer |
| AR-REQ0003-04 | Major | RunManifest 遗漏 budget 和 protocol limits 身份 | 固定 budget revision/snapshot 与 protocol_limits_ref | closed by reviewer |
| AR-REQ0003-05 | Major | replay_mode 无 source Run、fixture/recording digest 和条件不变量 | 定义 lineage 与 nondeterministic boundary records | closed by reviewer |
| AR-REQ0003-06 | Major | digest 未绑定完整 SchemaRef，LP、Revision hash view 和二进制 artifact 规则不完整 | 冻结 UTF-8 LP、完整 SchemaRef、逐类型 hash view 与 raw artifact envelope | closed by reviewer |
| AR-REQ0003-07 | Major | 有限 fixtures 不能保守证明 old-writer → new-reader 兼容 | checker 只允许白名单安全变换，无法证明则 fail closed | closed by reviewer |
| AR-REQ0003-08 | Minor | limit 的单位、测量阶段、depth 和错误顺序未冻结 | 定义精确测量语义与 N/N+1 跨平台 vectors | closed by reviewer |
| AR-REQ0003-RR-01 | Major | SchemaSet admission 与首个 RunManifest 验证形成 bootstrap 循环 | Kernel trust root 及 admit/create/established-run 三条独立路径 | closed by reviewer |
| AR-REQ0003-RR-02 | Major | 启动 Manifest 不能固定执行后才产生的 live boundary inventory | Manifest 固定 recording policy，事件记录事实，终止后生成 inventory revision | closed by reviewer |

治理测试和文档检查通过，但新增目标文档未跟踪，普通 `git diff --check` 不覆盖其正文。当前没有 Runtime 或协议实现证据，符合批准前冻结实现的状态。

所有初审 finding 已由独立 Reviewer 在 focused re-review 中关闭；实现者未自行关闭。

## First focused re-review

结果：0 Blocker、3 Major、1 Minor。Reviewer 关闭 AR-REQ0003-02/03/04/06/07；AR-REQ0003-01/05/08 保持开放，并新增 Major AR-REQ0003-RR-01（SchemaSet/RunManifest bootstrap 循环）。第二轮修订已同步 Spec exact scope，增加 live/derived execution union、boundary inventory 规则、typed/JCS limits 测量，以及 Kernel-owned bootstrap trust root 与三条 admission/create/established-run 路径。关闭状态等待 Reviewer 第二次 focused re-review，本文不由实现者自行改为 closed。

## Second focused re-review

结果：0 Blocker、1 Major、0 Minor。既有 AR-REQ0003-01/05/08/RR-01 均由 Reviewer 关闭；新增 AR-REQ0003-RR-02：启动前不可变 RunManifest 不能固定执行后才产生的 live boundary inventory。第三轮修订将 Manifest 改为固定 BoundaryRecordingPolicyRef，运行中以事件记录动态事实，终止后生成不可变 BoundaryInventoryRevision，派生 replay 才精确引用。该 finding 保持 open，等待第三次 focused re-review。

## Third focused re-review

结果：Approved for architecture finding closure，0 Blocker、0 Major、0 Minor。Reviewer 关闭 AR-REQ0003-RR-02，并确认本轮未引入新 Blocker/Major。正式接受 RFC、创建 ADR 和批准 Spec 仍由维护者执行。
