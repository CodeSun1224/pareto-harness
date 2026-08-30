---
id: REVIEW-0001
title: REQ-0002 SDD 与独立评审门禁代码评审
status: approved
owners: [independent-reviewer]
created: 2026-08-22
updated: 2026-08-30
links: [REQ-0002, SPEC-0001]
independence: independent
reviewed_revision: 46772c7fbb30e82f0e8fd4fb50915e8414acaa65
open_blockers: 0
open_majors: 0
---

# Verdict

Approved。独立 Reviewer 使用不继承实现讨论的评审上下文，检查指定 Git revision、Requirement、Spec、原始 Diff、测试和 Skills。四轮 focused re-review 后无开放 Blocker 或 Major。

# Findings

| ID | Severity | Location | Finding and impact | Required proof | Status |
|---|---|---|---|---|---|
| F-001 | Major | Review completion gate | self-review、开放 Finding 或错误 Spec 可伪装 approved | 独立性、计数、Spec 和 revision 负向测试 | closed |
| F-002 | Major | Work completion gate | 删除/共用 work、缺失 Validation 或未完成 Task 可绕过 | 唯一 work、完整文件和 Task 解析测试 | closed |
| F-003 | Major | Formal document discovery | 嵌套正式目录和 malformed links 可逃逸 | 递归识别和严格 links 测试 | closed |
| F-004 | Major | Review freshness | 旧 revision 可批准之后改变的 Requirement | 仅允许可证明的收尾 metadata 变化 | closed |
| F-005 | Major | Validation evidence | 空、全 skipped 或无风险说明的 Validation 可完成 | 至少一个 passed、skipped 原因和五列 Schema | closed |
| F-006 | Major | Findings parsing | Blocker/Major accepted 或非法 ID 可逃逸 | 严格 ID、severity 和 status 解析 | closed |
| F-007 | Major | Validation row parsing | 畸形 skipped 行可被静默忽略 | 所有 Results 数据行严格校验 | closed |
| F-008 | Minor | CI whitespace range | 多提交 push 只检查 HEAD | PR/push 使用事件范围 diff | closed |
| F-009 | Minor | `.github/workflows/protocol-matrix.yml:46-56` | 新增 protocol matrix 曾只检查 `HEAD`，遗漏 PR/多提交 push 范围 | PR 使用 base/head、普通 push 使用 before/after、零 before initial push 检查 exact HEAD | closed |

# Acceptance trace

AC-01 至 AC-04 由 AGENTS、Spec、模板和 Reviewer Profile 覆盖；AC-05/06 由 13 个 Skill 校验和独立 Review 覆盖；AC-07 由 18 个正负向检查器测试覆盖；AC-08/09 由 PR/CI 和 Requirement Backlog 覆盖；AC-10 由本 Review、归档 Work 和 Validation 证据覆盖。

# Compatibility, permission, and isolation review

没有 Runtime、网络、秘密或用户/租户数据变更。检查器只读扫描仓库，并以固定 Git revision 验证评审新鲜度。工作目录被绑定到唯一 Requirement，Reviewer 为只读角色。

# Regression and test review

Focused、Impacted 和 Core 治理测试通过。负向用例覆盖非法状态/链接、重复 ID、Review 绕过、Spec 错配、旧 revision、工作目录、Task、Validation 和嵌套目录。

# Scope and unrelated changes

变更只涉及 SDD 治理、Skills、文档结构、Roadmap 和对应检查器/测试；没有 Runtime 空壳或无关依赖。

# Re-review history

