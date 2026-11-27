#!/usr/bin/env python3
"""S7: Sail+dive against the standard TLC workflow (dive
docs/developer/release-performance-plan.md#s7-saildive-against-standard-tlc).

  s7.py fixtures              build Sail jobs (sail_temporal.py fixtures)
  s7.py latency  <label>      (a) per-check latency, every path, 5 alternating reps
  s7.py loop     <label>      (b) N small jobs in one session, every path, 5 alternating reps
  s7.py through  <label>      (b) throughput at the admission limit
  s7.py only     <label>      (d) Sail-only capabilities
  s7.py ratio    <label>      (c) S1's worker-body / dive CLI ratio on the large models Sail admits

<label> names the Sail build in ~/inst-<label>. Rows go to $OUT/s7-<part>-<label>.jsonl.

Sail 0.3.1 replays a completed check from its receipt store when the same job
(request bytes and wall_time_ms) has already completed in the same cache. With
S7_FRESH=1 every Sail check job gets its own wall_time_ms (300,000 ms minus a
counter), so no timed job can be a replay and every path does the check; the
rows record cache_hit either way.
"""
import itertools
import json
import os
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ThreadPoolExecutor

H = os.path.expanduser("~")
LABEL = sys.argv[2] if len(sys.argv) > 2 else "before"
os.environ.setdefault("SAIL_BIN", f"{H}/inst-{LABEL}/bin/sail")
os.environ.setdefault("WORKER", f"{H}/inst-{LABEL}/bin/sail-runtime-worker")
os.environ.setdefault("BENCH", f"{H}/inst-{LABEL}/temporal_bench")
os.environ.setdefault("PERF_MEM", f"{H}/src-{LABEL}/scripts/perf-mem.py")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import sail_temporal as st  # noqa: E402

JAVA = os.environ.get("JAVA", "")
JAR = os.environ.get("TLA2TOOLS", "")
OUT = st.OUT
FIX = st.FIX
REPS = 5
LAT = ["DieHard", "EWD840", "MCRealTimeHourClock", "MCLiveInternalMemory", "MCDisruptor_MPMC_liveliness",
       "MCLamportMutex"]
LOOP = ["DieHard", "EWD840", "MCRealTimeHourClock", "MCLiveInternalMemory"]
N = int(os.environ.get("S7_N", "100"))
FRESH = os.environ.get("S7_FRESH") == "1"
_SEQ = itertools.count(1)  # next() is atomic under the GIL (the throughput part uses threads)
_TMP = tempfile.mkdtemp(prefix="s7-env-")


def envelope(name):
    """The fixture's envelope, or under S7_FRESH a copy with a unique wall_time_ms."""
    path = os.path.join(FIX, name, "envelope.json")
    if not FRESH:
        return path
    doc = json.load(open(path))
    job = doc["input"]["job"]
    if job["request"]["operation"].get("kind") != "check":
        return path
    k = next(_SEQ)
    job["wall_time_ms"] = st.WALL - k
    out = os.path.join(_TMP, f"{k}.json")
    json.dump(doc, open(out, "w"))
    return out


def cache_hit(doc):
    """The temporal result's cache_hit flag (None when absent)."""
    if isinstance(doc, dict):
        if "request_sha256" in doc and "worker_sha256" in doc:
            return doc.get("cache_hit")
        for v in doc.values():
            r = cache_hit(v)
            if r is not None:
                return r
    elif isinstance(doc, list):
        for v in doc:
            r = cache_hit(v)
            if r is not None:
                return r
    return None


def fx(name):
    return json.load(open(os.path.join(FIX, name, "fixture.json")))


def emit(part, row):
    with open(os.path.join(OUT, f"s7-{part}-{LABEL}.jsonl"), "a") as f:
        f.write(json.dumps(row) + "\n")


