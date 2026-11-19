#!/usr/bin/env python3
"""Sail native temporal job measurements S1-S5 (dive
docs/developer/release-performance-plan.md#sail-measurements).

  sail_temporal.py fixtures      build jobs from the pinned Examples checkout
  sail_temporal.py s1|s2|s3|s4|s5

Layout (environment overrides): SAIL_BIN, WORKER, BENCH (temporal_bench
example), DIVE_BIN, EXAMPLES, GOLDEN, PERF_MEM, OUT (default ~/st).
Every row is one JSON line in $OUT/<s>.jsonl; raw responses stay in $OUT/raw.
"""
import json
import os
import random
import re
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import time

H = os.path.expanduser("~")
SAIL = os.environ.get("SAIL_BIN", f"{H}/inst/bin/sail")
WORKER = os.environ.get("WORKER", f"{H}/inst/bin/sail-runtime-worker")
BENCH = os.environ.get("BENCH", f"{H}/sail/target/release/examples/temporal_bench")
DIVE = os.environ.get("DIVE_BIN", f"{H}/bins/dive")
EX = os.environ.get("EXAMPLES", f"{H}/examples")
GOLD = os.environ.get("GOLDEN", f"{H}/dive/conformance/golden/tlc-2026.10.06.014338/examples")
PERF_MEM = os.environ.get("PERF_MEM", f"{H}/sail/scripts/perf-mem.py")
OUT = os.environ.get("OUT", f"{H}/st")
FIX = os.path.join(OUT, "fixtures")
WALL = 300_000
STD = {"Naturals", "Integers", "Sequences", "FiniteSets", "Bags"}
IMPORT = re.compile(r"\b(?:EXTENDS|INSTANCE)\s+([A-Za-z0-9_]+(?:\s*,\s*[A-Za-z0-9_]+)*)")

CHECKS = [
    ("MCLamportMutex", "specifications/lamport_mutex/MCLamportMutex.tla", "MCLamportMutex.cfg"),
    ("SlushSmall", "specifications/SlushProtocol/Slush.tla", "SlushSmall.cfg"),
    ("MCDisruptor_MPMC", "specifications/Disruptor/MCDisruptor_MPMC.tla", "MCDisruptor_MPMC.cfg"),
    ("CoffeeCan1000Beans", "specifications/CoffeeCan/CoffeeCan.tla", "CoffeeCan1000Beans.cfg"),
    ("ElevatorLivenessMedium", "specifications/MultiCarElevator/Elevator.tla", "ElevatorLivenessMedium.cfg"),
    ("MCLiveInternalMemory", "specifications/SpecifyingSystems/Liveness/MCLiveInternalMemory.tla", "MCLiveInternalMemory.cfg"),
    ("MCDisruptor_MPMC_liveliness", "specifications/Disruptor/MCDisruptor_MPMC.tla", "MCDisruptor_MPMC_liveliness.cfg"),
    ("MCRealTimeHourClock", "specifications/SpecifyingSystems/RealTime/MCRealTimeHourClock.tla", "MCRealTimeHourClock.cfg"),
    ("EWD840", "specifications/ewd840/EWD840.tla", "EWD840.cfg"),
    ("DieHard", "specifications/DieHard/DieHard.tla", "DieHard.cfg"),
    ("SlidingPuzzles", "specifications/SlidingPuzzles/SlidingPuzzles.tla", "SlidingPuzzles.cfg"),
]
SIMS = [
    ("MCLamportMutex-sim", "specifications/lamport_mutex/MCLamportMutex.tla", "MCLamportMutex.cfg"),
    ("bosco-sim", "specifications/bosco/bosco.tla", "bosco.cfg"),
]
TRACES = ["DieHard", "SlidingPuzzles", "MCRealTimeHourClock"]
LONG = ("ElevatorSafetyLarge", "specifications/MultiCarElevator/Elevator.tla", "ElevatorSafetyLarge.cfg")


def strip(text):
    return re.sub(r"\\\*[^\n]*", " ", re.sub(r"\(\*.*?\*\)", " ", text, flags=re.S))


