"""Integration controls for the original-source declared-orbit comparison."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from checked_orbit import Refused, run_json

SCRIPT = Path(__file__).with_name("checked_orbit.py")
DIVE = os.environ.get("DIVE_ORBIT_BIN")
SOURCE = r"""---- MODULE Workers ----
EXTENDS Naturals, FiniteSets
CONSTANT Actors, Fixed
VARIABLE done
Init == done = {}
Next == /\ Cardinality(done) < 1
        /\ \E p \in Actors : done' = done \cup {p}
TypeOK == done \subseteq Actors
====
"""
CONFIG = "INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\nCONSTANT Actors = {A,B,C} Fixed = X\nINVARIANT TypeOK\n"


class ReportStatus(unittest.TestCase):
    def test_failed_process_cannot_attest_success(self):
        with tempfile.TemporaryDirectory() as raw:
            folder = Path(raw)
            checker = folder / "checker"
            for ending in ["exit 17", "kill -TERM $$"]:
                checker.write_text("#!/bin/sh\nprintf '%s\\n' '{\"verdict\":\"success\"}'\n" + ending + "\n")
                checker.chmod(0o700)
                with self.assertRaises(Refused):
                    run_json(str(checker), ["check"], folder, 5)


@unittest.skipUnless(DIVE, "set DIVE_ORBIT_BIN to the built dive CLI")
class Orbit(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.folder = Path(self.tmp.name)

    def run_case(self, source=SOURCE, cfg=CONFIG, *flags):
        module = self.folder / "Workers.tla"
        conf = self.folder / "Workers.cfg"
        module.write_text(source)
        conf.write_text(cfg)
        result = subprocess.run([sys.executable, SCRIPT, module, "--config", conf,
                                 "--carrier", "Actors", "--workers", "1", "--dive", DIVE, *flags],
                                capture_output=True, timeout=30)
        self.assertEqual(module.read_text(), source)
        self.assertEqual(conf.read_text(), cfg)
        return result.returncode, json.loads(result.stdout)

    def test_source_changes_recompute_labeled_multiplicity(self):
        grew = SOURCE.replace("done) < 1", "done) < 2")
        prop = SOURCE.replace("done \\subseteq Actors", "TRUE /\\ done \\subseteq Actors")
        for source, distinct, generated, depth in [(SOURCE, 4, 4, 2), (prop, 4, 4, 2),
                                                   (grew, 7, 13, 3), (SOURCE, 4, 4, 2)]:
            rc, row = self.run_case(source)
            self.assertEqual((rc, row["status"]), (0, "verified-observables"), row)
            self.assertEqual(int(row["reduced"]["census"]["labeled_orbit_union"]), distinct)
            self.assertEqual(int(row["reduced"]["census"]["weighted_generated"]), generated)
            self.assertEqual(row["unreduced"]["statistics"]["depth"], depth)
        rc, row = self.run_case(SOURCE, CONFIG, "--assume-equivariance")
        self.assertEqual((rc, row["status"]), (3, "conditional-orbit"), row)
        self.assertNotIn("unreduced", row)

    def test_asymmetric_edits_and_shadowed_choice_refuse(self):
        picked = "Chosen == CHOOSE a \\in Actors : TRUE\n"
        prop = SOURCE.replace("TypeOK ==", picked + "TypeOK == Chosen \\notin done /\\")
        guard = SOURCE.replace("Next ==", picked + "Next ==").replace("done' = done", "p = Chosen /\\ done' = done")
        shadow = SOURCE.replace("TypeOK ==", "Pick(S) == CHOOSE p \\in S : TRUE\nTypeOK == Pick(Actors) \\notin done /\\")
        for source, cfg, reason in [
            (SOURCE, CONFIG.replace("Fixed = X", "Fixed = A"), "distinguishes"),
            (prop, CONFIG, "pointwise-fixed"), (guard, CONFIG, "pointwise-fixed"),
            (shadow, CONFIG, "direct fixed constant"),
        ]:
            rc, row = self.run_case(source, cfg, "--assume-equivariance")
            self.assertEqual((rc, row["status"]), (2, "refused"), row)
            self.assertIn(reason, row["reason"])

    def test_instantiated_choice_does_not_inherit_unrelated_root_binding(self):
        (self.folder / "Picker.tla").write_text(
            "---- MODULE Picker ----\nCONSTANT Fixed\nPick == CHOOSE a \\in Fixed : TRUE\n====\n")
        source = r"""---- MODULE Workers ----
