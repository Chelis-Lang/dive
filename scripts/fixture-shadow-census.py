#!/usr/bin/env python3
"""Account for every test fixture and shadow the core against the reference.

Stages disposable copies outside the checkout, then invokes the existing
dive-conformance shadow command, including its successor sampling and independent
trace validation. The harness supplies seed 0 and fingerprint 0 to both engines.
No hand-written comparator or golden is involved. All reports and subprocess
stdout/stderr remain under --out, never in the checkout.
"""

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

CORPUS = "fixture-shadow"
STATUSES = {"agree", "refused", "differ", "trace-refuted", "trace-unconfirmed", "successors-differ", "timeout"}
FINDINGS = {"differ", "trace-refuted", "trace-unconfirmed", "successors-differ"}


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def fixture_selection(repo):
    """Prefer the same stem, otherwise require exactly one adjacent module."""
    rows = []
    for cfg in sorted((repo / "crates").glob("*/tests/fixtures/**/*.cfg")):
        row = {"fixture": cfg.relative_to(repo).as_posix()}
        if cfg.relative_to(repo).parts[1] == "dive-config":
            row["selection"] = "excluded-config-parser"
        else:
            modules = [cfg.with_suffix(".tla")] if cfg.with_suffix(".tla").is_file() else sorted(cfg.parent.glob("*.tla"))
            if len(modules) == 1:
                row.update(selection="selected", module=modules[0].relative_to(repo).as_posix())
                rel = cfg.relative_to(repo).parts
                row["model"] = Path("specifications", rel[1], *rel[4:]).as_posix()
            else:
                row["selection"] = "skipped-ambiguous-module" if modules else "skipped-missing-module"
                row["candidates"] = [p.relative_to(repo).as_posix() for p in modules]
        rows.append(row)
    if not rows or not any(r["selection"] == "selected" for r in rows):
        raise ValueError("fixture selection is empty")
    return rows


def stage_module(path):
    rel = Path(path).parts
    return Path("specifications", rel[1], *rel[4:]).as_posix()


def prepare(repo, out):
    repo, out = repo.resolve(), out.resolve()
    if out.is_relative_to(repo):
        raise ValueError("--out must be outside the checkout")
    if out.exists():
        raise ValueError("--out must not exist; refusing to overwrite a census")
    rows = fixture_selection(repo)
    roots = sorted((repo / "crates").glob("*/tests/fixtures"))
    for root in roots:
        if root.parent.parent.name != "dive-config" and any(p.is_symlink() for p in root.rglob("*")):
            raise ValueError(f"fixture symlink under {root}; refusing to follow it")
    conf = out / "conformance"
    corpus = conf / "corpus" / CORPUS
    corpus.mkdir(parents=True)
    shutil.copyfile(repo / "conformance/oracles.toml", conf / "oracles.toml")
    (conf / "corpora.toml").write_text(
        f'[[corpus]]\nid = "{CORPUS}"\npath = "conformance/corpus/{CORPUS}"\n'
        'license = "MIT"\nlayout = "examples-manifest"\n')
    for root in roots:
        crate = root.parent.parent.name
        selected = [r for r in rows if r["selection"] == "selected" and Path(r["fixture"]).parts[1] == crate]
        if not selected:
            continue
        target = corpus / "specifications" / crate
        shutil.copytree(root, target)
        modules = {}
        for row in selected:
            module = stage_module(row["module"])
            modules.setdefault(module, {"path": module, "features": [], "models": []})["models"].append({
                "path": row["model"], "runtime": "unknown", "mode": "exhaustive search", "result": "unknown",
                "distinctStates": None, "totalStates": None, "stateDepth": None})
        write_json(target / "manifest.json", {
            "sources": [], "authors": [], "tags": [], "modules": list(modules.values())})
    selection = {"schema_version": 1, "seed": 0, "fp": 0,
                 "selection_counts": dict(Counter(r["selection"] for r in rows)), "fixtures": rows}
    write_json(out / "selection.json", selection)
    return selection


