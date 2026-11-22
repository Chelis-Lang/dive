#!/usr/bin/env python3
"""Run the two named-property checkers on every original Recomp-Verify CFG on Linux."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time
from tempfile import TemporaryDirectory


def measured(command, folder, seconds):
    # The upstream portfolio can return while losing JVMs are still running.
    # Each invocation has its own session and files; descendants cannot retain
    # a capture pipe and delay a winning result or compete with the next row.
    with (folder / "stdout").open("w") as out, (folder / "stderr").open("w") as err:
        start = time.monotonic()
        process = subprocess.Popen(command, cwd=folder, stdout=out, stderr=err, start_new_session=True)
        try:
            status = process.wait(timeout=seconds)
        except subprocess.TimeoutExpired:
            status = None
        finally:
            elapsed = time.monotonic() - start
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
    output = (folder / "stdout").read_text(errors="replace")
    return status, round(elapsed, 3), output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path, help="clean checkout at the recorded Recomp-Verify commit")
    parser.add_argument("--dive", type=Path, required=True)
    parser.add_argument("--seconds", type=int, default=30)
    parser.add_argument("--tool", choices=("both", "dive", "recomp"), default="both")
    parser.add_argument("--config", help="run one relative CFG path")
    args = parser.parse_args()
    root = args.root.resolve()
    pin = "0cbd95f0002af10f70f25c87638e8f2673229f2e"
    if subprocess.check_output(["git", "-C", root, "rev-parse", "HEAD"], text=True).strip() != pin:
        parser.error("unexpected Recomp-Verify checkout")
    if subprocess.check_output(["git", "-C", root, "status", "--porcelain", "--untracked-files=no"], text=True):
        parser.error("modified Recomp-Verify source")
    base = root / "benchmarks/tla"
    configs = sorted(base.glob("*/*/*.cfg"))
    if len(configs) != 34 or args.seconds < 1:
        parser.error("expected all 34 original configurations and a positive time budget")
    if args.config:
        configs = [config for config in configs if str(config.relative_to(base)) == args.config]
        if not configs:
            parser.error("unknown original configuration")
    for config in configs:
        names = re.findall(r"^INVARIANT\s+(\w+)\s*$", config.read_text(), re.MULTILINE)
        if len(names) != 1:
            raise ValueError(f"expected one state property: {config}")
        prop, = names
        model = config.with_suffix(".tla")
        row = dict(config=str(config.relative_to(base)), property=prop,
                   source_sha256=hashlib.sha256(model.read_bytes()).hexdigest())
        row["cfg_sha256"] = hashlib.sha256(config.read_bytes()).hexdigest()
        layout = "TCTypeOK" if config.parent.parent.name == "tcommit" else "TypeOK"
        row["layout"] = layout
        for tool in (("dive", "recomp") if args.tool == "both" else (args.tool,)):
            with TemporaryDirectory() as tmp:
                folder = Path(tmp)
                for item in config.parent.iterdir():
                    if item.is_file():
                        shutil.copyfile(item, folder / item.name)
                cmd = ([str(args.dive.resolve()), "check", model.name, "--config", config.name,
                        "--strategy", "symbolic", "--type-invariant", layout, "--property-proof", prop,
                        "--output", "json"] if tool == "dive" else
                       [sys.executable, "-u", root / "recomp-verify.py", model.name, config.name, "--parallel"])
                status, wall, out = measured(cmd, folder, args.seconds)
                result = dict(exit=status, wall_s=wall)
                if status is None:
                    result["result"] = "deadline"
                elif tool == "dive":
                    doc = json.loads(out)
                    result["result"] = doc.get("proof") or doc.get("verdict") or "unclassified"
                    if doc.get("verdict") == "invariant-violation":
                        result["trace_validated"] = doc.get("trace_validated", False)
                    if doc.get("reason"):
                        result["reason"] = doc["reason"][:300]
                    if doc.get("failed_obligation"):
                        result["failed_obligation"] = doc["failed_obligation"]
                elif status == 0 and ("Property satisfied!" in out or "Model checking completed. No error has been found." in out):
                    result["result"] = "property-satisfied"
                else:
                    result["result"] = "unclassified"
                    result["last_line"] = out.strip().splitlines()[-1][:180] if out.strip() else ""
                row[tool] = result
        print(json.dumps(row), flush=True)


if __name__ == "__main__":
    main()

