#!/usr/bin/env python3
"""Controls for campaign accounting, source isolation and strict report handling."""

import copy
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("frontend-campaign.py")
SPEC = importlib.util.spec_from_file_location("frontend_campaign", SCRIPT)
CAMPAIGN = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAMPAIGN)
REPO = SCRIPT.parent.parent


class CampaignTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "source"
        self.source.mkdir()
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.spec = self.source / "specifications/simple"
        self.spec.mkdir(parents=True)
        (self.spec / "Base.tla").write_text(
            "---- MODULE Base ----\nEXTENDS Dep\nVARIABLE x\n"
            "Init == x=FALSE\nNext == x'=~x\nF(p) == p\nQ == F(x)\n====\n")
        (self.spec / "Dep.tla").write_text("---- MODULE Dep ----\nD == TRUE\n====\n")
        (self.spec / "Wrong.tla").write_text("---- MODULE Other ----\n====\n")
        (self.spec / "Bad.tla").write_bytes(b"\xff")
        self.git("init", "-q")
        self.git("config", "user.name", "Jeff Smith")
        self.git("config", "user.email", "jeff@2ndset.ai")
        self.git("add", ".")
        self.git("commit", "-qm", "test: campaign source")
        self.pin = self.git("rev-parse", "HEAD")
        conf = self.repo / "conformance"
        conf.mkdir()
        (conf / "corpora.toml").write_text('[[corpus]]\nid = "examples"\ncommit = "' + self.pin + '"\n')
        (conf / "oracles.toml").write_text("# test oracle config\n")
        for crate in ["dive-syntax", "dive-sema"]:
            scripts = self.repo / "crates" / crate / "acceptance"
            scripts.mkdir(parents=True)
            shutil.copyfile(REPO / "crates" / crate / "acceptance/mutate.py", scripts / "mutate.py")
            cases = scripts.parent / "tests/sany-cases"
            cases.mkdir(parents=True)
            (cases / "Case.tla").write_text("---- MODULE Case ----\nD == TRUE\n====\n")

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.source), *args], text=True).strip()

    def prepare(self, name="out"):
        return CAMPAIGN.prepare(self.repo, self.source, self.root / name, 101, 2, 2)

    def test_replay_and_plain_dependency_copies_preserve_source(self):
        before = {str(p): p.read_bytes() for p in self.spec.glob("*.tla")}
        a, b = self.prepare("a"), self.prepare("b")
        self.assertEqual(a, b)
        self.assertEqual(before, {str(p): p.read_bytes() for p in self.spec.glob("*.tla")})
        self.assertEqual(self.git("status", "--porcelain"), "")
        self.assertEqual(len(a["source_census"]), 4)
        self.assertEqual({r["selection"] for r in a["source_census"]},
                         {"selected", "invalid-utf8", "header-name-mismatch"})
        self.assertEqual(len(a["mutation_attempts"]), 6)
        corpus = self.root / "a/conformance/corpus/frontend-campaign"
        for entry in a["modules"]:
            target = corpus / entry["module"]
            self.assertFalse(target.is_symlink())
            self.assertEqual(CAMPAIGN.digest(target.read_bytes()), entry["sha256"])
            if entry["source"].endswith("Base.tla"):
                self.assertEqual((target.parent / "Dep.tla").read_bytes(), before[str(self.spec / "Dep.tla")])
        targets = a["modules"]
        self.assertEqual(sum(r["kind"] == "sany-case" for r in targets), 2)
        self.assertEqual(len(targets), sum(r["generated"] for r in a["mutation_attempts"]) + 2)
        self.assertEqual((self.root / "a/selection.json").read_bytes(), (self.root / "b/selection.json").read_bytes())

    def test_wrong_or_dirty_source_pin_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "pinned commit"):
            CAMPAIGN.verify_source(self.source, "0" * 40)
        (self.spec / "Dep.tla").write_text("changed")
        with self.assertRaisesRegex(ValueError, "pristine"):
            self.prepare()
        self.assertFalse((self.root / "out").exists())

    def test_cache_links_and_output_inside_sources_are_rejected(self):
        # A tracked link also has a clean git status; path checks must catch it.
        (self.spec / "Linked.tla").symlink_to(self.spec / "Base.tla")
        self.git("add", ".")
        self.git("commit", "-qm", "test: source link")
        self.pin = self.git("rev-parse", "HEAD")
        (self.repo / "conformance/corpora.toml").write_text('[[corpus]]\nid = "examples"\ncommit = "' + self.pin + '"\n')
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.prepare()
        self.assertFalse((self.root / "out").exists())
        for out in [self.source / "campaign", self.repo / "campaign"]:
            with self.assertRaisesRegex(ValueError, "outside"):
                CAMPAIGN.prepare(self.repo, self.source, out, 101, 2, 2)

    def test_existing_output_and_missing_case_suite_fail(self):
        self.prepare()
        before = (self.root / "out/selection.json").read_bytes()
        with self.assertRaisesRegex(ValueError, "must not exist"):
            self.prepare()
        self.assertEqual((self.root / "out/selection.json").read_bytes(), before)
        shutil.rmtree(self.repo / "crates/dive-sema/tests/sany-cases")
        with self.assertRaisesRegex(ValueError, "missing SANY cases"):
            self.prepare("missing")

    def test_report_census_rejects_empty_duplicate_missing_and_extra_rows(self):
        entries = [{"module": "a/A.tla"}, {"module": "b/B.tla"}]
        report = {"models": [], "modules": [
            {"corpus": "frontend-campaign", "module": row["module"], "phase": "full", "outcome": "agree"}
            for row in entries]}
        self.assertEqual(CAMPAIGN.check_report(report, entries, "full"), {"agree": 2})
        for rows in [[], report["modules"][:1], [report["modules"][0]] * 2,
                     report["modules"] + [{"module": "c/C.tla"}]]:
            bad = {**report, "modules": rows}
            with self.assertRaisesRegex(ValueError, "omitted, duplicated or added"):
                CAMPAIGN.check_report(bad, entries, "full")
        for key, value in [("phase", "parse"), ("corpus", "examples")]:
            bad = copy.deepcopy(report)
            bad["modules"][0][key] = value
            with self.assertRaisesRegex(ValueError, "corpus/phase"):
                CAMPAIGN.check_report(bad, entries, "full")
        bad = {**report, "models": [{}]}
        with self.assertRaisesRegex(ValueError, "unexpectedly checked models"):
            CAMPAIGN.check_report(bad, entries, "full")

    def test_oracle_error_timeout_and_refusal_never_become_agreement(self):
        entries = [{"module": "A.tla"}]
        for outcome in ["oracle-error", "timeout", "unsupported", "out-of-scope", "disagree"]:
            report = {"models": [], "modules": [{"corpus": "frontend-campaign", "module": "A.tla", "phase": "parse", "outcome": outcome}]}
            totals = CAMPAIGN.check_report(report, entries, "parse")
            self.assertEqual(totals, {outcome: 1})
            self.assertNotEqual(set(totals), {"agree"})

    def test_parser_protocol_rejects_contradictory_and_unrecognized_results(self):
        base = {"schema_version": 1, "command": "parse", "accepted": True, "diagnostics": []}
        self.assertEqual(CAMPAIGN.parse_document(base, 0), "accept")
        self.assertEqual(CAMPAIGN.parse_document({**base, "schema_version": 2}, 0), "accept")
        self.assertEqual(CAMPAIGN.parse_document({**base, "schema_version": 3}, 0), "accept")
        mismatch = {**base, "accepted": False, "diagnostics": [{"severity": "error", "code": "DIVE-S0006"}]}
        self.assertEqual(CAMPAIGN.parse_document(mismatch, 1), "name-mismatch")
        reject = {**mismatch, "diagnostics": [{"severity": "error", "code": "DIVE-S0001"}]}
        self.assertEqual(CAMPAIGN.parse_document(reject, 1), "reject")
        for doc, status in [(base, 1), (mismatch, 0), ({**base, "accepted": False}, 0),
                            ({**reject, "accepted": True}, 1), ({**base, "schema_version": 99}, 0),
                            ({**base, "command": "analyze"}, 0), ({**base, "diagnostics": None}, 0),
                            ({**mismatch, "diagnostics": [{"severity": "error", "code": "DIVE-E0009"}]}, 1)]:
            with self.assertRaises(ValueError):
                CAMPAIGN.parse_document(doc, status)

    def test_pinned_grammar_bytecode_matches_the_reviewed_source(self):
        java = REPO / "scripts/java"
        self.assertEqual(CAMPAIGN.digest((java / "DiveSanyParse.java").read_bytes()), CAMPAIGN.PARSER_SOURCE_SHA)
        data = (java / "DiveSanyParse.class").read_bytes()
        self.assertEqual(CAMPAIGN.digest(data), CAMPAIGN.PARSER_CLASS_SHA)
        self.assertEqual(data[:4], b"\xca\xfe\xba\xbe")
        self.assertEqual(int.from_bytes(data[6:8], "big"), 61)  # Java 17


if __name__ == "__main__":
    unittest.main()
