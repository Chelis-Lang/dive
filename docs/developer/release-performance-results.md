# Release performance results

The measured results of the [release performance plan](release-performance-plan.md)
for dive 0.1.0. R2 is the only hard gate. Every other figure is directional:
it is an aim and is recorded so that regressions are visible. Correctness is
never directional: every timed run below agrees with its TLC golden unless
the table says otherwise. The records are compact JSON files in
[`conformance/reports/perf/`](../../conformance/reports/perf/); each section
names its record.

## Summary

| Measurement | Result | Record |
|---|---|---|
| D1, R2 gate | **Pass.** Core / TLC geomean 0.195x over the 169 agreeing models (fixed-set geomean 0.211x), median 0.148x. No model above 2x. Two above 1x: `MCKVsnap` 1.20x (#979) and `AllocatorImplementation` 1.17x (#940). | `2026-11-18-r2-procedure-08a14fba.json` |
| D2, memory | Benchmark set, default profile: dive peaks at 37 to 110 MiB at 1 worker and 147 to 286 MiB at 8, against 795 to 1,449 MiB for TLC at `-Xmx3g`. Liveness capture: 64 temporal models, none stopped; 62 match their golden and 2 match a registered divergence; the largest is `aba_asyn_byz` at 4,523 MiB for 5.8 million states. | `2026-11-18-release-d2-memory-08a14fba.json` |
| D3, exact store | All 15 models agree under both stores. Exact costs 1.00x to 1.30x the fingerprint store's time and 0.95x to 2.68x its peak RSS. | `2026-11-18-release-d3-store-08a14fba.json` |
| D4, worker scaling | **In progress.** The runs at 1, 4 and 8 workers are still timing; a follow-up change adds the record and this section's table. | pending |
| D5, startup and throughput | `dive --version` 1.5 ms, `dive parse` 1.3 ms and a full `dive check` of a small model 4.9 ms (medians of 50). Trace validation 7 to 47 ms per trace, all valid. The bosco simulation stops at its 16 GiB budget (#1024). | `2026-11-18-release-d5-throughput-08a14fba.json` |
| D6, stop latency | Time budgets stop 4 to 40 ms after the limit (median per cell, load time subtracted; worst single run 170 ms). State and depth ceilings stop exactly at the ceiling, with no overshoot. | `2026-11-18-release-d6-stop-latency-08a14fba.json` |
| D7, macOS (informational) | On an Apple M3 laptop, Core / TLC geomean 0.354x at 1 worker and 0.347x at 4 workers over six benchmark models, all agreeing. | `2026-11-18-release-d7-macos-08a14fba.json` |

## Source, machine and method

| Item | Value |
|---|---|
| Source | `08a14fbac83ff71a3247623fb10bf0584bc83be5` (`main`, dive 0.1.0 candidate) |
| dive binary | `cargo build --release --locked -p dive-cli --bins` on one node, no `RUSTFLAGS`, copied to the others; SHA-256 `97675f9d446dbd188684f9f519e1313a86ae7e7faf57a7964d42d1115498e014` |
| Oracle | TLC `tlc-2026.10.06.014338`, pinned JRE, `--xmx 24g` (D2's TLC column: `-Xmx3g`) |
| Corpus | tlaplus/Examples `6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f` |
| Nodes | c7i.metal-24xl in us-east-1: Intel Xeon Platinum 8488C, SMT off, turbo off, 2.4 GHz fixed, performance governor; one NUMA node with 48 CPUs; gp3 at 16000 IOPS and 1000 MiB/s; Ubuntu 24.04.5 LTS, kernel 7.0.0-1017-aws, THP `madvise` |
| Binding | `numactl --cpunodebind=0 --membind=0` for every timed run (all 48 CPUs) |
| Environment | `env -i` with only `PATH`, `HOME` and `DIVE_CACHE_DIR`; page cache warmed before timing; one checker at a time per node |
| Raw output | dive evidence archive bucket (us-east-1), prefix `release-perf/2026-11-18/`, cited by SHA-256 in each record |

Five on-demand nodes ran D1. One ran D1's small class and then D5, D3, D6 and
D2. Four each ran one D1 large shard and then D4 on that shard's models.
Large models were sharded by longest processing time on the golden TLC times,
and each node timed TLC and Core on its own shard. D4's later repetitions run on additional nodes of the same type, image and setup; the D4 record will say which node ran each repetition.

The binary was built once and copied, because independent builds of the same
source on different nodes were not byte-identical. Every node ran the binary
with the SHA-256 above.

Each run is checked against its TLC golden: verdict, distinct and generated
counts at one worker, depth, violated property and exit code. A run that
stops at its time or memory budget is reported as stopped. It is never
dropped or retried until it fits.

## D1: R2 gate

The [R2 measurement procedure](performance.md#r2-measurement-procedure),
driven by a script around `dive-conformance` while #983 is open. A `--jobs 1`
TLC pass over the 175-model selection, taken before any Core run, put 127
models in the small class (TLC under 10 s) and 48 in the large class.

- **Small class** (121 agreeing models): six repetitions in one session,
  alternating which checker runs first. The ratio is the median of six.
  Geomean 0.161x.
- **Large class** (48 models): one pass each. Two models landed in a band
  near 1x and got three fresh back-to-back pairs: `MCKVSSafetyMedium` (first
  pass 0.91x, pairs 0.90x, 0.91x, 0.90x) and `EWD998Chan` (0.98x; pairs
  1.02x, 0.98x, 0.99x). Geomean 0.315x.
- **Excluded:** six small models that Core refuses by name (unsupported
  features). They count as 2x in the fixed-set geomean of 0.211x.

The overall geomean is 0.195x, the same as the 2026-11-10 record to three
places, although 114 per-model ratios moved by more than 0.0001. The two
models above 1x are the same two as in that record and already have issues.

### D1 models above 0.9x and the largest models

| Model | Class | Core / TLC | TLC s | Core s |
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

## D2: memory

Two parts on the small-class node. The benchmark-set part runs the ten
models of `crates/dive-bench/benchmarks.toml` with `dive-bench` at 1 and 8
workers in both profiles, five repetitions, with TLC at `-Xmx3g`. TLC's peak
RSS is mostly the JVM's heap sizing, not TLC's minimum, so read the TLC
column as what a user sees with that setting.

The liveness part runs every R1 agreeing model whose report says
`check_kind: temporal`, found by probing each model with a `PROPERTY` once,
three repetitions at one worker under `/usr/bin/time -v`. The second batch of the liveness part was split across the small-class node and one large-shard node of the same type to finish sooner; the record says which runs ran where. `MCKVsnap` (D-005, unreduced symmetry with liveness) and `SpanTreeRandom` (D-002, dive's own seeded random streams) differ from the golden counts exactly as registered in `conformance/expected-divergences.toml`; every other model matches its golden. In the benchmark set, the `--compat=tlc` profile at one worker is slower than TLC on `GameOfLife` (20.58 s against 12.08 s) and `TLCSailfish1` (72.70 s against 51.69 s); the default profile is faster than TLC on every model.

### D2 benchmark set peak RSS (MiB, median of 5)

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

### D2 liveness capture RSS (median of 3)

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

## D3: exact versus fingerprint store

`dive check --store exact` against the default fingerprint store, three
repetitions with the store order alternating, on the ten benchmark-set models
and five liveness models. Both stores give the golden verdict, counts and
depth on every model. The exact store keeps every state, so it costs memory
on the models with many states per second of work (`MCbtree` 2.68x,
`MCBakery` 2.54x, `MCLamportMutex` 2.45x). On the liveness models, where the
behaviour graph dominates memory, it costs 0.95x to 1.15x.

### D3 exact versus fingerprint store (median of 3)

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

## D4: worker scaling

**In progress.** 1, 4 and 8 workers on the 47 large-class models that have a success golden (`dive-bench` needs one, so `MCCheckpointCoordinationFailure` is left out), three repetitions, `--timeout 3600`. The record and table land in a follow-up change.

## D5: startup, simulation and trace validation

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

## D6: stop latency

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

## D7: macOS arm64 spot check (informational)

AWS had no mac capacity, so by owner direction this ran on the owner's
laptop: a MacBook Air (Mac15,13) with an Apple M3 (4 performance and
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

## Issues

- #979 and #940: the two models above 1x in D1, unchanged from the 2026-11-10 record.
- #1024: `dive simulate` memory grows with the number of traces, and peak RSS reaches about 1.8x `--max-memory` after the stop (D5).

Against the 2026-11-10 R2 record on the same host type, the median per-model change in the Core / TLC ratio is zero and the largest increase is 8% (`MCConsensus`, 0.081x to 0.088x), inside the spread between repetitions. D2 to D7 have no earlier release record, so these figures are their baseline.
