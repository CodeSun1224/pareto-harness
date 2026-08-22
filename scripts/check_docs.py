#!/usr/bin/env python3
"""Validate durable Pareto Harness records without third-party packages."""

from __future__ import annotations

import re
import subprocess
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path


FORMAL_DIRS = {
    "product",
    "research",
    "architecture",
    "epics",
    "requirements",
    "specs",
    "rfcs",
    "adrs",
    "reviews",
    "fixes",
    "postmortems",
    "benchmarks",
    "roadmap",
}
REQUIRED_FIELDS = {"id", "title", "status", "owners", "created", "updated", "links"}
ID_PATTERN = re.compile(r"^[A-Z][A-Z0-9-]*-\d{4}$")
LINK_PATTERN = re.compile(r"(?<!!)\[[^\]]+\]\(([^)]+)\)")
PLACEHOLDER_PATTERN = re.compile(r"\b(?:TODO|TBD|FIXME)\b", re.IGNORECASE)
TASK_PATTERN = re.compile(r"^- \[([ xX])\] (TASK-REQ-\d{4}-\d{2,})[：:]", re.MULTILINE)
REVISION_PATTERN = re.compile(r"^[0-9a-f]{7,40}$")

ALLOWED_STATUS = {
    "EPIC": {"proposed", "active", "completed", "paused", "cancelled"},
    "REQ": {
        "proposed",
        "impact-analyzed",
        "specified",
        "approved",
        "planned",
        "implementing",
        "reviewing",
        "verified",
        "done",
        "accepted",  # Legacy design-baseline state.
        "rejected",
        "blocked",
    },
    "SPEC": {"draft", "approved", "superseded"},
    "RFC": {"proposed", "accepted", "rejected", "superseded"},
    "ADR": {"accepted", "superseded"},
    "FIX": {"investigating", "fixed", "verified", "closed"},
    "PM": {"draft", "accepted", "closed"},
    "REVIEW": {"open", "changes-requested", "approved"},
    "PRD": {"draft", "accepted", "superseded"},
    "CAP": {"proposed", "accepted", "superseded"},
    "RES": {"active", "archived"},
    "ARCH": {"proposed", "accepted", "superseded"},
    "BENCH": {"proposed", "accepted", "superseded"},
    "ROADMAP": {"active", "completed", "superseded"},
    "BACKLOG": {"active", "archived", "superseded"},
}


@dataclass(frozen=True)
class Record:
    path: Path
    metadata: dict[str, str]
    text: str

    @property
    def id(self) -> str:
        return self.metadata.get("id", "")

    @property
    def status(self) -> str:
        return self.metadata.get("status", "")

    @property
    def links(self) -> set[str]:
        return parse_links(self.metadata.get("links", "")) or set()


def parse_links(raw: str) -> set[str] | None:
    value = raw.strip()
    if not value.startswith("[") or not value.endswith("]"):
        return None
    inner = value[1:-1].strip()
    if not inner:
        return set()
    items = [item.strip().strip("'\"") for item in inner.split(",")]
    if any(not item or not ID_PATTERN.fullmatch(item) for item in items):
        return None
    return set(items)


def parse_frontmatter(path: Path, text: str) -> tuple[dict[str, str] | None, str | None]:
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return None, "missing YAML frontmatter"
    try:
        end = lines.index("---", 1)
    except ValueError:
        return None, "unterminated YAML frontmatter"
    values: dict[str, str] = {}
    for line in lines[1:end]:
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if ":" not in line:
            return None, f"invalid frontmatter line: {line!r}"
        key, value = line.split(":", 1)
        values[key.strip()] = value.strip()
    return values, None


def local_link_target(path: Path, raw_target: str) -> Path | None:
    target = raw_target.strip().split("#", 1)[0]
    if not target or target.startswith(("http://", "https://", "mailto:")):
        return None
    return (path.parent / target).resolve()


def record_prefix(record_id: str) -> str:
    return record_id.rsplit("-", 1)[0]


def formal_document(path: Path, docs: Path) -> bool:
    if not path.is_relative_to(docs):
        return False
    relative = path.relative_to(docs)
    return len(relative.parts) >= 2 and relative.parts[0] in FORMAL_DIRS


