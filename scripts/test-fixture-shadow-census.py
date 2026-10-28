#!/usr/bin/env python3
"""Controls for complete fixture selection and strict shadow accounting."""

import copy
import importlib.util
import json
from pathlib import Path
import signal
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("fixture-shadow-census.py")
SPEC = importlib.util.spec_from_file_location("fixture_shadow_census", SCRIPT)
CENSUS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CENSUS)


class FixtureCensusTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / "repo"
        (self.repo / "conformance").mkdir(parents=True)
        (self.repo / "conformance/oracles.toml").write_text("# test pins\n")
        self.fixtures = self.repo / "crates/dive-check/tests/fixtures"
        self.fixtures.mkdir(parents=True)
        self.fixture("Named", ["Named", "Helper"])
        self.fixture("Alternate", ["Only"])
        self.fixture("Ambiguous", ["A", "B"])
        self.fixture("Missing", [])
        parser = self.repo / "crates/dive-config/tests/fixtures/Parser.cfg"
        parser.parent.mkdir(parents=True)
        parser.write_text("not a model\n")

    def fixture(self, name, modules):
        directory = self.fixtures / name
        directory.mkdir()
        (directory / (name + ".cfg")).write_text("INIT Init\nNEXT Next\n")
        for module in modules:
            (directory / (module + ".tla")).write_text(f"---- MODULE {module} ----\n====\n")

    def prepare(self):
        return CENSUS.prepare(self.repo, self.root / "out")

    def test_every_config_has_one_selection_and_inputs_are_copied_unchanged(self):
        selection = self.prepare()
        self.assertEqual(selection["selection_counts"], {
            "selected": 2, "skipped-ambiguous-module": 1,
            "skipped-missing-module": 1, "excluded-config-parser": 1})
        self.assertEqual(len({r["fixture"] for r in selection["fixtures"]}), 5)
        corpus = self.root / "out/conformance/corpus/fixture-shadow"
        for row in selection["fixtures"]:
            if row["selection"] == "selected":
                self.assertEqual((corpus / row["model"]).read_bytes(), (self.repo / row["fixture"]).read_bytes())
                self.assertEqual((corpus / CENSUS.stage_module(row["module"])).read_bytes(),
                                 (self.repo / row["module"]).read_bytes())
        manifest = json.loads((corpus / "specifications/dive-check/manifest.json").read_text())
        self.assertEqual(sum(len(m["models"]) for m in manifest["modules"]), 2)
        self.assertFalse((corpus / "specifications/dive-config").exists())

    def test_links_existing_output_and_output_inside_checkout_are_refused(self):
        with self.assertRaisesRegex(ValueError, "outside"):
            CENSUS.prepare(self.repo, self.repo / "out")
        linked = self.fixtures / "Named/Link.tla"
        linked.symlink_to(linked.with_name("Named.tla"))
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.prepare()
        self.assertFalse((self.root / "out").exists())
        linked.unlink()
        self.prepare()
        with self.assertRaisesRegex(ValueError, "must not exist"):
            self.prepare()

    def report(self, selection):
        rows = [{"corpus": CENSUS.CORPUS, "model": r["model"], "workers": w, "status": "agree"}
                for r in selection["fixtures"] if r["selection"] == "selected" for w in [1, 4]]
        return {"profile": "dive", "workers": [1, 4], "rows": rows, "summary": {"agree": 4}}

    def test_tally_counts_skipped_configs_per_worker_and_retains_findings(self):
        selection = self.prepare()
        report = self.report(selection)
        report["rows"][0].update(status="differ", diffs=["depth: core 0, reference 1"])
        report["summary"] = {"agree": 3, "differ": 1}
        totals, findings = CENSUS.tally(selection, report, [1, 4])
        self.assertEqual(sum(totals.values()), 10)
        self.assertEqual(findings[0]["diffs"], ["depth: core 0, reference 1"])
        self.assertTrue(findings[0]["fixture"].endswith(".cfg"))

    def test_missing_duplicate_extra_unknown_or_bad_summary_cannot_pass(self):
        selection = self.prepare()
        original = self.report(selection)
        variants = []
        missing = copy.deepcopy(original); missing["rows"].pop(); variants.append(missing)
        duplicate = copy.deepcopy(original); duplicate["rows"].append(duplicate["rows"][0]); variants.append(duplicate)
        extra = copy.deepcopy(original); extra["rows"][0]["workers"] = 8; variants.append(extra)
        status = copy.deepcopy(original); status["rows"][0]["status"] = "new-status"; variants.append(status)
        wrong = copy.deepcopy(original); wrong["summary"] = {"agree": 3}; variants.append(wrong)
        wrong_profile = copy.deepcopy(original); wrong_profile["profile"] = "tlc"; variants.append(wrong_profile)
        for report in variants:
            with self.assertRaises(ValueError):
                CENSUS.tally(selection, report, [1, 4])

    def test_recorder_preserves_seed_flags_output_exit_and_signal(self):
        binary = self.root / "fake-dive"
        binary.write_text("#!/usr/bin/env python3\nimport os,signal,sys\n"
                          "print('raw stdout', flush=True)\nprint('raw stderr', file=sys.stderr, flush=True)\n"
                          "if '--signal' in sys.argv: os.kill(os.getpid(), signal.SIGTERM)\n"
                          "sys.exit(13)\n")
        binary.chmod(0o755)
        wrapper = self.root / "wrapper"
        CENSUS.recording_wrapper(wrapper, binary, self.root / "raw")
        args = [str(wrapper), "check", "--seed", "0", "--fp", "0"]
        first = subprocess.run(args, capture_output=True)
        self.assertEqual(first.returncode, 13)
        self.assertEqual(first.stdout, b"raw stdout\n")
        self.assertEqual(first.stderr, b"raw stderr\n")
        raw = next((self.root / "raw").iterdir())
        command = json.loads((raw / "invocation.json").read_text())["command"]
        self.assertEqual(command[1:], args[1:])
        second = subprocess.run([*args, "--signal"], capture_output=True)
        self.assertEqual(second.returncode, -signal.SIGTERM)

    def test_native_stop_retains_its_physical_exit_phase_and_name(self):
        selection = self.prepare()
        selected = next(r for r in selection["fixtures"] if r["selection"] == "selected")
        out = self.root / "out"
        raw = out / "raw/stop"
        raw.mkdir(parents=True)
        command = ["dive", "check", "--config", str(out / "conformance/corpus/fixture-shadow" / selected["model"]),
                   "--engine", "core", "--workers", "1", "--seed", "0", "--fp", "0"]
        CENSUS.write_json(raw / "invocation.json", {"command": command, "profile": "default"})
        CENSUS.write_json(raw / "exit.json", {"exit_code": 5})
        CENSUS.write_json(raw / "stdout", {"verdict": "stopped", "stopped": {"resource": "time", "phase": "init"}})
        rows = CENSUS.incomplete_runs(out, selection)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["fixture"], selected["fixture"])
        self.assertEqual(rows[0]["physical_exit"], 5)
        self.assertEqual(rows[0]["stopped"], {"resource": "time", "phase": "init"})
        (raw / "exit.json").unlink()
        (raw / "stdout").write_text("partial output")
        incomplete = CENSUS.incomplete_runs(out, selection)[0]
        self.assertIsNone(incomplete["physical_exit"])
        self.assertIsNone(incomplete["verdict"])


if __name__ == "__main__":
    unittest.main()
