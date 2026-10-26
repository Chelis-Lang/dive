#!/usr/bin/env python3
"""Controls for the model-mutation campaign's edits and seeded selection."""

import importlib.util
from pathlib import Path
import random
import unittest

SCRIPT = Path(__file__).with_name("model-mutation-campaign.py")
SPEC = importlib.util.spec_from_file_location("model_mutation_campaign", SCRIPT)
CAMPAIGN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAMPAIGN)
REPO = SCRIPT.parent.parent
SEMA = CAMPAIGN.load_script(REPO / "crates/dive-sema/acceptance/mutate.py", "semantic_mutations")

SOURCE = """---- MODULE M ----
EXTENDS Naturals
VARIABLES x, f
\\* x < 3 /\\ TRUE in a comment
Init == /\\ x = 0
        /\\ f = [i \\in 1..2 |-> 0]
Next == \\/ /\\ x < 3
           /\\ x' = x + 1
           /\\ f' = [f EXCEPT ![1] = 2]
        \\/ /\\ x >= 3
           /\\ UNCHANGED <<x, f>>
Inv == x # 9 /\\ "a<b" = "a<b" /\\ TRUE
====
"""


def apply(text, site):
    _, start, end, new = site
    return text[:start] + new + text[end:]


class MutationTests(unittest.TestCase):
    def setUp(self):
        self.sites = CAMPAIGN.sites_of(SOURCE, SEMA)

    def test_every_class_has_a_site(self):
        self.assertEqual({s[0] for s in self.sites}, set(CAMPAIGN.CLASSES))

    def test_operator_edits_keep_every_bullet_column(self):
        def bullets(text):
            return [[i for i in range(len(l)) if l[i:i + 2] in ("/\\", "\\/")]
                    for l in text.splitlines()]
        for site in self.sites:
            if site[0].startswith("drop-") or site[0] == "logic":
                continue
            mutated = apply(SOURCE, site)
            self.assertNotEqual(mutated, SOURCE, site)
            self.assertEqual(bullets(mutated), bullets(SOURCE), site)

    def test_comments_strings_and_except_paths_are_never_edited(self):
        comment = SOURCE.index("\\* x")
        strings = [SOURCE.index('"a<b"'), SOURCE.rindex('"a<b"')]
        except_eq = SOURCE.index("![1] =") + len("![1] ")
        for _, start, end, _ in self.sites:
            self.assertFalse(comment <= start < SOURCE.index("\n", comment))
            self.assertFalse(any(s <= start < s + len('"a<b"') for s in strings))
            self.assertNotEqual(start, except_eq)

    def test_drops_replace_whole_items_and_keep_bullet_columns(self):
        drops = [s for s in self.sites if s[0].startswith("drop-")]
        self.assertTrue(drops)
        for site in drops:
            mutated = apply(SOURCE, site)
            unit = "TRUE" if site[0] == "drop-conjunct" else "FALSE"
            self.assertIn(unit, site[3])
            line = site[3].rstrip("\n")
            original = SOURCE[site[1]:].splitlines()[0]
            self.assertEqual(len(line) - len(line.lstrip()), len(original) - len(original.lstrip()))
            self.assertTrue(mutated.endswith("====\n"))
        # The first disjunct shares its line with `Next ==`; the second
        # is dropped whole, its nested conjunction included.
        disjunct = next(s for s in drops if s[0] == "drop-disjunct")
        self.assertIn("UNCHANGED", SOURCE[disjunct[1]:disjunct[2]])

    def test_selection_is_seeded_and_spreads_over_classes(self):
        sites = [(Path("M.tla"), *s) for s in self.sites]
        a = CAMPAIGN.pick(list(sites), random.Random(7), 8)
        b = CAMPAIGN.pick(list(sites), random.Random(7), 8)
        self.assertEqual(a, b)
        self.assertEqual(len({s[1] for s in a}), 8)


if __name__ == "__main__":
    unittest.main()
