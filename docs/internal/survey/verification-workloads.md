# Verification workloads and peer comparisons

Dive's release benchmark measures the cost of replacing TLC on existing
Examples. The comparison panel adds questions about parallel scaling, storage,
fairness, repeated checks, implementation traces and proof maintenance.
Existing results establish strengths in first-run latency and finite symbolic
reachability, plus specific gaps in multi-worker explicit checking. A ranking
against native peers requires comparable runs on the same inputs.
[Roadmap](../../ROADMAP.md) · [Release results](../../developer/release-performance-results.md) ·
[Large-model results](../../developer/large-models.md)

## Published techniques and results

| Checker or study | What was measured | Technique to evaluate in dive |
| --- | --- | --- |
| tla-mc on Flint | A compiled run on 192 Graviton4 cores finished 756 million distinct states in 531 s. The roughly 9× TLC throughput ratio uses a TLC progress prefix. A larger configuration completed 9.82 billion distinct states in 99 min, spilling about 790 GB of queue. A separate 39-node run completed 693.2 million distinct states in 134.3 s. | Study generated Rust, private decoding of serialized frontier rows, smaller level batches, and distinct budgets for the seen set, frontier and trace data. Separate CPU contention from worker idle time before changing partitioning. [Published measurements](https://docs.rs/crate/tla-mc/0.3.0) · [Design and ablations](https://github.com/ddalton/tla-mc/blob/c8fce23493bb78fcd5405faf7b0653855db620ee/docs/DESIGN.md) |
| tlx | At one worker the compiled KVS Medium row reports 17,220,672 distinct states, 365,609,473 generated and depth 22, at 10.19× the recorded TLC baseline. The timed process can reuse the native cache populated by the benchmark warm-up. | One typed IR feeds a closure evaluator, Cranelift and generated C over fixed state layouts. Compare a specialized evaluator and a native tier on the same Core layout, with cold break-even and unsupported action handling measured. [Architecture](https://github.com/tlaplus-x/tlx/tree/77867a37d59525ce8866e8cce7f2dd32dc5d4d04) · [Timing protocol and records](https://github.com/tlaplus-x/tlx/blob/77867a37d59525ce8866e8cce7f2dd32dc5d4d04/docs/performance.md) |
| Manyan | On an 8-core M2, the published eight-worker result is 0.19 s versus TLC's 2.88 s on 823,543-state counters, and 0.07 s versus 2.99 s on 38,880-state liveness. The runner records the best time of ten runs by default. | Ordered parallel commit, compact states, incremental value hashes, worker-local allocation, ordered Init slices, and action atoms stored per edge. Check overlap with Core's existing scoped workers and compact graph work before running ablations. [Benchmark runner](https://github.com/soasme/manyan/blob/a39816a873a2c2aabc00dbcef50908e996f3d44b/bench/compare.py) · [Benchmark table](https://github.com/soasme/manyan/blob/a39816a873a2c2aabc00dbcef50908e996f3d44b/docs/benchmark.md) · [Optimization record](https://github.com/soasme/manyan/blob/a39816a873a2c2aabc00dbcef50908e996f3d44b/docs/optimizations.md) |
| tla-ts | Reports 403.3 s versus TLC's 986.6 s summed across 542 configurations at one worker with startup included. It also exposes per-obligation proof results in-process. | Compare on common inputs and use the proof API as an ergonomics comparator after checking supported obligations, backend methods and trust assumptions. [Checker and proof API](https://github.com/aidenybai/tla-ts/tree/bbe6c6cdac104c6a4c7399cf61f3bdb8138cd4a9) |
| TLC and sibling tools | The TLC maintainer's billion-states-per-minute figure is a bytecode proposal. Specl uses a bytecode VM with its own language and frame semantics. Apalache performs bounded SMT checks and induction queries on symbolic encodings. | Keep released TLC in the performance comparison. Treat Specl as design evidence, and use Apalache for constraint-heavy or induction tasks where the checking question matches. [TLC proposal](https://ahelwer.ca/post/2025-05-15-tla-dev-status/) · [Specl](https://github.com/danwt/specl) · [Apalache modes](https://apalache-mc.org/docs/apalache/running.html) |

MongoDB's snapshot-isolation model reached 8,454,961 distinct states after
symmetry reduction at depth 35. Its separate WiredTiger conformance workload
counted 490,360 states before symmetry and 132,981 after it. Generating the
labelled state graph took 71 s, selecting 87,143 test paths took 29 min 10 s,
and executing the tests took 13 min 49 s. This motivates a labelled graph
interface and end-to-end measurement. OmniLink adds search over event
timeboxes and substep orderings, a different contract from replaying a concrete
state sequence. [MongoDB tables 1–2](https://www.vldb.org/pvldb/vol18/p5045-schultz.pdf) ·
[OmniLink v2](https://arxiv.org/abs/2601.11836v2)

In Specula's evaluation, breadth-first search found 187 of the 200 bugs
found through model checking. Median counterexample length was nine steps.
Its CometBFT PBTS effort generated 1.9 billion successors and visited 393
million distinct states during iterative checking and repair. The former
supports measuring time to first replayable witness. The latter is evidence
of workload size within a workflow. [Specula evaluation](https://arxiv.org/html/2607.25333v2)

## Pinned workload selection

| Lane | Workloads and exact source |
| --- | --- |
| Shared explicit checks | Use the existing Examples pin for `lamport_mutex/MCLamportMutex.cfg`, `KeyValueStore/MCKVSSafetyMedium.cfg`, `dag-consensus/TLCSailfish2.cfg` and `byzpaxos/BPConProof.cfg`. Add Manyan's `bench/counters.tla` and `bench/liveness.tla` with their embedded configurations. Stage them through the pinned author's runner or reproduce its split exactly. [Examples pin](https://github.com/tlaplus/Examples/tree/6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f/specifications) · [Manyan inputs](https://github.com/soasme/manyan/tree/a39816a873a2c2aabc00dbcef50908e996f3d44b/bench) |
| External explicit controls | Run `model/EmergencyCall_Model_auto.tla` with `model/config/f3.cfg` and `tla/spec/PaxosLeaseChecked.tla` with `PaxosLeaseQuarantineEqualsProposer.cfg`. These have published final counts and local reproductions. [EmergencyCall source](https://github.com/FormalCellular/EmergencyCall/tree/093684f1bb02d733cae95d50f1690f1cb792296a/model) · [PaxosLease source and logs](https://github.com/mtrencseni/paxoslease-revisited/tree/5ca959624c5a2560ab1ad86dc71e8b161621d9bc) |
| Scale and symbolic crossover | Use `MCKVSSafetySmall.cfg` and Medium as count controls and `MCKVSSafetyLarge.cfg` for completion. Check the different structure of `transaction_commit/TCommit.cfg` on the Examples pin and retain its validated negative control in the existing evidence. Add Flint's `lean/formal/LeanP1ReaderScopeHolds.cfg` as a small control. Reserve `LeanP1AllHolds.cfg`, Flint's larger configurations, and PaxosLease Retry and RenewRetry for separate scale runs. Match the original result artifact before treating a config as a reproduction. [Existing scale measurements](../../developer/large-models.md) · [Flint source](https://github.com/ddalton/flint/tree/17585a15a516bd361e4997ea08d35c55a08763db/lean/formal) |
| CI and graph consumption | Reuse the existing etcd and historical CCF holdout revisions for model checks and pin a trace producer plus positive and negative traces. Use `Storage.tla` with `Storage.cfg` and the MongoDB snapshot-isolation case with `models/MCMultiShardTxn_snapshot.cfg` for labelled graph and refinement work. Confirm the published constants and obligations before using the paper's counts. [etcd pin](../../../conformance/holdout/holdout-etcd-raft.json) · [CCF pin](../../../conformance/holdout/holdout-ccf.json) · [MongoDB artifact](https://github.com/mongodb-labs/vldb25-dist-txns/tree/74526c1201109405172eb845413154f547a815ee) |
| Proof handoff and holdout | Use I3DP's `TCommit_IndProofs.tla` and `MongoLoglessDynamicRaft_IndProofs.tla`, then hold out TLAPS-Bench's SlateDB WAL tasks. Evaluate PdLedger as a large workload after reproducing its obligation inventory and result from the authors' materials. [I3DP inputs](https://github.com/SoftWiser-group/I3DP/tree/7ce4948fda3d558bc690c68ef01e2b620fa975fe/benchmarks) · [TLAPS-Bench inputs](https://github.com/specular-org/TLAPS-Bench/tree/e32e52be6146fecc9fffd21d9360e17faff66e75) · [VeriServe](https://github.com/hao-ai-lab/veriserve) |

Fetch pinned source through the corpus cache as each lane is enabled. Record
the entry module, config, transitive modules, tool and input digests, and
license per artifact. Keep sources and run outputs in the cache. Only vendor
small licensed fixtures read by a test. Keep published cases such as Pasture
and large proprietary runs as historical context until a runnable source and
matching configuration are identified. Historical CCF is a TLA+ workload.
Current CCF documents a Lean workflow.
[Corpus guide](../../conformance-corpora.md) · [Current CCF verification](https://github.com/microsoft/CCF/blob/main/doc/architecture/verification.rst)

## Comparison procedure

1. Run the original control first. Match source, constants, requested
   properties, reductions, identity mode and resource caps. Include v0.1.0,
   current Core, the pinned TLC oracle, TLC 1.8.0 when distinct, and peers on
   the inputs they admit. Keep comparisons against the existing oracle
   independent of results from newer TLC or peers.
2. Run one process at a time on the same quiet host. Record physical cores,
   affinity, frequency, NUMA, RAM, scratch disk, toolchain, seed, fingerprint
   and mode. Start the single-host completion panel at one and eight physical
   workers with a 600 s deadline, a 24 GiB process-group memory limit and no
   swap, matching the KVS reference envelope. Alternate three timed
   repetitions per cell, preserving each outcome. Time cold launch and
   compile-and-check separately from cached and in-process rechecks. Add
   a many-core run when a single-host profile identifies the limiting stage.
3. Keep every fixed-panel outcome visible. Report a named refusal, resource
   stop, violation, completed search, or disagreement. An unfinished run
   reports the last sampled prefix and its timestamp. Compare ratios only for
   matching complete work or explicitly labelled same-window prefixes. Gate
   on the configured observables and independently replay reported witnesses.
4. Record wall time to verdict or replayed witness, peak process RSS and
   spill bytes. For explicit work, record both generated occurrences and
   distinct states, the time denominator for each rate, depth convention, and
   collision evidence in fingerprint mode. For BDD search, report fixed-point
   time, represented concrete or quotient cardinality, and representation
   size. For trace and test workflows, report events or tests and the coverage
   they exercise. For proofs, report obligations per status at the same target
   coverage, with cold and edited whole-session timings.

Store compact results beside the existing scale evidence with a reproducible
invocation. Use focused controls on incremental changes and wider
qualification at integrated milestones. [Performance records](../../../conformance/reports/perf/README.md)

## Native peer measurements

The comparison used two dedicated EC2 c7i.4xlarge hosts. Each had eight
physical cores, one NUMA node, SMT and swap disabled, and a 24 GiB cgroup.
All arms for a given model ran on the same host. Values below are median
process wall seconds across three alternating samples. Each numeric cell
matches the reference verdict, distinct and generated counts, and depth.
Worker headings are the requested width. The record preserves observed
worker use where the checker reports it.
The frozen Core arm used the nominal TLC profile. Each host built Core from
the same source and release flags, with line information and LTO disabled.
The v0.1.0 arm used the published binary.
The full record includes both TLC jars, the release binary and elapsed-time
stops. Once a one-worker arm reached the cap, later repeats were omitted.
[Per-cell observations](../../../conformance/reports/perf/2026-11-20-native-peer-panel.json) ·
[Runner](https://github.com/Chelis-Lang/dive/blob/perf/peer-guard-spike/scripts/perf/scale/peer-panel.py)

To rerun, stage the pinned checker binaries under `bin/`, Java and the TLC
module jars under `inputs/`, and the staged modules under `models/` in a
scratch directory, following the names in the runner and evidence. Invoke
`peer-panel.py STAGE OUT --part panel --models counters liveness lamport`
within the memory cgroup. On the same host, resume with
`--models kvs --workers 8 --resume`, then `--models kvs --workers 1 --resume`.
Run EmergencyCall on its own host and output directory with
`--models emergency --workers 8 --resume`, then change the worker count to
one. The runner keeps the compiled cache within each cold/repeat
pair. Retain the final checker result before any progress samples when
normalizing counts and depth.

| Input | Checker and tier | 1 worker | 8 workers |
| --- | --- | ---: | ---: |
| Manyan counters | Core | 2.91 | 1.61 |
| | Manyan | 0.85 | 0.19 |
| Manyan liveness | Core | 0.76 | 0.81 |
| | Manyan | 0.21 | 0.07 |
| Lamport mutex | Core | 9.26 | 2.69 |
| | Pinned TLC | 12.11 | 4.09 |
| | tlx interpreted | 11.72 | 2.00 |
| | tlx compiled, clean cache | 1.69 | 0.27 |
| | tla-mc interpreted | 10.79 | 2.95 |
| | tla-mc compiled, clean cache | 28.62 | 23.80 |
| | tla-mc compiled, repeat | 5.70 | 0.80 |
| KVS Medium | Core | stopped | 149.93 |
| | tlx interpreted | stopped | 160.39 |
| | tlx compiled, clean cache | 84.41 | 13.74 |
| | tlx compiled, repeat | 84.75 | 13.67 |
| | tla-mc interpreted | 550.48 | 95.18 |
| | tla-mc compiled, clean cache | 378.83 | 70.28 |
| | tla-mc compiled, repeat | 358.89 | 47.60 |
| EmergencyCall f3 | Core | 232.89 | 104.78 |
| | Pinned TLC | 396.81 | 91.38 |

Lamport mutex finished at 724,274 distinct states, 2,729,079 generated and
depth 61. On this workload tlx reported a 97-word state layout, 27 fully lowered
actions and zero boxed or interpreter IR nodes. It started with JIT code.
At eight workers the run completed before tlx's background C compilation
threshold. The 0.27 s clean-cache result includes binding and JIT setup.
tla-mc's generated Rust has a larger first-check cost and pays back on
repeat checks of the same input.

On KVS Medium at eight workers, Core and compiled tlx matched the golden
17,220,672 distinct states, 365,609,473 generated and depth 22. tlx reported
a 14-word layout and all seven actions fully typed. In generated C,
closing a transaction iterates set bits for open transactions, looks up
write-set offsets in fixed tables, and copies each updated store slot
directly into the successor. Its median peak RSS was 1.59 GiB versus
Core's 0.85 GiB. The pinned TLC cell matched the state counts and reported
depth 23. At one worker compiled tlx completed in 84.41 s with the same
golden counts. Core reached 15,996,087 distinct and 292,049,699 generated
before its limit. Both TLC jars and dive v0.1.0 also hit the cap.
[tlx tier and compilation log](https://github.com/tlaplus-x/tlx/blob/77867a37d59525ce8866e8cce7f2dd32dc5d4d04/docs/usage.md#5-compiled-mode--compile)

On EmergencyCall f3, Core and pinned TLC agreed on 21,255,936 distinct,
135,871,232 generated and depth 27 at both widths. From one to eight
workers Core sped up 2.22× and pinned TLC sped up 4.34×.

On Manyan liveness, Core reported using all eight workers. Its median
exploration timer rose from 94.12 ms to 145.68 ms with 38,880 distinct
states at each width. The process wall time includes the subsequent
temporal check. The case gives a baseline for measuring both phases.
[Core temporal work](https://github.com/Chelis-Lang/dive/issues/1028)

Core's one-worker sampled self time included at least 13.2% in heap lookups
on Lamport mutex and 9.9% on a 40 s KVS Medium prefix. These lower bounds
sum the source rows at or above 0.4% in the profile. In the short compiled
tlx Lamport profile, fingerprinting encoded words accounted for at least
18.8% and the queue's XOR codec for at least 6.8%. The within-tlx tier
comparison captures the combined effect of layout specialization and native
execution. tlx also quantifies how much work returns to general evaluation.
In its published VSR example, changing the representation of one message
bag reduced host-region entries from 1.21 billion to 6.39 million at the
same counts and depth. That result motivates measuring actual fallback
frequency alongside static action coverage.
[tlx VSR measurements](https://github.com/tlaplus-x/tlx/blob/77867a37d59525ce8866e8cce7f2dd32dc5d4d04/docs/research/native-vsr.md)

### Scalar guard experiment

The experimental branch lowers a bounded pure guard from the existing
Core plan to a flat tape. It leaves memo keys, lookups and insertion on
Core and specializes only admitted memo bodies or unmemoized guards.
Both profiles matched on the duplicate-successor and overflow controls.
The deliberately failing invariant produced the same independently valid
witness. The two-model Core small-tier ratchet had zero regressions.

The tape ran zero times on the counters and KVS Medium admission checks.
It ran six times on completed Lamport mutex. Three same-binary off/on
repetitions on counters and Lamport at one and eight workers showed no
10% improvement in either profile. A local synthetic check with 500,002
tape evaluations and zero fallbacks took 1.31 s with the tape off and 1.38 s
with it on. The experimental code is available for review on the
[guard spike branch](https://github.com/Chelis-Lang/dive/tree/perf/peer-guard-spike).

The aggregate and resident experiments below use complete actions and
finite layouts to reach work outside the scalar tape's admission.
[tlx mixed-mode lowering](https://github.com/tlaplus-x/tlx/blob/77867a37d59525ce8866e8cce7f2dd32dc5d4d04/docs/mixed-mode.md)

### Packed states and complete actions

The follow-up rebuilt Core with the release profile and reran the pinned
native peers. The aggregate and temporal experiments ran on dedicated
eight-core CPU hosts. Core was frozen before the streaming symmetry and
disk membership changes. Cells compare three interleaved process samples
on one host, with nominal and default profiles measured separately. Timing includes
startup. The complete record names the identity mode, cap, result, counts,
depth, and RSS for each sample.
[Follow-up observations](../../../conformance/reports/perf/2026-11-20-packed-core-panel.json) ·
[Panel runner](../../../scripts/perf/scale/native-peer-panel.py)

| Experiment and input | Profile | Workers | Core wall | Candidate wall | Fastest matched peer |
| --- | --- | ---: | ---: | ---: | ---: |
| Resident counters | Default | 1 | 4.24 s | 23.2 ms | Manyan 0.87 s |
| Resident counters | TLC | 1 | 4.27 s | 23.2 ms | Manyan 0.86 s |
| Resident counters, wider configuration | Default | 1 | 10.06 s | 48.3 ms | Manyan 2.14 s |
| Resident counters, wider configuration | TLC | 1 | 10.08 s | 48.6 ms | Manyan 2.11 s |
| Packed KVS Small | Default | 1 | 102.90 s | 106.63 s | — |
| Packed KVS Small | TLC | 1 | 104.33 s | 108.85 s | — |
| Aggregate actions on Lamport | Default | 8 | 2.39 s | 2.13 s | tlx 0.27 s |
| Aggregate actions on Lamport | TLC | 8 | 2.64 s | 2.41 s | tlx 0.27 s |
| Aggregate actions on KVS Medium | Default | 8 | 164.26 s | 139.87 s | tlx 16.18 s |
| Aggregate actions on KVS Medium | TLC | 8 | 165.84 s | 140.70 s | tlx 15.98 s |
| Retained temporal rows | Default | 8 | 0.57 s | 0.49 s | Manyan 70.0 ms |
| Retained temporal rows | TLC | 8 | 0.76 s | 0.69 s | Manyan 70.0 ms |

The resident integer-function path derives its transition table from the bound
Core plan. It keeps an exact bitset for membership and updates packed frontier
words directly. The Manyan counters configurations use 21 and 24 bits per
state. Their matched results were 823,543 distinct states at depth 43 and
1,679,616 at depth 41. Each comparison to the Core baseline uses exact
identity. Manyan ran in fingerprint mode. A late layout miss and an invariant
violation returned to ordinary Core. In both profiles the violation produced
the original independently replayable trace.
[Resident layout experiment](https://github.com/Chelis-Lang/dive/tree/perf/resident-fixed-domain)

Layout inference derives a 32-bit encoding for KVS Medium from the configured
invariant. The five fields use 6, 2, 12, 6 and 6 bits. Round trips preserved
equality and the Core value hash for the initial state and 4,095 successor
emissions in both profiles. The KVS Small experiment used packed exact keys
and frontier rows throughout search. It used Core to expand those rows and
check invariants. The resulting RSS and wall-time measurements locate the next
work in the action evaluator and encoded symmetry. Nominal-profile peak RSS on
KVS Small fell from 801 MiB to 67 MiB. In the 60 s KVS Medium comparison, the
packed path reached 2,204,736 distinct and 21,650,441 generated states. Core
reached 2,421,218 distinct and 24,500,288 generated states.
[tlx fixed state layouts](https://github.com/tlaplus-x/tlx/blob/77867a37d59525ce8866e8cce7f2dd32dc5d4d04/docs/design.md)

The aggregate path replaces repeated function lookup and update with indexed
operations and lowers complete deterministic action bodies. It exercised 24
Lamport leaves in the instrumented run. At eight workers KVS Medium took 14.8%
less wall time in the default profile and 15.2% less wall time in the TLC
profile. The gap to compiled tlx is still substantial.
[Aggregate action experiment](https://github.com/Chelis-Lang/dive/tree/perf/packed-core-actions)

The temporal profile attributed 6.3% of counted instructions to
implicit-product traversal, plus 1.8% in its inlined slice indexing. The
trials reused discovered pairs and retained bounded product rows in discovery
order before converting to witness order. The weakened fairness control
replayed successfully in both profiles at one and eight workers. At eight
workers Manyan fairness took 14.0% less wall time in the default profile.
AllocatorImplementation saw no consistent gain.
[Temporal experiment](https://github.com/Chelis-Lang/dive/tree/perf/product-vertex-lookup) ·
[Manyan product layout](https://github.com/soasme/manyan/blob/a39816a873a2c2aabc00dbcef50908e996f3d44b/manyan.c#L15169)

The isolated Wasm batch probe spent most of its time copying 97-word Lamport
rows. The generated resident kernels instead compiled the bound counters
transition tables and retained the whole search in linear memory. The Rust and
Wasm versions matched the counts and depth on both configurations. Warm Rust
and generated Wasm medians were 18.56/18.65 ms and 47.18/46.42 ms for the two
configurations. Wasm compilation and instantiation added 2.97 ms and 3.16 ms.
[Wasmtime typed calls](https://docs.rs/wasmtime/50.0.3/wasmtime/struct.TypedFunc.html)

A profile-guided build trained on MCBakery, MCPaxos and TCommit sped up
Lamport and one-worker counters in both profiles. It slowed Manyan liveness in
both profiles. The 13-run startup medians were 4.00 ms for Core and 4.06 ms
for the profile-guided build.
[Rust profile-guided builds](https://doc.rust-lang.org/rustc/profile-guided-optimization.html)

On EmergencyCall f3, frozen Core took 254.67 s at one worker and 114.17 s at
eight, with the same counts and depth. BPConProof reached 29,711,908 distinct
states at its one-worker limit. The system cgroup killed the eight-worker
process for memory before it produced a report.

On KVS Medium, replacing general symmetry with exact bit permutations
increased the generated-state prefix after 60 seconds by a factor of 1.51× in
the default profile and 1.51× in the TLC profile.
[Packed symmetry experiment](https://github.com/Chelis-Lang/dive/tree/perf/ranked-symmetry)

### Compiling the finite relation

The optional compiled strategy exhausts the published KVS Medium model with 17,220,672 exact symmetry representatives, 365,609,473 generated occurrences and depth 22. The paired Core runs select exact state identity. The pinned tlx comparator uses fingerprints and reported the same counts. These whole-process samples ran serially on one dedicated host after the mimalloc 3.7.0 update. One-worker figures are single observations and eight-worker figures are medians of three.

| Workers | Exact Core | Compiled including binding and rustc | Compiled worker only | tlx cold | Cold speedup over Core |
|---:|---:|---:|---:|---:|---:|
| 1 | 835.14 s | 4.30 s | 3.32 s | 94.14 s | 194× |
| 8 | 302.36 s | 1.25 s | 0.72 s | 14.78 s | 242× |

KVS Small and 12-manager TCommit match the exact controls at both widths. SlateDB WAL Medium refuses by name during finite admission. Its TypeOK also lacks the fixed declarations needed to encode its nested manifest and WAL after recognizing its untyped model names. [Inputs, peaks, controls and per-run observations](../../../conformance/reports/perf/2026-11-21-compiled-finite-safety.json) · [Compiled strategy](../../user/symbolic.md#compile-a-finite-safety-check)

The finite safety binder already checks whether a configuration can be
expressed as Boolean guards and updates over a fixed layout. The new probe
turns those decision diagrams into Rust. It groups adjacent bits that copy
together, keeps state packed through generation and membership, and sorts
configured interchangeable lanes to identify exact symmetry orbits. It
verifies the configured invariants and checks for deadlock where requested.
The current worker uses one physical core.
[Source-driven finite checker](../../user/symbolic.md) ·
[Packed search emitter](../../../scripts/perf/scale/packed-codegen-probe/emit_finite_search.py)

| TLA+ input | Exact distinct states | Exact generated states | Depth | Cold bind, compile and check | Warm check | Same-host control |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| KVS Small | 3,409,605 | 56,349,379 | 23 | 1.00 s | 0.64 s | Core exact 104.33 s |
| KVS Medium | 17,220,672 | 365,609,473 | 22 | 2.98 s | 2.69 s | tlx 87.25 s at 1 worker |
| TCommit, 12 resource managers | 535,536 | 6,401,869 | 25 | 0.40 s | 50.0 ms | Core exact 23.36 s |

These medians cover three separate processes on the temporal-panel host. Cold
time includes loading the source, binding, emission, Rust compilation and
search. Each warm process uses a built worker. TCommit uses the original
module and an enlarged `RM` constant. The binder uses nominal TLC semantics.
The generated and Core controls use exact identity. Compiled tlx uses
fingerprints. Distinct states count exact orbits when symmetry is configured.
Generated states count every enabled successor occurrence and each initial
occurrence. Peak RSS is the maximum reported by GNU time for the measured
process. On KVS Medium the cold generated worker gave a 29.3× speedup over
compiled tlx at one worker. The eight-worker tlx run took 13.92 s.

A parallel worker retained canonical packed states and shared an exact atomic
bitset. On KVS Medium its warm process medians were 2.97 s at one worker and
0.71 s at eight. Both widths matched all counts and depth. At eight workers
the cold medians were 1.18 s for generated Rust and 13.92 s for tlx, giving a
11.8× speedup. The warm tlx median was 13.91 s.
[Parallel experiment](https://github.com/Chelis-Lang/dive/tree/perf/finite-atomic-source)

The KVS worker checks the unmodified source. A separate handwritten transition
loop reached the same counts in 1.49 s on the same host. This measures
the throughput of direct action code and packed membership on that input. On a deliberately
nonholding KVS edit, the generated worker declined. Ordinary Core reported
`NeverOpen` at depth two and the independent trace validator accepted both
profiles' witnesses.

For KVS Large the same lowering fitted each state into 45 bits and assigned
each orbit a dense rank among 23,299,718,715 possibilities. It completed in
236.11 s at 1 worker and 37.29 s at 8 workers on the dedicated host. The
search checked 594,616,815 distinct states and 13,385,471,536 generated states
at depth 30. Peak RSS on the 8-worker run was 4.91 GiB. A sparse exact
hash-table trial on the shared workstation stopped at its 620 s limit.
[Large controls and BDD fixed point](../../developer/large-models.md#repeat-on-the-r2-reference-hardware) ·
[Parallel dense orbit experiment](https://github.com/Chelis-Lang/dive/tree/perf/finite-atomic-orbits)

Use the symbolic development environment from the user guide to reproduce the
one-worker generated search. Set `module` and `config` to the original closed
input paths. For TCommit, pass `--type-invariant TCTypeOK`.

```sh
cargo build --release --locked -p dive-api --example symbolic_input
probe_dir=$(mktemp -d)
target/symbolic-python/bin/python scripts/perf/scale/packed-codegen-probe/emit_finite_search.py \
  target/release/examples/symbolic_input "$module" "$config" "$probe_dir/worker.rs"
rustc --edition 2024 -Copt-level=3 -Cpanic=abort -Clto=off -Ccodegen-units=1 \
  "$probe_dir/worker.rs" -o "$probe_dir/worker"
"$probe_dir/worker"
```

The existing binder declined the original Lamport configuration at its
operator override, MCPaxos at its temporal property, and BPConProof at the
configured `Value` binding. The dedicated gains establish the value of
complete action lowering within the admitted safety fragment. These three
refusals bound the transfer to general models.

Use the finite relation as the starting point for a compiled safety tier.
Qualify it on a consensus model with a wider layout, check compiler admission
and all exit paths against Core, and keep witness replay and budget
enforcement in the qualification. Preserve separate cold and warm
measurements. Qualify the parallel path at the same worker widths as Core. The
aggregate evaluator and bounded temporal rows merit targeted work on cases
that stay in the general engine.

## Selecting a supporting lemma on a published proof

On Scimitar’s TwoPhase protocol, receiving an abort message can violate
`H_TCConsistent` from a type-correct, consistent induction state. Dive
checked each of the 14 other candidate lemmas from the published inductive
invariant as a single strengthening over two resource managers. Only
`H_RMCommittedImpliesNoAbortMsg` closed the action on that finite instance
(2,688 states including the post-action states). The other 13 candidates
and the empty hypothesis each produced an independently replayed two-state
counterexample to induction.

External TLAPS proved the selected action obligation for arbitrary `RM`
after unfolding the source’s `msgsAbortCommit` set union. Removing the
selected premise failed the obligation. The same external checker proved
the published AbstractRaft rollback action preserves `H_OnePrimaryPerTerm`
and all 16 obligations in Lamport’s authored TCommit proof. Each of the
two Scimitar checks covers one action and one lemma. Finite screening uses
the published candidate pool. Synthesis of new candidate formulas and native
checking of hierarchical TLA+ proofs remain open.
[Artifact pins, candidates, controls and commands](../../../conformance/reports/perf/2026-11-22-lemma-selection.json)
· [Scimitar’s inductive decomposition](https://github.com/will62794/scimitar).

## TLAPS options and evidence

The charter requires proof-bearing modules to parse and pass semantic
analysis. The bundled TLAPS module resolves imports during model checking.
Deductive checking beyond parsing has a separate scope decision. I3DP
illustrates the handoff between finite candidate screening, induction queries
and a TLAPS proof. TLAPS-Bench's best result of 31 out of 70 targets at
pass@3 measures success at writing proofs. VeriServe reports 516,973
obligations across 226 modules for its PdLedger design under stated
assumptions. Those quantities describe different workloads.
[Charter](../../CHARTER.md#scope) · [I3DP](https://github.com/SoftWiser-group/I3DP) ·
[TLAPS-Bench](https://arxiv.org/html/2611.02849) · [VeriServe measurements](https://haoailab.com/blogs/verifiable-llm-infra/)

1. **Proof handoff.** Keep the theorem and module identity across the finite
   check and proof. Screen candidate invariants in batches when the existing
   closure and evaluator admit them. Distinguish a reachable counterexample
   from a counterexample to induction. Check the external proof using pinned
   `tlapm` and its own verdict.
2. **Upstream or companion.** Baseline the rolling upstream proof manager by
   executable revision and content hash. Exercise its existing obligation
   fingerprints, LSP and backend facilities. Prototype source-linked
   obligations, cancellation, progress and selective reruns only where
   current tools leave a measured gap. Prefer upstream contributions when
   the existing manager can support the improvements. [TLAPS source and backends](https://github.com/tlaplus/tlapm/tree/9a52fafb00245ace0877ec8d91c4a09a7caa8061) · [Proof decomposition work](https://conf.tlapl.us/2026-etaps/petrauskas-2026.pdf)
3. **Full Rust reimplementation.** Evaluate a separately versioned proof
   system with a maintainer and charter of its own. Share syntax and source
   maps through a stable boundary. Give proof elaboration its own
   representation for hierarchical proof steps, scoped assumptions,
   substitutions, library imports, backend directives and supported temporal
   obligations. Use deterministic obligation identities, dependency-aware
   scheduling, and a cache keyed by normalized obligation, transitive
   assumptions, encoding, backend and trust policy. Start with existing
   provers, and evaluate certificate checking or proof reconstruction for
   supported fragments. State the assumptions and trusted components behind
   every accepted result.

Measure cold module checks, time to first obligation result after an edit,
time to the final result, and backend work with upstream fingerprint reuse
enabled. Compare a vertical slice against both upstream and the best available
companion route. Use known proofs, intentionally broken proofs, changed
assumptions, corrupted certificates where used, and a held-out proof. A full
implementation needs zero observed false acceptance, matching theorem
statements and assumptions, independently checked evidence for the evaluated
fragment, and an owner for the proof system. The proposed investment threshold
is at least a halving of median edit-to-final time on two proof families over
the best upstream or companion prototype. Seek a separate owner decision on
scope and trust after this comparison.
[Open decision](../../open-questions.md#deductive-proof-system-scope)

## Bounded checking, fairness and proofs

Across all 46 length-five configurations in the authors' Examples panel, Manyan and Apalache agreed on 45 on a dedicated Linux c7i.4xlarge. Both found the one-state `FindSolution` counterexample in Einstein, reported `ExecutionsTooShort` on SpanningTree, and cleared 43 other configurations through the bound. On APtcp, Apalache cleared the bound and Manyan refused `IsPrefix` in `tcp` at line 120. The published run on Darwin reports 46 agreements. On the 45 matching Linux cells, Manyan took 1.57 s total and Apalache took 240.06 s, including process startup. Each configuration has one fresh-process observation per tool. [Per-configuration measurements, properties and source identities](../../../conformance/reports/perf/2026-11-22-bounded-peers.json) · [Authors' panel](https://github.com/soasme/manyan/blob/a39816a873a2c2aabc00dbcef50908e996f3d44b/docs/symbolic-bench.md)

Core's explicit breadth-first search on those same 46 inputs completed 15, hit the six-level admission limit on 27, timed out on one and rejected three. It reports a stop when the next new state would exceed the depth limit. A bounded checker also reasons about paths that continue through previously seen states. Two Disruptor configurations use a negative integer constant in the CFG. Einstein imports Apalache's module. Providing its upstream source allowed Core to start searching Einstein, which reached the five-second cap. Those controls identify a CFG compatibility change and a solver-friendly initial-state search as distinct opportunities. [Explicit search diagnostics](../../../conformance/reports/perf/2026-11-22-explicit-prefix.json)

On Elevator Medium with the original quantified weak and strong fairness, both tools completed 4,122 distinct and 14,296 generated states. At one worker, Dive took 1.496 s and Manyan took 0.109 s. Reducing the configuration to one person and one elevator gave 36 states and a passing fair check on both. Removing just strong fairness for `MoveElevator` made both return a violation of `TemporalInvariant`. Dive independently validated its six-state lasso and rejected both a substituted initial stutter and the original lasso under restored fairness. These controls exercise the transition, fairness and liveness contracts for changes to the product search. [Runs and replay results](../../../conformance/reports/perf/2026-11-22-elevator-fairness.json) · [Phase timings for larger configurations](../../../conformance/reports/perf/2026-11-21-temporal-analysis-phases.json)

For named safety proofs, the artifact importer joins all 33 rows in Recomp-Verify Figure 5 to their original module, CFG and named invariant. The artifact also ships ToyMutex, outside that table. Under a 30-second wall limit on one Linux host, Dive proves 26 of the 34 named invariants. The previous worker proved seven, tuple and subset relations raised that to 21, and finite function choices, quantified guards and integer operations added five. Recomp-Verify's parallel portfolio finishes 24. Both prove 18 and Dive finishes ahead on all 18. Dive alone proves eight, Recomp-Verify alone proves six, and neither finishes two. Sharded KV's two original properties take 1.017 and 1.167 seconds with Dive against 3.327 and 2.423 seconds with the portfolio. The learning-switch transition case still times out on Dive while the portfolio succeeds. [Per-CFG commands, digests, timings, refusals and negative control](../../../conformance/reports/perf/2026-11-22-recomp-originals.json) Scimitar's saved graphs for TwoPhase and two AsyncRaft properties contain 14, 13 and 31 lemma nodes respectively. Their discharge flags record the producer's result. The importer checks the graph hashes and extracts the nodes, dependencies and CTI counts. Independent proof checking and timing fresh inference require separate runs. The Raft files record zero seconds of invariant checking. [Pinned inputs and published results](../../../conformance/reports/perf/2026-11-22-published-peer-artifacts.json) · [Direct named-proof comparisons](../../../conformance/reports/perf/2026-11-21-named-property-proofs.json) · [Scimitar proof controls](https://github.com/Chelis-Lang/dive/issues/1283)

### Reproduce the comparisons

Use Linux with Python 3.11 or later, GNU time/coreutils, a C compiler, make and Java 21. From the pinned Manyan checkout install its own checksum-verified Apalache and libraries, then run the panel driver from Dive. The output path must be new. Its `--only` filter selects one configuration by path substring. A Java heap cap of 2 GiB applies to each Apalache child. The published Linux measurement used an 8 GiB systemd service limit with swap disabled.

```sh
git clone https://github.com/soasme/manyan "$MANYAN"
git -C "$MANYAN" checkout a39816a873a2c2aabc00dbcef50908e996f3d44b
make -C "$MANYAN" manyan .obj/apalache-0.63.0/bin/apalache-mc \
  .obj/CommunityModules-202611060952 .obj/tlapm-c8ddc44a80b621989f96a9ee2c371301c52b109f \
  .obj/Examples-ca168055339d5cbcee95b7eefcb97c1ee59f9914
python3 scripts/perf/scale/bounded-peer-panel.py "$MANYAN" bounded.json
python3 scripts/perf/scale/bounded-peer-panel.py "$MANYAN" prefix.json --dive-prefix "$DIVE" --seconds 6
python3 scripts/perf/scale/elevator-fairness.py "$EXAMPLES/specifications/MultiCarElevator" "$DIVE" "$MANYAN/manyan" fairness.json
python3 scripts/perf/scale/published-peer-artifacts.py published.json
```

For the 34 named properties, use the pinned Recomp-Verify checkout, its bundled
portfolio and an installed Dive worker. The driver makes a fresh directory
and process group for each command and verifies the checkout and each requested
property. To measure both process families' resident charges (including
descendant JVMs and charged file pages), use a fresh systemd service with
`Delegate=yes`, `MemoryMax=24G` and `MemorySwapMax=0`. Run this from the
service's shell on Linux cgroup v2, using the repository as its working directory.

```sh
group="/sys/fs/cgroup$(cut -d: -f3 /proc/self/cgroup)"
mkdir "$group/driver"
echo $$ > "$group/driver/cgroup.procs"
echo +memory > "$group/cgroup.subtree_control"
python3 scripts/perf/scale/recomp_property_panel.py "$RECOMP" --dive "$DIVE" \
  --seconds 30 --cgroup-parent "$group" > "$RESULTS"
```

Omit `--cgroup-parent` for time and outcomes alone. Each cell gets the same
external wall deadline, including binding, checker startup and compilation.

For the fairness run, use the Dive corpus pin of Examples. The script verifies the original module and CFG digests before creating its two smaller controls. The artifact importer reads immutable Scimitar and Recomp-Verify revisions and version 2 of the Recomp-Verify paper. For compiled explicit searches, add `--tla-mc` to the native panel invocation to run cold and warm compiled tla-mc as well as tlx on KVS and Lamport. Each compiler gets its own cache. [Native panel](../../../scripts/perf/scale/native-peer-panel.py) · [Historical explicit-search records with both peers](../../../conformance/reports/perf/2026-11-20-native-peer-panel.json)

## Quantified fairness on larger models

With batched action labels, ordinary Core took 2.04 s on the three-floor proof and 6.91 s to a validated four-floor violation. Manyan took 1.32 s and 7.77 s in five alternating cold processes per checker. On five floors Core reached a valid 146-state word after 667,943 distinct states in 149.59 s. Manyan exhausted its graph of 1,051,289 states and reported a different valid word in 38.79 s. Core spent 124.56 s in product search and 14.72 s evaluating graph predicates. The four-server Kafka control exhausted 28,440 states under both builds and used the ordinary evaluator. [Per-process times, memory, source identities and damaged-witness controls](../../../conformance/reports/perf/2026-11-22-batched-fairness-actions.json)

At six floors the model crosses the one-million-row cutoff for a complete index of successor actions. Reading that index in windows removed 6.16 million direct enabledness evaluations. In a production-only pair, time to the same 263-state witness fell from 77.40 to 72.40 s, with 550 MiB peak RSS using windows. Manyan took 162.48 s and 3.52 GiB for its 329-state witness in the earlier matched configuration. Both words passed independent replay. The seven-floor engineering candidates still exhausted their 240 s deadline. [Wall time, memory, source configuration, replay and phase counts](../../../conformance/reports/perf/2026-11-22-windowed-enabled.json)

With both improvements on a second dedicated host, Core took a median 1.79 s to prove the original three-floor model and 6.63 s to find a four-floor violation. Manyan took 1.12 s and 6.64 s on that host. Selecting a 100,000-state first check produced the same validated 102-state word in one 4.11 s run. [Combined wall times, peak RSS and word checks](../../../conformance/reports/perf/2026-11-22-closed-enabled.json)

Measured separately before batched action labels, closed-row enabledness lowered the three-floor median from 3.15 s to 2.65 s, and the four-floor median to a validated violation from 10.87 s to 9.37 s. Manyan took 1.12 s and 6.64 s on this host. All 525,108 direct three-floor `ENABLED` evaluations were eliminated. On four floors the checker still examined 256,299 queries on the unexpanded frontier. [Whole-action measurements, median RSS and independent replay](../../../conformance/reports/perf/2026-11-22-closed-enabled.json)

The earlier three-floor plan-selection comparison exhausted the same 50,653 states in 2.94 s at one worker. Manyan took 1.33 s in that session. Choosing component search for independent fairness alternatives cut Core's median product time from 924 ms to 295 ms and peak RSS from 100 MiB to 74 MiB. Atom labeling took 1.93 s before closed-row enabledness. [Three-floor plan selection and remaining gap](../../../conformance/reports/perf/2026-11-22-fairness-plan-selection.json)

Adding a fourth floor exposes a violation. Ordinary Core takes 9.62 s, down from 19.08 s before native label reuse and component search. Manyan takes 7.46 s. Its 202-state word and Core's 102-state word both pass an independent validator and both fail after changing their closing targets. [Matched cold runs, phase times and peak RSS](../../../conformance/reports/perf/2026-11-22-projected-fairness.json)

For iterative checks on the same four-floor model, the first prefix containing a valid word can fall between the default checks. A seven-threshold sweep located such a prefix at 104,122 states. With `--periodic-min-states 100000`, three fresh CLI invocations took a median 6.40 s to a validated word. The same executable and host took 9.73 s at the default. The two- and three-floor holding models exhaust their graphs under both settings. [All seven thresholds, repeated timings and independent replays](../../../conformance/reports/perf/2026-11-22-elevator-prefix-placement.json)

The planner completes the original SimpleAllocator with 14 clients, no resources and one state. With two active clients and two resources, Core and Manyan also agree on a constrained 64-state graph with 2,503 generated states, including both safety invariants and the original fairness property. This isolates the number of fairness conditions from graph size. A two-node CheckpointCoordination variant moves past the formula limit but Core still refuses and Manyan fails evaluating its initial state. The changed CFGs and both outcomes are in the four-floor data.