def recording_wrapper(path, binary, raw):
    """Keep the harness's physical process status and every raw output stream."""
    raw.mkdir(parents=True)
    path.write_text("#!/usr/bin/env python3\n" + '''import json, os, shutil, subprocess, sys, tempfile
from pathlib import Path
binary, raw = CONFIG
args = sys.argv[1:]
if not args or args[0] != "check":
    os.execv(binary, [binary, *args])
dest = Path(tempfile.mkdtemp(prefix="run-", dir=raw))
info = {"command": [binary, *args], "profile": os.environ.get("DIVE_COMPAT", "default")}
(dest / "invocation.json").write_text(json.dumps(info) + "\\n")
with (dest / "stdout").open("wb") as out, (dest / "stderr").open("wb") as err:
    result = subprocess.run([binary, *args], stdout=out, stderr=err)
(dest / "exit.json").write_text(json.dumps({"exit_code": result.returncode}) + "\\n")
for name, stream in [("stdout", sys.stdout.buffer), ("stderr", sys.stderr.buffer)]:
    with (dest / name).open("rb") as source:
        shutil.copyfileobj(source, stream)
    stream.flush()
if result.returncode < 0:
    os.kill(os.getpid(), -result.returncode)
sys.exit(result.returncode)
'''.replace("CONFIG", repr((str(binary.resolve()), str(raw.resolve())))))
    path.chmod(0o755)


def tally(selection, report, workers, profile="default"):
    if report.get("profile") != ("tlc" if profile == "tlc" else "dive") or report.get("workers") != workers:
        raise ValueError("shadow report profile or workers contradict the requested census")
    selected = {r["model"]: r for r in selection["fixtures"] if r["selection"] == "selected"}
    expected = {(model, worker) for model in selected for worker in workers}
    actual = [(r["model"], r["workers"]) for r in report["rows"]]
    if len(actual) != len(set(actual)) or set(actual) != expected:
        raise ValueError("shadow report has missing, duplicate or extra fixture/worker rows")
    totals = Counter()
    findings = []
    for row in report["rows"]:
        if row["corpus"] != CORPUS or row["status"] not in STATUSES:
            raise ValueError("unexpected shadow corpus or status")
        totals[row["status"]] += 1
        if row["status"] != "agree":
            findings.append({"fixture": selected[row["model"]]["fixture"], **row})
    if dict(totals) != report["summary"]:
        raise ValueError("shadow report summary does not equal its rows")
    # Each config has exactly one outcome per worker, including unrun fixtures.
    for row in selection["fixtures"]:
        if row["selection"] != "selected":
            totals[row["selection"]] += len(workers)
    if sum(totals.values()) != len(selection["fixtures"]) * len(workers):
        raise ValueError("census totals do not cover the complete fixture population")
    return dict(totals), findings


def incomplete_runs(out, selection):
    """Name native stops separately from missing physical completion reports."""
    corpus = out / "conformance/corpus" / CORPUS
    fixtures = {r["model"]: r["fixture"] for r in selection["fixtures"] if r["selection"] == "selected"}
    rows = []
    for path in sorted((out / "raw").glob("*/invocation.json")):
        invocation = json.loads(path.read_text())
        args = invocation["command"]
        config = Path(args[args.index("--config") + 1])
        physical = path.with_name("exit.json")
        try:
            report = json.loads(path.with_name("stdout").read_text())
        except (OSError, ValueError):
            report = {}
        verdict = report.get("verdict")
        exit_code = json.loads(physical.read_text())["exit_code"] if physical.exists() else None
        if exit_code is not None and exit_code >= 0 and verdict not in (None, "stopped", "resource-limit"):
            continue
        rows.append({"fixture": fixtures[config.relative_to(corpus).as_posix()],
                     "engine": args[args.index("--engine") + 1] if "--engine" in args else "reference",
                     "profile": invocation["profile"], "workers": int(args[args.index("--workers") + 1]),
                     "verdict": verdict, "stopped": report.get("stopped"),
                     "physical_exit": exit_code,
                     "raw": path.parent.relative_to(out).as_posix()})
    return rows


