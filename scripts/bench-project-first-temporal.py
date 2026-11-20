#!/usr/bin/env python3
"""Compare real ordinary completion and first-claim proof projection on S7."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("module", type=Path, help="pinned MCLiveInternalMemory.tla")
    parser.add_argument("--samples", type=int, default=7)
    args = parser.parse_args()
    if args.samples < 1 or args.module.name != "MCLiveInternalMemory.tla":
        parser.error("expected MCLiveInternalMemory.tla and at least one sample")
    binary = str(args.binary.resolve())
    module = str(args.module.resolve())
    config = args.module.with_suffix(".cfg").resolve()
    policy = ["8816", "100000"]

    def run(arguments):
        start = time.perf_counter_ns()
        process = subprocess.run([binary, *arguments], capture_output=True, text=True, check=True)
        return [json.loads(line) for line in process.stdout.splitlines()], (time.perf_counter_ns() - start) // 1000

    with tempfile.TemporaryDirectory() as tmp:
        edit, receipt = Path(tmp) / "edit.cfg", Path(tmp) / "receipt.json"
        text = config.read_text()
        line = "PROPERTY Liveness\n"
        assert text.count(line) == 1
        edit.write_text(text.replace(line, ""))
        (previous,), previous_wall = run(["--native-report", *policy, module, config])
        receipt.write_text(json.dumps(previous))
        old = [module, config]
        current = [module, edit]
        (actual,), _ = run(["--native-report", *policy, *current])
        expected = {"generated": 21400, "distinct": 4408, "depth": 10, "initial_generated": 8, "left_on_queue": 0}
        for report in (previous, actual):
            assert report["verdict"] == "success"
            assert {key: report["statistics"][key] for key in expected} == expected
            assert report["periodic"]["prefix_checks"] == 0 and report["periodic"]["final_check"]
        samples = []
        for i in range(args.samples):
            runs = ["fresh", "relation"] if i % 2 == 0 else ["relation", "fresh"]
            sample = {}
            for mode in runs:
                if mode == "fresh":
                    (row,), wall = run(["--fresh", *policy, *current])
                    assert row["path"] == "fresh_native_holds" and row["counts"] == list(expected.values())
                    sample["edited_fresh_us"], sample["edited_fresh_process_us"] = row["elapsed_us"], wall
                else:
                    (prepared, row), wall = run(["--relation", receipt, *policy, *old, *current])
                    assert prepared["path"] == "prepared" and row["path"] == "relation" and row["first"] == "LivenessProperty"
                    sample["previous_binding_us"] = prepared["elapsed_us"]
                    sample["edited_binding_and_qualified_relation_us"] = row["elapsed_us"]
                    sample["read_receipt_bind_both_process_us"] = wall
            samples.append(sample)
        def facts(report):
            atoms = report["core"]["metrics"]["liveness"]["atoms"]
            return {name: atoms[name] for name in ("total_atoms", "eligible_atoms", "evaluated", "fallback", "audit")}
        print(json.dumps({"statistics": expected, "ordinary_previous_process_us": previous_wall,
                          "previous_atoms": facts(previous), "edited_atoms": facts(actual),
                          "samples": samples}, indent=2))


if __name__ == "__main__":
    main()
