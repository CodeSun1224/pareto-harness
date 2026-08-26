#!/usr/bin/env python3
"""Static scope and frozen-database checks for the REQ-0007 vertical slice."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
APPROVED_BASELINE = "6de3598"
EVENT_STORE = Path("crates/pareto-kernel/src/event_store.rs")
RUNTIME = Path("crates/pareto-kernel/src/event_store/runtime_control.rs")
RUNTIME_TESTS = Path("crates/pareto-kernel/src/event_store/runtime_control/tests.rs")
FROZEN_CONSTANTS = (
    "APPLICATION_ID",
    "DB_VERSION",
    "UPDATE_TRIGGER",
    "DELETE_TRIGGER",
    "WRITER_EPOCH_TRIGGER",
    "SNAPSHOT_UPDATE_TRIGGER",
    "SNAPSHOT_DELETE_TRIGGER",
    "EVENTS_DDL",
    "WRITER_EPOCH_COLUMN_DDL",
    "SNAPSHOT_TABLE_DDL",
    "SNAPSHOT_INDEX_DDL",
    "V2_MIGRATION_CHECKSUM",
)


def extract_const(source: str, name: str) -> str:
    match = re.search(
        rf"(?ms)^const {re.escape(name)}\b.*?(?=^const [A-Z0-9_]+\b|^#\[|^//|^struct |^enum |^impl )",
        source,
    )
    if not match:
        raise ValueError(f"missing const {name}")
    return match.group(0).strip()


def git_baseline(path: Path) -> str:
    completed = subprocess.run(
        ["git", "show", f"{APPROVED_BASELINE}:{path.as_posix()}"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout


def validate() -> list[str]:
    errors: list[str] = []
    current = (ROOT / EVENT_STORE).read_text(encoding="utf-8")
    baseline = git_baseline(EVENT_STORE)
    for name in FROZEN_CONSTANTS:
        try:
            if extract_const(current, name) != extract_const(baseline, name):
                errors.append(f"frozen Event Store constant changed: {name}")
        except ValueError as error:
            errors.append(str(error))
    if "const DB_VERSION: i64 = 2;" not in current:
        errors.append("REQ-0007 must retain SQLite user_version 2")

    runtime = (ROOT / RUNTIME).read_text(encoding="utf-8")
    tests = (ROOT / RUNTIME_TESTS).read_text(encoding="utf-8")
    for token in (
        "std::thread::sleep",
        "tokio::time::sleep",
        "reqwest",
        "TcpStream",
        "UdpSocket",
        "std::process::Command",
        "tokio::process",
    ):
        if token in runtime or token in tests:
            errors.append(f"out-of-scope runtime boundary found: {token}")
    if "struct FakeClock" not in tests or "trait RuntimeClock" not in runtime:
        errors.append("Runtime tests must use the injected FakeClock contract")
    if "struct FakeOperation" not in runtime or "dispatch_fake_operation" not in runtime:
        errors.append("Runtime tests must use the Kernel-mediated FakeOperation boundary")
    if "struct KernelMeterSnapshot" not in runtime or "try_consume" not in runtime:
        errors.append("verified usage must be produced by the Kernel meter")
    replay = re.search(
        r"(?ms)pub\(super\) async fn replay_runtime_control\b.*?\n    }",
        runtime,
    )
    if not replay or "self.runtime_control_projection(registry, target).await" not in replay.group(0):
        errors.append("Recorded replay must remain a projection-only reader")
    for manifest in (
        Path("crates/pareto-kernel/Cargo.toml"),
        Path("crates/pareto-protocol/Cargo.toml"),
    ):
        if (ROOT / manifest).read_text(encoding="utf-8") != git_baseline(manifest):
            errors.append(f"REQ-0007 added or changed dependencies: {manifest}")
    return errors


def main() -> int:
    errors = validate()
    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1
    print("REQ-0007 scope check passed: DB v2 frozen, FakeClock only, no real I/O, replay read-only")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
