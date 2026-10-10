#!/usr/bin/env python3
"""Mutation controls for evidence references and honest parity accounting."""

import copy
import importlib.util
import json
from pathlib import Path
import re
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("check-parity-ledger.py")
SPEC = importlib.util.spec_from_file_location("parity_ledger", SCRIPT)
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)
REPO = SCRIPT.parent.parent


class LedgerControls(unittest.TestCase):
    def setUp(self):
        self.ledger = json.loads((REPO / "conformance/parity-features.json").read_text())
        self.markdown = (REPO / "docs/tlc-parity.md").read_text()

    def reject(self, mutation):
        ledger = copy.deepcopy(self.ledger)
        mutation(ledger)
        with self.assertRaises(ValueError):
            CHECK.validate(REPO, ledger, self.markdown)

    def test_current_census_and_individual_evidence(self):
        counts = CHECK.validate(REPO, self.ledger, self.markdown)
        self.assertEqual(sum(counts.values()), 37)

    def test_stable_ids_cannot_be_dropped_duplicated_or_reordered(self):
        for mutation in (
            lambda d: d["families"].pop(),
            lambda d: d["families"][1].update(id="F-001"),
            lambda d: d["families"].reverse(),
        ):
            with self.subTest(mutation=mutation):
                self.reject(mutation)

    def test_unknown_or_self_interactions_fail(self):
        for interactions in (["F-999"], ["F-001"], ["F-002", "F-002"]):
            self.reject(lambda d: d["families"][0].update(interactions=interactions))

    def test_evidence_is_an_existing_individual_repository_path(self):
        for path in ("docs/absent-evidence.md", "/tmp/evidence", "../README.md", "README.md; CHANGELOG.md"):
            with self.subTest(path=path):
                self.reject(lambda d: d["families"][0].update(conformance_evidence=path))
        self.reject(lambda d: d["families"][0].update(additional_evidence=["docs/absent.md"]))

    def test_symlink_evidence_cannot_escape(self):
        with tempfile.TemporaryDirectory(prefix="dive-ledger-control-") as scratch:
            scratch = Path(scratch)
            root = scratch / "repo"
            root.mkdir()
            outside = scratch / "outside"
            outside.write_text("not repository evidence")
            (root / "linked").symlink_to(outside)
            with self.assertRaisesRegex(ValueError, "escapes repository"):
                CHECK.evidence(root, "linked", "control")

    def test_status_and_certification_cannot_silently_credit_parity(self):
        self.reject(lambda d: d["families"][0].update(implementation_status="complete-ish"))
        self.reject(lambda d: d["families"][0].update(certification="certified"))
        self.reject(lambda d: d["families"][0].update(phase=True))
        self.reject(lambda d: d["families"][0].update(issues=[True]))

    def test_table_and_census_must_track_json(self):
        for markdown in (
            self.markdown.replace("| implemented | 1 |", "| partial | 1 |", 1),
            re.sub(r"(stable feature families\*\*: )[\w-]+", r"\g<1>999", self.markdown, count=1),
            self.markdown.replace("| F-001 |", "| F-999 |", 1),
        ):
            with self.subTest(markdown=markdown[:30]), self.assertRaises(ValueError):
                CHECK.validate(REPO, self.ledger, markdown)

    def test_schema_and_source_shape_fail_closed(self):
        for mutation in (
            lambda d: d.update(schema_version=2),
            lambda d: d.update(schema_version=True),
            lambda d: d.update(baseline_commit="moving-main"),
            lambda d: d["families"].__setitem__(0, "invalid row"),
            lambda d: d["families"][0].update(pinned_source="../Tool.java"),
        ):
            self.reject(mutation)


if __name__ == "__main__":
    unittest.main()
