#!/usr/bin/env python3
"""dive release measurements D2 (liveness part), D3, D5, D6 on a bench node.

  dmeasure.py d5|d3|d6 <out>
  dmeasure.py d2live <out> <models.json>   [{"model": cfg-id, "budget_s": n}, ...]

Runs one process at a time through runjobs.run_jobs (GNU time -v / time -l).
Paths are relative to the pinned Examples checkout.
"""
import glob
import json
import os
import platform
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runjobs  # noqa: E402

H = os.path.expanduser("~")
DIVE = os.environ.get("DIVE_BIN", f"{H}/bins/dive")
EX = glob.glob(f"{H}/.cache/dive/git/examples/*/")[0]
REPO = f"{H}/dive"
BENCH_SET = [
    "specifications/GameOfLife/GameOfLife.cfg", "specifications/dag-consensus/TLCSailfish1.cfg",
    "specifications/ewd998/EWD998PCal.cfg", "specifications/Bakery-Boulangerie/MCBakery.cfg",
    "specifications/lamport_mutex/MCLamportMutex.cfg", "specifications/TeachingConcurrency/SimpleRegular.cfg",
    "specifications/btree/MCbtree.cfg", "COFFEE_SAFETY", "specifications/Disruptor/MCDisruptor_MPMC.cfg",
    "specifications/braf/BufferedRandomAccessFile.cfg"]
LIVE = ["specifications/CoffeeCan/CoffeeCan1000Beans.cfg", "specifications/MultiCarElevator/ElevatorLivenessMedium.cfg",
        "specifications/SpecifyingSystems/Liveness/MCLiveInternalMemory.cfg",
        "specifications/FlashProtocol/MCFlashWithMutexLive.cfg", "specifications/aba-asyn-byz/aba_asyn_byz.cfg"]
COFFEE_SAFETY_CFG = "CONSTANTS\n    MaxBeanCount = 1000\n\nSPECIFICATION\n    Spec\n\nINVARIANTS\n    TypeInvariant\n"


def module_of(cfg):
    """The module a corpus model checks, from its manifest."""
    d = os.path.dirname(cfg)
    while d and not os.path.exists(os.path.join(EX, d, "manifest.json")):
        d = os.path.dirname(d)
    man = json.load(open(os.path.join(EX, d, "manifest.json")))
    for mod in man.get("modules", []):
        for m in mod.get("models", []):
            if os.path.normpath(m["path"]) == os.path.normpath(cfg):
                return mod["path"]
    raise SystemExit(f"no module for {cfg}")


def model_args(cfg, out):
    if cfg == "COFFEE_SAFETY":
        path = os.path.join(out, "CoffeeCan1000BeansSafety.cfg")
        open(path, "w").write(COFFEE_SAFETY_CFG)
        return "specifications/CoffeeCan/CoffeeCan.tla", path
    return module_of(cfg), cfg


def check(mod, cfg, *extra):
    return [DIVE, "check", mod, "--config", cfg, "--workers", "1", "--seed", "0", "--fp", "0", *extra,
            "--output", "json"]


def d3(out):
    jobs = []
    for cfg in BENCH_SET + LIVE:
        mod, c = model_args(cfg, out)
        for rep in range(3):
            stores = ["fp", "exact"] if rep % 2 == 0 else ["exact", "fp"]
            for store in stores:
                jobs.append({"id": f"{cfg}|{store}|{rep+1}", "cwd": EX,
                             "argv": check(mod, c, "--store", store, "--time-limit", "1800s",
                                           "--max-memory", "64g")})
    runjobs.run_jobs(jobs, out)


def d5(out):
    jobs = []
    ctla, ccfg = "docs/user/examples/Counter.tla", "docs/user/examples/Counter.cfg"
    for i in range(50):
        jobs.append({"id": "version", "cwd": REPO, "argv": [DIVE, "--version"], "time": False, "stdout_json": False})
    for i in range(50):
        jobs.append({"id": "parse", "cwd": REPO, "argv": [DIVE, "parse", ctla], "time": False, "stdout_json": False})
    for i in range(50):
        jobs.append({"id": "check", "cwd": REPO,
                     "argv": [DIVE, "check", ctla, "--config", ccfg, "--workers", "1", "--seed", "0", "--fp", "0",
                              "--output", "json"]})
    for cfg in ["specifications/lamport_mutex/MCLamportMutex.cfg", "specifications/bosco/bosco.cfg"]:
        for rep in range(3):
            jobs.append({"id": f"simulate|{cfg}|{rep+1}", "cwd": EX,
                         "argv": [DIVE, "simulate", module_of(cfg), "--config", cfg, "--traces", "10000", "--depth",
                                  "100", "--seed", "0", "--stats", "basic", "--time-limit", "600s", "--max-memory",
                                  "16g", "--output", "json"]})
    runjobs.run_jobs(jobs, os.path.join(out, "a"))
    # Counterexamples, produced once; then ten validations each.
    traces = os.path.join(out, "traces")
    os.makedirs(traces, exist_ok=True)
    jobs = []
    for cfg in ["specifications/DieHard/DieHard.cfg", "specifications/SlidingPuzzles/SlidingPuzzles.cfg",
                "specifications/SpecifyingSystems/RealTime/MCRealTimeHourClock.cfg"]:
        mod = module_of(cfg)
        run = os.path.join(traces, os.path.basename(cfg) + ".run.json")
        os.system(f"cd {EX} && " + " ".join([DIVE, "check", mod, "--config", cfg, "--workers", "1", "--seed", "0",
                                              "--fp", "0", "--output", "json"]) + f" > {run}")
        r = json.load(open(run))
        t = os.path.join(traces, os.path.basename(cfg) + ".trace.json")
        json.dump(r["counterexample"]["trace"], open(t, "w"))
        for i in range(10):
            jobs.append({"id": f"validate|{cfg}", "cwd": EX,
                         "argv": [DIVE, "validate-trace", mod, "--config", cfg, "--trace", t, "--seed", "0", "--fp",
                                  "0", "--output", "json"]})
    # --sample validation of 100 saved simulation traces. MCLamportMutex's
    # simulation stops at its first trace (a reported deadlock), so the 100
    # traces come from bosco's simulation.
    cfg = "specifications/bosco/bosco.cfg"
    mod = module_of(cfg)
    sim = os.path.join(traces, "bosco.sim.json")
    os.system(f"cd {EX} && {DIVE} simulate {mod} --config {cfg} --traces 100 --depth 100 --seed 0 --output json > {sim}")
    for i, tr in enumerate(json.load(open(sim))["traces"][:100]):
        t = os.path.join(traces, f"bosco-{i:03d}.trace.json")
        json.dump(tr["document"], open(t, "w"))
        jobs.append({"id": f"sample|{cfg}", "cwd": EX,
                     "argv": [DIVE, "validate-trace", mod, "--config", cfg, "--trace", t, "--sample", "--seed", "0",
                              "--output", "json"]})
    runjobs.run_jobs(jobs, os.path.join(out, "b"))


