"""Regression for cargo-build-sbf's nonfatal frame-overflow diagnostic."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("sbf_stack", Path(__file__).parents[1] / "check-sbf-stack.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class StackDiagnostics(unittest.TestCase):
    def test_actual_nonfatal_diagnostic_is_rejected(self):
        line = "Error: Function report_decode overflows the maximum allowed frame space. Estimated function frame size: 5312 bytes."
        self.assertEqual(MODULE.violations(line), [(1, line)])

    def test_legacy_and_estimate_only_diagnostics(self):
        for line in ["Stack offset of 5136 exceeded max offset of 4096 by 1040 bytes",
                     "Estimated function frame size: 4097 bytes.",
                     "stack frame size 7000 exceeds maximum size"]:
            self.assertTrue(MODULE.violations(line), line)
        self.assertFalse(MODULE.violations("Estimated function frame size: 4096 bytes."))

    def test_clean_build_does_not_receive_false_qualification_for_absent_log(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "build.log"
            self.assertEqual(MODULE.main([str(path)]), 2)
            path.write_text("")
            self.assertEqual(MODULE.main([str(path)]), 2)
            path.write_text("Compiling shortwin\nFinished release profile\n")
            self.assertEqual(MODULE.main([str(path)]), 0)
            path.write_text("Estimated function frame size: 5312 bytes.\nFinished release profile\n")
            self.assertEqual(MODULE.main([str(path)]), 1)


if __name__ == "__main__":
    unittest.main()