def closure(module):
    base = os.path.dirname(module)
    files, todo, seen = {}, [os.path.basename(module)[:-4]], set()
    while todo:
        name = todo.pop()
        if name in seen:
            continue
        seen.add(name)
        path = os.path.join(EX, base, name + ".tla")
        if not os.path.isfile(path):
            raise SystemExit(f"{module}: import {name} outside the closure and the admitted modules")
        text = open(path, encoding="utf-8").read()
        files[name + ".tla"] = text
        for m in IMPORT.finditer(strip(text)):
            todo += [n for n in re.split(r"\s*,\s*", m.group(1)) if n not in STD]
    return files


def golden(module, cfg):
    g = json.load(open(os.path.join(GOLD, os.path.dirname(module), cfg + ".json")))
    r = g["result"]
    return {"verdict": r["verdict"], "distinct": r["counts"]["distinct_states"],
            "depth": r["counts"]["depth"], "violated_property": r.get("violated_property")}


def request(module, cfg, operation):
    files = closure(module)
    files[cfg] = open(os.path.join(EX, os.path.dirname(module), cfg), encoding="utf-8").read()
    return {"version": 1, "model": {"files": dict(sorted(files.items())),
                                    "root": os.path.basename(module), "config": cfg},
            "operation": operation}


def write_fixture(name, module, cfg, req, kind, gold):
    d = os.path.join(FIX, name)
    os.makedirs(os.path.join(d, "model"), exist_ok=True)
    for p, text in req["model"]["files"].items():
        open(os.path.join(d, "model", p), "w").write(text)
    json.dump(req, open(os.path.join(d, "request.json"), "w"))
    job = {"obligation_id": f"perf/{name}", "requirement_ids": ["PERF-1"], "request": req,
           "wall_time_ms": WALL}
    json.dump({"version": 1, "input": {"kind": "temporal", "job": job}},
              open(os.path.join(d, "envelope.json"), "w"))
    json.dump({"name": name, "module": module, "config": cfg, "kind": kind, "golden": gold,
               "operation": {k: v for k, v in req["operation"].items() if k != "trace"}},
              open(os.path.join(d, "fixture.json"), "w"))


def sail_execute(envelope, extra=(), env=None, timeout=400):
    t = time.perf_counter()
    p = subprocess.run([SAIL, "runtime", "execute", "--file", envelope, *extra], capture_output=True,
                       timeout=timeout, env=env)
    wall = (time.perf_counter() - t) * 1000
    try:
        doc = json.loads(p.stdout)
    except ValueError:
        doc = None
    return wall, p.returncode, doc, p.stderr.decode(errors="replace")[-400:]


def result_of(doc):
    """(execution outcome, native outcome, completion, distinct, depth, response)."""
    if not isinstance(doc, dict):
        return None, None, None, None, None, None
    res = doc.get("output", doc).get("result", doc.get("result")) if isinstance(doc.get("output", doc), dict) else None
    if res is None:
        # search for the temporal result object
        def find(v):
            if isinstance(v, dict):
                if "request_sha256" in v and "worker_sha256" in v:
                    return v
                for x in v.values():
                    r = find(x)
                    if r:
                        return r
            return None
        res = find(doc)
    if not isinstance(res, dict):
        return None, None, None, None, None, None
    resp = res.get("response") or {}
    rep = resp.get("native_report") or {}
    st = rep.get("statistics") or {}
    return (res.get("outcome"), resp.get("outcome"), resp.get("completion"), st.get("distinct"),
            st.get("depth"), resp)


