# Performance

How fast dive 0.1.0 is against TLC, how much memory it uses, and how it
scales, measured on tlaplus/Examples on dedicated machines. In short: at one
worker dive takes about a fifth of TLC's time on the geometric mean, with no
model above twice TLC's time, and uses tens to hundreds of MiB where TLC uses
about a GiB. Safety checks scale with workers; temporal checks mostly do not.

Every timed run below agrees with its TLC result unless the table says
otherwise: verdict, distinct and generated counts at one worker, depth,
violated property and exit code. The headline speed measurement (R2 below) was
a release requirement; the other figures are recorded so that regressions are
visible. The records are compact JSON files in
[`conformance/reports/perf/`](../../conformance/reports/perf/README.md), whose
README indexes the data and fields; each section names its record. The scripts
that produced them are in
[`scripts/perf/release/`](../../scripts/perf/release/README.md).

For the subsequent large-model investigations, see the [engineering measurements and data files](../../conformance/reports/perf/README.md#large-model-structure-and-edit-reuse).

## Summary

| Measurement | Result | Record |
|---|---|---|
| Speed (R2) | **Pass.** dive / TLC geomean 0.195x over the 169 agreeing models (fixed-set geomean 0.211x), median 0.148x. No model above 2x. Two above 1x: `MCKVsnap` 1.20x (#979) and `AllocatorImplementation` 1.17x (#940). | `2026-11-18-r2-procedure-08a14fba.json` |
| Memory | Benchmark set, default profile: dive peaks at 37 to 110 MiB at 1 worker and 147 to 286 MiB at 8, against 795 to 1,449 MiB for TLC at `-Xmx3g`. Liveness capture: 64 temporal models, none stopped; 62 match their golden and 2 match a registered divergence; the largest is `aba_asyn_byz` at 4,523 MiB for 5.8 million states. | `2026-11-18-release-d2-memory-08a14fba.json` |
| Exact store | All 15 models agree under both stores. Exact costs 1.00x to 1.30x the fingerprint store's time and 0.95x to 2.68x its peak RSS. | `2026-11-18-release-d3-store-08a14fba.json` |
| Worker scaling | Safety models scale: median speed-up over the 47 large-class models 2.26x at 4 workers and 4.04x at 8, up to 7.6x (`PConProof`). Models with temporal properties mostly do not: 15 of the 17 that run at 8 workers stay between 0.50x and 1.40x, six of them slower than at 1 worker (#1028). Two refuse at more than one worker (#1020). | `2026-11-18-release-d4-scaling-08a14fba.json` |
| Startup and throughput | `dive --version` 1.5 ms, `dive parse` 1.3 ms and a full `dive check` of a small model 4.9 ms (medians of 50). Trace validation 7 to 47 ms per trace, all valid. The bosco simulation stops at its 16 GiB budget (#1024). | `2026-11-18-release-d5-throughput-08a14fba.json` |
| Stop latency | Time budgets stop 4 to 40 ms after the limit (median per cell, load time subtracted; worst single run 170 ms). State and depth ceilings stop exactly at the ceiling, with no overshoot. | `2026-11-18-release-d6-stop-latency-08a14fba.json` |
| macOS (informational) | On an Apple M3 laptop, dive / TLC geomean 0.354x at 1 worker and 0.347x at 4 workers over six benchmark models, all agreeing. | `2026-11-18-release-d7-macos-08a14fba.json` |
| Sail with dive against TLC | Best case 27x to 39x faster than TLC per check on the smallest specifications (warm MCP 18.4 ms and `sail runtime execute` 12.8 ms against 491 ms on DieHard), and 12.6x over 100 mixed small checks in one MCP session (6.7 s against 84.5 s). Typical: 9x to 17x on small liveness models, 2.4x at 14,365 states, 1.1x on MCLamportMutex. Before the Sail #765 fix the same paths were 0.9x to 4.0x. All jobs agreed with their goldens. | `2026-11-19-release-s7-sail-vs-tlc-ce94df74.json` |

## Source, machine and method

| Item | Value |
|---|---|
| Source | `08a14fbac83ff71a3247623fb10bf0584bc83be5` (`main`, dive 0.1.0 candidate) |
| dive binary | `cargo build --release --locked -p dive-cli --bins` on one node, no `RUSTFLAGS`, copied to the others; SHA-256 `97675f9d446dbd188684f9f519e1313a86ae7e7faf57a7964d42d1115498e014` |
| Oracle | TLC `tlc-2026.10.06.014338`, pinned JRE, `--xmx 24g` (memory table's TLC column: `-Xmx3g`) |
| Corpus | tlaplus/Examples `6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f` |
| Nodes | c7i.metal-24xl in us-east-1: Intel Xeon Platinum 8488C, SMT off, turbo off, 2.4 GHz fixed, performance governor; one NUMA node with 48 CPUs; gp3 at 16000 IOPS and 1000 MiB/s; Ubuntu 24.04.5 LTS, kernel 7.0.0-1017-aws, THP `madvise` |
| Binding | `numactl --cpunodebind=0 --membind=0` for every timed run (all 48 CPUs) |
| Environment | `env -i` with only `PATH`, `HOME` and `DIVE_CACHE_DIR`; page cache warmed before timing; one checker at a time per node |
| Raw output | dive evidence archive bucket (us-east-1), prefix `release-perf/2026-11-18/`, cited by SHA-256 in each record |

Five on-demand nodes ran the speed measurement. One ran its small class and
then the startup, store, stop-latency and memory measurements. Four each ran
one large shard and then worker scaling on that shard's models.
Large models were sharded by longest processing time on the golden TLC times,
and each node timed TLC and dive on its own shard. The scaling measurement's later repetitions ran in parallel on five additional spot nodes of the same type, image and setup; the scaling record names the node of every repetition.

The binary was built once and copied, because independent builds of the same
source on different nodes were not byte-identical. Every node ran the binary
with the SHA-256 above.

Each run is checked against its TLC golden: verdict, distinct and generated
counts at one worker, depth, violated property and exit code. A run that
stops at its time or memory budget is reported as stopped. It is never
dropped or retried until it fits.

## Speed against TLC (R2)

The [R2 measurement procedure](performance.md#r2-measurement-procedure),
driven by a script around `dive-conformance` while #983 is open. A `--jobs 1`
TLC pass over the 175-model selection, taken before any dive run, put 127
models in the small class (TLC under 10 s) and 48 in the large class.

- **Small class** (121 agreeing models): six repetitions in one session,
  alternating which checker runs first. The ratio is the median of six.
  Geomean 0.161x.
- **Large class** (48 models): one pass each. Two models landed in a band
  near 1x and got three fresh back-to-back pairs: `MCKVSSafetyMedium` (first
  pass 0.91x, pairs 0.90x, 0.91x, 0.90x) and `EWD998Chan` (0.98x; pairs
  1.02x, 0.98x, 0.99x). Geomean 0.315x.
- **Excluded:** six small models that dive refuses by name (unsupported
  features). They count as 2x in the fixed-set geomean of 0.211x.

The overall geomean is 0.195x, the same as the 2026-11-10 record to three
places, although 114 per-model ratios moved by more than 0.0001. The two
models above 1x are the same two as in that record and already have issues.

### Models above 0.9x and the largest models

| Model | Class | dive / TLC | TLC s | dive s |
|---|---|---:|---:|---:|
| `KeyValueStore/MCKVsnap` | large | 1.198 | 14.11 | 16.89 |
| `allocator/AllocatorImplementation` | small | 1.166 |  |  |
| `ewd998/EWD998Chan` | large | 0.990 | 940.30 | 921.01 |
| `KeyValueStore/MCKVSSafetyMedium` | large | 0.904 | 1,156.30 | 1,057.43 |
| `MultiCarElevator/ElevatorSafetyMedium` | large | 0.655 | 813.47 | 533.15 |
| `MultiCarElevator/ElevatorSafetyLarge` | large | 0.573 | 2,592.80 | 1,486.27 |
| `MultiPaxos-SMR/MultiPaxos_MC` | large | 0.543 | 4,826.11 | 2,619.62 |
| `SlushProtocol/SlushLarge` | large | 0.527 | 12,408.36 | 6,544.78 |
| `bosco/bosco` | large | 0.473 | 643.30 | 304.29 |
| `byzpaxos/PConProof` | large | 0.446 | 1,898.95 | 847.76 |
| `NanoBlockchain/MCNanoLarge` | large | 0.420 | 5,818.77 | 2,443.66 |
| `aba-asyn-byz/aba_asyn_byz` | large | 0.392 | 1,319.56 | 517.37 |
| `bcastFolklore/bcastFolklore` | large | 0.298 | 2,119.70 | 631.63 |
| `MultiCarElevator/ElevatorLivenessLarge` | large | 0.239 | 1,879.76 | 449.30 |
| `SpanningTree/SpanTreeTest5Nodes` | large | 0.167 | 676.69 | 113.20 |

`MCKVsnap` is above 1x because of its state count, not its per-state cost.
Under D-005 dive checks it without symmetry reduction, since TLC's symmetry
reduction is unsound with liveness, so it explores 189,664 distinct states
against TLC's 32,293, about 5.9 times as many. Per distinct state dive takes
about 89 µs against TLC's 437 µs, roughly 5x faster (#979).

## Memory

Two parts on the small-class node. The benchmark-set part runs the ten
models of `crates/dive-bench/benchmarks.toml` with `dive-bench` at 1 and 8
workers in both profiles, five repetitions, with TLC at `-Xmx3g`. TLC's peak
RSS is mostly the JVM's heap sizing, not TLC's minimum, so read the TLC
column as what a user sees with that setting.

The liveness part runs every R1 agreeing model whose report says
`check_kind: temporal`, found by probing each model with a `PROPERTY` once,
three repetitions at one worker under `/usr/bin/time -v`. The second batch of the liveness part was split across the small-class node and one large-shard node of the same type to finish sooner; the record says which runs ran where. `MCKVsnap` (D-005, unreduced symmetry with liveness) and `SpanTreeRandom` (D-002, dive's own seeded random streams) differ from the golden counts exactly as registered in `conformance/expected-divergences.toml`; every other model matches its golden. In the benchmark set, the `--compat=tlc` profile at one worker is slower than TLC on `GameOfLife` (20.58 s against 12.08 s) and `TLCSailfish1` (72.70 s against 51.69 s); the default profile is faster than TLC on every model.

### Benchmark set peak RSS (MiB, median of 5)

| Model | Profile | Workers | dive RSS | dive range | TLC RSS | dive s | TLC s |
|---|---|---:|---:|---|---:|---:|---:|
| BufferedRandomAccessFile | default | 1 | 37 | 36–37 | 1,104 | 18.98 | 50.81 |
| BufferedRandomAccessFile | tlc | 1 | 41 | 40–41 | 1,104 | 22.04 | 50.81 |
| BufferedRandomAccessFile | default | 8 | 147 | 141–168 | 1,170 | 2.67 | 8.09 |
| BufferedRandomAccessFile | tlc | 8 | 154 | 151–163 | 1,170 | 3.08 | 8.09 |
| CoffeeCan1000BeansSafety | default | 1 | 110 | 110–111 | 800 | 1.37 | 2.25 |
| CoffeeCan1000BeansSafety | tlc | 1 | 111 | 110–111 | 800 | 1.37 | 2.25 |
| CoffeeCan1000BeansSafety | default | 8 | 215 | 203–217 | 795 | 0.82 | 1.67 |
| CoffeeCan1000BeansSafety | tlc | 8 | 221 | 216–247 | 795 | 0.87 | 1.67 |
| EWD998PCal | default | 1 | 50 | 50–50 | 1,209 | 15.12 | 41.94 |
| EWD998PCal | tlc | 1 | 50 | 49–51 | 1,209 | 16.42 | 41.94 |
| EWD998PCal | default | 8 | 212 | 209–223 | 1,449 | 2.62 | 7.59 |
| EWD998PCal | tlc | 8 | 210 | 204–221 | 1,449 | 2.85 | 7.59 |
| GameOfLife | default | 1 | 57 | 57–57 | 1,108 | 10.83 | 12.08 |
| GameOfLife | tlc | 1 | 57 | 56–57 | 1,108 | 20.58 | 12.08 |
| GameOfLife | default | 8 | 167 | 137–167 | 1,107 | 1.70 | 2.65 |
| GameOfLife | tlc | 8 | 150 | 126–153 | 1,107 | 2.95 | 2.65 |
| MCBakery | default | 1 | 89 | 88–91 | 1,275 | 7.74 | 11.12 |
| MCBakery | tlc | 1 | 91 | 90–92 | 1,275 | 7.74 | 11.12 |
| MCBakery | default | 8 | 210 | 209–228 | 1,292 | 5.86 | 7.79 |
| MCBakery | tlc | 8 | 216 | 195–252 | 1,292 | 5.91 | 7.79 |
| MCDisruptor_MPMC | default | 1 | 43 | 43–43 | 1,073 | 1.11 | 4.46 |
| MCDisruptor_MPMC | tlc | 1 | 43 | 43–43 | 1,073 | 1.15 | 4.46 |
| MCDisruptor_MPMC | default | 8 | 161 | 140–175 | 1,081 | 0.37 | 1.57 |
| MCDisruptor_MPMC | tlc | 8 | 143 | 135–143 | 1,081 | 0.37 | 1.57 |
| MCLamportMutex | default | 1 | 109 | 108–109 | 1,275 | 12.48 | 17.17 |
| MCLamportMutex | tlc | 1 | 111 | 111–111 | 1,275 | 13.31 | 17.17 |
| MCLamportMutex | default | 8 | 286 | 277–289 | 1,329 | 3.10 | 4.38 |
| MCLamportMutex | tlc | 8 | 271 | 269–288 | 1,329 | 3.43 | 4.38 |
| MCbtree | default | 1 | 58 | 58–59 | 1,167 | 5.53 | 40.84 |
| MCbtree | tlc | 1 | 56 | 56–56 | 1,167 | 5.73 | 40.84 |
| MCbtree | default | 8 | 207 | 191–221 | 1,286 | 1.20 | 7.79 |
| MCbtree | tlc | 8 | 197 | 189–201 | 1,286 | 1.25 | 7.79 |
| SimpleRegular | default | 1 | 58 | 58–58 | 1,136 | 3.50 | 5.45 |
| SimpleRegular | tlc | 1 | 57 | 57–57 | 1,136 | 3.50 | 5.45 |
| SimpleRegular | default | 8 | 217 | 213–239 | 1,206 | 0.84 | 2.32 |
| SimpleRegular | tlc | 8 | 233 | 215–237 | 1,206 | 0.84 | 2.32 |
| TLCSailfish1 | default | 1 | 77 | 76–77 | 1,148 | 39.57 | 51.69 |
| TLCSailfish1 | tlc | 1 | 77 | 77–77 | 1,148 | 72.70 | 51.69 |
| TLCSailfish1 | default | 8 | 221 | 202–246 | 1,349 | 5.61 | 8.12 |
| TLCSailfish1 | tlc | 8 | 228 | 208–237 | 1,349 | 10.20 | 8.12 |

### Liveness capture RSS (median of 3)

| Model | Distinct | Peak RSS MiB | Observed max MiB | Store lower bound MiB | Bytes/state | Wall s | Agrees |
|---|---:|---:|---:|---:|---:|---:|---|
| `CoffeeCan/CoffeeCan1000Beans` | 501500 | 283 | 284 | 46 | 591.6 | 9.07 | yes |
| `CoffeeCan/CoffeeCan100Beans` | 5150 | 47 | 47 | 2 | 9526.6 | 0.10 | yes |
| `CoffeeCan/CoffeeCan3000Beans` | 4504500 | 2,707 | 2,708 | 641 | 630.2 | 88.50 | yes |
| `DiningPhilosophers/DiningPhilosophers` | 67 | 31 | 31 | 1 | 486690.4 | 0.02 | yes |
| `Disruptor/MCDisruptor_MPMC_liveliness` | 14365 | 47 | 47 | 2 | 3396.6 | 1.50 | yes |
| `Disruptor/MCDisruptor_SPMC` | 8496 | 43 | 43 | 2 | 5326.3 | 0.44 | yes |
| `FiniteMonotonic/MCCRDT` | 25000 | 83 | 83 | 13 | 3477.0 | 10.07 | yes |
| `FiniteMonotonic/MCReplicatedLog` | 1363 | 37 | 38 | 1 | 28792.2 | 0.14 | yes |
| `FlashProtocol/MCFlashWithMutexLive` | 8096 | 73 | 73 | 1 | 9419.9 | 1.04 | yes |
| `Huang/Huang` | 81256 | 97 | 98 | 6 | 1245.6 | 36.71 | yes |
| `KeyValueStore/MCKVsnap` | 189664 | 122 | 123 | 4 | 675.8 | 16.83 | registered D-005 |
| `LoopInvariance/MCBinarySearch` | 27953 | 131 | 130 | 1 | 4909.7 | 1.04 | yes |
| `LoopInvariance/MCQuicksort` | 4548 | 44 | 45 | 1 | 10149.9 | 5.76 | yes |
| `MisraReachability/MCParReach` | 393 | 99 | 99 | 1 | 264718.3 | 3.24 | yes |
| `MisraReachability/MCReachable` | 8 | 63 | 63 | 1 | 8294912.0 | 0.45 | yes |
| `Moving_Cat_Puzzle/CatEvenBoxes` | 48 | 23 | 23 | 1 | 503466.7 | 0.01 | yes |
| `Moving_Cat_Puzzle/CatOddBoxes` | 30 | 23 | 23 | 1 | 802269.9 | 0.01 | yes |
| `MultiCarElevator/ElevatorLivenessLarge` | 50653 | 117 | 118 | 1 | 2428.2 | 450.75 | yes |
| `MultiCarElevator/ElevatorLivenessMedium` | 4122 | 49 | 49 | 1 | 12408.2 | 6.68 | yes |
| `Prisoners/Prisoners` | 214 | 27 | 27 | 1 | 133368.8 | 0.01 | yes |
| `Prisoners_Single_Switch/Prisoner` | 16 | 25 | 25 | 1 | 1652480.0 | 0.01 | yes |
| `Prisoners_Single_Switch/PrisonerLightUnknown` | 62 | 25 | 25 | 1 | 420764.9 | 0.01 | yes |
| `Prisoners_Single_Switch/PrisonerSolo` | 2 | 25 | 25 | 1 | 13010944.0 | 0.01 | yes |
| `Prisoners_Single_Switch/PrisonerSoloLightUnknown` | 4 | 25 | 25 | 1 | 6497280.0 | 0.01 | yes |
| `ReadersWriters/MC` | 21527 | 51 | 51 | 2 | 2471.8 | 1.87 | yes |
| `SDP_Verification/SDP_Attack_New_Solution_Spec/MC` | 2881 | 53 | 53 | 1 | 19426.5 | 0.48 | yes |
| `SingleLaneBridge/MC` | 3605 | 51 | 51 | 2 | 14892.2 | 0.68 | yes |
| `SpanningTree/SpanTree` | 1236 | 31 | 31 | 1 | 26438.4 | 0.22 | yes |
| `SpanningTree/SpanTreeRandom` | 22040 | 73 | 73 | 2 | 3454.8 | 7.08 | registered D-002 |
| `SpanningTree/SpanTreeTest4Nodes` | 3984 | 35 | 35 | 1 | 9264.3 | 0.42 | yes |
| `SpanningTree/SpanTreeTest5Nodes` | 410112 | 239 | 239 | 2 | 610.5 | 113.17 | yes |
| `SpecifyingSystems/AdvancedExamples/MCInnerSequential` | 3528 | 39 | 39 | 2 | 11673.8 | 0.26 | yes |
| `SpecifyingSystems/AdvancedExamples/MCInnerSerial` | 195 | 43 | 44 | 2 | 229733.1 | 4.31 | yes |
| `SpecifyingSystems/Liveness/LiveHourClock` | 12 | 23 | 23 | 1 | 2000213.3 | 0.01 | yes |
| `SpecifyingSystems/Liveness/MCLiveInternalMemory` | 4408 | 37 | 37 | 1 | 8767.2 | 0.19 | yes |
| `SpecifyingSystems/Liveness/MCLiveWriteThroughCache` | 5196 | 45 | 45 | 1 | 9026.0 | 1.07 | yes |
| `SpecifyingSystems/RealTime/MCRealTimeHourClock` | 216 | 25 | 25 | 1 | 121950.8 | 0.01 | yes |
| `SpecifyingSystems/TLC/MCAlternatingBit` | 240 | 27 | 27 | 1 | 116957.9 | 0.03 | yes |
| `YoYo/MCYoYoNoPruning` | 60 | 31 | 32 | 1 | 548249.6 | 0.05 | yes |
| `YoYo/MCYoYoPruning` | 102 | 43 | 44 | 1 | 446745.1 | 0.07 | yes |
| `YoYo/YoYoAllGraphs` | 26731 | 73 | 74 | 6 | 2872.5 | 4.08 | yes |
| `aba-asyn-byz/aba_asyn_byz` | 5843977 | 4,523 | 4,524 | 1 | 811.6 | 517.27 | yes |
| `acp/ACP_NB_TLC` | 4284 | 47 | 47 | 1 | 11585.3 | 0.36 | yes |
| `allocator/AllocatorImplementation` | 17701 | 65 | 66 | 2 | 3854.7 | 11.38 | yes |
| `allocator/AllocatorRefinement` | 1690 | 45 | 46 | 1 | 28117.0 | 0.49 | yes |
| `allocator/SchedulingAllocator` | 1690 | 41 | 41 | 1 | 25434.0 | 0.29 | yes |
| `allocator/SimpleAllocator` | 400 | 31 | 31 | 1 | 81387.5 | 0.10 | yes |
| `bcastByz/bcastByz` | 14424 | 63 | 64 | 2 | 4590.7 | 4.45 | yes |
| `bcastByz/bcastByzNoBcast` | 45 | 37 | 38 | 1 | 869262.2 | 0.05 | yes |
| `bcastFolklore/bcastFolklore` | 501552 | 1,020 | 1,021 | 10 | 2132.9 | 632.14 | yes |
| `byzpaxos/Consensus` | 4 | 27 | 27 | 1 | 7181312.0 | 0.02 | yes |
| `cf1s-folklore/cf1s_folklore` | 2057174 | 1,266 | 1,267 | 1 | 645.4 | 177.58 | yes |
| `chang_roberts/MCChangRoberts` | 137 | 27 | 27 | 1 | 207520.7 | 0.02 | yes |
| `dijkstra-mutex/DijkstraMutex.toolbox/LSpec-model/MC` | 90882 | 103 | 103 | 1 | 1184.1 | 6.97 | yes |
| `ewd426/TokenRing` | 46656 | 61 | 61 | 8 | 1374.6 | 1.24 | yes |
| `ewd687a/MCEWD687a` | 18028 | 65 | 65 | 1 | 3765.0 | 3.07 | yes |
| `ewd840/EWD840` | 302 | 71 | 71 | 1 | 246356.8 | 0.04 | yes |
| `ewd840/SyncTerminationDetection` | 129 | 27 | 27 | 1 | 216961.0 | 0.01 | yes |
| `ewd998/AsyncTerminationDetection` | 4097 | 35 | 35 | 1 | 8905.8 | 0.12 | yes |
| `ewd998/EWD998Chan` | 1524022 | 874 | 875 | 2 | 601.5 | 921.09 | yes |
| `glowingRaccoon/product` | 305 | 69 | 69 | 1 | 236856.2 | 0.10 | yes |
| `glowingRaccoon/stages` | 83 | 25 | 25 | 1 | 313566.1 | 0.01 | yes |
| `nbacg_guer01/nbacg_guer01` | 24922 | 63 | 64 | 1 | 2662.5 | 0.86 | yes |
| `tcp/MCtcp` | 1182 | 39 | 39 | 1 | 34673.9 | 0.11 | yes |

## Exact versus fingerprint store

`dive check --store exact` against the default fingerprint store, three
repetitions with the store order alternating, on the ten benchmark-set models
and five liveness models. Both stores give the golden verdict, counts and
depth on every model. The exact store keeps every state, so it costs memory
on the models with many states per second of work (`MCbtree` 2.68x,
`MCBakery` 2.54x, `MCLamportMutex` 2.45x). On the liveness models, where the
behavior graph dominates memory, it costs 0.95x to 1.15x.

### Exact versus fingerprint store (median of 3)

| Model | fp s | exact s | time exact/fp | fp RSS MiB | exact RSS MiB | RSS exact/fp | Agrees |
|---|---:|---:|---:|---:|---:|---:|---|
| `CoffeeCan/CoffeeCan1000Beans (safety-only config: MaxBeanCount = 1000, INVARIANTS TypeInvariant)` | 1.31 | 1.70 | 1.295 | 110 | 215 | 1.942 | yes |
| `Bakery-Boulangerie/MCBakery` | 7.67 | 8.94 | 1.165 | 91 | 231 | 2.542 | yes |
| `CoffeeCan/CoffeeCan1000Beans` | 9.04 | 9.42 | 1.042 | 283 | 305 | 1.077 | yes |
| `Disruptor/MCDisruptor_MPMC` | 1.07 | 1.33 | 1.244 | 42 | 63 | 1.477 | yes |
| `FlashProtocol/MCFlashWithMutexLive` | 1.04 | 1.09 | 1.046 | 73 | 69 | 0.95 | yes |
| `GameOfLife/GameOfLife` | 10.78 | 10.93 | 1.014 | 57 | 68 | 1.209 | yes |
| `MultiCarElevator/ElevatorLivenessMedium` | 6.69 | 6.67 | 0.997 | 49 | 51 | 1.042 | yes |
| `SpecifyingSystems/Liveness/MCLiveInternalMemory` | 0.19 | 0.20 | 1.045 | 37 | 37 | 0.997 | yes |
| `TeachingConcurrency/SimpleRegular` | 3.46 | 3.96 | 1.142 | 58 | 117 | 2.004 | yes |
| `aba-asyn-byz/aba_asyn_byz` | 516.98 | 530.52 | 1.026 | 4,523 | 5,193 | 1.148 | yes |
| `braf/BufferedRandomAccessFile` | 18.94 | 18.95 | 1.001 | 41 | 41 | 0.999 | yes |
| `btree/MCbtree` | 5.47 | 6.51 | 1.189 | 56 | 150 | 2.681 | yes |
| `dag-consensus/TLCSailfish1` | 39.65 | 39.86 | 1.005 | 77 | 99 | 1.291 | yes |
| `ewd998/EWD998PCal` | 15.06 | 16.18 | 1.075 | 49 | 109 | 2.223 | yes |
| `lamport_mutex/MCLamportMutex` | 12.41 | 14.22 | 1.146 | 108 | 266 | 2.451 | yes |

## Worker scaling

`dive-bench --profile default --workers 1,4,8 --reps 3 --timeout 3600 --xmx 24g` on the 47
large-class models that have a success golden (`dive-bench` needs one, so
`MCCheckpointCoordinationFailure` is left out), bound with `numactl
--cpunodebind=0 --membind=0`, so 8 workers have all 48 CPUs. The speed-up is
the median over the three repetitions of the 1-worker time divided by the
same repetition's N-worker time. To finish in time, the four speed-measurement shard nodes
ran the first repetitions and five spot nodes of the same type, image and
setup ran the rest, with the binary copied and checked by SHA-256; the record
names the node of every repetition.

- **Safety models scale.** The median 8-worker speed-up over all 47 models is
  4.04x (2.26x at 4 workers); the best are `PConProof` 7.64x,
  `FastPaxos` 7.32x and `bosco` 7.16x.
- **Temporal models mostly do not.** Of the 17 models with temporal
  properties that run at 8 workers, 15 stay between 0.50x and 1.40x of their
  1-worker time (`Huang` reaches 1.73x and `MCInnerSerial` 2.08x). Six get slower as workers
  are added (`CoffeeCan1000Beans` 0.50x, `CoffeeCan3000Beans` 0.52x,
  `LSpec-model` 0.80x, `cf1s_folklore` 0.82x, `aba_asyn_byz` 0.84x,
  `MCFlashWithMutexLive` 0.90x), and the reports name 8 workers used with no
  sequential reason. Issue #1028.
- **Refusals.** `MCKVsnap` and `SpanTreeRandom` refuse at 4 and 8 workers
  (exit 3, "temporal properties: concrete pool capture with effects or
  runtime names"), #1020. At one worker both succeed.
- **SlushLarge at one worker** stops at `dive-bench`'s 3,570 s time limit in
  all three repetitions, so its speed-ups are lower bounds (1.55x and 2.71x).
  Against the speed measurement's completed 1-worker time on the same binary and host type
  (6,544.78 s) they are 2.84x and 4.97x.
- At one worker `MCCRDT`, `VoteProof`, `MCKVsnap` and `SpanTreeRandom`
  differ from the golden counts exactly as registered in
  `conformance/expected-divergences.toml`; every other run agrees with its
  golden. Generated counts at more than one worker are not compared.

### Worker scaling (median of 3)

| Model | 1 worker s | 4 workers s | 8 workers s | Speed-up 4 | Speed-up 8 | Note |
|---|---:|---:|---:|---:|---:|---|
| `SlushProtocol/SlushLarge` | 3,570.1 (stopped) | 2,307.9 | 1,316.4 | ≥1.55 | ≥2.71 | 1 worker stopped at the 3570 s limit; speed-ups are lower bounds |
| `MultiPaxos-SMR/MultiPaxos_MC` | 2,617.2 | 747.3 | 392.9 | 3.50 | 6.65 |  |
| `NanoBlockchain/MCNanoLarge` | 2,444.4 | 841.5 | 505.0 | 2.90 | 4.84 |  |
| `MultiCarElevator/ElevatorSafetyLarge` | 1,488.1 | 516.7 | 307.0 | 2.88 | 4.86 |  |
| `KeyValueStore/MCKVSSafetyMedium` | 1,037.1 | 353.9 | 197.1 | 2.93 | 5.26 |  |
| `ewd998/EWD998Chan` | 962.0 | 864.8 | 856.7 | 1.11 | 1.12 |  |
| `byzpaxos/PConProof` | 845.6 | 219.4 | 110.8 | 3.85 | 7.64 |  |
| `bcastFolklore/bcastFolklore` | 633.0 | 630.3 | 630.1 | 1.00 | 1.01 |  |
| `MultiCarElevator/ElevatorSafetyMedium` | 520.3 | 172.3 | 99.6 | 3.02 | 5.23 |  |
| `aba-asyn-byz/aba_asyn_byz` | 518.7 | 575.7 | 619.5 | 0.90 | 0.84 |  |
| `MultiCarElevator/ElevatorLivenessLarge` | 449.8 | 450.9 | 450.8 | 1.00 | 1.00 |  |
| `bosco/bosco` | 305.4 | 83.8 | 42.7 | 3.64 | 7.16 |  |
| `dijkstra-mutex/DijkstraMutex.toolbox/Safety-4-processors/MC` | 270.2 | 142.0 | 84.6 | 1.91 | 3.18 |  |
| `cf1s-folklore/cf1s_folklore` | 177.7 | 198.7 | 215.4 | 0.89 | 0.82 |  |
| `Bakery-Boulangerie/MCBoulanger` | 177.6 | 60.9 | 35.5 | 2.92 | 5.00 |  |
| `SlushProtocol/SlushMedium` | 163.8 | 59.5 | 35.6 | 2.75 | 4.60 |  |
| `KeyValueStore/MCKVSSafetySmall` | 115.4 | 40.9 | 24.3 | 2.82 | 4.76 |  |
| `SpanningTree/SpanTreeTest5Nodes` | 113.2 | 111.6 | 113.3 | 1.01 | 1.00 |  |
| `CoffeeCan/CoffeeCan3000Beans` | 88.0 | 136.4 | 169.8 | 0.65 | 0.52 |  |
| `CheckpointCoordination/MCCheckpointCoordination` | 87.9 | 27.5 | 14.6 | 3.22 | 6.02 |  |
| `transaction_commit/PaxosCommit` | 77.9 | 24.4 | 13.5 | 3.19 | 5.79 |  |
| `PaxosHowToWinATuringAward/MCPaxosSmall` | 55.4 | 15.2 | 7.8 | 3.66 | 7.09 |  |
| `dag-consensus/TLCSailfish1` | 39.6 | 10.9 | 5.6 | 3.63 | 7.07 |  |
| `Huang/Huang` | 36.8 | 23.7 | 21.2 | 1.55 | 1.73 |  |
| `ewd998/EWD998Small` | 21.1 | 8.8 | 5.1 | 2.41 | 4.13 |  |
| `braf/BufferedRandomAccessFile` | 19.0 | 5.2 | 2.7 | 3.69 | 7.11 |  |
| `SimplifiedFastPaxos/FastPaxos` | 18.1 | 4.8 | 2.5 | 3.75 | 7.32 |  |
| `KeyValueStore/MCKVsnap` | 16.9 | refused | refused |  |  | 1 worker: registered divergence; refused at 4 and 8 workers (#1020) |
| `ewd998/EWD998PCal` | 15.1 | 4.8 | 2.6 | 3.17 | 5.80 |  |
| `lamport_mutex/MCLamportMutex` | 12.5 | 5.5 | 3.1 | 2.26 | 4.04 |  |
| `GameOfLife/GameOfLife` | 10.8 | 3.0 | 1.7 | 3.57 | 6.40 |  |
| `FiniteMonotonic/MCCRDT` | 10.1 | 7.6 | 7.2 | 1.33 | 1.40 | 1 worker: registered divergence |
| `MultiPaxos-SMR/MultiPaxos_MC_small` | 9.4 | 3.0 | 1.6 | 3.20 | 5.74 |  |
| `CoffeeCan/CoffeeCan1000Beans` | 9.1 | 14.5 | 18.4 | 0.62 | 0.50 |  |
| `NanoBlockchain/MCNanoMedium` | 8.8 | 2.9 | 1.7 | 3.05 | 5.29 |  |
| `Bakery-Boulangerie/MCBakery` | 7.7 | 6.3 | 5.8 | 1.23 | 1.32 |  |
| `SpanningTree/SpanTreeRandom` | 7.1 | refused | refused |  |  | 1 worker: registered divergence; refused at 4 and 8 workers (#1020) |
| `dijkstra-mutex/DijkstraMutex.toolbox/LSpec-model/MC` | 7.0 | 8.0 | 8.8 | 0.88 | 0.80 |  |
| `MultiCarElevator/ElevatorLivenessMedium` | 6.8 | 6.8 | 6.8 | 1.00 | 0.99 |  |
| `btree/MCbtree` | 5.5 | 2.1 | 1.2 | 2.63 | 4.51 |  |
| `bcastByz/bcastByz` | 4.5 | 4.0 | 3.9 | 1.14 | 1.17 |  |
| `SpecifyingSystems/AdvancedExamples/MCInnerSerial` | 4.4 | 2.5 | 2.1 | 1.71 | 2.08 |  |
| `MisraReachability/MCParReach` | 3.3 | 3.2 | 3.2 | 1.05 | 1.05 |  |
| `LeastCircularSubstring/MCLeastCircularSubstringMedium` | 2.0 | 1.1 | 0.9 | 1.77 | 2.23 |  |
| `FlashProtocol/MCFlashWithMutexLive` | 1.1 | 1.2 | 1.2 | 0.94 | 0.90 |  |
| `byzpaxos/VoteProof` | 0.9 | 0.4 | 0.3 | 2.21 | 3.25 | 1 worker: registered divergence |
| `MisraReachability/MCReachable` | 0.5 | 0.5 | 0.5 | 1.00 | 1.00 |  |

### Open finding: #1028

Temporal checks do not scale with workers, and six models run slower at
8 workers than at 1. The cause is not known yet. The hypotheses below are unverified.

**What was measured.** The scaling record does not mark check kinds, so the split
below uses the memory measurement's liveness list (models whose report said `check_kind:
temporal`). By that list, the 28 safety models have a median 8-worker
speed-up of 5.2x (lowest `MCBakery`, 1.32x) and the 17 temporal models that
ran at 8 workers have a median of 1.00x; 15 of the 17 stay between 0.50x and
1.40x. The 17 temporal models:

| Model | Distinct states | 1 worker s | 4 workers s | 8 workers s | Speed-up 4 | Speed-up 8 | Peak RSS MiB 1w / 8w |
|---|---:|---:|---:|---:|---:|---:|---|
| `CoffeeCan/CoffeeCan1000Beans` | 501,500 | 9.1 | 14.5 | 18.4 | 0.62 | 0.50 | 283 / 283 |
| `CoffeeCan/CoffeeCan3000Beans` | 4,504,500 | 88.0 | 136.4 | 169.8 | 0.65 | 0.52 | 2,707 / 2,713 |
| `dijkstra-mutex/.../LSpec-model/MC` | 90,882 | 7.0 | 8.0 | 8.8 | 0.88 | 0.80 | 102 / 104 |
| `cf1s-folklore/cf1s_folklore` | 2,057,174 | 177.7 | 198.7 | 215.4 | 0.89 | 0.82 | 1,266 / 1,284 |
| `aba-asyn-byz/aba_asyn_byz` | 5,843,977 | 518.7 | 575.7 | 619.5 | 0.90 | 0.84 | 4,522 / 4,573 |
| `FlashProtocol/MCFlashWithMutexLive` | 8,096 | 1.1 | 1.2 | 1.2 | 0.94 | 0.90 | 72 / 78 |
| `MultiCarElevator/ElevatorLivenessMedium` | 4,122 | 6.8 | 6.8 | 6.8 | 1.00 | 0.99 | 50 / 58 |
| `MisraReachability/MCReachable` | 8 | 0.5 | 0.5 | 0.5 | 1.00 | 1.00 | 63 / 63 |
| `MultiCarElevator/ElevatorLivenessLarge` | 50,653 | 449.8 | 450.9 | 450.8 | 1.00 | 1.00 | 112 / 133 |
| `SpanningTree/SpanTreeTest5Nodes` | 410,112 | 113.2 | 111.6 | 113.3 | 1.01 | 1.00 | 238 / 241 |
| `bcastFolklore/bcastFolklore` | 501,552 | 633.0 | 630.3 | 630.1 | 1.00 | 1.01 | 1,036 / 1,033 |
| `MisraReachability/MCParReach` | 393 | 3.3 | 3.2 | 3.2 | 1.05 | 1.05 | 98 / 98 |
| `ewd998/EWD998Chan` | 1,524,022 | 962.0 | 864.8 | 856.7 | 1.11 | 1.12 | 876 / 870 |
| `bcastByz/bcastByz` | 14,424 | 4.5 | 4.0 | 3.9 | 1.14 | 1.17 | 62 / 83 |
| `FiniteMonotonic/MCCRDT` | 25,000 | 10.1 | 7.6 | 7.2 | 1.33 | 1.40 | 82 / 96 |
| `Huang/Huang` | 81,256 | 36.8 | 23.7 | 21.2 | 1.55 | 1.73 | 96 / 121 |
| `SpecifyingSystems/AdvancedExamples/MCInnerSerial` | 195 | 4.4 | 2.5 | 2.1 | 1.71 | 2.08 | 42 / 79 |

`MCKVsnap` and `SpanTreeRandom` are temporal too, but refuse at more than one
worker (#1020). Every run above agrees with its golden.

**What the reports say.** Every 8-worker report has `options.workers` =
`{"requested": 8, "used": 8, "sequential": false, "reason": null}`, so the scaling
record shows `workers_used: [8]` and no `sequential_reason`. The temporal
report's resource limits include `max_expansion_units: 8`, the only hint that
temporal capture works in waves of at most eight source states. `core.metrics`
has `explore_ms`, `expand_ms` and `commit_ms`, which separate the search from
the liveness analysis that follows it (`elapsed_ms` minus `explore_ms`).

**A local observation.** On a laptop (Apple M3, 4 performance and
4 efficiency cores, in use by other work; informational, not comparable with
the bench nodes), dive `95ee21a4` on `CoffeeCan1000Beans`:

| Workers | `elapsed_ms` | `explore_ms` | `expand_ms` | Wall s | System CPU s |
|---:|---:|---:|---:|---:|---:|
| 1 | 4,290 | 1,164 | 0 | 4.30 | 0.03 |
| 2 | 5,923 | 2,760 | 2,159 | 5.93 | 1.44 |
| 4 | 6,333 | 3,178 | 2,560 | 6.34 | 3.29 |
| 8 | 8,229 | 5,054 | 4,415 | 8.23 | 11.00 |

The time outside the search (`elapsed_ms - explore_ms`, about 3.1 s) is the
same at 1 and 8 workers. All of the slowdown is in the search, and kernel time
grows with the worker count.

**Hypotheses, unverified.**

1. *Thread churn in fixed eight-source waves.* With a graph sink, the explorer
   sizes each parallel chunk by `GraphSink::max_expansion_units()`
   (`crates/dive-explore/src/lib.rs`, around the "chunk of states" comment),
   and `NativeSink` returns `MAX_EXPANSION_UNITS = 8`
   (`crates/dive-runtime/src/capture.rs`). Safety runs use
   `workers x 4096` states per chunk instead. Each chunk calls
   `parallel::expand_level` (`crates/dive-explore/src/parallel.rs`), which
   opens a `std::thread::scope`, spawns one thread per worker, allocates a
   fresh claim table and joins. With eight sources and eight workers each
   thread expands about one state. `CoffeeCan1000Beans` then needs about
   62,700 waves, about 500,000 thread spawns at 8 workers. The extra wall time
   divided by (distinct states / 8 x workers) is 17 to 20 µs per worker per
   wave at 8 workers and 19 to 22 µs at 4 workers for all six models that slow
   down, which fits a fixed per-thread cost independent of the model. Models
   with expensive states (`Huang`, `MCInnerSerial`, `MCCRDT`) still gain,
   because one state per thread is then worth the cost. Test: `strace -f -c`
   or `perf record` at 8 workers, looking for `clone3`, `futex`, `mmap` and
   `munmap`; then try a persistent pool or wider waves. Wider waves change the
   documented capture buffer bound ("parallel capture buffers at most eight
   source states" in the checking guide), so they would change documented
   behavior.
2. *Sequential ordered commit.* `NativeSink::record_admitted`
   (`crates/dive-api/src/core_native_graph.rs`) requires sources in discovery
   order and runs on the committing thread: row retention, the fingerprint
   discovery index and edge buffers are single-threaded. This caps the
   speed-up (Amdahl) but should not make more workers slower. Test: the share
   of commit in a 1-worker profile. Note that `commit_ms` reads 0 in these
   reports even at 1 worker, so check how the graph path fills it before
   relying on it.
3. *Sequential liveness analysis after a parallel search.* The final check
   (`GraphCheck::finish_native_with_reader`, called from
   `crates/dive-api/src/core_temporal.rs`) runs on the calling thread;
   `dive-liveness` spawns no threads. Models that stay
   at 1.00x with large wall times for few states (`ElevatorLivenessLarge`,
   450 s for 50,653 states; `bcastFolklore`; `SpanTreeTest5Nodes`;
   `EWD998Chan`) are probably dominated by it. Test: `elapsed_ms - explore_ms`
   at 1 and 8 workers on those models.
4. *Periodic completed-level checks.* In the default profile, `level_done`
   calls the liveness check at completed levels on the committing thread, with
   no expansion running meanwhile (D-014). The schedule depends on distinct counts, so the
   number of checks does not change with workers; it adds serial time but
   should not grow with workers. Test: the same model with `--compat=tlc`
   (final check only) at 1 and 8 workers.
5. *Reporting.* Whatever the cause, a run with 8 workers that is slower than
   with 1 reports nothing about it. The issue asks for either no cost from the
   extra workers or a fallback to one worker with a stated
   `options.workers.reason`.

**Reproduce.** With the oracle and corpora fetched (`cargo run -p
dive-conformance --release -- corpus fetch`), the smallest affected model:

```sh
cargo build --release --locked -p dive-cli --bins
EX=$(ls -d ~/.cache/dive/git/examples/*/)
cd "$EX/specifications/CoffeeCan"
for w in 1 2 4 8; do
  /usr/bin/time -v "$OLDPWD/target/release/dive" check CoffeeCan.tla \
    --config CoffeeCan1000Beans.cfg --workers $w --seed 0 --fp 0 --output json \
    > "/tmp/cc-w$w.json"   # time -v prints wall, system time and peak RSS on stderr
  python3 -c 'import json,sys; r=json.load(open(sys.argv[1])); m=r["core"]["metrics"]; print(sys.argv[1], r["verdict"], r["check_kind"], r["elapsed_ms"], round(m["explore_ms"]), round(m["expand_ms"]), r["options"]["workers"])' "/tmp/cc-w$w.json"
done
```

On macOS use `/usr/bin/time -l`. The bench-node figures came from
`dive-bench --profile default --workers 1,4,8 --reps 3` under `numactl
--cpunodebind=0 --membind=0` (see
[`scripts/perf/release/`](../../scripts/perf/release/README.md)). To
profile, build symbols with `cargo build --profile profiling -p dive-cli
--bins` (binary in `target/profiling/dive`) and run, on Linux,
`perf record -g --call-graph dwarf target/profiling/dive check ...
--workers 8` followed by `perf report`, or `strace -f -c -o syscalls.txt`
for system-call counts. Compare against `--workers 1` and, for hypothesis 4,
against `--compat=tlc`.

## Startup, simulation and trace validation

Fifty fresh processes each, timed with `perf_counter` around
`subprocess.run`:

| Command | Median ms | p90 ms | Max ms |
|---|---:|---:|---:|
| `dive --version` | 1.49 | 1.70 | 1.86 |
| `dive parse Counter.tla` | 1.29 | 1.71 | 1.82 |
| `dive check Counter.tla` (peak RSS 16.5 MiB) | 4.88 | 5.22 | 5.53 |

Trace validation, `dive validate-trace` on a counterexample produced once:

| Trace | Runs | States | Median ms | Result |
|---|---:|---:|---:|---|
| `DieHard` | 10 | 7 | 6.72 | valid ×10 |
| `SlidingPuzzles` | 10 | 117 | 46.88 | valid ×10 |
| `MCRealTimeHourClock` (lasso) | 10 | 2 | 7.57 | valid ×10 |
| `bosco`, `--sample`, simulation traces | 100 | 101 | 82.56 | valid ×100 |

Simulation (`--traces 10000 --depth 100 --time-limit 600s --max-memory 16g`):

- `bosco` stopped at the 16 GiB memory budget in all three repetitions,
  after about 590 s and 9,901 started traces: 16.8 traces per second and
  3,474 states per second. Peak RSS from `time -v` was 30.9 GB, about 1.8x
  the budget, because the retained traces are published after the stop.
  Memory grows with the number of traces. Issue #1024.
- `MCLamportMutex` reports a constraint-pruned deadlock on its first trace
  (exit 11, 25 ms), as documented for simulation, so it gives no throughput
  figure. The 100 `--sample` traces therefore come from bosco.

## Stop latency

Latency is the wall time minus the load time (a `--max-states 1` run at the
same worker count) minus the limit. Five repetitions per cell.

### Time budgets

| Model | Workers | Limit | Wall s | Latency ms, median | Latency ms, max | Wall − limit ms, max | Phase |
|---|---:|---|---:|---:|---:|---:|---|
| `MultiCarElevator/ElevatorSafetyLarge` | 1 | 5s | 5.02 | 7.5 | 8.4 | 20.7 | safety-search |
| `MultiCarElevator/ElevatorSafetyLarge` | 1 | 30s | 30.02 | 12.4 | 13.9 | 26.2 | safety-search |
| `MultiCarElevator/ElevatorSafetyLarge` | 1 | 120s | 120.05 | 40.3 | 42.7 | 55.0 | safety-search |
| `MultiCarElevator/ElevatorSafetyLarge` | 8 | 5s | 5.03 | 20.2 | 22.3 | 34.5 | safety-search |
| `MultiCarElevator/ElevatorSafetyLarge` | 8 | 30s | 30.05 | 33.2 | 36.2 | 48.4 | safety-search |
| `MultiCarElevator/ElevatorSafetyLarge` | 8 | 120s | 120.04 | 27.2 | 169.8 | 182.0 | safety-search |
| `SlushProtocol/SlushLarge` | 1 | 5s | 5.02 | 5.8 | 7.0 | 20.6 | safety-search |
| `SlushProtocol/SlushLarge` | 1 | 30s | 30.02 | 4.1 | 4.3 | 18.0 | safety-search |
| `SlushProtocol/SlushLarge` | 1 | 120s | 120.04 | 21.9 | 23.8 | 37.5 | safety-search |
| `SlushProtocol/SlushLarge` | 8 | 5s | 5.04 | 22.4 | 24.6 | 38.6 | safety-search |
| `SlushProtocol/SlushLarge` | 8 | 30s | 30.05 | 33.4 | 36.8 | 50.8 | safety-search |
| `SlushProtocol/SlushLarge` | 8 | 120s | 120.04 | 25.4 | 52.4 | 66.4 | safety-search |
| `MultiCarElevator/ElevatorLivenessLarge` | 1 | 5s | 5.03 | -43.2 | -42.2 | 27.9 | temporal-analysis |
| `MultiCarElevator/ElevatorLivenessLarge` | 1 | 30s | 30.02 | -47.6 | -46.7 | 23.4 | temporal-analysis |
| `MultiCarElevator/ElevatorLivenessLarge` | 1 | 120s | 120.04 | -31.5 | -29.7 | 40.4 | temporal-analysis |
| `MultiCarElevator/ElevatorLivenessLarge` | 8 | 5s | 5.03 | -37.2 | -36.9 | 30.2 | temporal-analysis |
| `MultiCarElevator/ElevatorLivenessLarge` | 8 | 30s | 30.02 | -43.7 | -43.3 | 23.9 | temporal-analysis |
| `MultiCarElevator/ElevatorLivenessLarge` | 8 | 120s | 120.04 | -30.1 | -26.2 | 40.9 | temporal-analysis |

Every run exits 5 with a `stopped` report. For `ElevatorLivenessLarge` the `--max-states 1` load run itself stops in graph capture and then runs the temporal stop path (about 68 ms, against about 13 ms for the safety models), so subtracting it overstates the load and the computed latency is negative. Wall time minus the limit, which includes the load, is the upper bound for that model: at most 41 ms.

### State, depth and temporal ceilings

| Model | Workers | Ceiling | Wall s | Distinct | Depth | Overshoot | Phase |
|---|---:|---|---:|---:|---:|---:|---|
| `SlushProtocol/SlushLarge` | 1 | --max-states 1000000 | 11.00 | 1,000,000 | 18 | 0 | safety-search |
| `SlushProtocol/SlushLarge` | 1 | --max-states 10000000 | 144.14 | 10,000,000 | 23 | 0 | safety-search |
| `SlushProtocol/SlushLarge` | 1 | --max-depth 10 | 0.12 | 15,264 | 10 | 0 | safety-search |
| `SlushProtocol/SlushLarge` | 1 | --max-depth 30 | 1,421.82 | 70,238,348 | 30 | 0 | safety-search |
| `SlushProtocol/SlushLarge` | 8 | --max-states 1000000 | 2.67 | 1,000,000 | 18 | 0 | safety-search |
| `SlushProtocol/SlushLarge` | 8 | --max-states 10000000 | 33.28 | 10,000,000 | 23 | 0 | safety-search |
| `SlushProtocol/SlushLarge` | 8 | --max-depth 10 | 0.08 | 15,264 | 10 | 0 | safety-search |
| `SlushProtocol/SlushLarge` | 8 | --max-depth 30 | 306.84 | 70,238,348 | 30 | 0 | safety-search |
| `MultiCarElevator/ElevatorLivenessLarge` | 1 | --max-states 20000 --store exact | 0.46 | 20,000 | 15 | 0 | graph-capture |
| `MultiCarElevator/ElevatorLivenessLarge` | 1 | --max-depth 20 --store exact | 0.66 | 30,605 | 20 | 0 | graph-capture |

Three repetitions each; every run exits 5 with distinct states or depth exactly at the ceiling. The temporal ceilings stop in graph capture and make no claim. The time from the last progress report to exit was not measured: these runs used `--output json` without progress output.

## macOS arm64 spot check (informational)

This ran on a laptop: a MacBook Air (Mac15,13) with an Apple M3 (4 performance and
4 efficiency cores) and 24 GB of memory, on macOS 26.5. The laptop was in use
by other work at the time (one-minute load average 2.9 to 12.6 during the
runs), so the figures are indicative only and are not comparable with the
bench nodes. The selection is reduced: six benchmark-set models at 1 and
4 workers, default profile, three repetitions, with pinned TLC at `-Xmx3g`.
Every run agreed with its golden.

| Model | dive 1w s | TLC 1w s | dive/TLC 1w | dive 4w s | TLC 4w s | dive/TLC 4w | dive RSS MiB 1w | TLC RSS MiB 1w |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| CoffeeCan1000BeansSafety | 0.68 | 1.28 | 0.536 | 0.44 | 0.94 | 0.472 | 93 | 580 |
| MCBakery | 3.81 | 6.27 | 0.608 | 2.99 | 4.48 | 0.666 | 88 | 1,238 |
| MCDisruptor_MPMC | 0.56 | 2.52 | 0.221 | 0.24 | 1.16 | 0.204 | 17 | 925 |
| MCLamportMutex | 6.62 | 10.29 | 0.644 | 2.62 | 3.98 | 0.659 | 103 | 1,260 |
| MCbtree | 2.38 | 27.88 | 0.085 | 0.88 | 8.22 | 0.107 | 41 | 1,178 |
| SimpleRegular | 1.63 | 3.26 | 0.501 | 0.62 | 1.62 | 0.383 | 42 | 1,045 |

Geomean dive / TLC: 0.354x at 1 worker, 0.347x at 4 workers.

Startup on the laptop, 50 runs each: `dive --version` 2.75 ms, `dive parse`
2.93 ms and `dive check` of `Counter.tla` 5.44 ms (medians).

## Sail with dive against TLC

It compares the whole
Sail+dive workflow with the standard one: a fresh `java -XX:+UseParallelGC
-jar tla2tools.jar -workers 1 -config <cfg> <module>` per check (pinned
oracle jar and JRE, default heap). Sail ran D.I.V.E. `3d5923a1` in its
isolated worker with its fixed policy (exact store, seed and fingerprint
zero, one worker, explicit bounds). One fresh c7i.8xlarge in us-east-1
(Intel Xeon Platinum 8488C, 32 vCPUs, SMT on, frequency not controlled, no binding, nothing
else running), not the speed bench nodes. Five repetitions per cell, path order
rotated; multiples are TLC time over Sail time within each repetition, then
the median. Every job in every path agreed with its golden on the
holds/refuted class and the distinct-state count, and every Sail refutation
carried an independently validated trace. The Sail page with every table is
`docs/validation/sail-dive-vs-tlc.md` on Sail PR #763's branch.

Sail after the #765 fix (`ce94df74`), median milliseconds:

| Fixture | TLC | dive CLI | Sail execute | Sail MCP (warm) | TLC / execute | TLC / MCP |
|---|---:|---:|---:|---:|---:|---:|
| DieHard (14 states) | 491 | 6.4 | 12.8 | 18.4 | 38.9x | 26.5x |
| EWD840 (302) | 684 | 33.5 | 40.3 | 46.1 | 17.0x | 14.8x |
| MCRealTimeHourClock (216) | 568 | 10.4 | 18.9 | 24.8 | 29.6x | 22.8x |
| MCLiveInternalMemory (4,408) | 1,500 | 144 | 160 | 164 | 9.3x | 9.1x |
| MCDisruptor_MPMC_liveliness (14,365) | 2,648 | 1,032 | 1,087 | 1,101 | 2.4x | 2.4x |
| MCLamportMutex (724,274) | 12,765 | 11,477 | 11,704 | 11,730 | 1.1x | 1.1x |

- **Agent loop.** 100 checks cycling over the four small fixtures: TLC 84.5 s,
  one warm MCP session 6.70 s (12.6x), `sail runtime execute` 6.14 s
  (13.8x), the dive CLI 5.14 s (16.5x). Two concurrent clients on one daemon
  complete 29.7 jobs/s against 2.2 for two concurrent TLC processes (13.3x);
  beyond two active executions Sail refuses jobs with `SAIL6201 busy`.
- **Before the fix** (Sail `6592db0c`) a warm MCP call on DieHard took 376 ms
  (1.2x faster than TLC) and the loop 42.7 s (1.8x): every job hashed and
  copied the 76 MB worker, and the socket and MCP paths hashed it once more.
  The fix reuses the verified copy under settled stamps (Sail ADR 0173),
  links mimalloc into the worker (MCLamportMutex worker time 1.17x to 1.02x
  the CLI's) and wakes the service at once for new connections. The first
  job after installing a worker still takes 207 ms, then 12.3 ms.
- **Memory.** Sail jobs peak at 21 to 270 MiB against TLC's 164 MiB to
  3.5 GiB on the same models; a warm `sail serve` holds 12 MiB.
- **Large models** are the speed measurement's engine comparison and were not rerun: the worker
  is now within 2% of the CLI, so the speed measurement's ratios (for example 0.66x on
  MCLamportMutex, 0.26x on CoffeeCan1000Beans) carry over to Sail up to the
  exact-store cost of the store measurement. On this virtualized host the direct
  MCLamportMutex comparison is 1.1x rather than the speed measurement's 1.5x.

## Issues

- #979 and #940: the two models above 1x in the speed measurement, unchanged from the 2026-11-10 record. The unreduced state count under D-005 accounts for #979 (see the note under the speed table).
- #1024: `dive simulate` memory grows with the number of traces, and peak RSS reaches about 1.8x `--max-memory` after the stop (startup measurement).
- #1028: temporal checks do not scale with workers, and six models run slower at 8 workers than at 1 (scaling measurement; brief and hypotheses in [Open finding: #1028](#open-finding-1028)).
- #1020: `MCKVsnap` and `SpanTreeRandom` refuse temporal checking at more than one worker (scaling measurement).

Against the 2026-11-10 R2 record on the same host type, the median per-model change in the dive / TLC ratio is zero and the largest increase is 8% (`MCConsensus`, 0.081x to 0.088x), inside the spread between repetitions. D2 to D7 have no earlier release record, so these figures are their baseline.