def git_revision_exists(root: Path, revision: str) -> bool:
    if not (root / ".git").exists():
        return True
    result = subprocess.run(
        ["git", "cat-file", "-e", f"{revision}^{{commit}}"],
        cwd=root,
        capture_output=True,
        check=False,
    )
    return result.returncode == 0


def validate_repository(root: Path) -> tuple[list[str], int, int]:
    root = root.resolve()
    docs = root / "docs"
    errors: list[str] = []
    by_id: dict[str, list[Record]] = defaultdict(list)
    records: list[Record] = []
    markdown_files = sorted(root.rglob("*.md"))

    for path in markdown_files:
        if ".git" in path.parts:
            continue
        text = path.read_text(encoding="utf-8")
        relative = path.relative_to(root)

        if formal_document(path, docs):
            metadata, parse_error = parse_frontmatter(relative, text)
            if parse_error:
                errors.append(f"{relative}: {parse_error}")
            elif metadata is not None:
                missing = sorted(REQUIRED_FIELDS - metadata.keys())
                if missing:
                    errors.append(f"{relative}: missing fields {', '.join(missing)}")
                record = Record(relative, metadata, text)
                records.append(record)
                if parse_links(metadata.get("links", "")) is None:
                    errors.append(f"{relative}: links must be a bracketed list of formal IDs")
                if not ID_PATTERN.fullmatch(record.id):
                    errors.append(f"{relative}: invalid id {record.id!r}")
                else:
                    by_id[record.id].append(record)
                    prefix = record_prefix(record.id)
                    allowed = ALLOWED_STATUS.get(prefix)
                    if allowed is None:
                        errors.append(f"{relative}: unknown record prefix {prefix!r}")
                    elif record.status not in allowed:
                        errors.append(
                            f"{relative}: invalid status {record.status!r} for {prefix}; "
                            f"allowed: {', '.join(sorted(allowed))}"
                        )

        if path.is_relative_to(docs) and PLACEHOLDER_PATTERN.search(text):
            errors.append(f"{relative}: unresolved placeholder token")

        if path.is_relative_to(docs):
            for match in LINK_PATTERN.finditer(text):
                target = local_link_target(path, match.group(1))
                if target is not None and not target.exists():
                    errors.append(f"{relative}: broken local link {match.group(1)!r}")

    for doc_id, duplicate_records in sorted(by_id.items()):
        if len(duplicate_records) > 1:
            errors.append(
                f"duplicate id {doc_id}: "
                + ", ".join(str(record.path) for record in duplicate_records)
            )

    known_ids = set(by_id)
    for record in records:
        for linked_id in sorted(record.links):
            if linked_id not in known_ids:
                errors.append(f"{record.path}: unknown linked id {linked_id}")

        prefix = record_prefix(record.id) if ID_PATTERN.fullmatch(record.id) else ""
        if prefix == "SPEC" and not any(link.startswith("REQ-") for link in record.links):
            errors.append(f"{record.path}: SPEC must link at least one Requirement")
        if prefix == "REVIEW":
            if not any(link.startswith("REQ-") for link in record.links):
                errors.append(f"{record.path}: REVIEW must link a Requirement")
            if not any(link.startswith("SPEC-") for link in record.links):
                errors.append(f"{record.path}: REVIEW must link a Spec")
            for field in ("independence", "reviewed_revision", "open_blockers", "open_majors"):
                if not record.metadata.get(field):
                    errors.append(f"{record.path}: REVIEW missing field {field}")
            independence = record.metadata.get("independence")
            if independence not in {"independent", "self-review"}:
                errors.append(f"{record.path}: invalid independence {independence!r}")
            revision = record.metadata.get("reviewed_revision", "")
            if not REVISION_PATTERN.fullmatch(revision):
                errors.append(f"{record.path}: reviewed_revision must be a Git commit ID")
            elif not git_revision_exists(root, revision):
                errors.append(f"{record.path}: reviewed_revision {revision} does not exist")
            for field in ("open_blockers", "open_majors"):
                value = record.metadata.get(field, "")
                if not value.isdigit():
                    errors.append(f"{record.path}: {field} must be a non-negative integer")
            if record.status == "approved":
                if record.metadata.get("open_blockers") != "0" or record.metadata.get("open_majors") != "0":
                    errors.append(f"{record.path}: approved REVIEW has open Blocker or Major findings")

        if prefix == "REQ" and record.status != "accepted":
            if record.metadata.get("risk") not in {"lightweight", "standard", "high"}:
                errors.append(f"{record.path}: Requirement must declare risk")
            if not record.metadata.get("work"):
                errors.append(f"{record.path}: Requirement must declare work directory")

    specs_by_req: dict[str, list[Record]] = defaultdict(list)
    reviews_by_req: dict[str, list[Record]] = defaultdict(list)
    for record in records:
        for linked_id in record.links:
            if linked_id.startswith("REQ-"):
                if record_prefix(record.id) == "SPEC":
                    specs_by_req[linked_id].append(record)
                elif record_prefix(record.id) == "REVIEW":
                    reviews_by_req[linked_id].append(record)

    for record in records:
        if record_prefix(record.id) != "REQ" or record.status not in {"verified", "done"}:
            continue
        approved_specs = [
            spec
            for spec in specs_by_req[record.id]
            if spec.status == "approved" and spec.id in record.links
        ]
        if not approved_specs:
            errors.append(f"{record.path}: completed Requirement lacks an approved Spec")
        approved_spec_ids = {spec.id for spec in approved_specs}
        valid_reviews = [
            review
            for review in reviews_by_req[record.id]
            if review.status == "approved"
            and review.metadata.get("independence") == "independent"
            and review.metadata.get("open_blockers") == "0"
            and review.metadata.get("open_majors") == "0"
            and bool(review.links & approved_spec_ids)
            and review.id in record.links
        ]
        if not valid_reviews:
            errors.append(f"{record.path}: completed Requirement lacks an approved Review")

        work_value = record.metadata.get("work", "")
        work_path = (root / work_value).resolve() if work_value else None
        work_base = (root / ".agents" / "work").resolve()
        if work_path is None or not work_path.is_relative_to(work_base) or not work_path.is_dir():
            errors.append(f"{record.path}: completed Requirement lacks its declared work directory")
            continue
        required_work = {"PLAN.md", "TASKS.md", "HANDOFF.md", "VALIDATION.md"}
        missing_work = sorted(name for name in required_work if not (work_path / name).exists())
        if missing_work:
            errors.append(f"{record.path}: completed work missing {', '.join(missing_work)}")
        tasks_path = work_path / "TASKS.md"
        if tasks_path.exists():
            task_matches = list(TASK_PATTERN.finditer(tasks_path.read_text(encoding="utf-8")))
            if not task_matches:
                errors.append(f"{tasks_path.relative_to(root)}: no parseable Task IDs")
            elif any(match.group(1) == " " for match in task_matches):
                errors.append(f"{tasks_path.relative_to(root)}: completed Requirement has open Tasks")

    work_roots = [root / ".agents" / "work" / "active", root / ".agents" / "work" / "archived"]
    for work_root in work_roots:
        if not work_root.exists():
            continue
        for work_dir in sorted(path for path in work_root.iterdir() if path.is_dir()):
            required = {"PLAN.md", "TASKS.md", "HANDOFF.md"}
            missing = sorted(name for name in required if not (work_dir / name).exists())
            if missing:
                errors.append(f"{work_dir.relative_to(root)}: missing work files {', '.join(missing)}")
            tasks_path = work_dir / "TASKS.md"
            if tasks_path.exists():
                task_ids = [match.group(2) for match in TASK_PATTERN.finditer(tasks_path.read_text(encoding="utf-8"))]
                if len(task_ids) != len(set(task_ids)):
                    errors.append(f"{tasks_path.relative_to(root)}: duplicate task IDs")

    return errors, len(markdown_files), len(by_id)


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    errors, markdown_count, formal_count = validate_repository(root)
    if errors:
        print("Document validation failed:")
        for error in errors:
            print(f"- {error}")
        return 1
    print(
        f"Document validation passed: {markdown_count} Markdown files, "
        f"{formal_count} formal IDs."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
