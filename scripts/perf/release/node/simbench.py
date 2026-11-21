#!/usr/bin/env python3
"""Simulation throughput of dive against TLC -simulate (S4, ADR 0162 section 14).

Runs every bench at each worker count, alternating dive and TLC, and writes
one JSON object per run to --runs. Each bench samples --traces N traces of
depth 100 with seed 0. dive's --traces is the total; TLC's num= is per worker
(D-003), so TLC gets num=N/W and both tools sample N traces. A "-notemporal"
variant of a bench runs the same configuration with its PROPERTY lines
removed. Startup is one trace of the bench, ten fresh runs per tool.

Usage (on a bench node, from ~/tools):
  simbench.py --dive ~/bins/dive --java JAVA --cp TLA2TOOLS:COMMUNITY... \
      --examples EXAMPLES_ROOT --library TLAPM_LIBRARY --runs ~/sim/runs.jsonl \
      [--reps 3] [--workers 1,4,8]
"""
import argparse, json, os, re, shutil, subprocess, tempfile, time
from pathlib import Path

# name, module dir (relative to the Examples root), module, cfg, traces.
BENCHES = [
    ("WalkFree", None, "WalkFree.tla", "WalkFree.cfg", 200000),
    ("bosco", "specifications/bosco", "bosco.tla", "bosco.cfg", 4000),
    ("MCBakery", "specifications/Bakery-Boulangerie", "MCBakery.tla", "MCBakery.cfg", 400),
    ("SimpleAllocator", "specifications/allocator", "SimpleAllocator.tla", "SimpleAllocator.cfg", 4000),
    ("DijkstraMutex-LSpec", "specifications/dijkstra-mutex/DijkstraMutex.toolbox/LSpec-model", "MC.tla", "MC.cfg", 4000),
    ("EWD840", "specifications/ewd840", "EWD840.tla", "EWD840.cfg", 40000),
]
TEMPORAL = {"SimpleAllocator", "DijkstraMutex-LSpec", "EWD840"}

WALK = """---- MODULE WalkFree ----
\\* A random walk with nothing to violate: every trace runs to the depth.
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == x' = x + 1 \\/ x' = x - 1
Spec == Init /\\ [][Next]_x
====
"""


def strip_properties(text):
    """Removes PROPERTY/PROPERTIES sections from a TLC configuration."""
    out, skipping = [], False
    keywords = r"^(CONSTANTS?|INIT|NEXT|SPECIFICATION|INVARIANTS?|PROPERTY|PROPERTIES|CONSTRAINTS?|ACTION_CONSTRAINTS?|SYMMETRY|VIEW|ALIAS|POSTCONDITION|CHECK_DEADLOCK)\b"
    for line in text.splitlines():
        m = re.match(keywords, line.strip())
        if m:
            skipping = m.group(1) in ("PROPERTY", "PROPERTIES")
        if not skipping:
            out.append(line)
    return "\n".join(out) + "\n"


def stage(args, name, rel, module, cfg, notemporal):
    work = Path(tempfile.mkdtemp(prefix=f"sim-{name}-", dir=args.tmp))
    if rel is None:
        (work / module).write_text(WALK)
        (work / cfg).write_text("SPECIFICATION Spec\n")
    else:
        shutil.copytree(Path(args.examples) / rel, work, dirs_exist_ok=True)
        if notemporal:
            (work / cfg).write_text(strip_properties((work / cfg).read_text()))
    return work


def timed(argv, cwd, timeout):
    t0 = time.perf_counter()
    done = subprocess.run(["/usr/bin/time", "-f", "RSS %M", *argv], cwd=cwd, capture_output=True,
                          text=True, timeout=timeout)
    wall = time.perf_counter() - t0
    rss = None
    for line in done.stderr.splitlines()[::-1]:
        if line.startswith("RSS "):
            rss = int(line.split()[1]) * 1024
            break
    return done, wall, rss