def d6(out):
    jobs = []
    big = {"specifications/MultiCarElevator/ElevatorSafetyLarge.cfg": "safety",
           "specifications/SlushProtocol/SlushLarge.cfg": "safety",
           "specifications/MultiCarElevator/ElevatorLivenessLarge.cfg": "temporal"}
    for cfg in big:
        mod = module_of(cfg)
        for w in ["1", "8"]:
            for rep in range(3):
                jobs.append({"id": f"load|{cfg}|{w}|{rep+1}", "cwd": EX,
                             "argv": [DIVE, "check", mod, "--config", cfg, "--workers", w, "--seed", "0", "--fp", "0",
                                      "--max-states", "1", "--output", "json"]})
            for limit in ["5s", "30s", "120s"]:
                for rep in range(5):
                    jobs.append({"id": f"time|{cfg}|{w}|{limit}|{rep+1}", "cwd": EX,
                                 "argv": [DIVE, "check", mod, "--config", cfg, "--workers", w, "--seed", "0", "--fp",
                                          "0", "--time-limit", limit, "--max-memory", "64g", "--output", "json"]})
    cfg = "specifications/SlushProtocol/SlushLarge.cfg"
    mod = module_of(cfg)
    for w in ["1", "8"]:
        for flag, val in [("--max-states", "1000000"), ("--max-states", "10000000"), ("--max-depth", "10"),
                          ("--max-depth", "30")]:
            for rep in range(3):
                jobs.append({"id": f"ceiling|{cfg}|{w}|{flag}={val}|{rep+1}", "cwd": EX,
                             "argv": [DIVE, "check", mod, "--config", cfg, "--workers", w, "--seed", "0", "--fp", "0",
                                      flag, val, "--time-limit", "3600s", "--max-memory", "64g", "--output", "json"]})
    cfg = "specifications/MultiCarElevator/ElevatorLivenessLarge.cfg"
    mod = module_of(cfg)
    for flag, val in [("--max-states", "20000"), ("--max-depth", "20")]:
        for rep in range(3):
            jobs.append({"id": f"temporal-ceiling|{cfg}|{flag}={val}|{rep+1}", "cwd": EX,
                         "argv": [DIVE, "check", mod, "--config", cfg, "--workers", "1", "--seed", "0", "--fp", "0",
                                  "--store", "exact", flag, val, "--time-limit", "3600s", "--max-memory", "64g",
                                  "--output", "json"]})
    runjobs.run_jobs(jobs, out)


def d2live(out, models):
    jobs = []
    for m in json.load(open(models)):
        mod = module_of(m["model"])
        for rep in range(3):
            jobs.append({"id": f"{m['model']}|{rep+1}", "cwd": EX,
                         "argv": check(mod, m["model"], "--time-limit", f"{m['budget_s']}s", "--max-memory", "64g")})
    runjobs.run_jobs(jobs, out)


def probe(out, models):
    """One run per model to keep those whose report says check_kind temporal."""
    jobs = []
    for m in json.load(open(models)):
        jobs.append({"id": m["model"], "cwd": EX,
                     "argv": check(module_of(m["model"]), m["model"], "--time-limit", f"{m['budget_s']}s",
                                   "--max-memory", "64g")})
    runjobs.run_jobs(jobs, out)


if __name__ == "__main__":
    mode, out = sys.argv[1], os.path.abspath(sys.argv[2])
    os.makedirs(out, exist_ok=True)
    if mode in ("d2live", "probe"):
        {"d2live": d2live, "probe": probe}[mode](out, sys.argv[3])
    else:
        {"d3": d3, "d5": d5, "d6": d6}[mode](out)
