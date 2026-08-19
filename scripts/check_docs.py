#!/usr/bin/env python3
"""Validate durable Pareto Harness documents without third-party packages."""

from __future__ import annotations

import re
import sys
from collections import defaultdict
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"
FORMAL_DIRS = {
    "product",
    "research",
    "architecture",
    "requirements",
    "rfcs",
    "adrs",
    "fixes",
    "postmortems",
    "benchmarks",
    "roadmap",
}
REQUIRED_FIELDS = {"id", "title", "status", "owners", "created", "updated", "links"}
ID_PATTERN = re.compile(r"^[A-Z][A-Z0-9-]*-\d{4}$")
LINK_PATTERN = re.compile(r"(?<!!)\[[^\]]+\]\(([^)]+)\)")
PLACEHOLDER_PATTERN = re.compile(r"\b(?:TODO|TBD|FIXME)\b", re.IGNORECASE)


def frontmatter(path: Path, text: str) -> dict[str, str] | None:
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return None
    try:
        end = lines.index("---", 1)
    except ValueError:
        return None
    values: dict[str, str] = {}
    for line in lines[1:end]:
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if ":" not in line:
            raise ValueError(f"{path}: invalid frontmatter line: {line!r}")
        key, value = line.split(":", 1)
        values[key.strip()] = value.strip()
    return values


def local_link_target(path: Path, raw_target: str) -> Path | None:
    target = raw_target.strip().split("#", 1)[0]
    if not target or target.startswith(("http://", "https://", "mailto:")):
        return None
    return (path.parent / target).resolve()


def main() -> int:
    errors: list[str] = []
    ids: dict[str, list[Path]] = defaultdict(list)
    markdown_files = sorted(ROOT.rglob("*.md"))

    for path in markdown_files:
        if ".git" in path.parts:
            continue
        text = path.read_text(encoding="utf-8")
        relative = path.relative_to(ROOT)

        if path.is_relative_to(DOCS) and path.parent.name in FORMAL_DIRS:
            try:
                metadata = frontmatter(relative, text)
            except ValueError as error:
                errors.append(str(error))
                metadata = None
            if metadata is None:
                errors.append(f"{relative}: missing YAML frontmatter")
            else:
                missing = sorted(REQUIRED_FIELDS - metadata.keys())
                if missing:
                    errors.append(f"{relative}: missing fields {', '.join(missing)}")
                doc_id = metadata.get("id", "")
                if not ID_PATTERN.fullmatch(doc_id):
                    errors.append(f"{relative}: invalid id {doc_id!r}")
                else:
                    ids[doc_id].append(relative)

        if path.is_relative_to(DOCS) and PLACEHOLDER_PATTERN.search(text):
            errors.append(f"{relative}: unresolved placeholder token")

        for match in LINK_PATTERN.finditer(text):
            target = local_link_target(path, match.group(1))
            if target is not None and not target.exists():
                errors.append(f"{relative}: broken local link {match.group(1)!r}")

    for doc_id, paths in sorted(ids.items()):
        if len(paths) > 1:
            errors.append(f"duplicate id {doc_id}: {', '.join(map(str, paths))}")

    if errors:
        print("Document validation failed:")
        for error in errors:
            print(f"- {error}")
        return 1

    print(f"Document validation passed: {len(markdown_files)} Markdown files, {len(ids)} formal IDs.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
