---
title: Verified Procedure 路线重设计与 REQ-0034 设计计划
status: route-design-approved-f009-remediation-submitted
owner: maintainers
updated: 2026-09-05
links: [REQ-0034, SPEC-0010, RFC-0013]
---

# Goal and current state

从可信基线 `e7a939cad71a85ada97c3b60d61ba5c024d85ab9` 重新定义 Pareto Harness 为 Kernel 强制的已验证流程复用系统，并建立 REQ-0034 的 Requirement/Spec/RFC 设计门禁。本阶段只修改文档；不实现 runtime/schema，不恢复归档 Provider 路径，不开始 REQ-0010。

# Plan

1. 用仓库、Git 历史、远程与持久文档核验基线，封存旧 REQ-0010 尝试。
2. 对实际代码的 Manifest、Lifecycle、Evidence、Effect、Replay 与 Promotion 边界完成影响分析。
3. 分解稳定 Requirement 顺序，保留 REQ-0001..0033 ID，新增 REQ-0034..0036。
4. 编写 REQ-0034、SPEC-0010、RFC-0013，并同步 PRD、能力地图、架构、EPIC、路线图和 Backlog。
5. 运行 REDESIGN-TEST-PLAN 中的文档、Rust、Schema 和 Git 门禁，提交 exact candidate。
6. 启动 fresh independent Reviewer，仅允许其修改 REVIEW-0018；输入不含实现者批准结论。
7. Blocker/Major 由设计者修复并交同一 Reviewer 复审；只有 0/0 approved 才接受路线与 RFC/Spec。
8. 接受闭环后停止，不创建或实现新的 REQ-0010，先向用户报告准确 revision 与评审结论。

# Stop conditions

- 需要改动 Rust、Cargo、SQLite、Schema、已交付 REQ-0003..0009 合同或归档 Reviewer 原文。
- 发现 Procedure/Plan/Behavior 双重 authority、模型/adapter 旁路、审批自签或无法保持旧 Run 可解释。
- 需要重用已发布 Requirement ID、改写远程历史、重置/删除 main 或归档分支。
- 独立 Review 存在 open Blocker/Major。

# First review result

REVIEW-0018 对 exact `cfdc65af64675b8066b9bc429fbf998d588231bc` 判定 `changes-requested`：F-001 Blocker，F-002/F-003/F-004 Major，F-005/F-006 Minor。整改内容 exact `499116a8e93e00a737f0c112d0a0104eb9386840` 收窄 REQ-0034、冻结 closed Plan instantiation、选择零外部 I/O Plan bootstrap、冻结 principal-root 独立性，并修正文档事实；finding 只能由同一 Reviewer 关闭。
