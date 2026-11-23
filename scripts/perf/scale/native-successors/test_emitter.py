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
        self.assertIn("next_0(m,c,s,r)?;next_0(m,c,s,r)?;", rust)
        self.assertEqual(shape, "Guard(4)\nDisj([0, 0])\n")
        self.assertEqual(counts["roots"], 1)

    def test_mixed_unchanged_uses_the_original_generator(self):
        rust, _, _ = lower("root 0\nnode Unchanged([Var { var: 1, at: 0 }, Verify(2)])\n")
        self.assertIn("generic(m,c,s,r,0,&[])", rust)

    @staticmethod
    def choices(last_slot=2):
        nodes = [
            f"LabelGuard({i}, LabelTest {{ table: 0, slot: {slot}, field: Str#8, selector: 1, label: Str#{label} }})"
            for i, (slot, label) in enumerate([(2, 6), (2, 7), (last_slot, 6)])
        ]
        nodes += ["Disj([1, 0])", "Disj([3, 2])"]
        return "root 4\n" + "".join(f"node {node}\n" for node in nodes)

    def test_shared_discriminator_preserves_duplicate_survivors(self):
        rust, _, _ = lower(self.choices())
        self.assertIn("let active=(match label0{7=>1u32,6=>6u32,_=>0u32})", rust)
        masked = rust.split("let active=", 1)[1]
        self.assertLess(masked.index("active & 1u32"), masked.index("active & 2u32"))
        self.assertLess(masked.index("active & 2u32"), masked.index("active & 4u32"))
        self.assertIn("if c.native_blocks.enabled && m.assigned==0", rust)
        self.assertIn("m.label_matches(c,*p,*x)", rust)

    def test_scalar_label_keeps_its_resolved_table_position(self):
        scalar = self.choices().replace("field: Str#8", "field: Empty")
        self.assertIn("r.scalar_label(m,c,*p,*x)", lower(scalar)[0])
        self.assertNotIn("r.scalar_label(m,c,*p,*x)", lower(self.choices())[0])

    def test_different_actor_slots_use_original_alternatives(self):
        self.assertNotIn("let active=", lower(self.choices(last_slot=4))[0])

    def test_large_shared_disjunction_uses_ordinary_continuations(self):
        nodes = ["Guard(0)"] + [f"Disj([{g},{g}])" for g in range(40)]
        plan = "root 40\n" + "".join(f"node {node}\n" for node in nodes)
        rust, _, counts = lower(plan)
        self.assertNotIn("let active=", rust)
        self.assertLessEqual(counts["continuations"], 41)

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
