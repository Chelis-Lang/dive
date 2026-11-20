#!/usr/bin/env python3
"""Reproduce the S7 MCLamportMutex invariant inventory edit."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("module", type=Path, help="MCLamportMutex.tla in pinned examples checkout")
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--gnu-time", type=Path)
    args = parser.parse_args()
    if args.samples < 1 or args.module.name != "MCLamportMutex.tla":
        parser.error("expected MCLamportMutex.tla and at least one sample")
    binary = str(args.binary.resolve())
    config = args.module.with_suffix(".cfg")
    original = [str(args.module), str(config)]
    with tempfile.TemporaryDirectory() as temp:
        folder = Path(temp)
        old = config.read_text()
        new = old.replace("TypeOK BoundedNetwork Mutex", "TypeOK Mutex")
        assert new != old
        changed = folder / "DropBoundedNetwork.cfg"
        changed.write_text(new)
        edit = [str(args.module), str(changed)]
        paths = original + edit * args.samples

        def run(options):
            cmd = [binary, *options, *paths]
            rss = folder / "rss"
            if args.gnu_time:
                cmd = [str(args.gnu_time), "-f", "%M", "-o", str(rss), *cmd]
            result = subprocess.run(cmd, check=True, stdout=subprocess.PIPE, text=True)
            return [json.loads(line) for line in result.stdout.splitlines()], int(rss.read_text()) if args.gnu_time else None

        projected, rss = run([])
        fresh, fresh_rss = run(["--fresh"])
        for i, (a, b) in enumerate(zip(projected, fresh, strict=True)):
            assert a["path"] == ("capture" if i == 0 else "projection") and b["path"] == "fresh"
            assert a["verdict"] == b["verdict"] == "success"
            for key, count in a["statistics"].items():
                assert count == b["statistics"][key], key
        relation_us = []
        for _ in range(args.samples):
            began = time.monotonic_ns()
            result = subprocess.run([binary, "--relation", *original, *edit], check=True, stdout=subprocess.PIPE, text=True)
            relation_us.append((time.monotonic_ns() - began) // 1000)
            row = json.loads(result.stdout)
            assert row["path"] == "subset_relation" and row["invariants"] == ["TypeOK", "Mutex"]
            assert "verdict" not in row
        print(json.dumps({
            "capture_elapsed_us": projected[0]["elapsed_us"],
            "capture_bind_us": projected[0]["bind_us"],
            "original_fresh_us": fresh[0]["elapsed_us"],
            "edited_fresh_us": [r["elapsed_us"] for r in fresh[1:]],
            "edited_load_and_project_us": [r["elapsed_us"] for r in projected[1:]],
            "proof_comparison_us": [r["projection_us"] for r in projected[1:]],
            "fresh_process_bind_both_and_attest_relation_us": relation_us,
            "capture_and_project_process_peak_kib": rss,
            "fresh_series_process_peak_kib": fresh_rss,
            "statistics": projected[0]["statistics"],
        }, indent=2))


if __name__ == "__main__":
    main()