def fixtures():
    os.makedirs(FIX, exist_ok=True)
    for name, module, cfg in CHECKS + [LONG]:
        gold = golden(module, cfg)
        states = max(1000, 2 * gold["distinct"])
        write_fixture(name, module, cfg, request(module, cfg, {"kind": "check", "max_states": states,
                                                               "max_depth": 100_000}), "check", gold)
    for name, module, cfg in SIMS:
        write_fixture(name, module, cfg, request(module, cfg, {"kind": "simulate", "traces": 1000,
                                                               "depth": 100}), "simulate", None)
    # Validation fixtures: ITF traces from Sail check jobs, and from the simulation.
    for name in TRACES:
        _, rc, doc, err = sail_execute(os.path.join(FIX, name, "envelope.json"))
        outcome, native, *_ , resp = result_of(doc)
        if not resp or not resp.get("itf_traces"):
            raise SystemExit(f"{name}: no ITF trace ({outcome} {native} {err})")
        fx = json.load(open(os.path.join(FIX, name, "fixture.json")))
        req = request(fx["module"], fx["config"], {"kind": "validate", "trace": resp["itf_traces"][0]})
        write_fixture(name + "-validate", fx["module"], fx["config"], req, "validate", fx["golden"])
        json.dump(resp["native_traces"][0], open(os.path.join(FIX, name + "-validate", "native-trace.json"), "w"))
    # Sail's MCLamportMutex simulation returns one invalid-ending trace, so the
    # 100 validation traces come from the dive CLI's bosco simulation (seed 0),
    # converted to ITF with sail_dive::native_to_itf.
    module, cfg = SIMS[1][1], SIMS[1][2]
    d = os.path.join(FIX, "bosco-sim", "model")
    p = subprocess.run([DIVE, "simulate", os.path.basename(module), "--config", cfg, "--traces", "100",
                        "--depth", "100", "--seed", "0", "--output", "json"], cwd=d, capture_output=True)
    natives = [t["document"] for t in json.loads(p.stdout)["traces"]]
    for i, native in enumerate(natives[:100]):
        name = f"bosco-sim-trace-{i:03d}"
        os.makedirs(os.path.join(FIX, name), exist_ok=True)
        npath = os.path.join(FIX, name, "native-trace.json")
        json.dump(native, open(npath, "w"))
        itf = json.loads(subprocess.run([BENCH, "--itf", npath], capture_output=True, check=True).stdout)
        write_fixture(name, module, cfg, request(module, cfg, {"kind": "validate", "trace": itf}),
                      "validate", None)
    print("fixtures ok", len(natives))


def worker_sha():
    import hashlib
    return hashlib.sha256(open(WORKER, "rb").read()).hexdigest()


def emit(s, row):
    with open(os.path.join(OUT, f"{s}.jsonl"), "a") as f:
        f.write(json.dumps(row) + "\n")


def dive_cli(fx, d):
    m = os.path.join(d, "model")
    root, cfg = os.path.basename(fx["module"]), fx["config"]
    op = fx["operation"]
    if fx["kind"] == "check":
        argv = [DIVE, "check", root, "--config", cfg, "--store", "exact", "--workers", "1", "--seed", "0",
                "--fp", "0", "--max-states", str(op["max_states"]), "--max-depth", str(op["max_depth"]),
                "--output", "json"]
    elif fx["kind"] == "simulate":
        argv = [DIVE, "simulate", root, "--config", cfg, "--traces", str(op["traces"]), "--depth",
                str(op["depth"]), "--seed", "0", "--output", "json"]
    else:
        argv = [DIVE, "validate-trace", root, "--config", cfg, "--trace",
                os.path.join(d, "native-trace.json"), "--seed", "0", "--fp", "0", "--output", "json"]
    t = time.perf_counter()
    p = subprocess.run(argv, cwd=m, capture_output=True)
    wall = (time.perf_counter() - t) * 1000
    try:
        rep = json.loads(p.stdout)
    except ValueError:
        rep = {}
    st = rep.get("statistics") or {}
    return {"wall_ms": round(wall, 2), "rc": p.returncode, "verdict": rep.get("verdict") or rep.get("result")
            or (rep.get("validation") if isinstance(rep.get("validation"), str) else None),
            "distinct": st.get("distinct"), "depth": st.get("depth")}


def bench(d):
    p = subprocess.run([BENCH, os.path.join(d, "request.json"), WORKER, worker_sha(), str(WALL)],
                       capture_output=True)
    try:
        return json.loads(p.stdout)
    except ValueError:
        return {"error": p.stderr.decode(errors="replace")[-400:]}


