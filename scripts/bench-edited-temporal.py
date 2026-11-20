#!/usr/bin/env python3
"""Measure repeated edits through the ordinary and captured temporal Core APIs."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile


def snapshot(root, case, module, source):
    folder = root / case
    folder.mkdir()
    file = folder / f"{module}.tla"
    cfg = file.with_suffix(".cfg")
    file.write_text(source)
    cfg.write_text("SPECIFICATION Spec\nPROPERTY A B\n")
    return [str(file), str(cfg)]


def measure(binary, paths, options, rss_file, gnu_time):
    cmd = [str(binary), *options, *paths]
    if gnu_time:
        cmd = [str(gnu_time), "-f", "%M", "-o", str(rss_file), *cmd]
    finished = subprocess.run(cmd, text=True, check=True, stdout=subprocess.PIPE)
    rss = int(rss_file.read_text()) if gnu_time else None
    return [json.loads(line) for line in finished.stdout.splitlines()], rss


def compare(root, args, name, old, edited):
    paths = old + edited * args.samples
    records, rss = measure(args.binary, paths, [], root / "rss", args.gnu_time)
    fresh, fresh_rss = measure(args.binary, paths, ["--fresh"], root / "rss", args.gnu_time)
    for i, (a, b) in enumerate(zip(records, fresh, strict=True)):
        assert a["path"] == ("capture" if i == 0 else "recheck"), (name, a)
        assert a["result"] == b["result"], (name, a, b)
    outcome = records[-1]["result"]
    return {
        "outcome": {k: v for k, v in outcome.items() if k not in ("word", "native_word")},
        "false_properties": (outcome["word"] or {}).get("properties", []),
        "capture_elapsed_us": records[0]["elapsed_us"],
        "original_fresh_us": fresh[0]["elapsed_us"],
        "edited_recheck_us": [r["elapsed_us"] for r in records[1:]],
        "edited_bind_us": [r["bind_us"] for r in records[1:]],
        "edited_analysis_us": [r["analysis_us"] for r in records[1:]],
        "edited_fresh_us": [r["elapsed_us"] for r in fresh[1:]],
        "capture_and_edits_peak_kib": rss,
        "fresh_series_peak_kib": fresh_rss,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--diehard", type=Path, help="pinned examples checkout's DieHard.tla")
    parser.add_argument("--gnu-time", type=Path, help="GNU time path to record Linux peak RSS")
    args = parser.parse_args()
    if args.samples < 1:
        parser.error("need at least one edit")
    args.binary = args.binary.resolve()
    results = {}
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        for case, n in [("star", 3000), ("star", 20000), ("tree", 65535), ("tree", 131071)]:
            action = (f"IF x = 0 THEN x' \\in 1..{n} ELSE x' = x" if case == "star" else
                      f"IF x < {n} THEN x' \\in {{2*x+1,2*x+2}} ELSE x' = x")
            spec = f"""---- MODULE Graph ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == {action}
Spec == Init /\\ [][Next]_x
A == <>(x = 0)
B == <>(x < 2)
====
"""
            key = f"{case}-{n}"
            old = snapshot(root, key, "Graph", spec)
            hold = spec.replace("A == <>(x = 0)", "A == <>(x <= 0)").replace("x < 2)", "x < 3)")
            results[key + "-hold"] = compare(root, args, key, old, snapshot(root, key + "-hold", "Graph", hold))
            if case == "star":
                false = spec.replace("A == <>(x = 0)", "A == <>(x > 1)").replace("x < 2)", "x > 2)")
                results[key + "-false"] = compare(root, args, key, old, snapshot(root, key + "-false", "Graph", false))
        if args.diehard:
            src = args.diehard.read_text()
            i = src.index("====")
            src = src[:i] + "\nA == <>(big = 0)\nB == <>(small = 0)\n" + src[i:]
            edited = src.replace("A == <>(big = 0)", "A == <>(big = 4)")
            results["DieHard"] = compare(root, args, "DieHard", snapshot(root, "old", "DieHard", src),
                                         snapshot(root, "edit", "DieHard", edited))
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