def run(repo, out, selection, harness, dive, profiles, workers, jobs, timeout, memory, sample):
    wrapper = out / "record-dive.py"
    recording_wrapper(wrapper, dive, out / "raw")
    reports = []
    failed = False
    for profile in profiles:
        dest = out / profile
        dest.mkdir()
        command = [str(harness.resolve()), "shadow", "--repo-root", str(out), "--corpus", CORPUS,
                   "--tier", "all", "--impl-profile", "tlc" if profile == "tlc" else "dive",
                   "--dive-bin", str(wrapper), "--workers", ",".join(map(str, workers)),
                   "--jobs", str(jobs), "--timeout", str(timeout), "--tlc-budgets",
                   "--budget-cap", str(timeout), "--xmx", memory, "--sample", str(sample),
                   "--report-dir", str(dest)]
        with (dest / "harness.log").open("w") as log:
            status = subprocess.run(command, cwd=repo, stdout=log, stderr=subprocess.STDOUT).returncode
        files = list(dest.glob("*-shadow-*.json"))
        if len(files) != 1:
            raise ValueError(f"expected one {profile} shadow report; see {dest / 'harness.log'}")
        report = json.loads(files[0].read_text())
        totals, findings = tally(selection, report, workers, profile)
        reports.append({"profile": profile, "harness_exit": status, "totals": totals, "findings": findings,
                        "report": files[0].relative_to(out).as_posix()})
        failed |= status != 0 or any(row["status"] in FINDINGS for row in findings)
    with dive.open("rb") as stream:
        dive_sha256 = hashlib.file_digest(stream, "sha256").hexdigest()
    result = {"schema_version": 1, "kind": "fixture-shadow-census", "seed": 0, "fp": 0,
              "workers": workers, "timeout_seconds": timeout, "max_memory": memory, "sample": sample,
              "fixture_count": len(selection["fixtures"]), "selection_counts": selection["selection_counts"],
              "dive_sha256": dive_sha256, "profiles": reports, "incomplete_runs": incomplete_runs(out, selection)}
    write_json(out / "census.json", result)
    for report in reports:
        print(json.dumps({k: v for k, v in report.items() if k != "findings"}, sort_keys=True))
    return int(failed)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--harness", type=Path)
    parser.add_argument("--dive-bin", type=Path)
    parser.add_argument("--profile", choices=["default", "tlc", "both"], default="default")
    parser.add_argument("--workers", default="1")
    parser.add_argument("--jobs", type=int, default=2)
    parser.add_argument("--timeout", type=int, default=20)
    parser.add_argument("--max-memory", default="3g")
    parser.add_argument("--sample", type=int, default=32)
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    workers = [int(w) for w in args.workers.split(",")]
    if not workers or len(workers) != len(set(workers)) or min(workers) < 1:
        parser.error("--workers needs distinct positive counts")
    if not 1 <= args.jobs <= 8 or not 1 <= args.timeout <= 900 or args.sample < 0:
        parser.error("jobs, timeout or sample outside census limits")
    if not args.max_memory.isalnum():
        parser.error("--max-memory must be a size such as 3g")
    repo, out = args.repo_root.resolve(), args.out.resolve()
    selection = prepare(repo, out)
    print(json.dumps(selection["selection_counts"], sort_keys=True))
    if args.prepare_only:
        return 0
    profiles = ["default", "tlc"] if args.profile == "both" else [args.profile]
    return run(repo, out, selection, args.harness or repo / "target/release/dive-conformance",
               (args.dive_bin or repo / "target/release/dive").resolve(), profiles, workers,
               args.jobs, args.timeout, args.max_memory, args.sample)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ValueError, OSError, KeyError, subprocess.CalledProcessError) as error:
        print("fixture shadow census: " + str(error), file=sys.stderr)
        sys.exit(2)
