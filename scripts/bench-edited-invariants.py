#!/usr/bin/env python3
"""Compare the public recheck_invariants example with ordinary exact Core."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile


def model(root, case, name, src, cfg):
    directory = root / case
    directory.mkdir()
    file = directory / (name + ".tla")
    config = file.with_suffix(".cfg")
    file.write_text(src)
    config.write_text(cfg)
    return [str(file), str(config)]


def compare(binary, original, edited, samples):
    paths = original + edited * samples

    def run(options):
        result = subprocess.run([binary, *options, *paths], check=True,
                                stdout=subprocess.PIPE, text=True)
        return [json.loads(line) for line in result.stdout.splitlines()]

    reused = run([])
    fresh = run(["--fresh"])
    for i, (r, f) in enumerate(zip(reused, fresh, strict=True)):
        assert r["path"] == ("capture" if i == 0 else "recheck"), r
        outcome = r["result"]
        ordinary = f["result"]
        assert outcome["verdict"].replace("_", "-") == ordinary["verdict"]
        assert outcome.get("property") == ordinary["property"]
        for key in ["generated", "distinct", "depth", "left_on_queue", "initial_generated"]:
            assert outcome[key] == ordinary["statistics"][key], key
        if "trace" in outcome:
            assert outcome["trace"] == ordinary["counterexample"]["trace"]
    return {
        "capture_elapsed_us": reused[0]["elapsed_us"],
        "capture_search_us": reused[0]["capture_us"],
        "original_fresh_us": fresh[0]["elapsed_us"],
        "edited_recheck_us": [r["elapsed_us"] for r in reused[1:]],
        "edited_bind_us": [r["bind_us"] for r in reused[1:]],
        "edited_scan_and_validate_us": [r["scan_us"] for r in reused[1:]],
        "edited_fresh_us": [r["elapsed_us"] for r in fresh[1:]],
        "counts": {k: ordinary["statistics"][k] for k in
                   ["generated", "distinct", "depth", "left_on_queue", "initial_generated"]},
        "verdict": ordinary["verdict"],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--spokes", type=int, default=200000)
    parser.add_argument("--diehard", type=Path, help="pinned examples checkout's DieHard.tla")
    args = parser.parse_args()
    if args.spokes < 2 or args.samples < 1:
        parser.error("need at least two spokes and one sample")
    binary = args.binary.resolve()
    n = args.spokes
    with tempfile.TemporaryDirectory() as cwd:
        root = Path(cwd)
        cfg = "INIT Init\nNEXT Next\nINVARIANT Inv\n"

        def star(case, invariant):
            src = f"""---- MODULE Star ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == IF x = 0 THEN x' \\in 1..{n} ELSE IF x = {n} THEN x' = {n + 1} ELSE x' = x
Inv == {invariant}
====
"""
            return model(root, case, "Star", src, cfg)

        rows = {}
        for case, old, new in [("prefix", f"x /= {n + 1}", f"x < {n + 1}"),
                               ("closure", f"x <= {n + 1}", f"x < {n + 2}")]:
            rows[case] = compare(binary, star(case, old), star(case + "-edit", new), args.samples)
        if args.diehard:
            src = args.diehard.read_text()
            cfg = args.diehard.with_suffix(".cfg").read_text()
            edited = src.replace("NotSolved == big # 4", "NotSolved == big # 4 /\\ small <= 3")
            assert edited != src
            rows["DieHard"] = compare(binary, model(root, "d1", "DieHard", src, cfg),
                                      model(root, "d2", "DieHard", edited, cfg), args.samples)
        print(json.dumps(rows, indent=2))


if __name__ == "__main__":
    main()
