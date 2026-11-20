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

Leave the scalar tape experimental. The next compilation probe should
specialize one fixed-domain aggregate path in Lamport mutex, such as
`req[p][q]` reads and updates. Measure how often the admitted actions use
the representation, how often they fall back, and the effect on the full
check. Reuse Core's binding and identity rules, and compare successor order,
generated multiplicity and independently valid witnesses with the general
path. Use counters and KVS Medium to check transfer beyond Lamport.
[tlx mixed-mode lowering](https://github.com/tlaplus-x/tlx/blob/77867a37d59525ce8866e8cce7f2dd32dc5d4d04/docs/mixed-mode.md)

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
