#!/usr/bin/env python3
"""The single named EWD exception must not admit arbitrary refusals."""

import importlib.util
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True

SPEC = importlib.util.spec_from_file_location(
    "bare_use_smoke", Path(__file__).with_name("bare-use-smoke.py")
)
SMOKE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SMOKE)


class RefusalTests(unittest.TestCase):
    def test_only_exact_ewd_cases_are_admitted(self):
        report = {
            "verdict": "unsupported",
            "engine": "core",
            "message": "an operator parameter of a recursive operator (M1 core)",
        }
        self.assertTrue(SMOKE.expected_ewd_refusal("ewd998/EWD998ChanID", 3, report))
        for model in SMOKE.MODELS:
            if model != "ewd998/EWD998ChanID":
                self.assertFalse(SMOKE.expected_ewd_refusal(model, 3, report))
        for field, value in [
            ("message", "another unsupported feature"),
            ("message", report["message"] + " with unproved context"),
            ("verdict", "evaluation-error"),
            ("engine", "auto"),
        ]:
            self.assertFalse(SMOKE.expected_ewd_refusal(
                "ewd998/EWD998ChanID", 3, {**report, field: value}
            ))
        self.assertFalse(SMOKE.expected_ewd_refusal("ewd998/EWD998ChanID", 13, report))


if __name__ == "__main__":
    unittest.main()
