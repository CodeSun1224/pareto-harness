# REQ-0009 Handoff

最终状态：`done / archived`。REQ-0004、REQ-0007、REQ-0008前置均done；REQ-0009设计由fresh independent REVIEW-0012批准，最终实现exact `25e84603f09c3e3c47c29846e9cc3ef1fe6a4d72`由fresh independent REVIEW-0013批准，F-001至F-009全部由同一实现Reviewer关闭，0 Blocker/0 Major。

fresh independent REVIEW-0012最终批准设计exact `b7acbd82824d8410d432117c89be1bd56c8ce05c`及接受闭环exact `60cee6ed44d150185bf99ca3095a8ce803bcc0d3`，F-001至F-004全部closed，0 Blocker/0 Major。四项关键closure是：Recorded replay固定Inventory source horizon；Inventory/Record V2无损区分partial/unknown；executor内容地址identity贯穿全部边界；claim/crash recovery command及未claim/已claim结论闭合。

最终基线：SQLite v2不变；当前Effect-capable SchemaSet为`sha256-0d32378157c01117dc9b86a307cfc8d05aa299bc520ad0cb7ae29d67a79844ba`，89 files双生成byte-identical。首轮整改`sha256-70389…`、初始`sha256-ed548…`、REQ-0008 `sha256-0efc2e…`及全部更早Manifest/Inventory/SchemaSet/reader/reducer identity均保留不变；Cargo manifests与lock相对设计接受基线无diff。

已交付Manifest v3、Effect registry/executor descriptor、连续Effect Event stream、无损Projection、Boundary Inventory/Record V2、Intent/claim/opaque lease、sealed Fake executor、Receipt admission、Control/Effect原子pair、Kernel recovery authority、Manifest-pinned reconciliation producer与fixed-horizon Recorded replay。Run/Task success guard在同一writer transaction阻止未结清或待对账Effect成功。

最高风险不变量：Intent必须先于dispatch；same key只允许exact retry；dispatch lease绑定exact executor；未claim recovery只能`not_applied + verified zero + full release`；claim后只能partial/unknown + reconciliation且不redispatch；Effect结论与operation terminal/settlement必须原子一致；Recorded replay固定horizon且零executor/writer/settlement authority。

严格排除：真实文件/进程/network/Provider/Tool/Sandbox效果、外部Worker/RPC/队列、DB v3、mutable outbox/status/receipt表、alternate Event actor、background scanner、自动redispatch、跨边界exactly-once承诺、caller-selected reader/registry和新第三方依赖。触发任一项必须返回SPEC/RFC与独立设计复审。

实现测试全部使用Fake executor与FakeClock，不依赖真实sleep。最终证据：19个原始命名filter均matched 1，Effect 24/24，Workspace Kernel 185 passed/1 ignored，Protocol 9 unit + 25 contract passed/1 ignored，governance 27，scope/fmt/clippy/Schema/diff全部通过。ignored项均为既有非阈值performance observation；未声明质量、成本或延迟优化。

REVIEW-0013重点闭环：claim writer-lock再准入与retry无lease；executor implementation不可自报；authenticated-invalid Receipt的双stream terminal与mandatory audit同事务；Kernel-signed recovery Clock/epoch authority；sealed pinned reconciliation observation；counterpart双向重算；Task exact success guard；无损Projection；retained golden隔离；Receipt/recovery两类lineage由writer/fold/source共用互斥validator，hybrid resealed history fail closed且reconcile no-write。

后续REQ-0010 Provider、REQ-0011 Tool、REQ-0013 Sandbox与REQ-0014 Agent Loop只能提出Effect request或返回observation，不得取得Event、Capability、Budget、Receipt/evidence、recovery、reconciliation、Replay或terminal authority。真实外部效果必须新增Requirement/RFC与独立评审，不能把本Fake纵切冒充production provider proof。
