---
title: REQ-0003 版本化协议实施计划
status: active
owner: primary-agent
updated: 2026-08-22
links: [REQ-0003, SPEC-0002]
---

# Goal and acceptance

交付 REQ-0003 的独立协议纵切和 AC-01 至 AC-10 证据。RFC-0002、ADR-0003 和 SPEC-0002 已获维护者接受/批准；协议实现和本地分层验证已完成，正在关闭独立评审发现。

# Current state

- REQ-0002 已完成并归档，前置条件满足。
- 仓库已新增最小 Cargo workspace、独立 protocol crate 和版本化协议 Schema；仍无 Event Store、Provider 或其他 Runtime 调用方。
- RFC-0002 已 accepted，ADR-0003 已 accepted，SPEC-0002 已 approved；独立架构评审为 0 open Blocker/Major/Minor。

# Plan

1. 已完成：专项 RFC、独立架构评审、ADR、Spec 批准和批准后 impact analysis。
2. 已完成：REQ-0003 进入 `implementing`，建立最小 Rust workspace 和独立 protocol crate。
3. 已完成：公共类型、验证错误、确定性 Schema/manifest、规范化/digest vectors 和保守兼容检查。
4. 已完成：EventEnvelope、RunManifest、EvidenceRecord、隔离、limits、boundary inventory/reconciliation 与 replay lineage 本地 fixtures。
5. 已完成：本地 Focused → Impacted → Core 验证并记录证据；Linux/macOS 与 exact-revision 证据仍待完成。
6. 进行中：由 fresh reviewer 聚焦复审；实现者不自行关闭 Blocker/Major。
7. 待完成：维护者授权提交后评审 exact commit，补齐跨平台证据与文档 freshness，再进入 verified/done。

# Validation

批准后根据实际 package 名称校准命令：

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
cargo test -p pareto-protocol --test protocol_contract checked_in_schemas_equal_deterministic_generation -- --exact
cargo test -p pareto-protocol --test protocol_contract compatibility_proof_allows_only_optional_property_addition -- --exact
cargo test -p pareto-protocol --test protocol_contract closed_types_reject_unknown_fields_duplicate_keys_and_floats -- --exact
cargo test -p pareto-protocol isolation_boundaries_and_payload_schema_fail_closed -- --exact
cargo test -p pareto-protocol --test protocol_contract replay_lineage_and_boundary_finalization_fail_closed -- --exact
cargo tree -p pareto-protocol
python -m unittest discover -s scripts/tests -p "test_*.py"
python scripts/check_docs.py
git diff --check
git status --short
```

Windows、Linux、macOS CI 执行 Schema/digest golden 与 workspace tests。Schema 生成后执行 `git diff --exit-code -- schemas/`；若最终路径不同，先更新 Spec/Plan。

# Handoff notes

协议纵切已实现并处于独立评审整改阶段。下一步是关闭可在工作树内修复的 findings；exact commit、跨平台结果和旧 Review freshness 仍是完成门禁。改变公开字段、规范化、摘要、兼容性或隔离键的决定必须先回写 RFC/ADR/Spec 并重新评审。