def timed(argv, cwd=None, env=None, rss=True, timeout=900):
    """Wall ms, rc, stdout, max RSS KB (GNU time, the largest process of the tree)."""
    tf = None
    if rss:
        tf = tempfile.NamedTemporaryFile(delete=False, suffix=".time")
        tf.close()
        argv = ["/usr/bin/time", "-f", "%M", "-o", tf.name, *argv]
    t = time.perf_counter()
    p = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, timeout=timeout)
    wall = (time.perf_counter() - t) * 1000
    kb = None
    if tf:
        try:
            kb = int(open(tf.name).read().split()[-1])
        except (ValueError, IndexError):
            pass
        os.unlink(tf.name)
    return wall, p.returncode, p.stdout, kb


# ---------------------------------------------------------------- TLC (standard)

def tlc_parse(text):
    distinct = depth = None
    verdict = "unknown"
    for line in text.splitlines():
        if "distinct states found" in line:
            # "N states generated, M distinct states found, K states left on queue."
            try:
                distinct = int(line.split(" distinct states found")[0].split(",")[-1].strip().replace(",", ""))
            except ValueError:
                pass
        if "The depth of the complete state graph search is" in line:
            depth = int(line.rsplit(" ", 1)[-1].rstrip("."))
        if "Model checking completed. No error has been found." in line:
            verdict = "holds"
        if line.startswith("Error: Invariant") and "is violated" in line:
            verdict = "invariant-violation"
        if line.startswith("Error: Temporal properties were violated"):
            verdict = "liveness-violation"
        if line.startswith("Error: Deadlock reached"):
            verdict = "deadlock"
    return verdict, distinct, depth


def tlc(name):
    """TLC's normal invocation: a fresh JVM per check, the standard
    -XX:+UseParallelGC, one worker, the model's own config, in a scratch copy."""
    f = fx(name)
    work = tempfile.mkdtemp(prefix="s7-tlc-")
    shutil.copytree(os.path.join(FIX, name, "model"), work, dirs_exist_ok=True)
    root, cfg = os.path.basename(f["module"]), f["config"]
    argv = [JAVA, "-XX:+UseParallelGC", "-jar", JAR, "-workers", "1", "-config", cfg, root]
    wall, rc, out, kb = timed(argv, cwd=work)
    shutil.rmtree(work, ignore_errors=True)
    v, d, depth = tlc_parse(out.decode(errors="replace"))
    return {"wall_ms": round(wall, 2), "rc": rc, "verdict": v, "distinct": d, "depth": depth, "rss_kb": kb}


def agrees(name, holds, distinct):
    """Same holds/refuted class and distinct-state count as the TLC golden.
    (Each tool's own counterexample is not compared; Sail's is replayed by
    D.I.V.E.'s independent validator inside every job.)"""
    g = fx(name)["golden"]
    return holds is not None and holds == (g["verdict"] == "success") and distinct == g["distinct"]


# ---------------------------------------------------------------- dive and Sail paths

def dive_cli(name):
    f = fx(name)
    d = os.path.join(FIX, name)
    op = f["operation"]
    argv = [st.DIVE, "check", os.path.basename(f["module"]), "--config", f["config"], "--store", "exact",
            "--workers", "1", "--seed", "0", "--fp", "0", "--max-states", str(op["max_states"]),
            "--max-depth", str(op["max_depth"]), "--output", "json"]
    wall, rc, out, kb = timed(argv, cwd=os.path.join(d, "model"))
    try:
        rep = json.loads(out)
    except ValueError:
        rep = {}
    s = rep.get("statistics") or {}
    return {"wall_ms": round(wall, 2), "rc": rc, "verdict": rep.get("verdict"), "distinct": s.get("distinct"),
            "depth": s.get("depth"), "rss_kb": kb}


def sail_row(wall, rc, doc, kb=None):
    o = st.result_of(doc)
    return {"wall_ms": round(wall, 2), "rc": rc, "execution": o[0], "outcome": o[1], "completion": o[2],
            "distinct": o[3], "depth": o[4], "rss_kb": kb, "cache_hit": cache_hit(doc)}


