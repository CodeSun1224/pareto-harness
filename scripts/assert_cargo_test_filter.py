#!/usr/bin/env python3
"""Run a Cargo test filter only after proving that it selects at least one test."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def matching_tests(list_output: str, test_filter: str) -> list[str]:
    return sorted(
        line.removesuffix(": test")
        for line in list_output.splitlines()
        if line.endswith(": test") and test_filter in line.removesuffix(": test")
    )


def run(crate: str, test_filter: str) -> int:
    listed = subprocess.run(
        ["cargo", "test", "-p", crate, "--offline", "--lib", "--", "--list"],
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if listed.returncode:
        sys.stderr.write(listed.stdout)
        sys.stderr.write(listed.stderr)
        return listed.returncode
    matches = matching_tests(listed.stdout, test_filter)
    if not matches:
        print(json.dumps({"crate": crate, "filter": test_filter, "matched": 0}))
        return 2
    print(
        json.dumps(
            {"crate": crate, "filter": test_filter, "matched": len(matches), "tests": matches},
            sort_keys=True,
        )
    )
    completed = subprocess.run(
        ["cargo", "test", "-p", crate, test_filter, "--offline"],
        cwd=ROOT,
        check=False,
    )
    return completed.returncode


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print("usage: assert_cargo_test_filter.py <crate> <filter>", file=sys.stderr)
        return 2
    return run(argv[1], argv[2])


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