CONSTANT Actors, Fixed
VARIABLE owner
H == INSTANCE Picker WITH Fixed <- Actors
Init == owner = Fixed
Next == owner' \in Actors
TypeOK == owner = Fixed \/ owner = H!Pick
====
"""
        rc, row = self.run_case(source, CONFIG.replace("Fixed = X", "Fixed = {X}"), "--assume-equivariance")
        self.assertEqual((rc, row["status"]), (2, "refused"), row)
        self.assertIn("CHOOSE", row["reason"])

    def test_portable_request_and_pointwise_choice(self):
        source = SOURCE.replace("TypeOK ==", "Pick == CHOOSE v \\in Fixed : TRUE\nTypeOK == Pick \\in Fixed /\\")
        rc, row = self.run_case(source, CONFIG.replace("Fixed = X", "Fixed = {X}"), "--prepare-only")
        self.assertEqual((rc, row["status"]), (3, "prepared"), row)
        self.assertNotIn("reduced", row)
        inputs = row["inputs"]
        (self.folder / inputs["wrapper_module"]).write_text(inputs["wrapper_source"])
        cfg = self.folder / "WorkerOrbit.cfg"
        cfg.write_bytes((self.folder / "Workers.cfg").read_bytes() + inputs["config_suffix"].encode())
        second = subprocess.run([DIVE, "check", inputs["wrapper_module"], "--config", cfg.name,
                                 "--workers", "1", "--store", "exact", "--compat=tlc", "--output", "json"],
                                cwd=self.folder, env={**os.environ, "DIVE_CORE_OPT": "+orbit-census"},
                                capture_output=True)
        checked = json.loads(second.stdout)
        self.assertEqual(checked["core"]["metrics"]["orbit_census"]["labeled_orbit_union"], "4")
        rc, row = self.run_case(SOURCE, CONFIG.replace("Fixed = X", "Fixed = {{A,B},{B,C},{C,A}}"))
        self.assertEqual((rc, row["status"]), (0, "verified-observables"), row)

    def test_partial_order_sensitive_predicate_never_gets_verified(self):
        source = r"""---- MODULE Workers ----
EXTENDS Naturals
CONSTANT Actors, Fixed
VARIABLE owner
Init == owner = Fixed
Next == owner' \in Actors
TypeOK == owner = Fixed \/ (\E a \in Actors : a = owner \/ 1 \div 0 = 0)
====
"""
        rc, row = self.run_case(source)
        self.assertEqual((rc, row["status"]), (2, "refused"), row)
        self.assertIn("inconclusive", row["reason"])

    def test_false_property_has_an_independent_original_word(self):
        src = SOURCE.replace("====", "Never == done = {}\n====")
        cfg = CONFIG + "INVARIANT Never\n"
        rc, row = self.run_case(src, cfg)
        self.assertEqual((rc, row["status"]), (1, "validated-counterexample"), row)
        for name in ("unreduced", "reduced"):
            self.assertEqual(row[name]["property"], "Never")
            self.assertEqual(row[name]["original_trace_replay"], "valid")
        word = row["reduced"]["trace"]
        word["states"][-1]["values"]["done"] = 99
        trace = self.folder / "bad.json"
        trace.write_text(json.dumps(word))
        result = subprocess.run([DIVE, "validate-trace", "Workers.tla", "--config", "Workers.cfg",
                                 "--trace", "bad.json", "--compat=tlc", "--seed", "0", "--fp", "0",
                                 "--output", "json"], cwd=self.folder, capture_output=True)
        self.assertEqual(json.loads(result.stdout)["result"], "invalid")


if __name__ == "__main__":
    unittest.main()
