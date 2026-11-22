"""Small controls for occurrence order, opaque generic plans and input bounds."""
import unittest
from emitter import lower


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


if __name__ == "__main__":
    unittest.main()
