---
title: REQ-0003 版本化协议实施计划
status: completed
owner: primary-agent
updated: 2026-08-23
links: [REQ-0003, SPEC-0002]
---

# Goal and acceptance

已交付 REQ-0003 的独立协议纵切和 AC-01 至 AC-10 证据。RFC-0002、ADR-0003 和 SPEC-0002 已获维护者接受/批准；正式 REVIEW-0002 approved，全部 finding 与三平台门禁已关闭。

# Current state

- REQ-0002 已完成并归档，前置条件满足。
- 仓库已新增最小 Cargo workspace、独立 protocol crate 和版本化协议 Schema；仍无 Event Store、Provider 或其他 Runtime 调用方。
- RFC-0002 已 accepted，ADR-0003 已 accepted，SPEC-0002 已 approved；独立架构评审为 0 open Blocker/Major/Minor。

# Plan

1. 已完成：专项 RFC、独立架构评审、ADR、Spec 批准和批准后 impact analysis。
2. 已完成：REQ-0003 进入 `implementing`，建立最小 Rust workspace 和独立 protocol crate。
3. 已完成：公共类型、验证错误、确定性 Schema/manifest、规范化/digest vectors 和保守兼容检查。
4. 已完成：EventEnvelope、RunManifest、EvidenceRecord、隔离、limits、boundary inventory/reconciliation 与 replay lineage 本地 fixtures。
5. 已完成：本地 Focused → Impacted → Core 验证并记录证据；已创建 exact commit `d1daa0d`。
6. 已完成：fresh reviewer 对 exact commit `f8275e09103fe7702188c8298c5c2a791b9118b8` 聚焦复审；F001/F002/F003/F004/F006/F011 已由 reviewer 关闭。
7. 已完成：F005/F007/F008/F009 全部由独立 reviewer 关闭，trusted boundary lineage 与完整负例通过。
8. 已完成：exact `6447c37` GitHub Actions Windows/Linux/macOS matrix run `32642574089` 全部成功。
9. 已完成：正式 REVIEW-0002 approved，0 open Blocker/Major/Minor；完成门禁通过并归档 work records。

# Validation

批准后根据实际 package 名称校准命令：

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo test -p pareto-protocol --test protocol_contract checked_in_schemas_equal_deterministic_generation -- --exact
cargo test -p pareto-protocol --test protocol_contract compatibility_proof_allows_only_optional_property_addition -- --exact
cargo test -p pareto-protocol --test protocol_contract closed_types_reject_unknown_fields_duplicate_keys_and_floats -- --exact
cargo test -p pareto-protocol validation::tests::isolation_boundaries_and_payload_schema_fail_closed -- --exact
cargo test -p pareto-protocol --test protocol_contract replay_lineage_and_boundary_finalization_fail_closed -- --exact
cargo test -p pareto-protocol --test protocol_contract typed_record_and_payload_byte_limits_are_exact -- --exact
cargo test -p pareto-protocol --test protocol_contract every_retained_schema_set_is_complete_and_content_addressed -- --exact
cargo test -p pareto-protocol --test protocol_contract schema_publisher_handles_concurrency_and_stale_staging -- --exact
cargo tree -p pareto-protocol
python -m unittest discover -s scripts/tests -p "test_*.py"
python scripts/check_docs.py
git diff --check
git status --short
```

`.github/workflows/protocol-matrix.yml` 在 Windows、Linux、macOS 执行同一组 locked/offline workspace、Schema/digest golden 与治理测试。依赖先通过 `cargo fetch --locked` 获取，后续 Cargo 门禁均使用 `--locked --offline`。Schema 生成后执行 `git diff --exit-code -- schemas/`。

# Handoff notes

协议纵切已实现并处于独立评审整改阶段。下一步是关闭可在工作树内修复的 findings；exact commit、跨平台结果和旧 Review freshness 仍是完成门禁。改变公开字段、规范化、摘要、兼容性或隔离键的决定必须先回写 RFC/ADR/Spec 并重新评审。
