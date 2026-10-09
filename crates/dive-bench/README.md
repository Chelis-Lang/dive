# dive-bench

Benchmarks of `dive check` against the pinned TLC. They run by hand, never in
the per-change gate (AGENTS.md, "Testing tiers"). The crate itself is still a
placeholder (WP-G1); this file records the measurements taken so far and how
to repeat them.

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
