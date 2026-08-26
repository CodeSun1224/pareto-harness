from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parents[1]


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


filters = load("assert_cargo_test_filter")
scope = load("check_req0007_scope")


class Req0007HelperTests(unittest.TestCase):
    def test_filter_count_rejects_zero_and_ignores_non_tests(self) -> None:
        output = "alpha::one: test\nalpha::bench: benchmark\nbeta::two: test\n"
        self.assertEqual(["alpha::one"], filters.matching_tests(output, "alpha"))
        self.assertEqual([], filters.matching_tests(output, "missing"))

    def test_extracts_multiline_frozen_constant(self) -> None:
        source = 'const FIRST: &str = r#"a;b"#;\nconst SECOND: i64 = 2;\n\nstruct Next;\n'
        self.assertEqual('const FIRST: &str = r#"a;b"#;', scope.extract_const(source, "FIRST"))
        self.assertEqual("const SECOND: i64 = 2;", scope.extract_const(source, "SECOND"))


if __name__ == "__main__":
    unittest.main()
