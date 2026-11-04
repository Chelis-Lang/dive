#!/usr/bin/env python3
"""Regenerate compact logical-coverage references with the repository's pinned TLC."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--java", type=Path, required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--only", help="one fixture module name")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    pins = tomllib.loads((root / "conformance/oracles.toml").read_text())
    oracle = next(pin for pin in pins["tlc"] if pin.get("default"))
    if hashlib.sha256(args.jar.read_bytes()).hexdigest() != oracle["sha256"]:
        raise SystemExit("jar does not match the gating oracle SHA-256")
    fixtures = root / "conformance/corpus/tlc-coverage-primary/specifications/Coverage"
    arguments = ["-workers", "1", "-seed", "7", "-fp", "0", "-coverage", "0"]
    for source in sorted(fixtures.glob("*.tla")):
        if args.only and source.stem != args.only:
            continue
        config = source.with_suffix(".cfg")
        with tempfile.TemporaryDirectory(prefix="dive-tlc-coverage-") as temporary:
            run = subprocess.run(
                [str(args.java), "-cp", str(args.jar), "tlc2.TLC", *arguments,
                 "-metadir", str(Path(temporary) / "states"), str(source)],
                text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                check=False, timeout=60,
            )
        if run.returncode not in (0, 14):
            raise SystemExit(f"unexpected TLC exit {run.returncode}: {source.name}\n{run.stdout}")
        actions = []
        for line in run.stdout.splitlines():
            action = re.fullmatch(r"(<.*>): (\d+):(\d+)", line)
            if action:
                actions.append({"label": action[1], "initial": action[1].startswith("<Init "),
                                "distinct": int(action[2]), "generated": int(action[3]), "rows": []})
            row = re.fullmatch(r"  (\|*)line (\d+), col (\d+) to line (\d+), col (\d+) of module (\w+): (\d+)(?::(\d+))?", line)
            if row and actions:
                actions[-1]["rows"].append({"depth": len(row[1]), "begin": [int(row[2]), int(row[3])],
                                           "end": [int(row[4]), int(row[5])], "module": row[6],
                                           "invocations": int(row[7]), "cost": int(row[8] or 0)})
        stats = re.search(r"(\d+) states generated, (\d+) distinct states found,", run.stdout)
        depth = re.search(r"The depth of the complete state graph search is (\d+)", run.stdout)
        if not actions or not stats or not depth:
            raise SystemExit(f"missing coverage/statistics: {source.name}\n{run.stdout}")
        reference = {
            "pin": {"oracle": oracle["id"], "commit": oracle["upstream_commit"], "jar_sha256": oracle["sha256"],
                    "arguments": arguments, "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
                    "config_sha256": hashlib.sha256(config.read_bytes()).hexdigest()},
            "oracle_exit": run.returncode, "generated": int(stats[1]), "distinct": int(stats[2]),
            "depth": int(depth[1]), "actions": actions,
        }
        source.with_suffix(".json").write_text(json.dumps(reference, indent=2) + "\n")
        print(f"{source.name}: exit {run.returncode}, {len(actions)} action trees")


if __name__ == "__main__":
    main()
