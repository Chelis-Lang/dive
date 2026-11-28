# Performance

How fast dive 0.2.0 is against TLC, how much memory it uses, and how it
scales, measured on tlaplus/Examples on dedicated machines. In short: at one
worker dive takes about a sixth of TLC's time on the geometric mean, with no
model above TLC's time, and uses tens of MiB where TLC uses about a GiB.
Safety checks scale with workers; temporal checks mostly do not.

Every timed run below agrees with its TLC result unless the table says
otherwise: verdict, distinct and generated counts at one worker, depth,
violated property and exit code. The headline speed measurement (R2 below) is
a release requirement; the other figures are recorded so that regressions are
visible. The records are compact JSON files in
[`conformance/reports/perf/`](../../conformance/reports/perf/README.md), whose
README indexes the data and fields; each section names its record. The scripts
that produced them are in
[`scripts/perf/release/`](../../scripts/perf/release/README.md). The 0.1.0
figures are in the `2026-11-18-*-08a14fba` records.

For the subsequent large-model investigations, see the [engineering measurements and data files](../../conformance/reports/perf/README.md#large-model-structure-and-edit-reuse).

## Summary

| Measurement | Result | Record |
|---|---|---|
| Speed (R2) | **Pass.** dive / TLC geomean 0.170x over the 169 agreeing models (fixed-set geomean 0.185x), median 0.129x. No model above 1x; the highest is `EWD998Chan` at 0.870x. 0.1.0 (`08a14fba`): 0.1949x, two models above 1x. | `2026-11-27-r2-procedure-5bb88063.json` |
| Memory | Benchmark set, default profile: dive peaks at 24 to 102 MiB at 1 worker and 64 to 140 MiB at 8, against 798 to 1,448 MiB for TLC at `-Xmx3g` (0.36x to 0.92x of the 0.1.0 figures). Liveness capture: 64 temporal models, none stopped; 62 match their golden and 2 match a registered divergence; the largest is `CoffeeCan3000Beans` at 1,929 MiB for 4.5 million states (`aba_asyn_byz` fell from 4,523 to 1,490 MiB). One model uses more than before: `MCParReach` +3.1% (#1432). | `2026-11-27-release-d2-memory-5bb88063.json` |
| Exact store | All 15 models agree under both stores. Exact costs 1.00x to 1.25x the fingerprint store's time and 0.99x to 1.90x its peak RSS. | `2026-11-27-release-d3-store-5bb88063.json` |
| Worker scaling | Safety models scale: over the 28 safety models of the large class the median speed-up is 2.97x at 4 workers and 5.22x at 8, up to 7.3x (`FastPaxos`). Temporal models do not: 17 of 19 stay between 0.81x and 1.25x. The median over all 47 models fell from 4.04x to 3.25x at 8 workers, mostly because 1-worker temporal checks got much faster (#1431). | `2026-11-27-release-d4-scaling-5bb88063.json` |
| Startup and throughput | `dive --version` 1.3 ms, `dive parse` 1.4 ms and a full `dive check` of a small model 4.5 ms (medians of 50). Trace validation 6 to 46 ms per trace, all valid. The bosco simulation, which stopped at its 16 GiB budget in 0.1.0, now samples its 10,000 traces in 28.5 s at 46 MiB. | `2026-11-27-release-d5-throughput-5bb88063.json` |
| Stop latency | Time budgets stop 7 to 43 ms after the limit (median per cell, load time subtracted; worst single run 48 ms). State and depth ceilings stop exactly at the ceiling, with no overshoot. | `2026-11-27-release-d6-stop-latency-5bb88063.json` |
| macOS (informational) | Not repeated for 0.2.0. On 0.1.0, on an Apple M3 laptop, dive / TLC geomean 0.354x at 1 worker over six benchmark models, all agreeing. | `2026-11-18-release-d7-macos-08a14fba.json` |
| Sail with dive against TLC | **Pass.** 25x to 37x faster than TLC per check on the smallest specifications (DieHard 12.3 ms through `sail runtime execute` against 453 ms), 13.8x over 100 fresh small checks in one MCP session (5.14 s against 70.8 s), 5.5x at 14,365 states and 1.3x on MCLamportMutex. All jobs agreed with their goldens. | `2026-11-27-release-s7-sail-vs-tlc-5bb88063.json` |

## Source, machine and method

| Item | Value |
|---|---|
| Source | `5bb880635f46c7266e015a0d812f18e4411cdc6a` (`main`, dive 0.2.0 candidate) |
| dive binary | `cargo build --release --locked -p dive-cli --bins` on one node, no `RUSTFLAGS`, copied to the others; SHA-256 `f18c0f9a0411750a8aaeced079e625ada6c05c86581fb01871e3057ae6aa007a` |
| Oracle | TLC `tlc-2026.10.06.014338`, pinned JRE, `--xmx 24g` (memory table's TLC column: `-Xmx3g`) |
| Corpus | tlaplus/Examples `6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f` |
| Nodes | c7i.metal-24xl in us-east-1: Intel Xeon Platinum 8488C, SMT off, turbo off, 2.4 GHz fixed, performance governor; one NUMA node with 48 CPUs; gp3 at 16000 IOPS and 1000 MiB/s; Ubuntu 24.04.5 LTS, kernel 7.0.0-1014-aws, THP `madvise` |
| Binding | `numactl --cpunodebind=0 --membind=0` for every timed run (all 48 CPUs) |
| Environment | `env -i` with only `PATH`, `HOME` and `DIVE_CACHE_DIR`; page cache warmed before timing; one checker at a time per node |
| Raw output | dive evidence archive bucket (us-east-1), prefix `release-perf/2026-11-27-rc02/`, cited by SHA-256 in each record |

Five on-demand nodes ran every measurement here, and no other node was
added. One ran the classification and the small class of the speed
measurement, then the memory, startup, store and stop-latency measurements.
Four each ran one large shard and then worker scaling on that shard's models.
Large models were sharded by longest processing time on the golden TLC
times, and each node timed TLC and dive on its own shard. To finish sooner,
some scaling repetitions moved to whichever of the five nodes was free; the
scaling record names the node of every repetition.

The binary was built once and copied, because independent builds of the same
source on different nodes are not byte-identical. Every node ran the binary
with the SHA-256 above.

Each run is checked against its TLC golden: verdict, distinct and generated
counts at one worker, depth, violated property and exit code. A run that
stops at its time or memory budget is reported as stopped. It is never
dropped or retried until it fits.

## Speed against TLC (R2)

The [R2 measurement procedure](performance.md#r2-measurement-procedure),
carried out by `dive-conformance r2` with fresh TLC times on every node (no
cached TLC time, so no canaries). A `--jobs 1` TLC pass over the 175-model
selection, taken before any dive run, put 127 models in the small class (TLC
under 10 s) and 48 in the large class, the same split as 0.1.0. This is the
first complete conforming attempt on this source; no part needed triage.

- **Small class** (121 agreeing models): six repetitions in one session,
  alternating which checker runs first. The ratio is the median of six.
  Geomean 0.151x.
- **Large class** (48 models): one pass each. No ratio landed in a band near
  1x or 2x, so no band pairs ran. Geomean 0.229x.
- **Excluded:** six small models that dive refuses by name or that disagree
  (`EWD687a_anim`, #1424, fixed after this source by #1429). They count as 2x
  in the fixed-set geomean of 0.185x.

The overall geomean is 0.170x, against 0.1949x in 0.1.0 (`08a14fba`), when
`MCKVsnap` (1.198x) and `AllocatorImplementation` (1.166x) were above 1x.
Against that record, 125 per-model ratios are within 10 %, 44 are lower and
none is higher. The record also compares each model with the later
`c14687e6` run (geomean 0.190x): 134 ratios are within 10 %, 30 are lower and
five are higher (`SpanTreeTest4Nodes` 1.26x its old ratio, `CoffeeCan100Beans`
1.21x, `SpanTree` 1.14x, `SpanTreeRandom` 1.13x, `TokenRing` 1.11x). Those
five now run at 0.546x TLC (`TokenRing`), 0.387x (`SpanTreeTest4Nodes`),
0.272x (`SpanTree`), 0.219x (`SpanTreeRandom`) and 0.146x
(`CoffeeCan100Beans`), each within 10 % of its 0.1.0 ratio or below it. The
largest gains are in fairness checking: `ElevatorLivenessLarge` went from
0.239x in 0.1.0 to 0.001x (1,882 s for TLC, 2.1 s for dive) and
`ElevatorLivenessMedium` from 0.436x to 0.015x. `MCKVsnap` is now 0.708x and
`AllocatorImplementation` 0.527x. The highest small-class ratios are `MCEcho`
0.798x and `GameOfLife` 0.787x.

### The highest ratios and the largest models


| Model | Class | Core / TLC | TLC s | Core s |
|---|---|---:|---:|---:|
| `ewd998/EWD998Chan` | large | 0.870 | 955.59 | 831.38 |
| `KeyValueStore/MCKVSSafetyMedium` | large | 0.792 | 1,164.60 | 922.67 |
| `MultiCarElevator/ElevatorSafetyMedium` | large | 0.581 | 811.55 | 471.43 |
| `MultiCarElevator/ElevatorSafetyLarge` | large | 0.529 | 2,582.45 | 1,366.23 |
| `MultiPaxos-SMR/MultiPaxos_MC` | large | 0.526 | 4,816.65 | 2,532.69 |
| `SlushProtocol/SlushLarge` | large | 0.464 | 12,578.01 | 5,833.14 |
| `bosco/bosco` | large | 0.417 | 653.92 | 272.72 |
| `NanoBlockchain/MCNanoLarge` | large | 0.407 | 5,810.00 | 2,365.15 |
| `byzpaxos/PConProof` | large | 0.394 | 1,919.88 | 756.51 |
| `bcastFolklore/bcastFolklore` | large | 0.271 | 2,162.15 | 586.15 |
| `aba-asyn-byz/aba_asyn_byz` | large | 0.238 | 1,313.59 | 313.28 |
| `SpanningTree/SpanTreeTest5Nodes` | large | 0.111 | 670.29 | 74.05 |
| `MultiCarElevator/ElevatorLivenessLarge` | large | 0.001 | 1,881.88 | 2.15 |


## Memory

Two parts on the small-class node. The benchmark-set part runs the ten
models of `crates/dive-bench/benchmarks.toml` with `dive-bench` at 1 and 8
workers in both profiles, five repetitions, with TLC at `-Xmx3g`. TLC's peak
RSS is mostly the JVM's heap sizing, not TLC's minimum, so read the TLC
column as what a user sees with that setting. Every dive figure is lower than
in 0.1.0.

The liveness part runs every R1 agreeing model whose report says
`check_kind: temporal`, found by probing each model with a `PROPERTY` once,
three repetitions at one worker under `/usr/bin/time -v`. `MCKVsnap` (D-005,
unreduced symmetry with liveness) and `SpanTreeRandom` (D-002, dive's own
seeded random streams) differ from the golden counts exactly as registered in
`conformance/expected-divergences.toml`; every other model matches its golden.
`MCParReach` is the only model whose peak RSS rose (98 to 102 MiB, #1432). In
the benchmark set, the `--compat=tlc` profile at one worker is slower than TLC
on `GameOfLife` (20.43 s against 12.10 s) and `TLCSailfish1` (68.53 s against
52.29 s); the default profile is faster than TLC on every model.

### Benchmark set peak RSS (MiB, median of 5)

| Model | Profile | Workers | dive RSS | dive range | TLC RSS | dive s | TLC s |
|---|---|---:|---:|---|---:|---:|---:|
| BufferedRandomAccessFile | default | 1 | 28 | 28–28 | 1,098 | 18.80 | 50.81 |
| BufferedRandomAccessFile | tlc | 1 | 28 | 28–28 | 1,098 | 21.54 | 50.81 |
| BufferedRandomAccessFile | default | 8 | 106 | 104–116 | 1,186 | 2.60 | 8.06 |
| BufferedRandomAccessFile | tlc | 8 | 107 | 106–115 | 1,186 | 2.98 | 8.06 |
| CoffeeCan1000BeansSafety | default | 1 | 102 | 101–102 | 805 | 1.37 | 2.25 |
| CoffeeCan1000BeansSafety | tlc | 1 | 102 | 101–102 | 805 | 1.37 | 2.25 |
| CoffeeCan1000BeansSafety | default | 8 | 132 | 130–132 | 798 | 0.90 | 1.67 |
| CoffeeCan1000BeansSafety | tlc | 8 | 131 | 130–132 | 798 | 0.90 | 1.67 |
| EWD998PCal | default | 1 | 32 | 32–32 | 1,217 | 14.01 | 42.39 |
| EWD998PCal | tlc | 1 | 32 | 32–32 | 1,217 | 15.16 | 42.39 |
| EWD998PCal | default | 8 | 87 | 86–88 | 1,448 | 2.37 | 7.56 |
| EWD998PCal | tlc | 8 | 86 | 78–88 | 1,448 | 2.52 | 7.56 |
| GameOfLife | default | 1 | 46 | 45–46 | 1,107 | 10.88 | 12.10 |
| GameOfLife | tlc | 1 | 46 | 46–46 | 1,107 | 20.43 | 12.10 |
| GameOfLife | default | 8 | 78 | 78–78 | 1,108 | 1.70 | 2.65 |
| GameOfLife | tlc | 8 | 78 | 78–78 | 1,108 | 2.98 | 2.65 |
| MCBakery | default | 1 | 37 | 37–38 | 1,269 | 6.59 | 11.17 |
| MCBakery | tlc | 1 | 37 | 37–38 | 1,269 | 6.59 | 11.17 |
| MCBakery | default | 8 | 98 | 94–102 | 1,295 | 2.15 | 7.83 |
| MCBakery | tlc | 8 | 100 | 88–100 | 1,295 | 2.15 | 7.83 |
| MCDisruptor_MPMC | default | 1 | 24 | 23–24 | 1,086 | 1.06 | 4.47 |
| MCDisruptor_MPMC | tlc | 1 | 24 | 24–24 | 1,086 | 1.09 | 4.47 |
| MCDisruptor_MPMC | default | 8 | 64 | 64–66 | 1,090 | 0.32 | 1.59 |
| MCDisruptor_MPMC | tlc | 8 | 66 | 64–68 | 1,090 | 0.32 | 1.59 |
| MCLamportMutex | default | 1 | 83 | 83–86 | 1,277 | 11.32 | 17.44 |
| MCLamportMutex | tlc | 1 | 84 | 83–84 | 1,277 | 12.35 | 17.44 |
| MCLamportMutex | default | 8 | 140 | 138–141 | 1,331 | 2.90 | 4.38 |
| MCLamportMutex | tlc | 8 | 138 | 130–138 | 1,331 | 3.28 | 4.38 |
| MCbtree | default | 1 | 32 | 32–32 | 1,179 | 5.40 | 40.81 |
| MCbtree | tlc | 1 | 32 | 32–32 | 1,179 | 5.61 | 40.81 |
| MCbtree | default | 8 | 76 | 74–76 | 1,294 | 1.22 | 7.76 |
| MCbtree | tlc | 8 | 74 | 74–76 | 1,294 | 1.27 | 7.76 |
| SimpleRegular | default | 1 | 46 | 46–46 | 1,137 | 2.59 | 5.48 |
| SimpleRegular | tlc | 1 | 44 | 43–46 | 1,137 | 2.59 | 5.48 |
| SimpleRegular | default | 8 | 84 | 82–90 | 1,200 | 0.77 | 2.33 |
| SimpleRegular | tlc | 8 | 84 | 84–87 | 1,200 | 0.77 | 2.33 |
| TLCSailfish1 | default | 1 | 66 | 66–66 | 1,158 | 36.78 | 52.29 |
| TLCSailfish1 | tlc | 1 | 66 | 66–66 | 1,158 | 68.53 | 52.29 |
| TLCSailfish1 | default | 8 | 113 | 111–115 | 1,364 | 5.53 | 8.27 |
| TLCSailfish1 | tlc | 8 | 120 | 119–122 | 1,364 | 10.20 | 8.27 |

### Liveness capture RSS (median of 3)

| Model | Distinct | Peak RSS MiB | Observed max MiB | Store lower bound MiB | Bytes/state | Wall s | Agrees |
|---|---:|---:|---:|---:|---:|---:|---|
| `CoffeeCan/CoffeeCan1000Beans` | 501500 | 230 | 230 | 46 | 480.1 | 9.49 | yes |
| `CoffeeCan/CoffeeCan100Beans` | 5150 | 30 | 30 | 2 | 6087.5 | 0.10 | yes |
| `CoffeeCan/CoffeeCan3000Beans` | 4504500 | 1,929 | 1,930 | 641 | 449.0 | 92.04 | yes |
| `DiningPhilosophers/DiningPhilosophers` | 67 | 24 | 25 | 1 | 380744.6 | 0.02 | yes |
| `Disruptor/MCDisruptor_MPMC_liveliness` | 14365 | 36 | 37 | 2 | 2659.8 | 0.60 | yes |
| `Disruptor/MCDisruptor_SPMC` | 8496 | 30 | 30 | 2 | 3735.9 | 0.25 | yes |
| `FiniteMonotonic/MCCRDT` | 25000 | 53 | 54 | 13 | 2243.6 | 8.57 | yes |
| `FiniteMonotonic/MCReplicatedLog` | 1363 | 28 | 28 | 1 | 21850.3 | 0.12 | yes |
| `FlashProtocol/MCFlashWithMutexLive` | 8096 | 46 | 46 | 1 | 6019.0 | 0.97 | yes |
| `Huang/Huang` | 81256 | 70 | 70 | 6 | 902.8 | 35.12 | yes |
| `KeyValueStore/MCKVsnap` | 189664 | 84 | 85 | 4 | 465.8 | 9.42 | registered D-005 |
| `LoopInvariance/MCBinarySearch` | 27953 | 124 | 124 | 1 | 4636.7 | 0.96 | yes |
| `LoopInvariance/MCQuicksort` | 4548 | 36 | 37 | 1 | 8409.0 | 4.94 | yes |
| `MisraReachability/MCParReach` | 393 | 102 | 103 | 1 | 272806.1 | 3.10 | yes |
| `MisraReachability/MCReachable` | 8 | 40 | 41 | 1 | 5292032.0 | 0.43 | yes |
| `Moving_Cat_Puzzle/CatEvenBoxes` | 48 | 22 | 22 | 1 | 481536.0 | 0.01 | yes |
| `Moving_Cat_Puzzle/CatOddBoxes` | 30 | 22 | 22 | 1 | 773051.7 | 0.01 | yes |
| `MultiCarElevator/ElevatorLivenessLarge` | 50653 | 74 | 74 | 1 | 1535.3 | 2.08 | yes |
| `MultiCarElevator/ElevatorLivenessMedium` | 4122 | 32 | 32 | 1 | 8180.1 | 0.18 | yes |
| `Prisoners/Prisoners` | 214 | 24 | 24 | 1 | 117941.8 | 0.01 | yes |
| `Prisoners_Single_Switch/Prisoner` | 16 | 24 | 25 | 1 | 1583616.0 | 0.01 | yes |
| `Prisoners_Single_Switch/PrisonerLightUnknown` | 62 | 24 | 25 | 1 | 413167.5 | 0.01 | yes |
| `Prisoners_Single_Switch/PrisonerSolo` | 2 | 24 | 25 | 1 | 12666880.0 | 0.01 | yes |
| `Prisoners_Single_Switch/PrisonerSoloLightUnknown` | 4 | 24 | 25 | 1 | 6284288.0 | 0.01 | yes |
| `ReadersWriters/MC` | 21527 | 36 | 37 | 2 | 1777.7 | 0.64 | yes |
| `SDP_Verification/SDP_Attack_New_Solution_Spec/MC` | 2881 | 42 | 43 | 1 | 15451.3 | 0.28 | yes |
| `SingleLaneBridge/MC` | 3605 | 32 | 33 | 2 | 9432.7 | 0.64 | yes |
| `SpanningTree/SpanTree` | 1236 | 26 | 27 | 1 | 22166.8 | 0.22 | yes |
| `SpanningTree/SpanTreeRandom` | 22040 | 38 | 39 | 2 | 1824.1 | 6.46 | registered D-002 |
| `SpanningTree/SpanTreeTest4Nodes` | 3984 | 27 | 27 | 1 | 6978.8 | 0.44 | yes |
| `SpanningTree/SpanTreeTest5Nodes` | 410112 | 148 | 149 | 2 | 378.3 | 73.98 | yes |
| `SpecifyingSystems/AdvancedExamples/MCInnerSequential` | 3528 | 28 | 28 | 2 | 8337.1 | 0.24 | yes |
| `SpecifyingSystems/AdvancedExamples/MCInnerSerial` | 195 | 32 | 33 | 2 | 174300.6 | 3.07 | yes |
| `SpecifyingSystems/Liveness/LiveHourClock` | 12 | 22 | 22 | 1 | 1939456.0 | 0.01 | yes |
| `SpecifyingSystems/Liveness/MCLiveInternalMemory` | 4408 | 28 | 28 | 1 | 6680.2 | 0.14 | yes |
| `SpecifyingSystems/Liveness/MCLiveWriteThroughCache` | 5196 | 30 | 30 | 1 | 6068.3 | 0.90 | yes |
| `SpecifyingSystems/RealTime/MCRealTimeHourClock` | 216 | 25 | 24 | 1 | 119030.5 | 0.01 | yes |
| `SpecifyingSystems/TLC/MCAlternatingBit` | 240 | 24 | 24 | 1 | 103560.5 | 0.02 | yes |
| `YoYo/MCYoYoNoPruning` | 60 | 29 | 29 | 1 | 501145.6 | 0.05 | yes |
| `YoYo/MCYoYoPruning` | 102 | 29 | 29 | 1 | 294871.8 | 0.07 | yes |
| `YoYo/YoYoAllGraphs` | 26731 | 56 | 57 | 6 | 2215.6 | 3.47 | yes |
| `aba-asyn-byz/aba_asyn_byz` | 5843977 | 1,490 | 1,490 | 1 | 267.4 | 313.54 | yes |
| `acp/ACP_NB_TLC` | 4284 | 32 | 33 | 1 | 7943.4 | 0.34 | yes |
| `allocator/AllocatorImplementation` | 17701 | 52 | 52 | 2 | 3072.1 | 5.12 | yes |
| `allocator/AllocatorRefinement` | 1690 | 31 | 31 | 1 | 18953.1 | 0.35 | yes |
| `allocator/SchedulingAllocator` | 1690 | 30 | 31 | 1 | 18793.1 | 0.26 | yes |
| `allocator/SimpleAllocator` | 400 | 26 | 27 | 1 | 68454.4 | 0.06 | yes |
| `bcastByz/bcastByz` | 14424 | 42 | 43 | 2 | 3083.9 | 4.14 | yes |
| `bcastByz/bcastByzNoBcast` | 45 | 33 | 33 | 1 | 760763.7 | 0.05 | yes |
| `bcastFolklore/bcastFolklore` | 501552 | 742 | 743 | 10 | 1550.8 | 586.43 | yes |
| `byzpaxos/Consensus` | 4 | 26 | 27 | 1 | 6939648.0 | 0.02 | yes |
| `cf1s-folklore/cf1s_folklore` | 2057174 | 475 | 458 | 1 | 242.3 | 85.76 | yes |
| `chang_roberts/MCChangRoberts` | 137 | 24 | 25 | 1 | 186801.5 | 0.02 | yes |
| `dijkstra-mutex/DijkstraMutex.toolbox/LSpec-model/MC` | 90882 | 70 | 71 | 1 | 810.7 | 3.68 | yes |
| `ewd426/TokenRing` | 46656 | 42 | 42 | 8 | 945.5 | 1.24 | yes |
| `ewd687a/MCEWD687a` | 18028 | 36 | 37 | 1 | 2116.4 | 2.73 | yes |
| `ewd840/EWD840` | 302 | 68 | 36 | 1 | 236035.4 | 0.04 | yes |
| `ewd840/SyncTerminationDetection` | 129 | 22 | 22 | 1 | 179049.2 | 0.01 | yes |
| `ewd998/AsyncTerminationDetection` | 4097 | 26 | 26 | 1 | 6685.4 | 0.10 | yes |
| `ewd998/EWD998Chan` | 1524022 | 514 | 515 | 2 | 353.5 | 832.59 | yes |
| `glowingRaccoon/product` | 305 | 62 | 30 | 1 | 213529.2 | 0.10 | yes |
| `glowingRaccoon/stages` | 83 | 24 | 24 | 1 | 304781.9 | 0.01 | yes |
| `nbacg_guer01/nbacg_guer01` | 24922 | 35 | 35 | 1 | 1452.4 | 0.72 | yes |
| `tcp/MCtcp` | 1182 | 31 | 31 | 1 | 27074.5 | 0.09 | yes |


## Exact versus fingerprint store

`dive check --store exact` against the default fingerprint store, three
repetitions with the store order alternating, on the ten benchmark-set models
and five liveness models. Both stores give the golden verdict, counts and
depth on every model. The exact store keeps every state, so it costs memory
on the models with many states per second of work (`MCLamportMutex`,
`SimpleRegular`, `EWD998PCal`, `MCBakery`, up to 1.90x). On the liveness
models, where the behavior graph dominates memory, it costs about 1.00x to
1.15x. A few ratios are slightly worse than in 0.1.0, though both stores got
faster on those models (#1433).

### Exact versus fingerprint store (median of 3)

| Model | fp s | exact s | time exact/fp | fp RSS MiB | exact RSS MiB | RSS exact/fp | Agrees |
|---|---:|---:|---:|---:|---:|---:|---|
| `CoffeeCan/CoffeeCan1000Beans (safety-only config: MaxBeanCount = 1000, INVARIANTS TypeInvariant)` | 1.32 | 1.59 | 1.209 | 102 | 121 | 1.192 | yes |
| `Bakery-Boulangerie/MCBakery` | 6.51 | 7.42 | 1.141 | 38 | 72 | 1.897 | yes |
| `CoffeeCan/CoffeeCan1000Beans` | 9.48 | 9.68 | 1.021 | 228 | 225 | 0.987 | yes |
| `Disruptor/MCDisruptor_MPMC` | 1.01 | 1.26 | 1.249 | 24 | 30 | 1.258 | yes |
| `FlashProtocol/MCFlashWithMutexLive` | 0.97 | 1.01 | 1.05 | 46 | 46 | 1.004 | yes |
| `GameOfLife/GameOfLife` | 10.80 | 10.91 | 1.01 | 46 | 50 | 1.083 | yes |
| `MultiCarElevator/ElevatorLivenessMedium` | 0.18 | 0.18 | 1.019 | 32 | 32 | 0.996 | yes |
| `SpecifyingSystems/Liveness/MCLiveInternalMemory` | 0.14 | 0.14 | 1.044 | 28 | 28 | 0.997 | yes |
| `TeachingConcurrency/SimpleRegular` | 2.54 | 3.00 | 1.182 | 46 | 58 | 1.268 | yes |
| `aba-asyn-byz/aba_asyn_byz` | 313.36 | 324.28 | 1.035 | 1,489 | 1,497 | 1.005 | yes |
| `braf/BufferedRandomAccessFile` | 18.69 | 18.67 | 0.999 | 28 | 28 | 0.997 | yes |
| `btree/MCbtree` | 5.35 | 6.35 | 1.187 | 32 | 46 | 1.44 | yes |
| `dag-consensus/TLCSailfish1` | 36.71 | 37.03 | 1.009 | 66 | 68 | 1.028 | yes |
| `ewd998/EWD998PCal` | 13.91 | 15.00 | 1.078 | 32 | 42 | 1.309 | yes |
| `lamport_mutex/MCLamportMutex` | 11.31 | 13.03 | 1.152 | 85 | 120 | 1.414 | yes |


## Worker scaling

`dive-bench --profile default --workers 1,4,8 --reps 3 --timeout 3600 --xmx 24g` on the 47
large-class models that have a success golden (`dive-bench` needs one, so
`MCCheckpointCoordinationFailure` is left out), bound with `numactl
--cpunodebind=0 --membind=0`, so 8 workers have all 48 CPUs. The speed-up is
the median over the three repetitions of the 1-worker time divided by the
same repetition's N-worker time. The four large-shard nodes ran their shard's
models, and repetitions moved to whichever node was free; the record names
the node of every repetition.

- **Safety models scale.** Over the 28 safety models the median 8-worker
  speed-up is 5.22x (2.97x at 4 workers); the best are `FastPaxos` 7.30x,
  `PConProof` 7.14x and `bosco` 7.05x, and the lowest is
  `MCLeastCircularSubstringMedium` at 2.39x.
- **Temporal models mostly do not** (#1028). Of the 19 temporal models, 17
  stay between 0.81x and 1.25x at 8 workers; `Huang` reaches 1.88x and
  `MCInnerSerial` 1.96x. `ElevatorLivenessLarge` (0.83x) and
  `ElevatorLivenessMedium` (0.90x) are slower with workers, but their
  1-worker checks now take 2.1 s and 0.2 s (450 s and 6.8 s in 0.1.0).
  `CoffeeCan1000Beans` and `CoffeeCan3000Beans`, 0.50x in 0.1.0, now run with
  one worker and say why in `options.workers.reason`.
- **The all-model median** is 3.25x at 8 workers and 2.25x at 4 (4.04x and
  2.26x in 0.1.0). It fell because 1-worker temporal checks got faster and
  now sit near 1x, and because `MCKVsnap` and `SpanTreeRandom`, refused at 4
  and 8 workers in 0.1.0 (#1020), now run with one worker at 1.00x (#1431).
- **SlushLarge at one worker** stops at `dive-bench`'s 3,570 s time limit in
  all three repetitions, so its speed-ups are lower bounds (1.89x and 3.25x).
  Against the speed measurement's completed 1-worker time on the same binary
  and host type (5,833.14 s) they are 3.08x and 5.31x.
- At one worker `MCCRDT`, `VoteProof`, `MCKVsnap` and `SpanTreeRandom`
  differ from the golden counts exactly as registered in
  `conformance/expected-divergences.toml`; every other run agrees with its
  golden. Generated counts at more than one worker are not compared.

### Worker scaling (median of 3)

| Model | 1 worker s | 4 workers s | 8 workers s | Speed-up 4 | Speed-up 8 | Note |
|---|---:|---:|---:|---:|---:|---|
| `SlushProtocol/SlushLarge` | 3,570.1 (stopped) | 1,890.9 | 1,098.1 | ≥1.89 | ≥3.25 | 1 worker stopped at the 3570 s limit; speed-ups are lower bounds |
| `MultiPaxos-SMR/MultiPaxos_MC` | 2,531.5 | 753.6 | 394.9 | 3.37 | 6.41 |  |
| `NanoBlockchain/MCNanoLarge` | 2,357.4 | 816.4 | 492.9 | 2.89 | 4.78 |  |
| `MultiCarElevator/ElevatorSafetyLarge` | 1,371.3 | 468.3 | 269.5 | 2.95 | 5.11 |  |
| `KeyValueStore/MCKVSSafetyMedium` | 913.9 | 300.3 | 170.3 | 3.04 | 5.34 |  |
| `ewd998/EWD998Chan` | 833.4 | 757.2 | 745.1 | 1.10 | 1.12 |  |
| `byzpaxos/PConProof` | 758.5 | 204.9 | 106.2 | 3.70 | 7.14 |  |
| `bcastFolklore/bcastFolklore` | 587.2 | 580.3 | 583.1 | 1.01 | 1.01 |  |
| `MultiCarElevator/ElevatorSafetyMedium` | 469.3 | 163.5 | 89.7 | 2.87 | 5.23 |  |
| `aba-asyn-byz/aba_asyn_byz` | 313.5 | 310.5 | 306.9 | 1.01 | 1.02 |  |
| `bosco/bosco` | 272.8 | 74.1 | 38.7 | 3.68 | 7.05 |  |
| `dijkstra-mutex/DijkstraMutex.toolbox/Safety-4-processors/MC` | 163.5 | 87.5 | 56.1 | 1.88 | 2.91 |  |
| `Bakery-Boulangerie/MCBoulanger` | 143.0 | 50.5 | 29.7 | 2.83 | 4.82 |  |
| `SlushProtocol/SlushMedium` | 142.1 | 50.2 | 29.8 | 2.83 | 4.76 |  |
| `KeyValueStore/MCKVSSafetySmall` | 101.7 | 37.0 | 20.8 | 2.76 | 4.91 |  |
| `CoffeeCan/CoffeeCan3000Beans` | 91.9 | 92.6 | 92.8 | 0.99 | 0.99 |  |
| `cf1s-folklore/cf1s_folklore` | 85.8 | 83.2 | 82.8 | 1.03 | 1.04 |  |
| `CheckpointCoordination/MCCheckpointCoordination` | 80.2 | 24.4 | 13.1 | 3.29 | 6.13 |  |
| `SpanningTree/SpanTreeTest5Nodes` | 74.1 | 68.5 | 67.7 | 1.08 | 1.09 |  |
| `transaction_commit/PaxosCommit` | 70.5 | 21.1 | 12.1 | 3.34 | 5.84 |  |
| `PaxosHowToWinATuringAward/MCPaxosSmall` | 51.2 | 14.3 | 7.4 | 3.58 | 6.91 |  |
| `dag-consensus/TLCSailfish1` | 36.8 | 10.5 | 5.5 | 3.48 | 6.65 |  |
| `Huang/Huang` | 35.2 | 21.1 | 18.7 | 1.66 | 1.88 |  |
| `ewd998/EWD998Small` | 19.5 | 7.6 | 4.7 | 2.54 | 4.11 |  |
| `braf/BufferedRandomAccessFile` | 18.7 | 5.0 | 2.6 | 3.72 | 7.20 |  |
| `SimplifiedFastPaxos/FastPaxos` | 17.8 | 4.7 | 2.4 | 3.80 | 7.30 |  |
| `ewd998/EWD998PCal` | 14.0 | 4.2 | 2.3 | 3.34 | 5.95 |  |
| `lamport_mutex/MCLamportMutex` | 11.3 | 5.0 | 2.9 | 2.25 | 3.91 |  |
| `GameOfLife/GameOfLife` | 10.9 | 3.1 | 1.7 | 3.50 | 6.34 |  |
| `CoffeeCan/CoffeeCan1000Beans` | 9.5 | 9.6 | 9.6 | 0.99 | 0.99 |  |
| `KeyValueStore/MCKVsnap` | 9.5 | 9.5 | 9.5 | 1.00 | 1.00 | registered divergence at every worker count; runs with one worker at 4 and 8 |
| `MultiPaxos-SMR/MultiPaxos_MC_small` | 8.9 | 2.9 | 1.6 | 3.11 | 5.53 |  |
| `FiniteMonotonic/MCCRDT` | 8.6 | 6.3 | 5.9 | 1.37 | 1.46 | 1 worker: registered divergence |
| `NanoBlockchain/MCNanoMedium` | 8.6 | 2.9 | 1.6 | 2.99 | 5.21 |  |
| `Bakery-Boulangerie/MCBakery` | 6.6 | 2.8 | 2.1 | 2.34 | 3.10 |  |
| `SpanningTree/SpanTreeRandom` | 6.5 | 6.5 | 6.5 | 1.00 | 1.00 | registered divergence at every worker count; runs with one worker at 4 and 8 |
| `btree/MCbtree` | 5.4 | 2.1 | 1.2 | 2.54 | 4.36 |  |
| `bcastByz/bcastByz` | 4.2 | 3.5 | 3.3 | 1.20 | 1.25 |  |
| `dijkstra-mutex/DijkstraMutex.toolbox/LSpec-model/MC` | 3.7 | 3.8 | 3.8 | 0.98 | 0.98 |  |
| `MisraReachability/MCParReach` | 3.2 | 3.0 | 3.0 | 1.05 | 1.05 |  |
| `SpecifyingSystems/AdvancedExamples/MCInnerSerial` | 3.1 | 1.9 | 1.6 | 1.65 | 1.96 |  |
| `MultiCarElevator/ElevatorLivenessLarge` | 2.1 | 2.6 | 2.5 | 0.81 | 0.83 |  |
| `LeastCircularSubstring/MCLeastCircularSubstringMedium` | 2.1 | 1.2 | 0.9 | 1.70 | 2.39 |  |
| `FlashProtocol/MCFlashWithMutexLive` | 1.0 | 1.0 | 1.0 | 1.03 | 1.05 |  |
| `byzpaxos/VoteProof` | 0.7 | 0.3 | 0.2 | 2.23 | 3.09 | 1 worker: registered divergence |
| `MisraReachability/MCReachable` | 0.5 | 0.5 | 0.5 | 1.05 | 1.05 |  |
| `MultiCarElevator/ElevatorLivenessMedium` | 0.2 | 0.3 | 0.3 | 0.82 | 0.90 |  |

### Temporal models

| Model | Distinct states | 1 worker s | 4 workers s | 8 workers s | Speed-up 4 | Speed-up 8 | Workers used at 8 |
|---|---:|---:|---:|---:|---:|---:|---|
| `MultiCarElevator/ElevatorLivenessLarge` | 50,653 | 2.1 | 2.6 | 2.5 | 0.81 | 0.83 | 8 |
| `MultiCarElevator/ElevatorLivenessMedium` | 4,122 | 0.2 | 0.3 | 0.3 | 0.82 | 0.90 | 8 |
| `dijkstra-mutex/DijkstraMutex.toolbox/LSpec-model/MC` | 90,882 | 3.7 | 3.8 | 3.8 | 0.98 | 0.98 | 8 |
| `CoffeeCan/CoffeeCan1000Beans` | 501,500 | 9.5 | 9.6 | 9.6 | 0.99 | 0.99 | 1 |
| `CoffeeCan/CoffeeCan3000Beans` | 4,504,500 | 91.9 | 92.6 | 92.8 | 0.99 | 0.99 | 1 |
| `KeyValueStore/MCKVsnap` | 189,664 | 9.5 | 9.5 | 9.5 | 1.00 | 1.00 | 1 |
| `SpanningTree/SpanTreeRandom` | 22,040 | 6.5 | 6.5 | 6.5 | 1.00 | 1.00 | 1 |
| `bcastFolklore/bcastFolklore` | 501,552 | 587.2 | 580.3 | 583.1 | 1.01 | 1.01 | 8 |
| `aba-asyn-byz/aba_asyn_byz` | 5,843,977 | 313.5 | 310.5 | 306.9 | 1.01 | 1.02 | 8 |
| `cf1s-folklore/cf1s_folklore` | 2,057,174 | 85.8 | 83.2 | 82.8 | 1.03 | 1.04 | 8 |
| `FlashProtocol/MCFlashWithMutexLive` | 8,096 | 1.0 | 1.0 | 1.0 | 1.03 | 1.05 | 8 |
| `MisraReachability/MCParReach` | 393 | 3.2 | 3.0 | 3.0 | 1.05 | 1.05 | 8 |
| `MisraReachability/MCReachable` | 8 | 0.5 | 0.5 | 0.5 | 1.05 | 1.05 | 8 |
| `SpanningTree/SpanTreeTest5Nodes` | 410,112 | 74.1 | 68.5 | 67.7 | 1.08 | 1.09 | 8 |
| `ewd998/EWD998Chan` | 1,524,022 | 833.4 | 757.2 | 745.1 | 1.10 | 1.12 | 8 |
| `bcastByz/bcastByz` | 14,424 | 4.2 | 3.5 | 3.3 | 1.20 | 1.25 | 8 |
| `FiniteMonotonic/MCCRDT` | 25,000 | 8.6 | 6.3 | 5.9 | 1.37 | 1.46 | 8 |
| `Huang/Huang` | 81,256 | 35.2 | 21.1 | 18.7 | 1.66 | 1.88 | 8 |
| `SpecifyingSystems/AdvancedExamples/MCInnerSerial` | 195 | 3.1 | 1.9 | 1.6 | 1.65 | 1.96 | 8 |

"Workers used at 8" is what the report states in `options.workers.used`;
where it is 1 the report also gives the reason.

### Open finding: #1028

The likely cause is that temporal checks spend most of their time in work
that more workers do not share: ordered commit of the behavior graph on one thread, and the liveness
analysis after the search, which runs on the calling thread. In 0.2.0 the
models whose search could not gain (many initial states with short,
non-enumerating actions, such as `CoffeeCan1000Beans`) fall back to one worker
and say so, so they no longer slow down. The two Elevator liveness models,
whose whole check now takes seconds, are the remaining models that are
slower with workers. To compare the phases, run a model at `--workers 1` and
`--workers 8` with `--output json` and compare `elapsed_ms`,
`core.metrics.explore_ms` and `core.metrics.expand_ms`; `options.workers`
states the workers used and any reason for running sequentially.

## Startup, simulation and trace validation

Fifty fresh processes each, timed with `perf_counter` around
`subprocess.run`:

| Command | Median ms | p90 ms | Max ms |
|---|---:|---:|---:|
| `dive --version` | 1.28 | 1.42 | 1.54 |
| `dive parse Counter.tla` | 1.40 | 1.57 | 1.79 |
| `dive check Counter.tla` (peak RSS 17.4 MiB) | 4.46 | 4.79 | 5.01 |

Trace validation, `dive validate-trace` on a counterexample produced once:

| Trace | Runs | States | Median ms | Result |
|---|---:|---:|---:|---|
| `DieHard` | 10 | 7 | 6.06 | valid ×10 |
| `SlidingPuzzles` | 10 | 117 | 46.12 | valid ×10 |
| `MCRealTimeHourClock` (lasso) | 10 | 2 | 7.33 | valid ×10 |
| `bosco`, `--sample`, simulation traces | 10 | 101 | 81.12 | valid ×10 |

Simulation (`--traces 10000 --depth 100 --time-limit 600s --max-memory 16g`):

- `bosco` samples all 10,000 traces in every repetition, none dropped or
  stalled: 28.5 s median wall time, 46 MiB peak RSS, 2,069,707 generated
  states, about 350 traces and 72,500 states per second. In 0.1.0 it stopped
  at the 16 GiB budget after 9,901 traces (#1024, fixed).
- `MCLamportMutex` reports a constraint-pruned deadlock on its first trace
  (exit 11), as documented for simulation, so it gives no throughput figure.
  The `--sample` traces therefore come from bosco.

## Stop latency

Latency is the wall time minus the load time (a `--max-states 1` run at the
same worker count) minus the limit. Five repetitions per cell.

### Time budgets

| Model | Workers | Limit | Wall s | Latency ms, median | Latency ms, max | Wall − limit ms, max | Phase |
|---|---:|---|---:|---:|---:|---:|---|
| `MultiCarElevator/ElevatorSafetyLarge` | 1 | 5s | 5.02 | 6.9 | 7.7 | 18.6 | safety-search |
| `MultiCarElevator/ElevatorSafetyLarge` | 1 | 30s | 30.02 | 9.5 | 10.2 | 21.2 | safety-search |
| `MultiCarElevator/ElevatorSafetyLarge` | 1 | 120s | 120.05 | 39.8 | 42.6 | 53.5 | safety-search |
| `MultiCarElevator/ElevatorSafetyLarge` | 8 | 5s | 5.02 | 10.7 | 10.9 | 21.3 | safety-search |
| `MultiCarElevator/ElevatorSafetyLarge` | 8 | 30s | 30.02 | 10.3 | 10.9 | 21.4 | safety-search |
| `MultiCarElevator/ElevatorSafetyLarge` | 8 | 120s | 120.05 | 35.0 | 39.3 | 49.8 | safety-search |
| `SlushProtocol/SlushLarge` | 1 | 5s | 5.02 | 6.8 | 7.1 | 19.1 | safety-search |
| `SlushProtocol/SlushLarge` | 1 | 30s | 30.02 | 7.1 | 10.0 | 22.0 | safety-search |
| `SlushProtocol/SlushLarge` | 1 | 120s | 120.06 | 43.0 | 47.7 | 59.7 | safety-search |
| `SlushProtocol/SlushLarge` | 8 | 5s | 5.02 | 8.8 | 8.9 | 22.0 | safety-search |
| `SlushProtocol/SlushLarge` | 8 | 30s | 30.02 | 9.1 | 9.6 | 22.7 | safety-search |
| `SlushProtocol/SlushLarge` | 8 | 120s | 120.05 | 33.8 | 36.2 | 49.2 | safety-search |

Every run exits 5 with a `stopped` report. `ElevatorLivenessLarge`, which in
0.1.0 measured the stop in temporal analysis, now completes its whole check
(holds, exit 0) in about 2.1 s at 1 worker and 2.5 s at 8, before every limit,
so it no longer measures a stop; the record keeps its rows with a note (#1433).

### State, depth and temporal ceilings

| Model | Workers | Ceiling | Wall s | Distinct | Depth | Overshoot | Phase |
|---|---:|---|---:|---:|---:|---:|---|
| `SlushProtocol/SlushLarge` | 1 | --max-states 1000000 | 9.31 | 1,000,000 | 18 | 0 | safety-search |
| `SlushProtocol/SlushLarge` | 1 | --max-states 10000000 | 124.48 | 10,000,000 | 23 | 0 | safety-search |
| `SlushProtocol/SlushLarge` | 1 | --max-depth 10 | 0.10 | 15,264 | 10 | 0 | safety-search |
| `SlushProtocol/SlushLarge` | 1 | --max-depth 30 | 1,257.20 | 70,238,348 | 30 | 0 | safety-search |
| `SlushProtocol/SlushLarge` | 8 | --max-states 1000000 | 2.13 | 1,000,000 | 18 | 0 | safety-search |
| `SlushProtocol/SlushLarge` | 8 | --max-states 10000000 | 26.20 | 10,000,000 | 23 | 0 | safety-search |
| `SlushProtocol/SlushLarge` | 8 | --max-depth 10 | 0.04 | 15,264 | 10 | 0 | safety-search |
| `SlushProtocol/SlushLarge` | 8 | --max-depth 30 | 246.52 | 70,238,348 | 30 | 0 | safety-search |
| `MultiCarElevator/ElevatorLivenessLarge` | 1 | --max-states 20000 --store exact | 0.44 | 20,000 | 15 | 0 | graph-capture |
| `MultiCarElevator/ElevatorLivenessLarge` | 1 | --max-depth 20 --store exact | 0.62 | 30,605 | 20 | 0 | graph-capture |

Three repetitions each; every run exits 5 with distinct states or depth
exactly at the ceiling. The temporal ceilings stop in graph capture and make
no claim.

## macOS arm64 spot check (informational)

Not repeated for 0.2.0. The 0.1.0 spot check ran on a MacBook Air (Apple M3,
24 GB, macOS 26.5) in use by other work: six benchmark-set models at 1 and
4 workers, default profile, three repetitions, with pinned TLC at `-Xmx3g`.
Every run agreed with its golden; the dive / TLC geomean was 0.354x at
1 worker and 0.347x at 4. The record is
`2026-11-18-release-d7-macos-08a14fba.json`.

## Sail with dive against TLC

It compares the whole Sail+dive workflow with the standard one: a fresh
`java -XX:+UseParallelGC -jar tla2tools.jar -workers 1 -config <cfg> <module>`
per check (pinned oracle jar and JRE, default heap). Sail 0.3.1 (`b00b6de9`)
ran D.I.V.E. `5bb88063` in its isolated worker with its fixed policy (exact
store, seed and fingerprint zero, one worker, explicit bounds). One fresh
c7i.8xlarge in us-east-1 (Intel Xeon Platinum 8488C, 32 vCPUs, SMT on,
frequency not controlled, no binding, nothing else running). Five repetitions
per cell, path order rotated; multiples are TLC time over Sail time within
each repetition, then the median. Every job in every path agreed with its
golden on the holds/refuted class and the distinct-state count, and every
Sail refutation carried an independently validated trace. Sail 0.3.1 replays
a check whose identical job already completed in the same cache, so every
timed Sail job below carries its own `wall_time_ms` and is a fresh check. The
Sail page is `docs/validation/sail-dive-0.2-vs-tlc.md` (Chelis-Lang/sail#815);
the record is `2026-11-27-release-s7-sail-vs-tlc-5bb88063.json`.

Median milliseconds:

| Fixture | TLC | dive CLI | Sail execute | Sail MCP (warm) | TLC / execute | TLC / MCP |
|---|---:|---:|---:|---:|---:|---:|
| DieHard (14 states) | 453 | 5.5 | 12.3 | 18.2 | 36.8x | 24.9x |
| EWD840 (302) | 634 | 31.4 | 39.9 | 43.7 | 15.9x | 14.6x |
| MCRealTimeHourClock (216) | 481 | 9.3 | 16.5 | 22.4 | 28.9x | 21.6x |
| MCLiveInternalMemory (4,408) | 1,296 | 98.4 | 113 | 116 | 11.5x | 11.1x |
| MCDisruptor_MPMC_liveliness (14,365) | 2,386 | 410 | 436 | 441 | 5.5x | 5.4x |
| MCLamportMutex (724,274) | 12,515 | 9,184 | 9,587 | 9,649 | 1.3x | 1.3x |

- **Agent loop.** 100 fresh checks cycling over the four small fixtures:
  TLC 70.8 s, one warm MCP session 5.14 s (13.8x), `sail runtime execute`
  4.57 s (15.5x), the dive CLI 3.63 s (19.5x). Two concurrent clients on one
  daemon complete 39.8 jobs/s against 2.7 for two concurrent TLC processes
  (14.7x); beyond two active executions Sail refuses jobs with
  `SAIL6201 busy`.
- **Repeated checks.** With the jobs repeated as S7 builds them, Sail replays
  its receipts: a warm MCP call returns in about 10 ms on every fixture
  (1,274x on MCLamportMutex) and the 100-check loop takes 1.19 s through MCP
  (56.7x).
- **Memory.** Sail jobs peak at 25 to 141 MiB against TLC's 161 MiB to
  3.5 GiB on the same models; a warm `sail serve` holds 12 MiB.
- **First job.** The first job after installing a worker takes 238 ms, then
  12.2 ms (0.3.0: 207 ms; Chelis-Lang/sail#814).
- **Large models** are the speed measurement's engine comparison and were not
  rerun. The Sail worker takes 1.05x (MCLamportMutex) to 1.12x
  (ElevatorLivenessMedium) the CLI's time with Sail's settings, so a large
  model's Sail/TLC ratio is its speed-measurement ratio times that factor.
  Composed this way: `MCLamportMutex` 0.645x (0.615x × 1.05),
  `SlushSmall` 0.431x (0.403x × 1.07), `CoffeeCan1000Beans` 0.309x
  (0.278x × 1.11), `MCDisruptor_MPMC` 0.244x (0.226x × 1.08) and
  `ElevatorLivenessMedium` 0.016x (0.015x × 1.12).

## Issues

- #1028: temporal checks do not gain from workers; `ElevatorLivenessLarge`
  and `ElevatorLivenessMedium` are slower at 4 and 8 workers than at 1.
- #1431: the worker-scaling figures that are worse than 0.1.0's (median
  8-worker speed-up 3.25x against 4.04x, and per-model drops).
- #1432: `MCParReach` liveness peak RSS is 3.1% higher than in 0.1.0.
- #1433: directional exact-store ratios and `SlushLarge` 1-worker stop
  latencies worse than in 0.1.0, and D6 needs a temporal model that outlasts
  its limits.
- Chelis-Lang/sail#814: Sail's first job after install and the worker / CLI
  factor on MCLamportMutex.

Closed since 0.1.0: #979 (`MCKVsnap` above 1x), #940 (`AllocatorImplementation`
near 1x, now 0.527x), #1020 (`MCKVsnap` and `SpanTreeRandom` refused at more
than one worker) and #1024 (`dive simulate` memory).
