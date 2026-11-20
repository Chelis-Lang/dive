#!/usr/bin/env python3
"""Simulate refusal histogram over every .cfg of the pinned tlaplus/Examples checkout.

Runs `dive simulate` once per configuration (10 traces, depth 100, seed 0) and
buckets the result by kind and by the leading clause of the refusal message.
Writes compact JSON (per-config rows plus the histogram). Raw output is not kept.
"""
import argparse, collections, concurrent.futures, json, re, subprocess
from pathlib import Path

def bucket(result, message):
    if result != "unsupported":
        return result
    m = message or ""
    m = re.sub(r"independent simulation trace validation: ", "validation: ", m)
    m = re.sub(r"simulation temporal properties: .*", "simulation temporal properties", m)
    m = re.sub(r"^SYMMETRY .*", "SYMMETRY", m)
    m = re.sub(r"the standard operator (\S+).*", r"the standard operator \1", m)
    m = re.sub(r"\s*\(.*?\)\s*$", "", m)
    return "unsupported: " + m[:140]

def config_lines(cfg):
    text = re.sub(r"\\\*.*", "", cfg.read_text(errors="replace"))
    text = re.sub(r"\(\*.*?\*\)", "", text, flags=re.S)
    keys = set(re.findall(r"^\s*(PROPERTY|PROPERTIES|SYMMETRY|VIEW|ALIAS|POSTCONDITION|CONSTRAINT|CONSTRAINTS|ACTION_CONSTRAINT|ACTION_CONSTRAINTS)\b", text, re.M))
    plural = {"PROPERTIES": "PROPERTY", "CONSTRAINTS": "CONSTRAINT", "ACTION_CONSTRAINTS": "ACTION_CONSTRAINT"}
    return sorted({plural.get(k, k) for k in keys})

def modules(root):
    mapping = {}
    for manifest in root.glob("specifications/**/manifest.json"):
        data = json.loads(manifest.read_text())
        for module in data.get("modules", []):
            for model in module.get("models", []):
                mapping[(root / model["path"]).resolve()] = (root / module["path"]).resolve()
    return mapping

def run(args, cfg, tla):
    command = [args.dive, "simulate", str(tla), "--config", str(cfg), "--traces", "10", "--depth", "100",
               "--seed", "0", "--time-limit", f"{args.time_limit}s", "--max-memory", "4g", "--output", "json"]
    for path in args.module_path:
        command += ["--module-path", path]
    try:
        done = subprocess.run(command, cwd=tla.parent, capture_output=True, text=True, timeout=args.time_limit + 60)
    except subprocess.TimeoutExpired:
        return "timeout", None, None
    try:
        report = json.loads(done.stdout)
    except json.JSONDecodeError:
        tail = (done.stderr or done.stdout).strip().splitlines()[-1:] or [""]
        return "cli-error", tail[0][:200], done.returncode
    if report.get("result") == "stopped" or report.get("stopped"):
        return "stopped", (report.get("stopped") or {}).get("resource"), done.returncode
    return report.get("result"), report.get("message"), done.returncode

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--examples", type=Path, required=True)
    parser.add_argument("--dive", required=True)
    parser.add_argument("--module-path", action="append", default=[])
    parser.add_argument("--time-limit", type=int, default=30)
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--source", required=True, help="dive commit the binary was built from")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    root = args.examples.resolve()
    mapping = modules(root)
    cfgs = sorted(root.glob("specifications/**/*.cfg"))
    jobs = []
    for cfg in cfgs:
        tla = mapping.get(cfg.resolve()) or cfg.with_suffix(".tla")
        jobs.append((cfg, tla))
    rows = []
    with concurrent.futures.ThreadPoolExecutor(args.jobs) as pool:
        futures = {pool.submit(run, args, cfg, tla): (cfg, tla) for cfg, tla in jobs if tla.exists()}
        for cfg, tla in jobs:
            if not tla.exists():
                rows.append(dict(config=str(cfg.relative_to(root)), result="no-module", message=None, exit=None, keys=config_lines(cfg)))
        for future in concurrent.futures.as_completed(futures):
            cfg, tla = futures[future]
            result, message, code = future.result()
            rows.append(dict(config=str(cfg.relative_to(root)), module=str(tla.relative_to(root)),
                             result=result, message=message, exit=code, keys=config_lines(cfg)))
    rows.sort(key=lambda r: r["config"])
    histogram = collections.Counter(bucket(r["result"], r["message"]) for r in rows)
    keys = collections.Counter(k for r in rows for k in r["keys"])
    lifted = {"PROPERTY", "SYMMETRY", "VIEW", "ALIAS"}
    keys["any of PROPERTY, SYMMETRY, VIEW, ALIAS"] = sum(1 for r in rows if lifted & set(r["keys"]))
    out = dict(schema=1, purpose="dive simulate result and refusal histogram over every pinned Examples configuration",
               source=args.source, examples_commit=root.name,
               command="dive simulate FILE --config CFG --traces 10 --depth 100 --seed 0 --time-limit %ds --max-memory 4g --output json" % args.time_limit,
               configs=len(rows), histogram=dict(histogram.most_common()), config_keys=dict(keys.most_common()), rows=rows)
    args.out.write_text(json.dumps(out, indent=1) + "\n")
    for name, count in histogram.most_common():
        print(f"{count:4d}  {name}")

if __name__ == "__main__":
    main()
