#!/usr/bin/env python3
"""R2 procedure driver (docs/developer/performance.md#r2-measurement-procedure)
until #983 lands. Runs on a bench node as user `bench`.

  r2drive.py classify <models.txt> <out>      one --jobs 1 TLC pass, --timeout 15
  r2drive.py small <models.txt> <out> <classify-report>   six alternating reps
  r2drive.py large <models.txt> <out>         first pass + banded pairs

Writes harness reports under <out>/ and a compact <out>/summary.json.
"""
import json
import os
import statistics
import subprocess
import sys
import time

HOME = os.path.expanduser("~")
CACHE = os.path.join(HOME, ".cache", "dive")
HARNESS = os.path.join(HOME, "bins", "dive-conformance")
DIVE = os.path.join(HOME, "bins", "dive")
PATH = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
PREFIX = ["numactl", "--cpunodebind=0", "--membind=0",
          "env", "-i", f"PATH={PATH}", f"HOME={HOME}", f"DIVE_CACHE_DIR={CACHE}"]
COMMON = ["run", "--corpus", "examples", "--tier", "all", "--workers", "1",
          "--jobs", "1", "--xmx", "24g", "--no-summary"]
AGREE = ("agree", "expected-divergence")


def log(out, msg):
    with open(os.path.join(out, "log.txt"), "a") as f:
        f.write(time.strftime("%Y-%m-%dT%H:%M:%SZ ", time.gmtime()) + msg + "\n")


def filters(models):
    args = []
    for m in models:
        args += ["--filter", m]
    return args


def harness(out, tag, args, models):
    d = os.path.join(out, tag)
    os.makedirs(d, exist_ok=True)
    cmd = PREFIX + [HARNESS] + COMMON + args + ["--report-dir", d] + filters(models)
    log(out, f"start {tag} ({len(models)} models)")
    with open(os.path.join(d, "console.txt"), "w") as f:
        rc = subprocess.call(cmd, cwd=os.path.join(HOME, "dive"), stdout=f, stderr=subprocess.STDOUT)
    reps = sorted(p for p in os.listdir(d) if p.endswith(".json") and not p.endswith("-r2.json"))
    log(out, f"end {tag} rc={rc} reports={reps}")
    if not reps:
        raise SystemExit(f"{tag}: no report")
    return os.path.join(d, reps[-1])


def tlc(out, tag, models, extra=()):
    return harness(out, tag, ["--impl", "tlc", "--no-sany", "--timeout-cap", "30000", *extra], models)


def core(out, tag, models, timings):
    return harness(out, tag, ["--impl", "dive:core", "--impl-profile", "dive", "--dive-bin", DIVE,
                              "--tlc-timings", timings, "--budget-cap", "30000"], models)


def rows(report):
    with open(report) as f:
        d = json.load(f)
    res = {}
    for m in d["models"]:
        run = (m.get("runs") or [{}])[0]
        res[m["model"]] = {"outcome": m["outcome"], "elapsed_ms": run.get("elapsed_ms"),
                           "detail": (m.get("detail") or run.get("detail"))}
    return res


def scaled(report, factor, dest):
    """A copy of a TLC report with every elapsed_ms scaled, so that the harness's
    2 x TLC budget becomes 2 x factor x TLC."""
    with open(report) as f:
        d = json.load(f)
    for m in d["models"]:
        for r in m.get("runs") or []:
            if r.get("elapsed_ms") is not None:
                r["elapsed_ms"] = int(r["elapsed_ms"] * factor)
    with open(dest, "w") as f:
        json.dump(d, f)
    return dest


def ratio(c, t):
    if c is None or t is None or not t.get("elapsed_ms") or c.get("elapsed_ms") is None:
        return None
    return round(c["elapsed_ms"] / t["elapsed_ms"], 4)


def save(out, summary):
    with open(os.path.join(out, "summary.json"), "w") as f:
        json.dump(summary, f, indent=1, sort_keys=True)


def classify(models, out):
    rep = tlc(out, "classify", models, ["--timeout", "15"])
    save(out, {"classify_report": rep, "rows": rows(rep)})