- 初审：changes-requested，4 Major、2 Minor。
- 第一次 focused re-review：changes-requested，3 Major。
- 第二次 focused re-review：changes-requested，1 Major。
- revision `98e882a` 最终 re-review：approved，无新 Finding。
- exact HEAD `ff614b5` focused re-review：changes-requested。独立复核 `98e882a..ff614b5` 全部 substantive changes；AGENTS/README/index/ARCH/EPIC 和 protocol SDD 增量未削弱生命周期、风险、独立 Review、freshness 或完成门禁，18 个治理测试通过。但新三平台 workflow 的 whitespace 步骤仅检查 `HEAD`，重新引入 F-008 的多提交范围缺口，记录为 F-009。按门禁要求 `reviewed_revision` 保持 `98e882a`，不批准 `ff614b5` 的治理增量。
- exact commit `c115b9241f47b129f255c582b78a20ea3b75513a` focused re-review：approved。workflow 分别以 PR `base.sha...HEAD`、普通 push `before...sha` 和零 before initial push 的 exact `HEAD` 执行 whitespace gate，关闭 F-009；HANDOFF/PLAN/VALIDATION 只修正执行事实和命令，未削弱 REQ-0002/SPEC-0001 生命周期、独立 Review、freshness 或完成门禁。0 open Blocker/Major，reviewed revision 前移至 `c115b92`。
- exact commit `e761dc772ffb9987e57b8376d681a8c417719fbb` closure re-review：仅落盘上述独立 Review 结论与 freshness metadata，无产品、CI 或治理行为变化；0 open Blocker/Major，reviewed revision 前移至 `e761dc7`。
- exact commit `f8275e09103fe7702188c8298c5c2a791b9118b8` freshness re-review：该增量未修改 REQ-0002/SPEC-0001 检查器、生命周期、独立性、finding 计数或完成判定规则；18 个治理测试通过。REQ-0003 产品 finding 由独立 CODE-REVIEW 继续以 changes-requested 跟踪，不冒充本治理 Review 的批准。REVIEW-0001 保持 0 open Blocker/Major，reviewed revision 前移至 exact f8275e0。
- exact commit `201e19c65805697c279af3195c9abfca195a75e2` freshness re-review：`f8275e0..201e19c` 仅改变 REQ-0003 work evidence 与 protocol 产品类型、验证和合同测试；未修改 REQ-0002/SPEC-0001 检查器、生命周期、独立性、finding 计数或完成判定规则。18 个治理测试通过。REQ-0003 的 F-007/F-010 仍由独立 CODE-REVIEW 标为 open Major，未创建或伪装产品批准。REVIEW-0001 保持 0 open Blocker/Major，reviewed revision 前移至 exact 201e19c。
- exact commit `175bed4a20d02cc6d6428894deceab179b5b1727` freshness re-review：增量仅补充 REQ-0003 boundary recording-policy 产品校验、负例与评审/验证证据；未改变 REQ-0002/SPEC-0001 治理规则。18 个治理测试通过。独立 CODE-REVIEW 关闭 F-007，但 F-010 仍为 open Major，未创建产品批准。REVIEW-0001 保持 0 open Blocker/Major，reviewed revision 前移至 exact 175bed4。
- exact commit `2224772597b465b26d46a713f402e1bef0ebec5b` freshness-only re-review：`175bed4..2224772` 只提交独立 Reviewer 已完成的 F-007 disposition 与 REVIEW-0001 freshness 记录，无产品、测试、CI 或治理行为变化。F-010 继续作为 REQ-0003 唯一 open Major，未创建产品批准。REVIEW-0001 保持 0 open Blocker/Major，reviewed revision 前移至 exact 2224772。
- exact commit `72914b7bbad3491112e58b12c89647f3829d5696` freshness re-review：`2224772..72914b7` 仅提交上一轮 REVIEW-0001 freshness closure，无产品、测试、CI 或治理行为变化；exact Actions matrix 随后在 Windows/macOS/Ubuntu 完整成功。独立 REQ-0003 CODE-REVIEW 据此关闭最后的 F-010，但本治理 Review 不替代 Requirement 生命周期收尾。REVIEW-0001 保持 0 open Blocker/Major，reviewed revision 前移至 exact 72914b7。
- exact commit `b00928d5af24da75334595bbdeb63b8e6411e6ff` final freshness-only re-review：`72914b7..b00928d` 只提交独立 Reviewer 已完成的 F-010 closure、Actions/local raw validation evidence 与 REVIEW-0001 freshness 记录；无产品、测试、CI、治理行为或 Requirement lifecycle 变化。REQ-0003 CODE-REVIEW 保持 0 open Blocker/Major，REVIEW-0001 reviewed revision 前移至 exact b00928d。
- exact commit `e5d1b9b36de61a961b42b0c8148142cb98ae816f` lifecycle/archive freshness re-review：`b00928d..e5d1b9b` 未改变产品、Schema、测试、CI 或治理行为；增量创建 REVIEW-0002，将 REQ-0003 标为 done并链接正式 Review，将 work 从 active 移至 archived，并同步 README、Plan、Tasks、Handoff 与 Validation 最终事实。该 completion 由 REVIEW-0002 的 0 open findings 和 exact 三平台成功证据支持；REVIEW-0001 reviewed revision 前移至 exact e5d1b9b。
- exact commit `5ef949dd084b1e6ae82015f4c66adb8281aebf65` lifecycle/archive freshness re-review：自 `e5d1b9b` 起仅新增经 REQ-0004 approved Spec、独立 REVIEW-0003 与分层验证约束的 `pareto-kernel` Event Store、锁定依赖及其 durable docs/work archive；`AGENTS.md`、`.agents/skills/`、`scripts/` 与 REQ-0002/SPEC-0001 治理规则 byte-diff 无变化，18 个治理测试在 REQ-0004 completion evidence 中通过。`b7cf277..5ef949d` 本身只同步 REQ-0004 status/docs 并归档 work，无产品、测试、Schema 或治理行为变化。REVIEW-0001 保持 approved、0 open Blocker/Major，freshness 前移至 exact `5ef949d`。
- exact commit `b5850b76325bbc31825303215224d60c931e27c6` final freshness-only re-review：自 `5ef949d` 起的 REQ-0005 产品增量已由独立 REVIEW-0004 在 exact `675e3f8` 批准；closure diff `675e3f8..b5850b7` 只同步 README/index/EPIC/ARCH implemented facts、将 REQ-0005 标为 done并归档work、落盘 reviewer-owned approval。`AGENTS.md`、`.agents/skills/`、`scripts/`及REQ-0002/SPEC-0001治理行为未改变；18个治理测试与最终`check_docs.py`通过。REVIEW-0001保持approved、0 open Blocker/Major，freshness前移至exact `b5850b7`。
- exact commit `5c4f6e7f304c55fb61b6cc7e08d5bbe902b8d82c` substantive freshness re-review：`b5850b7..5c4f6e7` 新增高风险 REQ-0006 SDD、Runtime、SchemaSet与工作证据，但未修改 `AGENTS.md`、`.agents/skills/`、`.agents/agents/`、`.agents/templates/`、`scripts/`、PR/CI治理规则或REQ-0002/SPEC-0001生命周期、独立性、finding/open-count、freshness和完成判定逻辑。独立复跑18个governance tests通过；新REVIEW-0005如实保持3个open Major和`changes-requested`，未把未批准产品伪装成verified/done。REVIEW-0001保持approved、0 open Blocker/Major，freshness前移至exact `5c4f6e7`。
- exact commit `1d271549c2607f9c00377bdaa0fa999a131dafe3` substantive freshness re-review：`5c4f6e7..1d27154`修复REQ-0006独立代码评审发现并补充test traceability/work evidence；未修改`AGENTS.md`、skills/agents/templates、`scripts/`、CI或REQ-0002/SPEC-0001治理规则。Reviewer独立复跑18个governance tests，并以REVIEW-0005逐项关闭3个Major后才批准；finding/open-count/freshness门禁未被绕过。REVIEW-0001保持approved、0 open Blocker/Major，freshness前移至exact `1d27154`。
- exact closure commit `907eee7295a7c3e7c2fa408a035c52d684f52fb4` freshness-only re-review：`14b5438..907eee7`仅同步REQ-0006 done/navigation/architecture implemented facts、final validation/handoff/tasks/plan并将active work归档；`crates/`、Schema、Cargo、scripts、skills/agents/templates和CI零差异。正式REVIEW-0005已在`14b5438`落盘0 open Blocker/Major；本轮预期freshness失败后只由独立reviewer前移Review records，治理生命周期和完成判定未被绕过。REVIEW-0001保持approved、0 open Blocker/Major，freshness前移至exact `907eee7`。
- exact candidate `cfa7a06c3588a6ad975a9511140d0984f5eb1b8f` substantive freshness re-review：独立检查完整`907eee7..cfa7a06`，只新增/修订planned高风险REQ-0007 Requirement/Spec/RFC/ADR、active work与REVIEW-0006；`AGENTS.md`、scripts、skills/agents/templates、CI和REQ-0002/SPEC-0001零差异。REVIEW-0006如实保持`changes-requested`和1 open Major，Runtime继续阻塞，没有绕过独立finding/open-count/freshness或完成判定。REVIEW-0001保持approved、0 open Blocker/Major，freshness前移至exact `cfa7a06`。
- exact candidate `a4e34785908207e622365250ae1466b85b4baecb` substantive freshness re-review：独立检查`cfa7a06..a4e3478`，只补齐REQ-0007 timeout recovery identity的Requirement/Spec/RFC/ADR、active work与Review记录；`AGENTS.md`、scripts、skills/agents/templates、CI和REQ-0002/SPEC-0001零差异。REVIEW-0006仅在independent focused re-review确认唯一Major required proof闭合后改为approved、0 open Blocker/Major，未绕过finding/open-count/freshness或完成判定。REVIEW-0001保持approved、0 open Blocker/Major，freshness前移至exact `a4e3478`。
- exact implementation candidate `1b40e92be11e73a497ec821118b7cb4e0c1af1ce` substantive freshness re-review：独立检查`a4e3478..1b40e92`及20个governance tests。新增两个REQ-0007 helper和其tests，未修改`check_docs.py`、AGENTS/skills/agents/templates、CI或REQ-0002/SPEC-0001的lifecycle、independence、finding/open-count与freshness规则。正式REVIEW-0007如实记录9个open Major并给出`changes-requested`，REQ-0007保持reviewing，没有把未通过的产品伪装成verified/done。REVIEW-0001保持approved、0 open Blocker/Major，freshness前移至exact `1b40e92`。
- exact remediation candidate `ab2fbc6d2e979ef12bcffd5df1cfe76b975a9684` substantive freshness re-review：独立检查完整`1b40e92..ab2fbc6`；变更只修复REQ-0007产品/Schema/测试、记录FIX-0001及交付证据，未修改`AGENTS.md`、skills/agents/templates、CI或`check_docs.py`的lifecycle、独立性、finding/open-count与freshness规则。21个治理/Python tests通过；REVIEW-0007仍如实保留5个open Major和`changes-requested`，没有绕过完成判定。REVIEW-0001保持approved、0 open Blocker/Major，freshness前移至exact `ab2fbc6`。
- exact second-repair candidate `26b63ca2abb99bf3d6216d395994d006c1b3e2b5` substantive freshness re-review：完整`ab2fbc6..26b63ca`仅修复REQ-0007产品/Schema/测试、FIX和handoff，不修改`AGENTS.md`、skills/agents/templates、CI或`check_docs.py`的independence、finding/open-count、freshness与完成判定。21个治理tests通过；REVIEW-0007如实保留3个open Major和`changes-requested`，未把绿测试伪装成批准。REVIEW-0001保持approved、0 open Blocker/Major，freshness前移至exact `26b63ca`。
- exact third-repair candidate `97bca8b7b34ceadd5ab4f8ad01f49e10b3377adb` substantive freshness re-review：完整`26b63ca..97bca8b`只修复REQ-0007 Runtime/测试与FIX/work证据；`AGENTS.md`、skills/agents/templates、CI、`check_docs.py`及REQ-0002/SPEC-0001 lifecycle、independence、finding/open-count、freshness和完成判定均未修改。21个治理tests通过；REVIEW-0007如实保留F-007 open Major和`changes-requested`，未因53个focused绿灯绕过批准门禁。REVIEW-0001保持approved、0 open Blocker/Major，freshness前移至exact `97bca8b`。
- exact fourth-repair candidate `80249cc5c73575a3f92027f843cc657536905b9e` substantive freshness re-review：`97bca8b..80249cc`只修复REQ-0007 callback durable authority、pure fold、Schema/测试及交付证据；`AGENTS.md`、skills/agents/templates、CI、`check_docs.py`和REQ-0002/SPEC-0001治理规则未改。21个治理tests通过；REVIEW-0007仅在fresh independent reviewer独立关闭最后F-007后批准，finding/open-count/freshness门禁未绕过。REVIEW-0001保持approved、0 open Blocker/Major，freshness前移至exact `80249cc`。
- exact closure candidate `87be5391c40fdaa5b423c921747e7c941f7e2d42` substantive freshness re-review：`f18f410..87be539`未修改`AGENTS.md`、skills/agents/templates、CI、checker或REQ-0002/SPEC-0001规则，只同步REQ-0007 done/archive事实。21个治理tests通过；`check_docs.py`准确拒绝归档Validation仍含三条历史failed结果，REVIEW-0007记录F-010 open Major并撤回closure批准，没有用done metadata绕过completion gate。REVIEW-0001保持approved、0 open Blocker/Major，freshness前移至exact `87be539`。
- exact F-010 remediation `53338a836f646cdcefb6858ce07b0b0e8e12b11e` substantive freshness re-review：`828f9aa..53338a8`只把三条历史failed从归档Validation最终Results表迁入明确历史叙述，完整保留changes-requested/freshness失败与最终独立批准事实；未修改checker或治理规则。21个治理tests和更新freshness后的docs gate通过，F-010由独立Reviewer关闭。REVIEW-0001保持approved、0 open Blocker/Major，freshness前移至exact `53338a8`。
- exact architecture clarification `8bb885bda678f5f785706e9eb335f472b5244974` substantive freshness re-review：`53338a8..8bb885b`先完成REQ-0007 reviewer-owned closure，再仅澄清`ARCH-0004`分阶段语言边界；`AGENTS.md`、skills/agents/templates、CI、checker及REQ-0002/SPEC-0001治理规则零差异。REVIEW-0008由fresh independent Reviewer批准exact candidate，finding/open-count/freshness门禁未绕过。REVIEW-0001保持approved、0 open Blocker/Major。
- exact polyglot design remediation `1748f69d01044a936727b3b5b7659882981b9129` substantive freshness re-review：完整`8bb885b..1748f69`只新增proposed RFC-0007、研究证据/综合判断、导航、REVIEW-0009及其evidence-only整改；`AGENTS.md`、skills/agents/templates、CI、checker与REQ-0002/SPEC-0001治理规则零差异。REVIEW-0009首轮如实保持1 open Major，只有fresh Reviewer逐项打开E-017..E-027官方来源并确认原子声明及fact/inference边界后才关闭F-001；finding/open-count/freshness门禁未被绕过。REVIEW-0001保持approved、0 open Blocker/Major。
- exact accepted-doc candidate `b42ccdc3216f518ff60303cec20da92b78d190a1` substantive freshness re-review：`2c80b89..b42ccdc`只接受RFC-0007、新增ADR-0008并同步accepted architecture/Roadmap/Backlog/research/index；未修改治理规则。接受发生在REVIEW-0009已独立批准、0 open Blocker/Major之后，且ADR明确后续Runtime仍需accepted Requirement，未绕过finding、freshness或完成门禁。REVIEW-0001保持approved、0 open Blocker/Major。
- exact REQ-0008 design candidate `8507bae4ad979232e69ba282ee9c97ee71e3520e` substantive freshness re-review：`754798d..8507bae`只新增specified/draft/proposed REQ-0008设计并同步导航/Epic；AGENTS、skills/agents/templates、checker、CI及REQ-0002/SPEC-0001治理规则零差异。本fresh independent REVIEW-0010如实记录4 open Major并给出`changes-requested`，禁止设计批准和实现，未绕过finding/open-count/freshness或完成门禁。REVIEW-0001保持approved、0 open Blocker/Major。
- exact REQ-0008 remediation `3aee02adf8815466b02f51de247ae19922efc126` substantive freshness re-review：`43f3a5b..3aee02a`只修订REQ/SPEC/RFC并同步REVIEW链接，不改治理规则。REVIEW-0010仅由原fresh independent Reviewer逐项关闭4个Major后批准exact remediation；Requirement/Spec/RFC仍待正式接受且产品代码继续禁止，finding/open-count/freshness门禁未绕过。REVIEW-0001保持approved、0 open Blocker/Major。
- exact REQ-0008 accepted-doc `3318cbc6fe8bc8c9717a5a2b4aea1153f0d281d6` substantive freshness re-review：`ea9633c..3318cbc`仅新增ADR-0009、接受REQ/SPEC/RFC并同步ARCH/index/Epic；接受发生在REVIEW-0010 exact `3aee02a`独立批准0/0之后，且明确Runtime未实现。AGENTS/checker/skills/CI零差异，finding/freshness/完成门禁未绕过。REVIEW-0001保持approved、0 open Blocker/Major。
- exact REQ-0008 planning `e3d8d805b46fb4e1e25b23bc53bead71de730853` substantive freshness re-review：`5546d1f..e3d8d80`只在设计独立批准0/0与ADR接受后创建PLAN/TASKS/HANDOFF，并把REQ从approved推进planned；仅治理TASK-00完成，后续实现、fresh code Review、原Reviewer关闭Major/Blocker与REQ-0009禁启门禁均明确。AGENTS/checker/skills/CI和产品路径零差异，治理合同无回退。REVIEW-0001保持approved、0 open Blocker/Major。
- 2026-08-29：exact REQ-0008 closure `84ce5a705edb20f268898938be4579f4946d5e4f` substantive freshness re-review。REQ-0008实现先经fresh independent REVIEW-0011多轮finding/整改/同Reviewer复审，exact `e4877834`达到approved 0/0；closure只同步README/index/Epic/architecture/Requirement done和work归档，AGENTS、skills、checker、CI及finding/open-count规则未改。REQ-0009仍未实现，治理门禁无绕过。REVIEW-0001保持approved、0/0。
- 2026-08-30：focused REQ-0009 design freshness re-review exact `aba3a33703e681c542fd58b32f3d0ae41cff369d`。`84ce5a7..aba3a33`只新增并修订REQ-0009 impact-analyzed/proposed/draft Requirement/RFC/Spec及独立REVIEW-0012；AGENTS、skills、templates、checker、CI和REQ-0002/SPEC-0001零变化。REVIEW-0012仍如实保留1 open Major，REQ-0009未接受、未实现，治理门禁无绕过。REVIEW-0001保持approved、0/0。
- 2026-08-30：final REQ-0009 design freshness re-review exact `021b353d0efc923ef8739e3cb97d88f586c4fe06`。`aba3a33..021b353`仅6+/5-修订REQ-0009 proposed/draft timeout文字与test trace；治理规则、checker、代码和Schema零变化。REVIEW-0012仍保留F-004 open，REQ-0009未接受/实现。REVIEW-0001保持approved、0/0。
- 2026-08-30：one-line REQ-0009 freshness exact `b7acbd82824d8410d432117c89be1bd56c8ce05c`。仅收紧proposed RFC recovery accounting；治理/代码/Schema零变化。REVIEW-0012独立关闭F-004至0/0，但REQ-0009仍未接受/实现。REVIEW-0001保持approved、0/0。
- 2026-08-30：REQ-0009 design-acceptance closure freshness exact `60cee6ed44d150185bf99ca3095a8ce803bcc0d3`。仅接受已由REVIEW-0012在`b7acbd8`批准0/0的设计、创建ADR-0010并同步共享文档；无代码/Schema/Runtime/旧治理合同变化，且未声称实现。REVIEW-0001保持approved、0/0。
- 2026-08-30：REQ-0009 focused planning freshness exact `46772c7fbb30e82f0e8fd4fb50915e8414acaa65`。仅推进planned并创建active PLAN/TASKS/HANDOFF；fixed planning补齐planned→implementing与AC-21 exact命令。治理规则、代码、Schema、Runtime和旧合同零变化，未声称实现。REVIEW-0001保持approved、0/0。
