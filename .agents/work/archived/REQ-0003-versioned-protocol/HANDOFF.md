# REQ-0003 Handoff

状态：REQ-0003 已完成。RFC-0002 与 ADR-0003 accepted，SPEC-0002 approved，正式 REVIEW-0002 approved；全部独立评审 finding 关闭，exact `6447c37` 三平台 CI matrix 全部成功。Rust protocol crate、12 个公开 Schema、不可变内容寻址 SchemaSet 发布与可信 admission 已交付。仓库仍无 Event Store、Replay executor、Provider 或 CLI Runtime 调用方。

权威输入：`REQ-0003`、`SPEC-0002`、`RFC-0001`、`ADR-0001`、`ADR-0002`、`ARCH-0001` 至 `ARCH-0004`。

实现边界：只允许按批准 Spec 建立最小 protocol 纵切，不得提前实现 Event Store、状态机、Replay executor 或 CLI。协议公开合同变化需先更新 RFC/ADR/Spec 并重新评审。

已完成的既有证据包括 locked/offline Rust/Python 门禁、Schema file-set/byte golden 和 digest vectors。本轮新增 public admission authorizer、typed event decoder、规范版本语法、统一顶层 record admission、边界 hash-view、limits 顺序/矩阵与内容寻址发布；最终数字以本轮 VALIDATION 重新执行结果为准。GitHub Windows/Linux/macOS 结果仍须在新 commit push 后取得。

下一步：REQ-0004 可将本协议作为已验证 prerequisite；不得把本需求的协议测试表述为 Event Store/Replay Runtime E2E 证据。