def sail_execute(name, env, socket=None):
    argv = [st.SAIL, "runtime", "execute", "--file", envelope(name)]
    if socket:
        argv += ["--socket", socket]
    wall, rc, out, kb = timed(argv, env=env, rss=socket is None)
    try:
        doc = json.loads(out)
    except ValueError:
        doc = None
    return sail_row(wall, rc, doc, kb)


def embedded(name):
    p = st.bench(os.path.join(FIX, name))
    return {"wall_ms": p.get("embedded_ms"), "outcome": p.get("outcome"), "completion": p.get("completion"),
            "distinct": p.get("distinct_states"), "verify_copy_ms_old_procedure": p.get("verify_copy_ms"),
            "worker_body_ms": p.get("worker_body_ms"), "dive_call_ms": p.get("dive_call_ms")}


class Daemon:
    def __init__(self):
        self.root, self.env = st.fresh_env()
        self.sock = self.env["SAIL_DAEMON_SOCKET"]
        t0 = time.perf_counter()
        self.p = subprocess.Popen([st.SAIL, "serve", "--socket", self.sock], env=self.env,
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        self.start_ms = st.wait_socket(self.sock, t0)
        time.sleep(0.3)

    def hwm_kb(self):
        try:
            for line in open(f"/proc/{self.p.pid}/status"):
                if line.startswith("VmHWM:"):
                    return int(line.split()[1])
        except OSError:
            return None

    def close(self):
        self.p.terminate()
        self.p.wait()
        st.stop_daemons()
        shutil.rmtree(self.root, ignore_errors=True)


class McpSession:
    def __init__(self):
        self.root, self.env = st.fresh_env()
        self.env["PATH"] = os.path.dirname(st.SAIL) + ":" + self.env["PATH"]
        proj = os.path.join(self.root, "proj")
        os.makedirs(proj)
        t0 = time.perf_counter()
        self.m = st.Mcp(self.env, proj)
        self.init_ms = (time.perf_counter() - t0) * 1000

    def call(self, name):
        env = envelope(name)
        t = time.perf_counter()
        msg = self.m.execute(env)
        wall = (time.perf_counter() - t) * 1000
        o, doc = (None,) * 6, None
        try:
            doc = json.loads(msg["result"]["content"][0]["text"])
        except (KeyError, TypeError, ValueError, IndexError):
            try:
                doc = msg["result"]["structuredContent"]
            except (KeyError, TypeError):
                pass
        if doc is not None:
            o = st.result_of(doc)
        return {"wall_ms": round(wall, 2), "execution": o[0], "outcome": o[1], "completion": o[2],
                "distinct": o[3], "depth": o[4], "cache_hit": cache_hit(doc)}

    def close(self):
        self.m.close()
        st.stop_daemons()
        shutil.rmtree(self.root, ignore_errors=True)


def tlc_holds(r):
    """TLC's exit status: 0 no error; 11 deadlock, 12 safety, 13 liveness violation."""
    if r["rc"] == 0 and r["verdict"] == "holds":
        return True
    return False if r["rc"] in (11, 12, 13) else None


def dive_holds(r):
    if r["rc"] == 0 and r["verdict"] == "success":
        return True
    return False if r["rc"] in (12, 13) and r["verdict"] else None


def sail_holds(row):
    o = row.get("outcome")
    return None if o is None else o == "holds"


def sail_ok(name, row):
    return row.get("execution") == "completed" and agrees(name, sail_holds(row), row.get("distinct"))


def order(paths, rep):
    k = rep % len(paths)
    o = paths[k:] + paths[:k]
    return o[::-1] if rep % 2 else o


# ---------------------------------------------------------------- (a) latency

def latency():
    paths = ["tlc", "dive", "embedded", "execute", "serve", "mcp"]
    for name in LAT:
        for rep in range(REPS):
            row = {"fixture": name, "rep": rep + 1, "order": order(paths, rep)}
            for path in row["order"]:
                if path == "tlc":
                    r = tlc(name)
                    r["agrees"] = agrees(name, tlc_holds(r), r["distinct"])
                elif path == "dive":
                    r = dive_cli(name)
                    r["agrees"] = agrees(name, dive_holds(r), r["distinct"])
                elif path == "embedded":
                    r = embedded(name)
                    r["agrees"] = r["distinct"] == fx(name)["golden"]["distinct"]
                elif path == "execute":
                    root, env = st.fresh_env()
                    r = sail_execute(name, env)
                    shutil.rmtree(root, ignore_errors=True)
                    r["agrees"] = sail_ok(name, r)
                elif path == "serve":
                    d = Daemon()
                    sail_execute(name, d.env, d.sock)  # warm: one job already run
                    r = sail_execute(name, d.env, d.sock)
                    r["daemon_hwm_kb"] = d.hwm_kb()
                    d.close()
                    r["agrees"] = sail_ok(name, r)
                else:
                    m = McpSession()
                    m.call(name)  # warm: daemon autostarted and one job already run
                    r = m.call(name)
                    r["mcp_init_ms"] = round(m.init_ms, 2)
                    m.close()
                    r["agrees"] = sail_ok(name, r)
                row[path] = r
            emit("latency", row)


def first_job():
    """The first job after installation: no reusable worker copy yet."""
    for rep in range(REPS):
        subprocess.run(["bash", "-c", "rm -rf ${TMPDIR:-/tmp}/sail-runtime-images-$(id -u)"])
        root, env = st.fresh_env()
        r = sail_execute("DieHard", env)
        r2 = sail_execute("DieHard", env)
        shutil.rmtree(root, ignore_errors=True)
        emit("first", {"fixture": "DieHard", "rep": rep + 1, "first": r, "second": r2})


# ---------------------------------------------------------------- (b) agent loop

def jobs():
    return [LOOP[i % len(LOOP)] for i in range(N)]


def loop_path(path):
    names = jobs()
    walls, bad, hits = [], 0, 0
    extra = {}
    t0 = time.perf_counter()
    if path == "tlc":
        for n in names:
            r = tlc(n)
            walls.append(r["wall_ms"])
            bad += not agrees(n, tlc_holds(r), r["distinct"])
    elif path == "dive":
        for n in names:
            r = dive_cli(n)
            walls.append(r["wall_ms"])
            bad += not agrees(n, dive_holds(r), r["distinct"])
    elif path == "execute":
        root, env = st.fresh_env()
        for n in names:
            r = sail_execute(n, env)
            walls.append(r["wall_ms"])
            bad += not sail_ok(n, r)
            hits += bool(r.get("cache_hit"))
        shutil.rmtree(root, ignore_errors=True)
    elif path == "serve":
        d = Daemon()
        extra["daemon_start_ms"] = d.start_ms
        for n in names:
            r = sail_execute(n, d.env, d.sock)
            walls.append(r["wall_ms"])
            bad += not sail_ok(n, r)
            hits += bool(r.get("cache_hit"))
        extra["daemon_hwm_kb"] = d.hwm_kb()
        d.close()
    else:
        m = McpSession()
        extra["mcp_init_ms"] = round(m.init_ms, 2)
        for n in names:
            r = m.call(n)
            walls.append(r["wall_ms"])
            bad += not sail_ok(n, r)
            hits += bool(r.get("cache_hit"))
        m.close()
    total = (time.perf_counter() - t0) * 1000
    return {"jobs": len(names), "total_ms": round(total, 1), "median_ms": round(statistics.median(walls), 2),
            "p90_ms": round(sorted(walls)[int(0.9 * len(walls)) - 1], 2), "max_ms": round(max(walls), 2),
            "first_ms": round(walls[0], 2), "disagreements": bad, "cache_hits": hits, **extra}


def loop():
    paths = ["tlc", "dive", "execute", "serve", "mcp"]
    for rep in range(REPS):
        row = {"rep": rep + 1, "n": N, "mix": LOOP, "order": order(paths, rep)}
        for path in row["order"]:
            row[path] = loop_path(path)
        emit("loop", row)


def through():
    """N jobs with C concurrent clients: TLC with C concurrent JVMs, Sail
    through one warm daemon (two active executions; more clients queue)."""
    names = jobs()
    for rep in range(REPS):
        cases = [("tlc", 2), ("serve", 2), ("serve", 8)]
        if rep % 2:
            cases.reverse()
        row = {"rep": rep + 1, "n": N}
        for path, c in cases:
            if path == "tlc":
                t0 = time.perf_counter()
                with ThreadPoolExecutor(c) as ex:
                    rs = list(ex.map(tlc, names))
                bad = sum(not agrees(n, tlc_holds(r), r["distinct"]) for n, r in zip(names, rs))
                extra = {}
            else:
                d = Daemon()
                sail_execute(names[0], d.env, d.sock)
                t0 = time.perf_counter()
                with ThreadPoolExecutor(c) as ex:
                    rs = list(ex.map(lambda n: sail_execute(n, d.env, d.sock), names))
                bad = sum(not sail_ok(n, r) for n, r in zip(names, rs))
                extra = {"daemon_hwm_kb": d.hwm_kb(), "cache_hits": sum(bool(r.get("cache_hit")) for r in rs),
                         "no_result": sum(r.get("rc") not in (0, None) and r.get("execution") is None for r in rs)}
            total = (time.perf_counter() - t0) * 1000
            if path != "tlc":
                d.close()
            row[f"{path}-c{c}"] = {"total_ms": round(total, 1), "jobs_per_s": round(len(names) / total * 1000, 2),
                                    "disagreements": bad, **extra}
        emit("through", row)


# ---------------------------------------------------------------- (d) Sail-only

def only():
    """Structured stop at a state bound with the exact store, and
    independent replay of a returned counterexample (validate jobs)."""
    name = "ElevatorSafetyLarge-bounded"
    d = os.path.join(FIX, name)
    if not os.path.isdir(d):
        src = os.path.join(FIX, "ElevatorSafetyLarge")
        shutil.copytree(src, d)
        env = json.load(open(os.path.join(d, "envelope.json")))
        env["input"]["job"]["request"]["operation"]["max_states"] = 1_000_000
        json.dump(env, open(os.path.join(d, "envelope.json"), "w"))
    for rep in range(REPS):
        root, e = st.fresh_env()
        r = sail_execute(name, e)
        doc_bound = r
        rows = {"bounded": doc_bound}
        for v in ["DieHard-validate", "SlidingPuzzles-validate", "MCRealTimeHourClock-validate"]:
            rows[v] = sail_execute(v, e)
        shutil.rmtree(root, ignore_errors=True)
        emit("only", {"rep": rep + 1, **rows})


# ---------------------------------------------------------------- (c) composition input

LARGE = ["SlushSmall", "MCDisruptor_MPMC", "CoffeeCan1000Beans", "ElevatorLivenessMedium", "MCLamportMutex"]


def ratio():
    """S1's in-worker difference on the large models: temporal_bench's worker
    body against the dive CLI with Sail's settings, 5 alternating repetitions."""
    for name in LARGE:
        for rep in range(REPS):
            row = {"fixture": name, "rep": rep + 1, "order": ["embedded", "dive"] if rep % 2 == 0 else
                   ["dive", "embedded"]}
            for path in row["order"]:
                if path == "embedded":
                    r = embedded(name)
                    r["agrees"] = r["distinct"] == fx(name)["golden"]["distinct"]
                else:
                    r = dive_cli(name)
                    r["agrees"] = agrees(name, dive_holds(r), r["distinct"])
                row[path] = r
            emit("ratio", row)


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    {"fixtures": st.fixtures, "latency": latency, "first": first_job, "loop": loop, "through": through,
     "only": only, "ratio": ratio}[sys.argv[1]]()
