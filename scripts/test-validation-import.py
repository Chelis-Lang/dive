#!/usr/bin/env python3
"""Check import accounting and reject corrupted or misclassified models."""

import copy
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("import-validation-suite.py")
SPEC = importlib.util.spec_from_file_location("validation_import", SCRIPT)
IMPORT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(IMPORT)
CORPUS = SCRIPT.parent.parent / "conformance/corpus/validation-smoke"


class ImportTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="dive-import-control-")
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        self.source = self.root / "specification"
        self.source.mkdir()
        self.metadata = json.loads((CORPUS / "specification.json").read_text())
        self.selection = json.loads((CORPUS / "selection.json").read_text())
        for row in self.selection["cases"]:
            if row["status"] == "imported":
                model = self.metadata["testcases"][row["id"]]["tlc"]
                shutil.copytree(CORPUS / "specifications" / row["id"], self.source / model["path"], dirs_exist_ok=True)
        self.case_id = next(row["id"] for row in self.selection["cases"] if row["status"] == "imported")

    def run_import(self, metadata):
        return IMPORT.import_cases(self.source, metadata, self.root / "result")

    def test_complete_census_and_sources(self):
        result = self.run_import(self.metadata)
        self.assertEqual(result, self.selection)
        self.assertEqual(sum(result["counts"].values()), 36)
        self.assertEqual(result["counts"], {"imported": 16, "out-of-scope": 16, "upstream-skipped": 4})
        gate = SCRIPT.parent.parent / "conformance/gates/validation-smoke.txt"
        selected = [line.split("#")[0].strip() for line in gate.read_text().splitlines()]
        selected = [line for line in selected if line]
        self.assertEqual(sorted(selected), sorted(row["model"] for row in result["cases"] if row["status"] == "imported"))
        for row in result["cases"]:
            if row["status"] == "imported":
                folder = Path("specifications") / row["id"]
                for path in (CORPUS / folder).iterdir():
                    self.assertEqual(path.read_bytes(), (self.root / "result" / folder / path.name).read_bytes())

    def test_options_types_paths_and_digests_fail_closed(self):
        mutations = (
            lambda c: c.update(type="anomalous"),
            lambda c: c["tlc"].update(cmd_options=["-workers", "1", "-nowarning"]),
            lambda c: c["tlc"].update(path="../outside"),
            lambda c: c["tlc"].update(search_paths=["extra"]),
            lambda c: c["tlc"]["cfg"].update(digest="0" * 40),
            lambda c: c["tlc"]["cfg"].update(file="other.cfg"),
            lambda c: c["tlc"]["modules"][0].update(digest="0" * 40),
        )
        # Each control gets an independent destination; partial failure output
        # must never be reused as a valid generated corpus.
        for index, mutation in enumerate(mutations):
            metadata = copy.deepcopy(self.metadata)
            case = metadata["testcases"][self.case_id]
            mutation(case)
            with self.subTest(index=index), self.assertRaises(ValueError):
                IMPORT.import_cases(self.source, metadata, self.root / ("bad-" + str(index)))

    def test_empty_and_unaccounted_selections_are_errors(self):
        for metadata in (
            {"workers_options": ["1", "2"], "testcases": {}},
            {"workers_options": ["1", "auto"], "testcases": self.metadata["testcases"]},
            {**self.metadata, "new_field": True},
        ):
            with self.assertRaises(ValueError):
                self.run_import(metadata)

    def test_symlink_cannot_escape_generated_root(self):
        model = self.metadata["testcases"][self.case_id]["tlc"]
        folder = self.source / model["path"]
        outside = self.root / "outside"
        folder.rename(outside)
        folder.symlink_to(outside, target_is_directory=True)
        with self.assertRaises(ValueError):
            self.run_import(self.metadata)


if __name__ == "__main__":
    unittest.main()
