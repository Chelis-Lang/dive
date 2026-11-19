#!/usr/bin/env python3
"""D7 Mac spot-check record from a local dive-bench runs.jsonl and startup.json.

d7record.py <runs.jsonl> <startup.json> <sha.txt> <raw-sha256> <out.json>
"""
import json
import statistics
import sys

runs_p, start_p, sha_p, raw_sha, out = sys.argv[1:6]
rows = [json.loads(l) for l in open(runs_p)]
shas = dict(reversed(l.split()) for l in open(sha_p))
g = {}
for r in rows:
    g.setdefault((r["bench"], r["impl"]), []).append(r)
loads = [r["run"]["load_before"] for r in rows]
models = {}
for (b, impl), rs in sorted(g.items()):
    w = [x["wall_s"] for x in rs]
    rss = [x["max_rss_bytes"] for x in rs]
    models.setdefault(b, {"model": rs[0]["run"]["inputs"]["model"]})[impl] = {
        "reps": len(rs), "wall_ms_median": round(statistics.median(w) * 1000, 1),
        "wall_ms_range": [round(min(w) * 1000, 1), round(max(w) * 1000, 1)],
        "peak_rss_median": statistics.median(rss), "all_correct": all(x["correct"] for x in rs),
        "workers_used": sorted({x["workers_used"] for x in rs if x.get("workers_used") is not None})}
ratios = {}
for b, m in models.items():
    for w in (1, 4):
        d, t = m.get(f"dive-default-w{w}"), m.get(f"tlc-w{w}")
        if d and t:
            m[f"dive_over_tlc_w{w}"] = round(d["wall_ms_median"] / t["wall_ms_median"], 3)
            ratios.setdefault(w, []).append(m[f"dive_over_tlc_w{w}"])
geo = lambda xs: round(statistics.geometric_mean(xs), 3)
p0 = rows[0]["provenance"]
rec = {
    "measurement": "D7", "informational": True,
    "plan": "docs/developer/release-performance-plan.md#d7-macos-arm64-spot-check",
    "date": "2026-11-18",
    "source": "08a14fbac83ff71a3247623fb10bf0584bc83be5",
    "dive_binary_sha256": shas["target/release/dive"],
    "dive_bench_sha256": shas["target/release/dive-bench"],
    "oracle": p0["oracle"]["id"], "corpus": "examples@" + p0["corpus_commit"],
    "host": {"machine": "MacBook Air (Mac15,13), owner's shared laptop", "chip": "Apple M3, 8 cores (4 performance, 4 efficiency)",
             "memory": "24 GB", "os": "macOS 26.5 (25F71), arm64", "java": p0["oracle_runtime"]["java"]["version_line"],
             "quiet": False, "binding": "none (macOS has no NUMA binding); one measured process at a time",
             "load_average_1m_range": [min(float(l.strip('{} ').split()[0]) for l in loads),
                                       max(float(l.strip('{} ').split()[0]) for l in loads)]},
    "command": {
        "benchmark_set": "dive-bench --dive-bin target/release/dive --dive-revision 08a14fbac83ff71a3247623fb10bf0584bc83be5 "
                         "--profile default --workers 1,4 --reps 3 --tlc --xmx 3g --timeout 300 --filter MCDisruptor_MPMC "
                         "--filter SimpleRegular --filter MCbtree --filter MCBakery --filter MCLamportMutex "
                         "--filter CoffeeCan1000BeansSafety (crates/dive-bench/benchmarks.toml), under env -i",
        "startup": "50 fresh runs each of dive --version, dive parse docs/user/examples/Counter.tla and dive check "
                   "docs/user/examples/Counter.tla --config docs/user/examples/Counter.cfg --workers 1 --seed 0 --fp 0 "
                   "--output json, timed with perf_counter around subprocess.run",
        "build": "cargo build --release --locked -p dive-cli --bins and -p dive-bench, CARGO_BUILD_JOBS=4, no RUSTFLAGS"},
    "benchmark_set_sha256": p0["benchmark_set_sha256"],
    "raw": {"location": "dive evidence archive bucket (us-east-1), prefix release-perf/2026-11-18/mac-local-raw.tar.gz",
            "sha256": raw_sha},
    "deviations": [
        "AWS mac capacity was unavailable, so D7 ran on the owner's laptop instead of the Sail mac-m4.metal host; the laptop "
        "was in use by other work (see load averages), so timings are indicative only.",
        "A reduced selection by owner direction: six of the ten benchmark-set models at 1 and 4 workers (the laptop has 4 "
        "performance cores), default profile only, three repetitions.",
        "Peak RSS from /usr/bin/time -l (macOS)."],
    "summary": {"models": len(models), "all_correct": all(v["all_correct"] for m in models.values() for k, v in m.items() if isinstance(v, dict)),
                "geomean_dive_over_tlc_w1": geo(ratios[1]), "geomean_dive_over_tlc_w4": geo(ratios[4]),
                "max_dive_over_tlc_w1": max(ratios[1]), "max_dive_over_tlc_w4": max(ratios[4])},
    "rows": {"benchmark_set": models, "startup": json.load(open(start_p))},
}
json.dump(rec, open(out, "w"), indent=1)
open(out, "a").write("\n")
print(json.dumps(rec["summary"]))
