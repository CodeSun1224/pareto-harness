from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path
from unittest import mock


SCRIPT = Path(__file__).resolve().parents[1] / "check_docs.py"
FIXTURES = Path(__file__).resolve().parent / "fixtures"
SPEC = importlib.util.spec_from_file_location("check_docs", SCRIPT)
assert SPEC and SPEC.loader
check_docs = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = check_docs
SPEC.loader.exec_module(check_docs)


class DocumentValidationTests(unittest.TestCase):
    def errors(self, fixture: str) -> list[str]:
        errors, _, _ = check_docs.validate_repository(FIXTURES / fixture)
        return errors

    def test_accepts_verified_requirement_with_spec_and_review(self) -> None:
        self.assertEqual([], self.errors("valid_completed"))

    def test_rejects_completed_requirement_without_review(self) -> None:
        errors = self.errors("missing_review")
        self.assertTrue(any("lacks an approved Review" in error for error in errors))
        self.assertTrue(any("lacks its declared work directory" in error for error in errors))

    def test_rejects_invalid_status_and_unknown_id_link(self) -> None:
        errors = self.errors("invalid_status")
        self.assertTrue(any("invalid status" in error for error in errors))
        self.assertTrue(any("unknown linked id SPEC-9999" in error for error in errors))

    def test_rejects_broken_markdown_link_and_duplicate_id(self) -> None:
        errors = self.errors("broken_duplicate")
        self.assertTrue(any("broken local link" in error for error in errors))
        self.assertTrue(any("duplicate id REQ-0001" in error for error in errors))

    def test_rejects_self_review_and_open_major(self) -> None:
        errors = self.errors("invalid_review")
        self.assertTrue(any("approved REVIEW has open" in error for error in errors))
        self.assertTrue(any("lacks an approved Review" in error for error in errors))

    def test_rejects_nested_formal_record_and_malformed_links(self) -> None:
        errors = self.errors("nested_malformed")
        self.assertTrue(any("invalid status" in error for error in errors))
        self.assertTrue(any("links must be a bracketed list" in error for error in errors))

    def test_parses_fullwidth_task_separator_and_rejects_open_task(self) -> None:
        errors = self.errors("open_fullwidth_task")
        self.assertTrue(any("completed Requirement has open Tasks" in error for error in errors))
        self.assertFalse(any("no parseable Task IDs" in error for error in errors))

    def test_rejects_invalid_review_revision(self) -> None:
        errors = self.errors("invalid_revision")
        self.assertTrue(any("reviewed_revision must be a Git commit ID" in error for error in errors))

    def test_rejects_review_of_undeclared_spec(self) -> None:
        errors = self.errors("spec_mismatch")
        self.assertTrue(any("lacks an approved Review" in error for error in errors))

    def test_rejects_completed_work_without_validation(self) -> None:
        errors = self.errors("missing_validation")
        self.assertTrue(any("completed work missing VALIDATION.md" in error for error in errors))

    def test_rejects_finding_counter_mismatch(self) -> None:
        errors = self.errors("finding_mismatch")
        self.assertTrue(any("open_majors does not match" in error for error in errors))
        self.assertTrue(any("approved REVIEW has open" in error for error in errors))
        self.assertTrue(any("invalid Finding ID" in error for error in errors))
        self.assertTrue(any("Major Finding cannot use accepted" in error for error in errors))

    @mock.patch.object(check_docs, "changed_paths_since", return_value={"src/changed_after_review.rs"})
    def test_rejects_stale_review_revision(self, _changed_paths) -> None:
        errors = self.errors("valid_completed")
        self.assertTrue(any("reviewed revision is stale" in error for error in errors))

    def test_rejects_empty_validation(self) -> None:
        errors = self.errors("empty_validation")
        self.assertTrue(any("no parseable validation result rows" in error for error in errors))

    def test_rejects_shared_work_directory(self) -> None:
        errors = self.errors("shared_work")
        self.assertTrue(any("claimed by multiple Requirements" in error for error in errors))

    def test_rejects_unidentified_checkbox(self) -> None:
        errors = self.errors("malformed_task")
        self.assertTrue(any("every checkbox must use a valid Task ID" in error for error in errors))

    def test_requirement_acceptance_change_invalidates_review(self) -> None:
        path = FIXTURES / "valid_completed" / "docs" / "requirements" / "req.md"
        current_text = path.read_text(encoding="utf-8")
        metadata, error = check_docs.parse_frontmatter(Path("docs/requirements/req.md"), current_text)
        self.assertIsNone(error)
        assert metadata is not None
        current = check_docs.Record(Path("docs/requirements/req.md"), metadata, current_text)
        previous_text = current_text.replace("# Valid", "# Different acceptance contract")
        self.assertFalse(check_docs.closure_only_requirement_change(previous_text, current))

    def test_rejects_all_skipped_validation(self) -> None:
        path = FIXTURES / "all_skipped" / "VALIDATION.md"
        _, _, errors = check_docs.validation_results(path)
        self.assertTrue(any("has no passed result" in error for error in errors))

    def test_rejects_malformed_skipped_row_after_pass(self) -> None:
        path = FIXTURES / "malformed_skipped" / "VALIDATION.md"
        _, _, errors = check_docs.validation_results(path)
        self.assertTrue(any("malformed validation result row" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
