"""Exact occurrence, diagnostic and cross-process regression controls for both certificate methods."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

DRIVER = Path(__file__).with_name("checked_orbit.py")
BINARY = os.environ.get("DIVE_ORBIT_BIN")
GRAPH = os.environ.get("DIVE_ORBIT_GRAPH")
EXPORTER = os.environ.get("DIVE_ORBIT_EXPORTER")
MODULE = r"""---- MODULE Carrier ----
CONSTANT Actors, Fixed
VARIABLE x
TypeOK == x \in Actors \cup {Fixed}
Init == x = Fixed
Next == \E a \in Actors : x' = a
Safe == x = x
Step == [][x' \in Actors]_x
====
"""
CONFIG = "INIT Init\nNEXT Next\nCONSTANT Actors = {A,B,C} Fixed = X\nINVARIANT TypeOK Safe\nPROPERTY Step\nCHECK_DEADLOCK FALSE\n"


@unittest.skipUnless(BINARY and GRAPH and EXPORTER, "set DIVE_ORBIT_BIN, DIVE_ORBIT_GRAPH and DIVE_ORBIT_EXPORTER")
class CertifiedOrbit(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.folder = Path(self.tmp.name)
        self.key = self.folder / "authority"
        self.key.write_bytes(os.urandom(32))
        self.key.chmod(0o600)

    def audit(self, method, source=MODULE, config=CONFIG, cache=False):
        (self.folder / "Carrier.tla").write_text(source)
        (self.folder / "Carrier.cfg").write_text(config)
        args = [sys.executable, DRIVER, "Carrier.tla", "--config", "Carrier.cfg", "--carrier", "Actors",
                "--dive", BINARY, "--certify-equivariance", method, "--type-invariant", "TypeOK",
                "--graph-auditor", GRAPH, "--exporter", EXPORTER]
        if cache:
            args.extend(["--certificate-cache", "receipts"])
        run = subprocess.run(args, cwd=self.folder, env={**os.environ, "DIVE_ORBIT_PROOF_KEY": str(self.key)},
                             capture_output=True, timeout=30)
        doc = json.loads(run.stdout)
        self.assertEqual(run.returncode, {"certified-orbit": 0, "validated-counterexample": 1, "refused": 2}[doc["status"]])
        return doc

    def test_matrix_and_authenticated_edit_cycle(self):
        for method in ("graph", "finite"):
            with self.subTest(method=method):
                first = self.audit(method, cache=True)["certificate"]
                self.assertEqual(first["ordinary_counts"], [4, 13, 2])
                self.assertEqual(first["reuse"], "fresh")
                self.assertEqual(self.audit(method, cache=True)["certificate"]["reuse"], "authenticated")
                changed = self.audit(method, config=CONFIG.replace("{A,B,C}", "{A,B}"), cache=True)["certificate"]
                self.assertEqual(changed["ordinary_counts"], [3, 7, 2])
                self.assertNotEqual(first["context"], changed["context"])
                self.assertEqual(self.audit(method, cache=True)["certificate"]["reuse"], "authenticated")
                receipt = self.folder / "receipts" / (first["context"] + ".json")
                envelope = json.loads(receipt.read_text())
                envelope["proof"]["ordinary_counts"] = [1, 1, 1]
                receipt.write_text(json.dumps(envelope))
                self.assertEqual(self.audit(method, cache=True)["certificate"]["reuse"], "fresh")
                receipt.write_text("[" * 2000 + "]" * 2000)
                self.assertEqual(self.audit(method, cache=True)["certificate"]["reuse"], "fresh")

    def test_occurrence_multiplicity(self):
        arm = r"\E a \in Actors : x' = a"
        for method in ("finite", "graph"):
            for source, expected in [(MODULE.replace(arm, f"({arm}) \\/ ({arm})"), 25),
                                     (MODULE.replace("Init == x = Fixed", r"Init == (x \in Actors) \/ (x \in Actors)"), 15)]:
                result = self.audit(method, source)
                self.assertEqual(result["certificate"]["ordinary_counts"][1], expected, result)
        pick = r"Pick == CHOOSE a \in Actors : TRUE"
        source = MODULE.replace("Init ==", pick + "\nInit ==")
        for asymmetric in [source.replace(arm, f"({arm}) \\/ (x' = Pick)"),
                           source.replace("Init == x = Fixed", r"Init == (x \in Actors) \/ (x = Pick)")]:
            result = self.audit("graph", asymmetric)
            self.assertEqual(result["status"], "refused", result)
            self.assertIn("occurrence multiset", result["reason"])

    def test_definedness_and_asymmetric_constant(self):
        for method in ("graph", "finite"):
            self.assertIn("distinguishes", self.audit(method, config=CONFIG.replace("Fixed = X", "Fixed = A"))["reason"])
            for left, right in [(r'[u \in {} |-> x]', r'[v \in {"k"} |-> x]'),
                                (r'[u \in {Fixed,1} |-> IF u=Fixed THEN 1 ELSE "hi"]',
                                 r'[v \in {Fixed,1} |-> IF v=Fixed THEN 2 ELSE "hi"]')]:
                pred = rf"x = Fixed \/ (\E a \in Actors : a = x \/ ({left} = {right}))"
                result = self.audit(method, MODULE.replace("Safe == x = x", "Safe == " + pred))
                self.assertEqual(result["status"], "refused", result)

    def test_original_hidden_obligations_and_damaged_word(self):
        (self.folder / "Picker.tla").write_text(r"""---- MODULE Picker ----
CONSTANT Fixed
Pick == CHOOSE a \in Fixed : TRUE
====
""")
        root = MODULE.replace("Init ==", "H == INSTANCE Picker WITH Fixed <- Actors\nInit ==")
        for source, prop in [(root.replace("Safe == x = x", r"Safe == x = Fixed \/ x = H!Pick"), "Safe"),
                             (root.replace(r"[x' \in Actors]", r"[x = Fixed \/ x = H!Pick]"), "Step")]:
            doc = self.audit("graph", source)
            self.assertEqual(doc["status"], "validated-counterexample", doc)
            word = doc.get("unreduced", doc["reduced"])
            self.assertEqual((word["property"], word["original_trace_replay"]), (prop, "valid"))
            trace = word["trace"]
            trace["states"][0] = trace["states"][-1]
            (self.folder / "damaged.json").write_text(json.dumps(trace))
            run = subprocess.run([BINARY, "validate-trace", "Carrier.tla", "--config", "Carrier.cfg",
                                  "--trace", "damaged.json", "--compat=tlc", "--output", "json"],
                                 cwd=self.folder, capture_output=True)
            self.assertEqual(json.loads(run.stdout)["result"], "invalid")


if __name__ == "__main__":
    unittest.main()
