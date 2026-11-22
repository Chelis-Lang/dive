#!/usr/bin/env python3
"""Check the scoreboard inventory and optionally repeat its focused corpus observations."""

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tomllib


ROOT = Path(__file__).resolve().parents[1]
DOC = ROOT / "docs/scoreboard.md"
RESULT = ROOT / "conformance/reports/2026-11-22-family-refusals.json"
R2 = ROOT / "conformance/reports/perf/2026-11-20-r2-procedure-c14687e6.json"
GOLDENS = ROOT / "conformance/golden/tlc-2026.10.06.014338"


def selection():
    records = json.loads(R2.read_text())["models"]
    selected = [("examples", k) for k, v in records.items() if v["cat"] == "excluded"]
    return sorted(selected + [
        ("examples", "specifications/ewd998/SmokeEWD998_SC.cfg"),
        ("holdout-blocking-queue", "BlockingQueueDebug.cfg"),
        ("holdout-tidb", "DistributedTransaction/Test3.cfg"),
    ])


def golden(corpus, config):
    return json.loads((GOLDENS / corpus / (config + ".json")).read_text())["model"]


def pins():
    corpus = tomllib.loads((ROOT / "conformance/corpora.toml").read_text())["corpus"]
    libraries = tomllib.loads((ROOT / "conformance/oracles.toml").read_text())["module_library"]
    ids = {corpus_id for corpus_id, _ in selection()} | {"community-modules"}
    inputs = {row["id"]: row["commit"] for row in corpus if row["id"] in ids}
    paths = [("community-modules", "modules")]
    for library in libraries:
        inputs[library["id"]] = library["commit"]
        paths.append((library["id"], library["subdir"]))
    return inputs, paths


def probe(binary, cache):
    commits, modules = pins()
    checkout = {name: cache / name / commit for name, commit in commits.items()}
    rows = []
    for corpus, config in selection():
        model = golden(corpus, config)
        mode = model["mode"]["kind"]
        if mode == "simulate":
            args = ["simulate", model["module"], "--traces", str(model["mode"]["traces"]),
                    "--depth", "100", "--keep-traces", "all"]
        elif mode == "exhaustive":
            args = ["check", model["module"], "--fp", "0"]
        else:
            raise ValueError(f"review unhandled manifest mode {mode}")
        args += ["--config", config, "--workers", "1", "--seed", "0", "--output", "json",
                 "--engine", "core", "--exit-codes", "tlc", "--time-limit", "5s", "--max-memory", "512m"]
        for name, subdir in modules:
            args += ["--module-path", str(checkout[name] / subdir)]
        run = subprocess.run([binary, *args], cwd=checkout[corpus], capture_output=True,
                             text=True, timeout=20)
        try:
            report = json.loads(run.stdout)
        except ValueError:
            raise SystemExit(f"{corpus}:{config}: no JSON report, exit {run.returncode}: {run.stderr[:500]}")
        outcome = report["verdict"] if mode == "exhaustive" else report["result"]
        row = {"corpus": corpus, "config": config, "module": model["module"], "mode": mode,
               "outcome": outcome, "exit": run.returncode, "message": report.get("message")}
        if mode == "simulate" and outcome == "violation":
            row["property"] = report["observed_violation"]["name"]
            row["validated"] = report["violation_validated"]
        rows.append(row)
    return {"pins": commits, "rows": rows}


def check():
    document = DOC.read_text()
    table = document.split("## Feature families\n", 1)[1]
    counts = Counter()
    names = set()
    for row in table.splitlines():
        if not row.startswith("| "):
            continue
        cells = [c.strip() for c in row.strip("|").split("|")]
        if cells[0] == "Family":
            continue
        if len(cells) != 6 or cells[0] in names or cells[1] not in ("implemented", "partial", "missing"):
            raise ValueError(f"invalid or duplicate family row: {row}")
        names.add(cells[0])
        counts[cells[1]] += 1
        if cells[1] != "partial":
            continue
        if not all(cells[3:]) or "→" not in cells[4]:
            raise ValueError(f"missing workflow, exercised behavior or boundary: {cells[0]}")
        links = re.findall(r"\[([^\]]+)\]\(([^)]+)\)", cells[5])
        if not links:
            raise ValueError(f"missing evidence: {cells[0]}")
        for label, href in links:
            path = (DOC.parent / href).resolve()
            if not path.is_relative_to(ROOT) or not path.is_file():
                raise ValueError(f"missing local evidence for {cells[0]}: {href}")
            if path.suffix == ".rs" and label != path.stem:
                raise ValueError(f"test target name differs from file: {label}")
    totals = re.search(r"Totals: (\d+) implemented, (\d+) partial, (\d+) missing\.", table)
    if not totals or tuple(map(int, totals.groups())) != tuple(counts[s] for s in ("implemented", "partial", "missing")):
        raise ValueError(f"review scoreboard totals: {counts}")
    data = json.loads(RESULT.read_text())
    if data["pins"] != pins()[0]:
        raise ValueError("the pinned inputs moved")
    if [(r["corpus"], r["config"]) for r in data["rows"]] != selection():
        raise ValueError("review the measured cohort against the historical exclusions")
    histogram = Counter()
    for row in data["rows"]:
        model = golden(row["corpus"], row["config"])
        if (row["module"], row["mode"]) != (model["module"], model["mode"]["kind"]):
            raise ValueError(f"recorded command differs from golden mode or module: {row['config']}")
        histogram[row["bucket"]] += 1
    summary = "\n".join(f"| {name} | {n} |" for name, n in sorted(histogram.items(), key=lambda kv: (-kv[1], kv[0])))
    measured_table = document.split("<!-- refusal-counts -->\n", 1)[1].split("\n<!-- /refusal-counts -->", 1)[0]
    if measured_table.strip().splitlines()[2:] != summary.splitlines():
        raise ValueError(f"stale refusal counts; measured rows give:\n{summary}")
    print(f"{len(names)} families, {counts['partial']} partial with evidence, {len(data['rows'])} corpus observations")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe", type=Path, metavar="DIVE", help="print new observations as JSON")
    parser.add_argument("--git-cache", type=Path, default=Path(os.environ.get("DIVE_CACHE_DIR", Path.home() / ".cache/dive")) / "git")
    args = parser.parse_args()
    if args.probe:
        binary = args.probe.resolve()
        result = probe(binary, args.git_cache.resolve())
        with binary.open("rb") as stream:
            result["binary_sha256"] = hashlib.file_digest(stream, "sha256").hexdigest()
        result["version"] = subprocess.check_output([binary, "--version"], text=True).strip()
        print(json.dumps(result, indent=2))
    else:
        check()


if __name__ == "__main__":
    main()
