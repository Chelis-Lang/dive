"""Small controls for occurrence order, opaque generic plans and input bounds."""
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from emitter import lower
from prepare import EXPORT_MARKER, cached_binary, compiler_environment, require_export


class EmitterTest(unittest.TestCase):
    def test_duplicate_arms_emit_twice(self):
        rust, shape, counts = lower("root 1\nnode Guard(4)\nnode Disj([0, 0])\n")
        self.assertIn("next_0(m,c,s)?;next_0(m,c,s)?;", rust)
        self.assertEqual(shape, "Guard(4)\nDisj([0, 0])\n")
        self.assertEqual(counts["roots"], 1)

    def test_mixed_unchanged_uses_the_original_generator(self):
        rust, _, _ = lower("root 0\nnode Unchanged([Var { var: 1, at: 0 }, Verify(2)])\n")
        self.assertIn("generic(m,c,s,0,&[])", rust)

    def test_operand_text_never_becomes_source(self):
        with self.assertRaises(ValueError):
            lower("root 0\nnode Disj(['arbitrary_text'])\n")
        with self.assertRaises(ValueError):
            lower("root -1\nnode Guard(0)\n")


class PreparationTest(unittest.TestCase):
    def test_corrupt_manifest_refuses(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            (folder / "dive").write_bytes(b"worker")
            for value in ({}, None, [], {"binary_sha256": 7}, {"binary_sha256": "short"}):
                with self.subTest(value=value):
                    (folder / "build.json").write_text(json.dumps(value))
                    with self.assertRaisesRegex(ValueError, "invalid cached executable manifest"):
                        cached_binary(folder)

    def test_export_requires_its_specific_process_result(self):
        report = {"verdict": "unsupported", "message": EXPORT_MARKER}
        require_export(3, report)
        for status in (0, 1, -9):
            with self.assertRaisesRegex(ValueError, "plan unavailable"):
                require_export(status, report)
        with self.assertRaisesRegex(ValueError, "plan unavailable"):
            require_export(3, {"verdict": "unsupported", "nested": report})

    def test_compiler_identity_uses_the_build_environment(self):
        with patch.dict("os.environ", {"RUSTUP_TOOLCHAIN": "other", "RUSTC_WRAPPER": "other"}):
            self.assertNotIn("RUSTUP_TOOLCHAIN", compiler_environment())
            self.assertNotIn("RUSTC_WRAPPER", compiler_environment())


if __name__ == "__main__":
    unittest.main()
