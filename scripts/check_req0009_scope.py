#!/usr/bin/env python3
"""Static scope, dependency, retained-contract, and DB checks for REQ-0009."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
APPROVED_BASELINE = "60cee6ed44d150185bf99ca3095a8ce803bcc0d3"
EVENT_STORE = Path("crates/pareto-kernel/src/event_store.rs")
EFFECT_RUNTIME = Path("crates/pareto-kernel/src/event_store/effect_runtime.rs")
EFFECT_TESTS = Path("crates/pareto-kernel/src/event_store/effect_runtime/tests.rs")
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
FORBIDDEN_BOUNDARIES = (
    "std::thread::sleep",
    "tokio::time::sleep",
    "reqwest",
    "TcpStream",
    "UdpSocket",
    "std::process::Command",
    "tokio::process",
    "wasmtime",
    "wasmer",
    "WASI",
)


def extract_const(source: str, name: str) -> str:
    match = re.search(
        rf"(?ms)^const {re.escape(name)}\b.*?(?=^const [A-Z0-9_]+\b|^#\[|^//|^struct |^enum |^impl )",
        source,
    )
    if not match:
        raise ValueError(f"missing const {name}")
    return match.group(0).strip()


def git(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args], cwd=ROOT, check=False, capture_output=True, text=True
    )


def baseline_text(path: Path) -> str:
    completed = git("show", f"{APPROVED_BASELINE}:{path.as_posix()}")
    if completed.returncode:
        raise RuntimeError(completed.stderr)
    return completed.stdout


def retained_schema_dirs() -> list[str]:
    completed = git("ls-tree", "-d", "--name-only", f"{APPROVED_BASELINE}:schemas/sets")
    if completed.returncode:
        raise RuntimeError(completed.stderr)
    return sorted(line.strip() for line in completed.stdout.splitlines() if line.strip())


def validate() -> list[str]:
    errors: list[str] = []
    current = (ROOT / EVENT_STORE).read_text(encoding="utf-8")
    baseline = baseline_text(EVENT_STORE)
    for name in FROZEN_CONSTANTS:
        try:
            if extract_const(current, name) != extract_const(baseline, name):
                errors.append(f"frozen Event Store constant changed: {name}")
        except ValueError as error:
            errors.append(str(error))
    if "const DB_VERSION: i64 = 2;" not in current:
        errors.append("REQ-0009 must retain SQLite user_version 2")

    runtime = (ROOT / EFFECT_RUNTIME).read_text(encoding="utf-8")
    tests = (ROOT / EFFECT_TESTS).read_text(encoding="utf-8")
    for token in FORBIDDEN_BOUNDARIES:
        if token in runtime or token in tests:
            errors.append(f"out-of-scope external Effect boundary found: {token}")
    for required in (
        "trait FakeEffectExecutor",
        "append_effect_reserve_intent_pair",
        "append_effect_terminal_pair",
        "recorded_effect_replay",
        "ensure_effects_complete_for_run",
    ):
        if required not in runtime:
            errors.append(f"missing bounded Effect invariant: {required}")
    replay = re.search(r"(?ms)async fn recorded_effect_replay\b.*?\n    }", runtime)
    if not replay:
        errors.append("missing Recorded Effect replay reader")
    else:
        for forbidden in ("append", "dispatch_effect", "invoke(", "reserve", "settle"):
            if forbidden in replay.group(0):
                errors.append(f"Recorded Effect replay gained write/execute authority: {forbidden}")

    for manifest in (
        Path("crates/pareto-kernel/Cargo.toml"),
        Path("crates/pareto-protocol/Cargo.toml"),
        Path("Cargo.lock"),
    ):
        if (ROOT / manifest).read_text(encoding="utf-8") != baseline_text(manifest):
            errors.append(f"REQ-0009 added or changed dependencies: {manifest}")

    for directory in retained_schema_dirs():
        path = f"schemas/sets/{directory}"
        if not (ROOT / path).is_dir():
            errors.append(f"retained SchemaSet missing: {directory}")
        elif git("diff", "--quiet", APPROVED_BASELINE, "--", path).returncode:
            errors.append(f"retained SchemaSet changed: {directory}")
    return errors


def main() -> int:
    errors = validate()
    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1
    print(
        "REQ-0009 scope check passed: DB v2 and retained sets frozen; "
        "Fake Effect only; replay read-only; dependencies unchanged"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
