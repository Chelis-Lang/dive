# Performance measurement

Use the pinned inputs and runner described in
[`crates/dive-bench/README.md`](../../crates/dive-bench/README.md).
[ADR 0104](../decisions/0104-evaluator-performance-plan.md) orders the remaining
evaluator work. Instruction counts assess individual optimizations; wall time
against the pinned TLC assesses the release goal. Neither substitutes for the
other, and incomplete or refused runs earn no credit.

## AWS baseline, 2026-10-20

The recovered campaign at clean source `73259029b7689a55ddfe77ed648cb5dd14519274`
completed all **300 planned runs**: ten models, five repetitions, TLC and both
dive profiles, at requested worker counts 1 and 8. TLC's observations are shared
by the two comparisons. All runs passed the runner's pinned-golden checks;
there were no censored, refused or failed runs. Single-worker counts match,
and counterexample validation is unnecessary because these are success models.

The descriptive equal-model geometric means of paired TLC/dive wall time are
**0.329× / 0.149×** in the TLC profile and **0.335× / 0.213×** in the default
profile, at 1 / requested 8 workers respectively. A value below 1 means dive
is slower. Every individual model remains below 1× TLC in this campaign.

These are **provisional measurements with no release-target acceptance credit**.
The bare-metal host was dedicated, but the campaign did not independently
certify its quiet window or thermal stability (`quiet_window_certified=false`).
Turbo remained enabled. Recorded before/after load samples are context rather
than proof of a quiet run. Repeat a certified campaign at the performance
milestones, preserving the same population and all failed observations.

| Model | TLC profile, 1 | TLC profile, requested 8 | Default, 1 | Default, 8 |
|-------|---------------:|------------------------:|-----------:|-----------:|
| GameOfLife | 0.237 | 0.091 | 0.227 | 0.119 |
| TLCSailfish1 | 0.362 | 0.060 | 0.360 | 0.238 |
| EWD998PCal | 0.244 | 0.171 | 0.249 | 0.182 |
| MCBakery | 0.351 | 0.355 | 0.376 | 0.372 |
| MCLamportMutex | 0.326 | 0.113 | 0.300 | 0.204 |
| SimpleRegular | 0.299 | 0.417 | 0.309 | 0.421 |
| MCbtree | 0.440 | 0.092 | 0.614 | 0.536 |
| CoffeeCan1000BeansSafety | 0.462 | 0.305 | 0.388 | 0.294 |
| MCDisruptor_MPMC | 0.406 | 0.311 | 0.407 | 0.282 |
| BufferedRandomAccessFile | 0.253 | 0.040 | 0.254 | 0.026 |

Ratios are medians of the five within-repetition wall-time ratios, including
startup, parsing and reporting; they are not ratios of historical golden times
or separately selected median runs. The full summary retains arm timing ranges,
RSS medians, config/module hashes and every comparison's pair counts.

In the TLC profile, TLCSailfish1, MCLamportMutex, MCbtree and
BufferedRandomAccessFile requested eight workers but actually used one because
of runtime-name encounter-order checks. Their wall-time comparisons remain
visible; they receive **no scaling credit**, and the all-model TLC-profile
scaling aggregate is absent. Default-profile 1-to-8-worker scaling is 1.986×
across all ten models. This does not assess ADR 0006's distinct-state population
or 3.5× scaling target; small and initial-state-only models remain in the table.
CoffeeCan1000BeansSafety drops temporal properties in its explicit safety
configuration and earns no original-liveness parity credit.

The compact [machine-readable summary](../survey/data/aws-benchmark-2026-10-20.json)
records the source/binary, benchmark set, oracle, Java, corpus and module pins.
The release binary SHA-256 is
`2987ec6480544a3d69de2e7f3de794cc9035678611c03bb85ae217bedfdd4ca5`.
Raw rows and console output are retained externally, never committed. Recovery
independently recomputed all paired medians and geometric means from the raw
rows and verified archive hashes before terminating the completed instance.

Machine: c7i.metal-24xl, Intel Xeon Platinum 8488C, 188 GiB RAM, Ubuntu 24.04.5,
Linux 7.0.0-1014-aws, pinned AMI `ami-02a9cdd4e98bc3c78`, rustc 1.99.0. SMT was
off, governor `performance`, affinity CPUs 0–15 and memory NUMA node 0. The
runner executed one process at a time, counterbalancing worker/arm blocks on odd
repetitions. Seed and fingerprint polynomial were zero, timeout 1,800 seconds,
TLC heap 3g. The node was terminated after evidence recovery.

Reproduce the workload from that clean commit after oracle/corpus fetch, on a
reserved Linux host with the recorded CPU/memory controls:

```sh
cargo build --release --locked -p dive-cli -p dive-bench
taskset -c 0-15 numactl --membind=0 target/release/dive-bench \
  --profile both --workers 1,8 --reps 5 --tlc \
  --dive-revision 73259029b7689a55ddfe77ed648cb5dd14519274 \
  --timeout 1800 --xmx 3g \
  --runs <external-directory>/runs.jsonl \
  --summary <external-directory>/summary.json
```

Do not certify a quiet window by changing metadata after the run. Record the
reservation, CPU/thermal controls and absence of interfering work for the next
campaign; retain all original measurements when repeating a declared block.
