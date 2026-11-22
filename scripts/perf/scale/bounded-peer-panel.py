#!/usr/bin/env python3
"""Measure the 46 original Examples length-five requests with fresh processes on Linux."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import tempfile
import time

MANYAN = "a39816a873a2c2aabc00dbcef50908e996f3d44b"
EXAMPLES = "ca168055339d5cbcee95b7eefcb97c1ee59f9914"
TLAPM = "c8ddc44a80b621989f96a9ee2c371301c52b109f"
APALACHE = "0.63.0"
COMMUNITY = "202611060952"


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def core_prefix(output):
    try:
        report = json.loads(output)
    except json.JSONDecodeError:
        return {"outcome": "unclassified"}
    reasons = [d["message"] for d in report.get("diagnostics", []) if d.get("severity") == "error"]
    return {"outcome": report["verdict"], "property": report.get("property"),
            "reason": (report.get("stopped") or {}).get("resource") or report.get("message") or "; ".join(reasons) or None,
            "distinct": (report.get("statistics") or {}).get("distinct"),
            "depth": (report.get("statistics") or {}).get("depth"),
            "workers_used": (report.get("options") or {}).get("workers", {}).get("used")}


def run(tool, paths, cfg, module, directory, temp, seconds):
    from symbolic_runner import apalache_outcome, manyan_outcome

    if tool == "manyan":
        command = [paths[tool], "-symbolic", "-length", "5", "-config", cfg, module.stem]
    elif tool == "core_prefix":
        command = [paths[tool], "check", module.name, "--config", cfg, "--workers", "1", "--seed", "0",
                   "--fp", "0", "--max-depth", "6", "--max-memory", "2g", "--output", "json",
                   "--time-limit", f"{seconds - 1}s"]
    else:
        command = [paths[tool], "check", "--length=5", f"--config={cfg}",
                   f"--out-dir={temp}/out", module.name]
    out = temp / "log"
    rss = temp / "rss"
    start = time.monotonic()
    with out.open("wb") as stream:
        process = subprocess.run(
            ["/usr/bin/time", "-qf", "%M", "-o", rss, "timeout", "-k", "5s", f"{seconds}s", *command],
            cwd=directory, env={**os.environ, "JVM_ARGS": "-Xmx2g", "LC_ALL": "C"},
            stdout=stream, stderr=subprocess.STDOUT, check=False)
    elapsed = time.monotonic() - start
    code = process.returncode
    output = out.read_text(errors="replace")
    timing = {"exit": code, "wall_s": round(elapsed, 5), "peak_rss_kib": int(rss.read_text().strip())}
    if tool == "core_prefix":
        detail = {"outcome": "timeout" if code == 124 else "killed"} if code in (124, 137) else core_prefix(output)
        return {**detail, **timing}
    if code in (124, 137):
        outcome = "timeout" if code == 124 else "killed"
    elif tool == "manyan":
        outcome = manyan_outcome(output, code).strip()
    else:
        outcome = apalache_outcome(output, code, temp / "out", (directory / cfg).read_text()).strip()
    # The upstream normalizer maps equivalent tool-specific exits. Keep the raw
    # exit separately and never identify two refusals as semantic agreement.
    lines = outcome.splitlines()
    if lines[0] in ("NoError", "ExecutionsTooShort") and code != 0:
        outcome = "unclassified"
    elif lines[0].startswith(("Error ", "Deadlock ")) and code == 0:
        outcome = "unclassified"
    elif not re.fullmatch(r"NoError|ExecutionsTooShort|Error [\w]+(?: [1-9][0-9]*)?|Deadlock [1-9][0-9]*", lines[0]):
        outcome = lines[0] if lines[0] in {"timeout", "killed", "Refused"} else "unclassified"
    else:
        outcome = lines[0]
    error = re.search(r"^Error: (.*)$", output, re.M)
    detail = {"diagnostic": error[1]} if error and outcome == "Refused" else {}
    return {"outcome": outcome, **detail, **timing}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manyan_checkout", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--only", default="", help="substring of the configuration path")
    parser.add_argument("--seconds", type=int, default=90)
    parser.add_argument("--dive-prefix", type=Path,
                        help="instead run a separate explicit Core prefix diagnostic on the same input bytes")
    args = parser.parse_args()
    root = args.manyan_checkout.resolve()
    if args.seconds < 2:
        parser.error("--seconds must be at least two")
    revision = subprocess.check_output(["git", "-C", root, "rev-parse", "HEAD"], text=True).strip()
    if revision != MANYAN:
        parser.error(f"expected Manyan {MANYAN}")
    subprocess.run(["git", "-C", root, "diff", "--quiet", "HEAD", "--", "bench", "tests", "Makefile",
                    "manyan.c", "manyan.h", "manyan-std.c", "manyan-std.h"], check=True)
    sys.path[:0] = [str(root / "bench"), str(root / "tests")]
    from symbolic import models, stage
    from symbolic_runner import named

    obj = root / ".obj"
    examples = obj / f"Examples-{EXAMPLES}"
    apalache = obj / f"apalache-{APALACHE}"
    libraries = [obj / f"CommunityModules-{COMMUNITY}", obj / f"tlapm-{TLAPM}" / "library"]
    paths = {"manyan": root / "manyan", "apalache": apalache / "bin/apalache-mc"}
    if args.dive_prefix:
        paths = {"core_prefix": args.dive_prefix.resolve()}
    selected = list(models(examples, args.only))
    if not selected or (not args.only and len(selected) != 46):
        parser.error("selection does not match the pinned Examples manifests")
    report = {
        "schema": "dive-bounded-peers-v1", "examples": EXAMPLES, "manyan": MANYAN, "apalache": APALACHE,
        "tlapm_library": TLAPM, "community_modules": COMMUNITY,
        "linux": platform.release(), "arch": platform.machine(),
        "cpu": next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines()
                     if line.startswith("model name")), None),
        "java": subprocess.check_output(["java", "-version"], stderr=subprocess.STDOUT, text=True).splitlines()[0],
        "binaries_sha256": {p.name: digest(p) for p in [*paths.values(), *(
            [] if args.dive_prefix else sorted((apalache / "lib").glob("*.jar")))]},
        "question": "Core explicit search, stopping before admitting a new state deeper than level 6"
                    if args.dive_prefix else "bounded symbolic search up to five transitions",
        "transition_bound": None if args.dive_prefix else 5, "seconds_per_process": args.seconds, "processes_per_cell": 1,
        "checker_workers": "one requested" if args.dive_prefix else "both upstream defaults; bounded CLI has no worker flag",
        "measurement": "one fresh process per tool/config; monotonic wall seconds; GNU time maximum RSS, KiB",
        "rows": [],
    }
    with args.output.open("x") as target:
        for i, (mod, model) in enumerate(selected):
            path = Path(model["path"])
            module = Path(mod["path"])
            cfg = (examples / path).read_text()
            declarations = re.sub(r"\\\*[^\n]*", "", cfg)
            row = {"cfg": path.as_posix(), "module": module.as_posix(),
                   "cfg_sha256": digest(examples / path), "module_sha256": digest(examples / module),
                   "invariants": named(declarations, "INVARIANT"), "properties": named(declarations, "PROPERT")}
            for tool in (list(paths) if i % 2 == 0 else list(reversed(paths))):
                with tempfile.TemporaryDirectory() as raw:
                    temp = Path(raw)
                    directory = temp / "spec"
                    stage(examples / path.parent, libraries, directory)
                    row[tool] = run(tool, paths, path.name, module, directory, temp, args.seconds)
            report["rows"].append(row)
            print(path, *(row[tool]["outcome"] for tool in paths), flush=True)
        json.dump(report, target, indent=2)
        target.write("\n")


if __name__ == "__main__":
    main()
