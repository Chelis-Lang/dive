# dive-bench

Benchmarks of `dive check` against the pinned TLC. They run by hand, never in
the per-change gate (AGENTS.md, "Testing tiers"). The runner records CPU and
wall time, peak RSS, verdict and state counts for each repetition and worker
count. Failed or unverified runs fail the command and do not contribute to
timing summaries. It checks corpus integrity and typed golden provenance.

```sh
cargo build --release --locked -p dive-cli -p dive-bench
cargo run --release --locked -p dive-bench -- --list
cargo run --release --locked -p dive-bench -- \
  --filter SimpleRegular --profile both --workers 1,4,8 --reps 3 --tlc \
  --runs conformance/reports/benchmark-runs.jsonl
```

Use a quiet machine and the pinned cache fetched by the conformance harness.
Every JSON line includes the machine, runner revision and dirty status,
binary SHA-256, corpus commit, oracle pin, seed and fingerprint polynomial.
The binary hash identifies the actual executable, which may differ from the
runner's checkout. Keep the console report alongside the raw JSON lines.

Speed-up means the median of **paired wall-time ratios** from successful
TLC and dive runs of the same configuration and repetition on this machine.
Historical survey and golden timings are displayed as context; they never
serve as a denominator for current performance claims. CPU rates and RSS
are reported separately. Without `--tlc`, no TLC speed-up is claimed.

The CoffeeCan benchmark removes temporal properties explicitly and applies
the identical safety configuration to both tools. Its timings cannot be
compared with the historical full-liveness survey. The runner currently
requires successful goldens with generated and distinct counts; violation
and error-path performance measurements require a separate benchmark set.

## Parallel scaling (WP-D3, 2026-10-09, indicative)

Results and caveats are in ADR 0015, "Implementation and measurements (WP-D3)".
In short:
- the machine was heavily shared (load average 20–44 on 8 cores);
- dive's results are byte-identical at every worker count;
- wall-clock gains from 1 to 8 workers were 1.9–2.4× (harness runs: 3.3–4.6×);
- CPU time grows about 2.8× from 1 to 8 workers, which is the main target for
  WP-G2.

### Method

For each model, one repetition, in this order:

1. `dive check <Module>.tla --config <cfg> --compat=tlc --seed 0 --fp 0
   --workers W --output json`, for W in 1, 2, 4, 8, with the harness's module
   path (the CommunityModules jar's modules, the Apalache and TLAPS module
   libraries).
2. The pinned TLC (`tlc-2026.10.06.014338`, Temurin 17.0.20.1+1):
   `java -Xss8m -XX:+UseParallelGC -cp tla2tools.jar:CommunityModules*.jar:<libraries>
   tlc2.TLC -workers W -fp 0 -seed 0 -metadir <tmp> -config <cfg> <Module>.tla`,
   for W in 1 and 8.

Wall time is the whole process: start-up, parsing and checking, and the JVM
for TLC. Every run's generated and distinct counts are checked against the
others. CPU time comes from `/usr/bin/time -p`, with the load average recorded
alongside each run.

Models: `TeachingConcurrency/SimpleRegular`, `lamport_mutex/MCLamportMutex`
and `btree/MCbtree` from the survey's benchmark set
(`docs/survey/competitive.md`), plus `Disruptor/MCDisruptor_MPMC` for a deep
state graph (Examples `6a7b26a`). The survey's other models are left out:
- `CoffeeCan1000Beans` checks liveness, which dive does not do yet;
- `MCBakery` and `GameOfLife` have depth 1: every state is an initial state,
  so the search is the inline initial-state phase and does not exercise the
  parallel engine.

### To repeat

- Use a quiet machine.
- Interleave the tools and repeat at least three times, as the survey's
  method does.
- Record the machine, the load and the commit.