def s1():
    names = [n for n, *_ in CHECKS] + [n for n, *_ in SIMS] + [t + "-validate" for t in TRACES]
    for name in names:
        d = os.path.join(FIX, name)
        fx = json.load(open(os.path.join(d, "fixture.json")))
        for rep in range(5):
            order = ["sail", "cli"] if rep % 2 == 0 else ["cli", "sail"]
            row = {"fixture": name, "kind": fx["kind"], "rep": rep + 1, "order": order}
            for side in order:
                if side == "sail":
                    row["phases"] = bench(d)
                    w, rc, doc, err = sail_execute(os.path.join(d, "envelope.json"))
                    o = result_of(doc)
                    row["sail_execute"] = {"wall_ms": round(w, 2), "rc": rc, "execution": o[0],
                                           "outcome": o[1], "completion": o[2], "distinct": o[3], "depth": o[4]}
                else:
                    row["cli"] = dive_cli(fx, d)
            emit("s1", row)


class Mcp:
    def __init__(self, env, cwd):
        self.p = subprocess.Popen([SAIL, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, env=env, cwd=cwd)
        self.n = 0
        self.call("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                 "clientInfo": {"name": "perf", "version": "0"}})
        self.send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def send(self, msg):
        self.p.stdin.write((json.dumps(msg) + "\n").encode())
        self.p.stdin.flush()

    def call(self, method, params):
        self.n += 1
        self.send({"jsonrpc": "2.0", "id": self.n, "method": method, "params": params})
        while True:
            line = self.p.stdout.readline()
            if not line:
                return None
            msg = json.loads(line)
            if msg.get("id") == self.n:
                return msg

    def execute(self, envelope):
        req = json.load(open(envelope))
        return self.call("tools/call", {"name": "runtime_execute", "arguments": {"request": req}})

    def close(self):
        self.p.stdin.close()
        try:
            self.p.wait(10)
        except subprocess.TimeoutExpired:
            self.p.kill()


def mcp_outcome(msg):
    try:
        text = msg["result"]["content"][0]["text"]
        return result_of(json.loads(text))[:3]
    except (KeyError, TypeError, ValueError, IndexError):
        try:
            return result_of(msg["result"]["structuredContent"])[:3]
        except (KeyError, TypeError):
            return (None, None, None)


def stop_daemons():
    subprocess.run(["pkill", "-f", f"{SAIL} serve"], capture_output=True)
    for _ in range(100):
        if subprocess.run(["pgrep", "-f", f"{SAIL} serve"], capture_output=True).returncode != 0:
            return
        time.sleep(0.05)


def fresh_env():
    root = tempfile.mkdtemp(prefix="sail-perf-")
    env = {"PATH": os.environ.get("PATH", "/usr/bin:/bin"), "HOME": H,
           "SAIL_CACHE_DIR": os.path.join(root, "cache"),
           "SAIL_DAEMON_SOCKET": os.path.join(root, "cache", "sail.sock")}
    os.makedirs(env["SAIL_CACHE_DIR"], mode=0o700)
    return root, env


def wait_socket(path, t0, limit=60):
    while time.perf_counter() - t0 < limit:
        if os.path.exists(path):
            return (time.perf_counter() - t0) * 1000
        time.sleep(0.005)
    return None


def s2():
    for name in ["MCLiveInternalMemory", "DieHard-validate"]:
        env_path = os.path.join(FIX, name, "envelope.json")
        for state in ["cold", "warm"]:
            for rep in range(10):
                # embedded
                root, env = fresh_env()
                if state == "warm":
                    sail_execute(env_path, env=env)
                w, rc, doc, _ = sail_execute(env_path, env=env)
                emit("s2", {"fixture": name, "path": "embedded", "state": state, "rep": rep + 1,
                            "wall_ms": round(w, 2), "rc": rc, "outcome": result_of(doc)[:3]})
                shutil.rmtree(root, ignore_errors=True)
                # socket
                stop_daemons()
                root, env = fresh_env()
                t0 = time.perf_counter()
                d = subprocess.Popen([SAIL, "serve", "--socket", env["SAIL_DAEMON_SOCKET"]], env=env,
                                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                start_ms = wait_socket(env["SAIL_DAEMON_SOCKET"], t0)
                time.sleep(0.2)
                if state == "warm":
                    sail_execute(env_path, ["--socket", env["SAIL_DAEMON_SOCKET"]], env=env)
                w, rc, doc, err = sail_execute(env_path, ["--socket", env["SAIL_DAEMON_SOCKET"]], env=env)
                emit("s2", {"fixture": name, "path": "socket", "state": state, "rep": rep + 1,
                            "wall_ms": round(w, 2), "rc": rc, "outcome": result_of(doc)[:3],
                            "daemon_socket_ms": start_ms, "err": None if rc == 0 else err})
                d.terminate()
                d.wait()
                stop_daemons()
                shutil.rmtree(root, ignore_errors=True)
                # MCP (autostarts sail serve when none is running)
                root, env = fresh_env()
                env["PATH"] = os.path.dirname(SAIL) + ":" + env["PATH"]
                proj = os.path.join(root, "proj")
                os.makedirs(proj)
                t0 = time.perf_counter()
                m = Mcp(env, proj)
                init_ms = (time.perf_counter() - t0) * 1000
                if state == "warm":
                    m.execute(env_path)
                t1 = time.perf_counter()
                msg = m.execute(env_path)
                w = (time.perf_counter() - t1) * 1000
                emit("s2", {"fixture": name, "path": "mcp", "state": state, "rep": rep + 1,
                            "wall_ms": round(w, 2), "mcp_init_ms": round(init_ms, 2),
                            "daemon_present": os.path.exists(env["SAIL_DAEMON_SOCKET"]),
                            "outcome": mcp_outcome(msg) if msg else None,
                            "is_error": (msg or {}).get("result", {}).get("isError")})
                m.close()
                stop_daemons()
                shutil.rmtree(root, ignore_errors=True)


def s3():
    names = [n for n, *_ in CHECKS] + [n for n, *_ in SIMS] + [t + "-validate" for t in TRACES]
    for name in names:
        env_path = os.path.join(FIX, name, "envelope.json")
        for rep in range(3):
            mem = os.path.join(OUT, "raw", f"s3-{name}-{rep}.memory.json")
            os.makedirs(os.path.dirname(mem), exist_ok=True)
            outp = os.path.join(OUT, "raw", f"s3-{name}-{rep}.out.json")
            t = time.perf_counter()
            with open(outp, "wb") as f:
                rc = subprocess.call(["python3", "-I", PERF_MEM, "--output", mem, "--", SAIL, "runtime",
                                      "execute", "--file", env_path], stdout=f)
            w = (time.perf_counter() - t) * 1000
            try:
                doc = json.load(open(outp))
            except ValueError:
                doc = None
            o = result_of(doc)
            m = json.load(open(mem))
            emit("s3", {"fixture": name, "rep": rep + 1, "wall_ms": round(w, 2), "rc": rc,
                        "command_wall_s": m.get("wall_seconds"), "tree_peak_kb": m.get("tree_peak_kb"),
                        "largest_kb": m.get("largest_kb"), "execution": o[0], "outcome": o[1],
                        "completion": o[2], "distinct": o[3], "depth": o[4]})
    # 100 validation jobs from the MCLamportMutex simulation
    walls, outs = [], []
    for i in range(100):
        w, rc, doc, _ = sail_execute(os.path.join(FIX, f"bosco-sim-trace-{i:03d}", "envelope.json"))
        walls.append(w)
        outs.append(result_of(doc)[1])
    emit("s3", {"fixture": "bosco-sim-100-validate", "jobs": 100, "median_ms": round(statistics.median(walls), 2),
                "p90_ms": round(sorted(walls)[89], 2), "max_ms": round(max(walls), 2), "total_ms": round(sum(walls), 2),
                "outcomes": {o: outs.count(o) for o in set(outs)}})


def workers():
    p = subprocess.run(["pgrep", "-f", "sail-runtime-worker"], capture_output=True, text=True)
    return [int(x) for x in p.stdout.split()]


def wait_worker(t0, limit=60):
    while time.perf_counter() - t0 < limit:
        if workers():
            return (time.perf_counter() - t0) * 1000
        time.sleep(0.005)
    return None


def wait_reaped(t0, limit=60):
    while time.perf_counter() - t0 < limit:
        if not workers():
            return (time.perf_counter() - t0) * 1000
        time.sleep(0.005)
    return None


def long_envelope(deadline):
    src = os.path.join(FIX, LONG[0], "envelope.json")
    doc = json.load(open(src))
    doc["input"]["job"]["wall_time_ms"] = deadline
    path = os.path.join(FIX, LONG[0], f"envelope-{deadline}.json")
    json.dump(doc, open(path, "w"))
    return path


def s4(seed=20261118):
    rng = random.Random(seed)
    emit("s4", {"seed": seed})
    for deadline in [1000, 5000, 30000]:
        env_path = long_envelope(deadline)
        for rep in range(10):
            w, rc, doc, _ = sail_execute(env_path)
            t = time.perf_counter()
            reaped = wait_reaped(t)
            emit("s4", {"case": "deadline", "deadline_ms": deadline, "rep": rep + 1, "wall_ms": round(w, 2),
                        "after_deadline_ms": round(w - deadline, 2), "rc": rc, "execution": result_of(doc)[0],
                        "reaped_after_exit_ms": reaped})
    env_path = long_envelope(300000)
    # Ctrl-C on the embedded CLI.
    for rep in range(10):
        delay = rng.uniform(0.2, 3.0)
        p = subprocess.Popen([SAIL, "runtime", "execute", "--file", env_path], stdout=subprocess.PIPE,
                             stderr=subprocess.PIPE)
        started = wait_worker(time.perf_counter())
        time.sleep(delay)
        t = time.perf_counter()
        p.send_signal(signal.SIGINT)
        out, _ = p.communicate()
        exit_ms = (time.perf_counter() - t) * 1000
        reaped = wait_reaped(t)
        try:
            ex = result_of(json.loads(out))[0]
        except ValueError:
            ex = None
        emit("s4", {"case": "ctrl-c", "rep": rep + 1, "delay_s": round(delay, 3), "worker_start_ms": started,
                    "exit_ms": round(exit_ms, 2), "rc": p.returncode, "execution": ex, "reaped_ms": reaped})
    # Socket client disconnect.
    root, env = fresh_env()
    d = subprocess.Popen([SAIL, "serve", "--socket", env["SAIL_DAEMON_SOCKET"]], env=env,
                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    wait_socket(env["SAIL_DAEMON_SOCKET"], time.perf_counter())
    time.sleep(0.3)
    for rep in range(10):
        delay = rng.uniform(0.2, 3.0)
        p = subprocess.Popen([SAIL, "runtime", "execute", "--file", env_path, "--socket", env["SAIL_DAEMON_SOCKET"]],
                             env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        started = wait_worker(time.perf_counter())
        time.sleep(delay)
        t = time.perf_counter()
        p.kill()
        p.wait()
        reaped = wait_reaped(t)
        emit("s4", {"case": "socket-disconnect", "rep": rep + 1, "delay_s": round(delay, 3),
                    "worker_start_ms": started, "reaped_ms": reaped})
    # Capacity: two active executions, cancel one, submit another.
    for rep in range(10):
        a = subprocess.Popen([SAIL, "runtime", "execute", "--file", env_path, "--socket", env["SAIL_DAEMON_SOCKET"]],
                             env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        b = subprocess.Popen([SAIL, "runtime", "execute", "--file", env_path, "--socket", env["SAIL_DAEMON_SOCKET"]],
                             env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        t0 = time.perf_counter()
        while len(workers()) < 2 and time.perf_counter() - t0 < 60:
            time.sleep(0.005)
        time.sleep(0.5)
        before = set(workers())
        t = time.perf_counter()
        a.kill()
        a.wait()
        c = subprocess.Popen([SAIL, "runtime", "execute", "--file", long_envelope(5000), "--socket",
                              env["SAIL_DAEMON_SOCKET"]], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        admitted = None
        while time.perf_counter() - t < 60:
            now = set(workers())
            if now - before:
                admitted = (time.perf_counter() - t) * 1000
                break
            if c.poll() is not None:
                break
            time.sleep(0.005)
        out, _ = c.communicate()
        try:
            ex = result_of(json.loads(out))[0]
        except ValueError:
            ex = None
        b.kill()
        b.wait()
        wait_reaped(time.perf_counter())
        emit("s4", {"case": "capacity", "rep": rep + 1, "admitted_ms": admitted, "third_execution": ex,
                    "third_rc": c.returncode})
    d.terminate()
    d.wait()
    stop_daemons()
    shutil.rmtree(root, ignore_errors=True)
    # MCP cancellation.
    for rep in range(10):
        delay = rng.uniform(0.2, 3.0)
        root, env = fresh_env()
        env["PATH"] = os.path.dirname(SAIL) + ":" + env["PATH"]
        proj = os.path.join(root, "proj")
        os.makedirs(proj)
        m = Mcp(env, proj)
        m.n += 1
        rid = m.n
        m.send({"jsonrpc": "2.0", "id": rid, "method": "tools/call",
                "params": {"name": "runtime_execute", "arguments": {"request": json.load(open(env_path))}}})
        started = wait_worker(time.perf_counter())
        time.sleep(delay)
        t = time.perf_counter()
        m.send({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": rid}})
        reaped = wait_reaped(t)
        m.close()
        stop_daemons()
        shutil.rmtree(root, ignore_errors=True)
        emit("s4", {"case": "mcp-cancel", "rep": rep + 1, "delay_s": round(delay, 3),
                    "worker_start_ms": started, "reaped_ms": reaped})


def s5():
    open(os.path.join(OUT, "s5.jsonl"), "w").close()
    for fixture in ["MCLamportMutex", "CoffeeCan1000Beans"]:
        env_path = os.path.join(FIX, fixture, "envelope.json")
        for n in [2, 3, 4]:
            root, env = fresh_env()
            script = os.path.join(root, "clients.sh")
            sock = env["SAIL_DAEMON_SOCKET"]
            # The daemon runs inside the measured tree so its workers count.
            lines = ["#!/bin/bash", "set -u", f"{SAIL} serve --socket {sock} >/dev/null 2>&1 & D=$!",
                     f"while [ ! -S {sock} ]; do sleep 0.01; done", "sleep 0.3", "P="]
            for i in range(n):
                if i == 2:
                    lines.append("sleep 1")  # third and fourth arrive while both run
                lines.append(f"( s=$(date +%s.%N); {SAIL} runtime execute --file {env_path} --socket {sock} "
                             f"> {root}/out{i}.json 2>{root}/err{i}.txt; echo $s $(date +%s.%N) $? > {root}/t{i}.txt ) & P=\"$P $!\"")
            lines += ["wait $P", "kill $D", "wait $D"]
            open(script, "w").write("\n".join(lines) + "\n")
            mem = os.path.join(OUT, "raw", f"s5-{fixture}-{n}.memory.json")
            state = os.path.join(root, "daemons.json")
            subprocess.call(["python3", "-I", PERF_MEM, "--output", mem, "--state", state, "--", "bash", script], env=env)
            subprocess.call(["python3", "-I", PERF_MEM, "--snapshot", "--output", mem, "--state", state], env=env)
            jobs = []
            for i in range(n):
                s, e, rc = open(f"{root}/t{i}.txt").read().split()
                try:
                    o = result_of(json.load(open(f"{root}/out{i}.json")))
                except ValueError:
                    o = (None,) * 6
                jobs.append({"i": i + 1, "wall_ms": round((float(e) - float(s)) * 1000, 2), "rc": int(rc),
                             "execution": o[0], "outcome": o[1], "completion": o[2], "distinct": o[3],
                             "err": open(f"{root}/err{i}.txt").read()[-300:] or None})
            m = json.load(open(mem))
            emit("s5", {"fixture": fixture, "clients": n, "tree_peak_kb": m.get("tree_peak_kb"),
                        "daemon_peak_kb": m.get("daemon_peak_kb"), "daemon_hwm_kb": m.get("daemon_hwm_kb"),
                        "largest_kb": m.get("largest_kb"), "daemon_in_tree": True, "jobs": jobs})
            stop_daemons()
            shutil.rmtree(root, ignore_errors=True)


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    {"fixtures": fixtures, "s1": s1, "s2": s2, "s3": s3, "s4": s4, "s5": s5}[sys.argv[1]]()
