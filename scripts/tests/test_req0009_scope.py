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


scope = load("check_req0009_scope")


class Req0009ScopeTests(unittest.TestCase):
    def test_uses_exact_accepted_baseline(self) -> None:
        self.assertEqual(
            "60cee6ed44d150185bf99ca3095a8ce803bcc0d3", scope.APPROVED_BASELINE
        )

    def test_extracts_frozen_constant(self) -> None:
        source = 'const FIRST: &str = r#"a;b"#;\nconst SECOND: i64 = 2;\n\nstruct Next;\n'
        self.assertEqual(
            'const FIRST: &str = r#"a;b"#;', scope.extract_const(source, "FIRST")
        )

    def test_forbids_real_runtime_boundaries(self) -> None:
        self.assertIn("tokio::time::sleep", scope.FORBIDDEN_BOUNDARIES)
        self.assertIn("std::process::Command", scope.FORBIDDEN_BOUNDARIES)
        self.assertIn("WASI", scope.FORBIDDEN_BOUNDARIES)


if __name__ == "__main__":
    unittest.main()