def small(models, out, classify_report):
    summary = {"reps": []}
    prev_tlc = classify_report
    for i in range(6):
        order = ["tlc", "core"] if i % 2 == 0 else ["core", "tlc"]
        got = {}
        for side in order:
            if side == "tlc":
                got["tlc"] = tlc(out, f"rep{i+1}-tlc", models)
            else:
                timings = got.get("tlc", prev_tlc)
                got["core"] = core(out, f"rep{i+1}-core", models, timings)
        prev_tlc = got["tlc"]
        t, c = rows(got["tlc"]), rows(got["core"])
        summary["reps"].append({"order": order, "tlc_report": got["tlc"], "core_report": got["core"],
                                "models": {m: {"tlc_ms": t.get(m, {}).get("elapsed_ms"),
                                               "core_ms": c.get(m, {}).get("elapsed_ms"),
                                               "tlc_outcome": t.get(m, {}).get("outcome"),
                                               "core_outcome": c.get(m, {}).get("outcome"),
                                               "core_detail": c.get(m, {}).get("detail"),
                                               "ratio": ratio(c.get(m), t.get(m))} for m in models}})
        save(out, summary)
    log(out, "small DONE")


def banded(r, outcome):
    if outcome not in AGREE:
        return True  # a Core budget stop (or other non-agreement) gets pairs
    return r is not None and (0.9 <= r <= 1.1 or 1.8 <= r <= 2.0)


def large(models, out):
    summary = {"first_pass": {}, "pairs": {}}
    t_rep = tlc(out, "first-tlc", models)
    c_rep = core(out, "first-core", models, t_rep)
    t, c = rows(t_rep), rows(c_rep)
    for m in models:
        summary["first_pass"][m] = {"tlc_ms": t.get(m, {}).get("elapsed_ms"),
                                    "core_ms": c.get(m, {}).get("elapsed_ms"),
                                    "tlc_outcome": t.get(m, {}).get("outcome"),
                                    "core_outcome": c.get(m, {}).get("outcome"),
                                    "core_detail": c.get(m, {}).get("detail"),
                                    "ratio": ratio(c.get(m), t.get(m))}
    summary["first_reports"] = [t_rep, c_rep]
    save(out, summary)
    for m in models:
        fp = summary["first_pass"][m]
        if fp["tlc_outcome"] not in AGREE or not banded(fp["ratio"], fp["core_outcome"]):
            continue
        safe = m.replace("/", "_")
        pairs = []
        for k in range(3):
            if k % 2 == 0:
                tr = tlc(out, f"pair-{safe}-{k+1}-tlc", [m])
                cr = core(out, f"pair-{safe}-{k+1}-core", [m],
                          scaled(tr, 1.25, os.path.join(out, f"pair-{safe}-{k+1}-timings.json")))
                order = ["tlc", "core"]
            else:
                cr = core(out, f"pair-{safe}-{k+1}-core", [m],
                          scaled(t_rep, 1.25, os.path.join(out, f"pair-{safe}-{k+1}-timings.json")))
                tr = tlc(out, f"pair-{safe}-{k+1}-tlc", [m])
                order = ["core", "tlc"]
            tt, cc = rows(tr).get(m), rows(cr).get(m)
            pairs.append({"order": order, "tlc_ms": tt and tt["elapsed_ms"], "core_ms": cc and cc["elapsed_ms"],
                          "core_outcome": cc and cc["outcome"], "ratio": ratio(cc, tt)})
            summary["pairs"][m] = pairs
            save(out, summary)
    log(out, "large DONE")


if __name__ == "__main__":
    mode, models_file, out = sys.argv[1:4]
    os.makedirs(out, exist_ok=True)
    with open(models_file) as f:
        models = [line.strip() for line in f if line.strip()]
    if mode == "classify":
        classify(models, out)
    elif mode == "small":
        small(models, out, sys.argv[4])
    elif mode == "large":
        large(models, out)
    log(out, f"{mode} exit")
