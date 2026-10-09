# Benchmark runner smoke, 2026-10-09

This verifies the runner and its result checks; it is **not a performance
baseline**. The shared Apple M3 machine had load averages 64/77/62 on eight
CPUs. Wall times varied widely, so the ratios below support no speed claim.
All nine runs reproduced the pinned successful verdict and counts in both
dive profiles and TLC. The dive binary was built from PR #60; its SHA-256
and all pins are recorded in the raw JSON lines.

Reproduce with a release build of `dive-cli` and `dive-bench`:

```sh
target/release/dive-bench --filter SimpleRegular --profile both \
  --workers 1 --reps 3 --tlc --runs conformance/reports/benchmark-runs.jsonl
```

Raw measurements: `conformance/reports/benchmarks/20261009-simple-regular-smoke.jsonl`.
Runner source was dirty during this smoke, explicitly recorded in every line.
A quiet-machine campaign, worker scaling and the performance targets remain
outstanding; rerun them before making a throughput claim.

Captured console output follows (local binary path omitted).

```text
machine: Apple M3, 8 CPUs, Darwin arm64
load average at start: { 64.12 77.13 62.13 }

provenance: {"binary_sha256":"46d0a398f5bd24be35a6486e10d2d6d8ad3d8cc37a6544a909ee169052f7b58a","binary_version":"dive 0.0.0","corpus_commit":"6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f","cpu":"Apple M3","cpus":8,"fp":0,"load_at_start":"{ 64.12 77.13 62.13 }","oracle":{"id":"tlc-2026.10.06.014338","sha256":"7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d"},"os":"Darwin arm64","runner_dirty":true,"runner_revision":"b167126d4fea558dd72a445e56f2b1d1cb26ad9e","seed":0}
[rep 1] SimpleRegular        dive-tlc     cpu   14.91s wall    27.28s rss  216MB ok 
[rep 1] SimpleRegular        dive-default cpu   13.77s wall    38.05s rss  214MB ok 
[rep 1] SimpleRegular        tlc          cpu    7.44s wall     7.72s rss 1066MB ok 
[rep 2] SimpleRegular        dive-tlc     cpu   14.41s wall    25.10s rss  218MB ok 
[rep 2] SimpleRegular        dive-default cpu   15.00s wall    47.01s rss  221MB ok 
[rep 2] SimpleRegular        tlc          cpu    7.74s wall    12.03s rss 1043MB ok 
[rep 3] SimpleRegular        dive-tlc     cpu   14.55s wall    59.16s rss  217MB ok 
[rep 3] SimpleRegular        dive-default cpu   14.96s wall   131.16s rss  214MB ok 
[rep 3] SimpleRegular        tlc          cpu    8.61s wall    54.20s rss  928MB ok 
## Per run medians (CPU = user + sys)

| Benchmark | Implementation | CPU s (min–max) | wall s | distinct/CPU s | peak RSS MB | correct |
|---|---|---|---|---|---|---|
| SimpleRegular | dive-tlc | 14.55 (14.41–14.91) | 27.28 | 19088 | 218 | 3/3 |
| SimpleRegular | dive-default | 14.96 (13.77–15.00) | 47.01 | 18565 | 215 | 3/3 |
| SimpleRegular | tlc | 7.74 (7.44–8.61) | 12.03 | 35882 | 1044 | 3/3 |

## Against TLC and tla-mc at one worker

TLC here: this machine, `-workers 1`, wall (CPU). Golden and survey times are historical context only. Speed-up is the median paired TLC/dive wall-time ratio from successful runs of the same model configuration and repetition on this machine. Without --tlc, no speed-up is claimed.

| Benchmark | distinct | dive tlc CPU s | dive default CPU s | TLC here s | TLC golden s | TLC survey s | tla-mc survey s | speed-up (tlc profile) |
|---|---|---|---|---|---|---|---|---|
| SimpleRegular | 277726 | 14.55 | 14.96 | 12.03 (7.74) | 14.7 | 3.52 | 7.57 | 0.48× |
```
