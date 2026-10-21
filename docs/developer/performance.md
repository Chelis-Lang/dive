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

## Substitution-cache probe

On 2026-10-21, a small paired probe confirmed that caching every forced
`INSTANCE` substitution would change the pinned TLC's evaluation behavior.
`Core::force` already reuses unprimed substitution thunks. Primed references
pass a null successor and bypass the cache, matching the explicitly retained
branch in pinned `Tool.java` at commit `94d0c507`.

Use these two modules in an external scratch directory:

```tla
---- MODULE Inner ----
VARIABLE a
Repeated == a = a
PrimedRepeated == a' = a'
====
```

```tla
---- MODULE Outer ----
EXTENDS Naturals, TLC
VARIABLE x
I == INSTANCE Inner WITH a <- Print(<<"force",x>>,x)
Init == x = 0
Next == x' = IF x < 1 THEN x+1 ELSE x
Unprimed == [][I!Repeated]_x
Primed == [][I!PrimedRepeated]_x
====
```

Each config selects `INIT Init`, `NEXT Next`, and either `PROPERTY Unprimed`
or `PROPERTY Primed`. Run both with pinned TLC (`-workers 1 -lncheck final
-seed 0 -fp 0`) and `dive check --compat=tlc --workers 1 --seed 0 --fp 0`.
Both implementations succeed with two distinct, three generated and depth two.
Unprimed emits two `Print` lines, while Primed emits four. This is a control on
cache semantics, not a timing benchmark. The current paired dive executable
was a debug binary (SHA-256
`3b0cff81cd88dfb68a198fd4f52e97e92fd78f8c3242912387180a4381400eb8`)
with recorded source context `d1eb36e3912a88032c24de025e9b11f125a72943`;
a complete clean-build attestation was not captured. The relevant cache source
blobs are identical at that context and integrated main `39f4e483`:
`eval.rs` blob `022d85c523c1de4ca6696a411365673de0db627d`, and `env.rs`
blob `2bad80a1ae4d265e5d0c27ab0b56bb1e0b984a4e`. No evaluator change
or performance improvement follows from the probe. Investigate cache misses within the existing TLC rule before
claiming that ADR 0104 item 6 offers new memoization savings.


## Discarded evaluator prototypes

A factorized instruction experiment on 2026-10-21 compared the run-budget
source `d1eb36e3912a88032c24de025e9b11f125a72943` with two local candidates:
stack arrays for strict unary/binary native operands (`a2a9461f`), and those
arrays plus evaluator-private compact dispatch (`48d323e6`). Both preserve the
original expression error wrapper, left-to-right forcing, host checks and lazy
semantics. The compact candidate retains IR fallback and differential controls;
it does not change `INSTANCE` caching.

| Reduced model / profile | Baseline instructions | Stack arrays | Arrays + compact dispatch |
|---|---:|---:|---:|
| GameOfLife / dive | 1,224,031,470 | −1.697% | −0.834% |
| GameOfLife / TLC | 1,179,946,797 | −1.759% | −0.864% |
| SimpleRegular / dive | 2,793,329,298 | −0.985% | −1.141% |
| SimpleRegular / TLC | 2,872,622,761 | −0.908% | −1.079% |

All twelve measurements completed successfully with identical counts and
depth: GameOfLife 512 distinct / 1,024 generated / depth 1; SimpleRegular
12,097 / 47,584 / depth 19. Measurements used the ADR 0104 reduced inputs,
Rust 1.99.0's profiling build, Valgrind 3.22.0
`--tool=cachegrind --cache-sim=no`, one worker, seed/fingerprint zero and a
330-second external backstop. The c7i.8xlarge Xeon 8488C host also ran gates;
these are instruction comparisons, with no quiet-host wall-time claim.

