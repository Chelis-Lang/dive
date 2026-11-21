"""Exercise the EmergencyCall import and partial-predicate regressions."""

import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile

from emergencycall import PhoneModel, Unsupported


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("module", type=Path, help="published EmergencyCall module")
    parser.add_argument("--exporter", type=Path, required=True)
    args = parser.parse_args()
    exporter = args.exporter.resolve()
    helper = Path(__file__).with_name("emergencycall.py")
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        module = root / args.module.name
        module.write_bytes(args.module.read_bytes())
        cfg = root / "check.cfg"
        cfg.write_text("INIT Init1\nNEXT Next\nINVARIANT F1\n")
        bound = json.loads(subprocess.check_output([exporter, module, cfg]))
        standard = next(f["source"] for f in bound["ir"]["files"] if f["module"] == "TLC")
        lines = standard.splitlines(keepends=True)
        end = next(i for i, line in enumerate(lines) if line.startswith("===="))
        lines.insert(end, "AuditPartial == (CASE FALSE -> 0) /= 0\n")
        (root / "TLC.tla").write_text("".join(lines))
        cfg.write_text("INIT Init1\nNEXT Next\nINVARIANT AuditPartial\n")
        check = subprocess.run(
            [sys.executable, helper, module, cfg, "--exporter", exporter],
            capture_output=True,
            text=True,
        )
        if (check.returncode, check.stdout, check.stderr.strip()) != (
            1, "", "unsupported: unrecognized bound EmergencyCall module or import"
        ):
            raise AssertionError((check.returncode, check.stdout, check.stderr))
        bound = json.loads(subprocess.check_output([exporter, module, cfg]))
        try:
            PhoneModel(bound).check(0)
        except Unsupported as error:
            if str(error) != "partial finite value in predicate or quantifier domain":
                raise
        else:
            raise AssertionError("undefined CASE operand admitted in a predicate")


if __name__ == "__main__":
    main()