def run_dive(args, work, module, cfg, traces, workers):
    argv = [*args.bind, args.dive, "simulate", module, "--config", cfg, "--traces", str(traces),
            "--depth", "100", "--seed", "0", "--workers", str(workers), "--time-limit", "1800s",
            "--max-memory", "64g", "--output", "json"]
    done, wall, rss = timed(argv, work, 2000)
    row = dict(tool="dive", exit=done.returncode, wall_s=wall, peak_rss_bytes=rss, argv=argv[len(args.bind):])
    try:
        rep = json.loads(done.stdout)
        st = rep.get("statistics", {})
        row.update(result=rep.get("result"), traces=st.get("traces_started"), generated=st.get("generated"),
                   temporal=[(p["name"], p["checked_traces"], p["unchecked_traces"])
                             for p in st.get("properties", []) if p.get("part") == "temporal"])
    except json.JSONDecodeError:
        row.update(result="cli-error", error=done.stderr[-400:])
    return row


def run_tlc(args, work, module, cfg, traces, workers):
    meta = Path(tempfile.mkdtemp(prefix="meta-", dir=args.tmp))
    library = [f"-DTLA-Library={args.library}"] if args.library else []
    argv = [*args.bind, args.java, "-XX:+UseParallelGC", f"-Xmx{args.xmx}", "-Xss4m", *library, "-cp", args.cp,
            "tlc2.TLC", module, "-config", cfg, "-simulate", f"num={traces // workers}", "-depth", "100",
            "-workers", str(workers), "-seed", "0", "-metadir", str(meta)]
    done, wall, rss = timed(argv, work, 2000)
    shutil.rmtree(meta, ignore_errors=True)
    out = done.stdout
    progress = re.findall(r"Progress: ([\d,]+) states checked, ([\d,]+) traces generated", out)
    error = re.search(r"^Error: (.*)$", out, re.M)
    row = dict(tool="tlc", exit=done.returncode, wall_s=wall, peak_rss_bytes=rss, argv=argv[len(args.bind):],
               result="violation" if error else ("sampled" if done.returncode == 0 else "error"),
               error=error.group(1) if error else None)
    if progress:
        states, tr = progress[-1]
        row.update(generated=int(states.replace(",", "")), traces=int(tr.replace(",", "")))
    return row


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--dive", required=True)
    p.add_argument("--java", required=True)
    p.add_argument("--cp", required=True)
    p.add_argument("--examples", required=True)
    p.add_argument("--runs", type=Path, required=True)
    p.add_argument("--reps", type=int, default=3)
    p.add_argument("--workers", default="1,4,8")
    p.add_argument("--xmx", default="16g")
    p.add_argument("--library", help="TLA-Library directory for TLC (the tlapm library, for TLAPS)")
    p.add_argument("--only", action="append", default=[])
    p.add_argument("--tmp", default=os.environ.get("TMPDIR", "/tmp"))
    p.add_argument("--bind", default="numactl --cpunodebind=0 --membind=0")
    args = p.parse_args()
    args.bind = args.bind.split()
    workers = [int(w) for w in args.workers.split(",")]
    args.runs.parent.mkdir(parents=True, exist_ok=True)
    benches = []
    for name, rel, module, cfg, traces in BENCHES:
        benches.append((name, rel, module, cfg, traces, False))
        if name in TEMPORAL:
            benches.append((name + "-notemporal", rel, module, cfg, traces, True))
    with args.runs.open("a") as sink:
        def emit(row):
            sink.write(json.dumps(row) + "\n")
            sink.flush()
            print(row["bench"], row["kind"], row["tool"], row.get("workers"), row["rep"], row.get("result"),
                  row.get("traces"), f'{row["wall_s"]:.3f}s', flush=True)
        for name, rel, module, cfg, traces, notemporal in benches:
            if args.only and name not in args.only:
                continue
            work = stage(args, name, rel, module, cfg, notemporal)
            for rep in range(10):
                for tool in (run_dive, run_tlc):
                    row = tool(args, work, module, cfg, 1, 1)
                    emit(dict(row, bench=name, kind="startup", workers=1, requested_traces=1, rep=rep))
            for w in workers:
                for rep in range(args.reps):
                    for tool in (run_dive, run_tlc):
                        row = tool(args, work, module, cfg, traces, w)
                        emit(dict(row, bench=name, kind="throughput", workers=w, requested_traces=traces, rep=rep))
            shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    main()