**Neither candidate met the 5% bar, and neither runtime change was landed.**
The experiment stopped after these two models; it is not evidence for an
improvement across the complete benchmark population. In particular, a second
plan representation added little beyond the smaller operand-storage change.
The shared IR already provides typed variable IDs and resolved locals, and
`Core::eval_inner` borrows its expression. Further work should profile remaining
activation and context ownership costs before introducing more dispatch
scaffolding. Raw observations, source overlays, binary hashes and callgrind
profiles are retained externally; the quick comparison archive SHA-256 is
`d90b2ea46d7f95761cfa8b96521af44ee0a9012f15dbafdf17d80632f7677665`.

## Packed temporal atom truth

The native temporal label store keeps one bit per actual state/action observation.
It retains global atom IDs and original-edge order while omitting false data for
unused atom domains. Public owned labels remain supported, and existing temporal
count limits are unchanged.

A diagnostic campaign on ElevatorLivenessMedium compared baseline e2b9d9f8 with
candidate c4fa21d2, using five alternating pairs per profile. Every run completed
with 4,122 distinct states, 14,296 generated states and depth 36, under identical
native 391,180 ms and 4 GiB limits. Median sampled resident memory changed from
41,172,992 to 34,947,072 bytes in the TLC profile (-15.1%), and 40,878,080 to
34,865,152 bytes in the default profile (-14.7%).

These are sampled resident pages on a shared Mac15,13 host, polled every 500 ms;
they are not an exact high-water mark or a quiet wall-clock benchmark. The compact
summary records source/binary pins, all repetition ranges, host/kernel/toolchain
and command settings. No temporal count limit is removed on this evidence.

A separate single-pair profiling diagnostic on the same source pins used
Valgrind 3.22.0 on a shared Xeon 8488C host, with native 900-second/4 GiB
budgets. Whole-CLI instructions changed by -1.031% in the default profile and
-1.017% in the TLC profile, with the same success, counts and depth. This is
not the evaluator performance acceptance campaign and earns no throughput
credit. Both measurements are preserved in the compact
[machine-readable summary](../survey/data/packed-atom-truth-2026-10-21.json).

## Search-budget overhead qualification

Optional state/depth budgets were measured with both ceilings unset, comparing
`9be2ba41` against `c82dd72c`. All eight ADR 0104 model/profile instruction cells
retain their distinct/generated counts and depth. Whole-CLI Cachegrind deltas
range from −0.0155% to +0.0953%, with geometric mean +0.0225%.

A separate genuine eight-worker `SimpleRegular` Callgrind run used three
interleaved baseline/candidate pairs per profile. Both arms report eight workers
and execute the parallel candidate-publication path. Paired-median instruction
deltas are −0.0679% (default) and +0.0144% (TLC); every pair lies between
−0.1355% and +0.0833%. Counts and depth remain identical. These satisfy the
one-percent normal-input instruction overhead bar. Cachegrind and Callgrind
totals are not pooled. The shared host and reduced diagnostic workloads make
no wall-clock, quiet-host or TLC throughput claim; this campaign executes no
TLC oracle.

The [compact evidence](../survey/data/search-budget-overhead-2026-10-21.json)
retains all cells/pairs, exact source/binary and workload hashes, reductions,
machine/toolchain, constraints and archive identity. Raw outputs stay external.

## Interval-streaming overhead qualification

Comparing `c82dd72c` against `f299eb07`, all eight ADR 0104 model/profile
Cachegrind cells retain their distinct/generated counts and depth. Whole-CLI
instruction deltas range from −0.0023% to +0.0462%, geometric mean +0.0112%.
Separate genuine eight-worker SimpleRegular Callgrind runs use three interleaved
pairs per profile, actual eight-worker reports and positive parallel-publication
call evidence. Their paired-median deltas are −0.0209% default / −0.0428% TLC,
with identical counts/depth. Every measured cell and pair satisfies the
one-percent normal-input overhead bar. The two instrument totals are not pooled.

The [compact evidence](../survey/data/interval-streaming-overhead-2026-10-21.json)
retains all rows/pairs, exact source/binary/workload/library identities, machine,
toolchain, constraints and archive hash. The shared host and reduced diagnostic
workloads earn no wall-clock or TLC throughput credit; no TLC oracle executes
in this paired instruction campaign. Raw output remains external.
