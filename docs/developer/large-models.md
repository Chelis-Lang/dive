# Large-model performance and exact symbolic checking

The [source-driven symbolic checker](../user/symbolic.md) checks finite safety specifications using binary decision diagrams (BDDs). Dive resolves the original modules, binds the configuration and exports its IR. The Python compiler reads the actual state types, initial predicate, actions, invariants, symmetry group and deadlock setting. KVS runs directly from its existing source with an inferred bit layout.

The [performance evidence index](../../conformance/reports/perf/README.md) collects compact records for reuse in other reports. The tables below identify which comparisons ran on the same host, which reached a complete fixed point and which stopped at a limit.

## Next experiments

- **Frontier storage.** [Resident row compression](#resident-frontier-rows) cut peak memory most on SlateDB and DiskSynod. Compare admissions at the same memory ceiling, then separate the contributions of predecessor links, seen fingerprints and the compound-value store using matched counts.
- **Symbolic checking.** Keep Base and QuarantineEqualsProposer as [exact sparse-message controls](#reproduce-the-released-paxoslease-runs). Compare alternative BDD orders on Retry using reachable intermediate sizes, then cross-check a completed result against both explicit runs.
- **Published scale.** The [original SlateDB WAL configuration](#reproduce-the-released-slatedb-wal-model) provides a public 49-billion-state target. Compare [external membership](#external-fingerprint-membership) at full exhaustion to its resident and TLC controls. Use the completed two-writer model for a full-count comparison and small exact-state runs for collision controls. The EmergencyCall 10.59-billion case still needs the original unconstrained configuration.
- **Remaining work.** The [thirteen original outcomes](#where-the-thirteen-historical-non-finishes-stand) identify each memory or feature limit. Kafka now reaches periodic temporal analysis, where the measured run stopped at its time budget. The all-17-model temporal worker comparison belongs at the integrated qualification boundary. Use [Sail’s measured edit cases](#which-evidence-can-decide-a-source-edit) as regression controls when changing proof reuse.

## Results on KeyValueStore

A row is a completed breadth-first search. The two unmodified smaller configurations agree with their pinned TLC goldens on verdict, generated states, distinct states and depth. Large has no completed TLC golden. Its result agrees with a separate hand translation, a combinatorial count based on possible transaction histories, and the completed eight-worker Core run below. The altered configurations use the original program and change only a finite constant domain.

| Configuration | Keys / values / transaction IDs | Distinct states | Generated | Depth | Nodes in final reachable-set BDD | Local search, seconds |
|---|---|---:|---:|---:|---:|---:|
| Small | 2 / 2 / 3 | 3,409,605 | 56,349,379 | 23 | 1,278 | 4.7 |
| Medium | 3 / 3 / 2 | 17,220,672 | 365,609,473 | 22 | 328 | 0.47 |
| Large | 3 / 2 / 3 | 594,616,815 | 13,385,471,536 | 30 | 1,961 | 123 |
| Medium + one key | 4 / 3 / 2 | 4,527,151,872 | 123,139,562,753 | 27 | 459 | 3.7 |
| Medium + two keys | 5 / 3 / 2 | 1,183,519,405,056 | 39,234,075,936,769 | 32 | 590 | 14.0 |
| Large + one value | 3 / 3 / 3 | 13,408,088,064 | 422,642,244,865 | 30 | 2,112 | 248 |

Local timings include source binding, compilation and checking. Peak RSS was 613 MiB for the Large run with two subsequent invariant edits and 196 MiB for the process that checked Medium and then its five-key extension. The final BDD node count describes reachable-set representation size. BFS layers, intermediate images and the decision-diagram manager account for most of the peak. Measurements used a shared Ryzen AI MAX+ 395 workstation. These are individual engineering measurements. Core comparison binaries used the locked release build with LTO disabled and 16 codegen units on both sides.

**Dependency structure controls the result.** A transaction couples its snapshot, write log, missed-write log and the shared store at each key. Placing fields for the same key together makes many subproblems identical. With Medium, grouping by the common inner index gave 328 final nodes, 0.58 s wall and 67 MiB peak RSS. In declaration order the same model gave 3,552 nodes, 9.99 s and 202 MiB. Switching to a single simultaneous image per action gave 0.62 s and 8.48 s respectively. Layout explains most of this difference. More transaction IDs also couple more fields through conflict checks and commit. Adding keys to the two-transaction model was much cheaper than adding a third transaction to Medium, even when the five-key model had many more states.

### Seven keys with exact counts

Extending Medium to seven keys reached **80,269,130,471,718,912 symmetry classes**
and **3,618,160,532,973,355,009 generated** at depth 42. Removing transaction
symmetry gave **160,538,259,663,421,440 concrete reachable states** and
**7,236,321,007,066,677,249 generated** at the same depth. Both searches reached
a fixed point with the original invariants. Both counts in each run agree with
the independent transaction-history formula. The runs took 70.60 s and 97.30 s
respectively on the shared workstation. The [measurement record](../../conformance/reports/perf/2026-11-20-seven-key-reachability.json)
identifies the inputs and peak memory. Run the seven-key reduced check with
`scripts/symbolic/check_source.py --growth`. For the concrete check, remove
`SYMMETRY TxIdSymmetric` from the expanded configuration and use `check.py`.

## Transaction commit

Lamport's published `transaction_commit/TCommit.tla` includes a global
consistency invariant. Its original three-manager configuration agrees with
the pinned TLC golden on success, 34 distinct states, 94 generated and depth 7.
With only the participant set expanded to 128, the same checker completed
depth 257 with over **10⁶¹ concrete reachable states** and 1,015 nodes in
the final decision diagram. The run took 119.66 s on the shared workstation.
There is no symmetry reduction. If nobody has committed, each manager can be
working, prepared or aborted. Once a manager commits, each manager must be
prepared or committed. The all-prepared state is in both regions, giving
exactly `3^n + 2^n − 1` distinct states. The [data for four domain sizes](../../conformance/reports/perf/2026-11-20-tcommit-domain-growth.json)
and `scripts/symbolic/check_transaction_commit.py` include generated counts,
depth and a validated negative control. The three-manager property forbidding
commit fails at depth five, after all three managers prepare.

### Isolated symbolic measurements

Three fresh processes per configuration ran sequentially on a fixed 2.4 GHz
c7i.metal-24xl with SMT and turbo disabled. Seven-key KVS reached depth 42 in
85.60 seconds median with symmetry and 114.80 seconds without. Transaction
commit with 128 managers reached depth 257 in 153.22 seconds median. Full
process times include binding, IR export and BDD construction. Every repetition
matched the independent counts for its configuration. The record includes all
18 samples, software digests and the measured worker revision’s scope.
[Isolated KVS and transaction-commit data](../../conformance/reports/perf/2026-11-20-isolated-symbolic-growth.json)

## Same-host Large comparison

Each cell ran alone on one EC2 m6id.4xlarge with eight physical Xeon Platinum 8375C cores, SMT enabled and local NVMe. The limit was 600 s and a 24 GiB cgroup with swap disabled. TLC used the pinned rolling jar and JRE, a 16 GiB heap and parallel GC. Explicit Core used the sharded table, packed parents and shorter safety batches, the TLC profile, seed 0 and fingerprint 0. These are bounded engineering runs on a different instance type from the release R2 host.

| Checker | Workers | Outcome | Distinct reported | Generated reported | Depth | Peak RSS, MiB |
|---|---:|---|---:|---:|---:|---:|
| Source-driven BDD | 1 | Completed in 200.27 s | 594,616,815 | 13,385,471,536 | 30 | 604 |
| Explicit Core | 1 | Time limit | 12,986,018 | 121,895,574 | 16 | 814 |
| Explicit Core | 8 | Time limit | 59,795,065 | 675,515,083 | 18 | 3,359 |
| TLC | 1 | Time limit | 12,181,287 | 113,927,331 | 16 | 5,073 |
| TLC | 8 | Time limit | 69,272,647 | 846,500,254 | 20 | 8,224 |

Core emitted final counters at its limit. TLC's counters come from its last periodic report, about 543 s into each run. At eight workers TLC had already progressed further by then. The symbolic process also checked a holding edit in 52 ms and a false edit with a validated witness in 51 ms. Its final exit status of 1 records that deliberately false edit. The BDD run counts transition witnesses symbolically. Each explicit run generates them individually.

### Repeat on the R2 reference hardware

On a dedicated EC2 c7i.metal-24xl, the source-driven checker finished the
unmodified Large model in **133.90 s wall time at 606.6 MiB peak RSS**.
The two original invariants held. Ordinary Core and pinned TLC both reached
the common 600-second deadline at one and eight workers. These are single
engineering runs. The table uses full process wall time and peak RSS from
GNU time. Core emitted its final counts at the deadline. TLC last sampled at
about 543 seconds. [Reference-host cells, CPU controls and tool identities](../../conformance/reports/perf/2026-11-20-kvs-large-reference.json).

| Checker | Workers | Process wall, s | Peak RSS, MiB | Distinct | Generated | Depth |
|---|---:|---:|---:|---:|---:|---:|
| Source-driven BDD, complete | 1 | 133.90 | 607 | 594,616,815 | 13,385,471,536 | 30 |
| Core, stopped | 1 | 600.03 | 704 | 10,952,063 | 101,737,056 | 16 |
| Core, stopped | 8 | 600.07 | 3,148 | 52,475,302 | 586,597,033 | 18 |
| TLC, last progress | 1 | 600.19 | 5,087 | 10,942,077 | 101,649,631 | 16 |
| TLC, last progress | 8 | 600.35 | 8,215 | 64,390,612 | 775,088,829 | 20 |

All processes used CPU affinity 0–15 on NUMA node 0 with SMT and turbo
disabled, the performance governor fixed at 2.4 GHz, a 24 GiB cgroup and
swap disabled. One cell ran at a time. The [cell runner](../../scripts/perf/scale/kvs-large-cell.sh)
records exit codes, process measurements and checker output in a new
directory. The source checker spent 132.889 s binding and checking. The
eight-worker explicit result shows that adding workers alone could not
account for the symbolic completion.

## Ordinary Core completes the original Large configuration

On the shared Ryzen workstation, the experimental eight-worker safety build
with sharded fingerprints, packed predecessor records and shorter candidate
batches completed Large in **6,693.531 seconds**. It reported success,
594,616,815 distinct, 13,385,471,536 generated, depth 30 and an empty frontier.
The sampled maximum process RSS plus swap was **18.31 GiB**. The run used
the fingerprint store, CPU affinity 0–7, a 7,200 s deadline and a 24 GiB
checker memory limit. These counts and depth independently match the symbolic
fixed point. Issue #1034 reported that the earlier checker stopped around
402,653,184 distinct states. The original issue and this completion used
different machines. The isolated resize experiment below measures the table
growth cost directly.

This was the captured ordinary safety candidate from before the temporal-pool
experiments. Its SHA-256 is
`9ef3ff1326eba796a316954f8111efa5432c6cba12fe42c36cc2c48fbf65cdbf`.
The ordinary eight-worker measurement used this invocation.

```sh
taskset -c 0-7 target/release/dive check "$KVS/MCKVS.tla" \
  --config "$KVS/MCKVSSafetyLarge.cfg" --workers 8 --fp 0 --seed 0 \
  --compat=tlc --time-limit 7200s --max-memory 24g --output json
```

On the merged storage implementation, a second run of the same command on
the shared workstation finished in **6,596.232 s** with **18.26 GiB** peak sampled
RSS plus swap. It agreed on success, distinct and generated counts, depth and
the empty queue. At the end the shared value store reported 20,242 nodes and
3.99 MiB of accounted storage. This is a component lower bound at teardown.
The process peaked at 18.26 GiB over the full search, which also retains seen
fingerprints, predecessors and queued states. The 600-second R2 comparison
above used a different host and budget. [Merged Core source, binary, input hashes, limits and counters](../../conformance/reports/perf/2026-11-20-kvs-large-main.json).

## Three ways to check an edit

- **Reachability.** Enumerate bounded action witnesses, compile guards and deterministic updates into Boolean functions, and compute exact images to a fixed point. Group writes by index and consume readers before overwriting their inputs. Cyclic dependencies use a simultaneous image. Symmetry counting requires a configured full group and checks that its generators preserve Init, every property and the multiset of guarded updates. Burnside counting preserves duplicate successor witnesses. Reverse images through BFS layers recover a shortest concrete counterexample. The CLI runs `dive validate-trace` on each reported violation.
- **Reuse a completed search.** Rebind and compile the edited input into the same BDD manager. Compare the concrete layout, initial predicate, symmetry and Boolean transition functions, including witness multiplicity. A match allows checking the new property directly on the reachable set. On Large, a holding property edit took 33 ms. A false property took 29 ms including independent validation of the three-state witness. Restricting Medium to open and rollback invalidated reuse and gave three distinct states, seven generated, depth three. A fresh explicit Core search agreed. Editing the imported OpenTx guard with the original entry point and configuration gave a validated initial deadlock. Adding a synthetic module with 1,024 live helper definitions of the same local property reused Medium’s closure in 0.23–0.30 s, including parsing and binding.
- **Prove safety by induction.** Use the conjunction of the configured invariants as a candidate predicate, or load a predicate from another process. Check Init inclusion, preservation under every source action, and implication of every configured invariant and requested deadlock-freedom condition. A failed obligation is inconclusive. Success proves safety for the finite configuration and supplies no exploration counts or diameter. The original Large invariants are inductive. The proof took 28 ms, and the same proof on a configuration expanded to 16 keys took 0.50 s. The full Large reachable predicate serializes to 24,809 bytes. A fresh process verified it against the original transitions and an added invariant in 52 ms, and against a restricted Next in 34 ms. The receiver checks the predicate against its own source. This is a candidate artifact for Sail to transport across workers.

The explicit Core edit experiment also captured 3,599 exact integer rows and parents. Rebinding and scanning a second property reused the completed closure, and a rebuilt transition produced 3,658 rows. Scanning took about 0.9 ms versus 12 ms for a fresh search, excluding binding. This gives an explicit alternative when a model has poor BDD structure.

### Which evidence can decide a source edit?

| Evidence retained | Eligible change | Measured edited check and ordinary comparison |
|---|---|---|
| Previously proved invariant inventory | Remove selected invariants from an exhausted ordinary safety proof. Constants, constraints, model source, policy and ordered subset must match. Aggregate state is eligible. | S7 LamportMutex, 724,274 states. Current bind and projection took 3.308 ms, versus 9.251 s fresh. The first complete proof cost 9.799 s. |
| Ordered exact states and parents | Change a safety predicate on the same admitted transition system. An exhausted closure can prove a holding predicate. A prior failure prefix can establish a currently validated earlier violation. | On a 200,002-state star, a late edited failure took 7.417 ms versus 103.215 ms fresh. A holding edit took 7.413 ms versus 104.208 ms. |
| Complete labelled graph | Change a temporal predicate on the same admitted state space and edges. Recheck due periodic boundaries, fairness, stuttering and the entire false-property set on the actual reported word. | On a 262,143-state tree with two due periodic checks, the edited check took 46.965 ms versus 241.009 ms fresh. On 16-state DieHard it took 1.914 ms versus 1.706 ms. |
| Previously proved first temporal property | Retain the original first property alone on the same closed pure program. Bind both inputs, compare the compiled first product and verify an authenticated, exhausted all-native execution with no reference fallback or periodic prefix. | On S7 internal memory, loading both inputs and comparing in a new process took 6.830 ms versus 64.914 ms fresh. With the previous binding available, the edit took 2.239 ms versus 63.369 ms fresh. |
| Symbolic reachable predicate | Rebind changed properties against the same finite layout, initial condition, symmetry and weighted transition relation. | On KVS Large, the current holding invariant took 35.952 ms. The edited failure took 34.408 ms including witness validation. The preceding fixed point took 131.080 s. |

The Rust comparisons used matching one-worker exact-store, state and
depth limits and unlimited process time and memory. They include current
binding. An installed caller also pays for transport, worker authentication
and receipt lookup. The symbolic row is a separate source-checkout experiment.
[Projection samples](../../conformance/studies/s7-invariant-projection.json),
[ordered-state samples](../../conformance/studies/edited-invariant-recheck.json),
[temporal graph samples](../../conformance/studies/edited-temporal-recheck.json),
[first temporal property samples](../../conformance/studies/s7-temporal-first-projection.json)
and [symbolic samples](../../conformance/reports/perf/2026-11-20-symbolic-qualification.json)
retain the timing scopes and individual runs.

### What an installed Sail worker saves

For the original S7 LamportMutex invariant removal, Sail authenticated a
previously completed receipt in a new process and bound the edit to the same
worker and transition program. The edited check took **22.46 ms** versus
**10.014 s** for fresh execution. The median paired speedup was **447.4×**
across five trials. Establishing the original receipt took 10.731 s.
The Rust-only 3.308 ms result above excludes process startup and transport.

In the installed batch API, counting the initial check and all edited checks,
the measured speedups were 2.14× for the safety star, 1.51× after a prior
refutation, 2.00× for the temporal star and 1.46× for the DieHard
fail-then-fix sequence. The original 100-check loop took 1.462 s with 76
exact receipt hits per trial. [Sail methods and reproduction](https://github.com/Chelis-Lang/sail/blob/e609cad17b45d6e8b3d1259d45561bd269a53041/docs/validation/temporal-reuse-s7.md#native-checks-after-substantive-edits) · [Every substantive edit, control and sample](https://github.com/Chelis-Lang/sail/blob/e609cad17b45d6e8b3d1259d45561bd269a53041/docs/validation/data/2026-11-20-native-edit-session.json) · [All 333 exact-replay checks](https://github.com/Chelis-Lang/sail/blob/e609cad17b45d6e8b3d1259d45561bd269a53041/docs/validation/data/2026-11-20-integrated-s7.json).

Sail also qualified the original S7 internal-memory property removal on
aggregate states. A new process checked the remaining `LivenessProperty` in
**15.565 ms versus 86.734 ms** fresh. All five trials agreed on the original
8 initial states, 21,400 generated, 4,408 distinct, depth 10 and empty queue.
Including establishment, the two-check session took 156.053 ms versus
231.862 ms. The same native build checked the mutex invariant removal in
20.347 ms versus 9.576 s fresh. The 100-check exact-receipt workload took
1.344 s and had 76 hits per trial. The separate-process figures use an
already established ordinary proof. [All five paired trials and Mac control](https://github.com/Chelis-Lang/sail/blob/main/docs/validation/data/2026-11-20-first-temporal-claim-native.json) · [All 333 repeat-workload results](https://github.com/Chelis-Lang/sail/blob/main/docs/validation/data/2026-11-20-first-temporal-claim-s7.json) · [Admission rules and reproduction](https://github.com/Chelis-Lang/sail/blob/main/docs/validation/temporal-reuse-s7.md#retaining-the-first-temporal-property).

### Rechecking public edit histories with Sail

Across nine chronological Disruptor project checks, separate installed Sail processes took 7.129 s when each check was fresh and 5.160 s when using authenticated edit reuse. The unchanged MPMC safety and liveness roots improved 59.76× and 66.85×. Sail resolves the loaded source and compares its exact bytes and origin before retaining a completed proof across a sibling edit. Changed selected code ran fresh. On the CCF history, all three unsupported requests still refused; removing repeated admission took the batch from 57.77 ms to 32.96 ms. [Input revisions, commands and five paired trials](https://github.com/Chelis-Lang/sail/blob/b8f084052d08928f234684f26cc9b1fea4ad950c/docs/validation/data/2026-11-20-installed-public-edit-histories.json).

The older two-writer Disruptor source refuted `NoDataRaces` at depth three, with four distinct and four generated states. The installed validator distinguished the model value `w1` from the TLA+ string `"w1"` when checking the trace. The corrected source rejected the two-writer assumption. [Model-valued witness, fresh and edited results, source pins and reproducer](https://github.com/Chelis-Lang/sail/blob/f19082a2b07bd5ba3fe107cfd84033eb1cb42768/docs/validation/data/2026-11-20-model-valued-witness.json).

## Explicit Core and temporal checking

Six of the thirteen original non-finishes reached a memory ceiling and seven reported feature refusals. The latter need support for the named constructs. Frontier width also differs within the memory cases. BPConProof had 41.07 million rows queued at 41.10 million distinct states. TLCSailfish2 reported an empty queue at its stop. A queue count alone cannot identify the retained allocation in that case.

| Mechanism | Controlled observation | Implication |
|---|---|---|
| Fingerprint table growth | At 402,653,185 inserted keys the original process peaked at 12.00 GiB RSS in 39.83 s. Independently growing shards peaked at 6.10 GiB in 36.24 s. Both passed insertion and lookup checks. | Doubling the original table at 402,653,185 insertions briefly needs roughly 4 GiB of old slots and 8 GiB of new slots. Sharding limits the overlap to one shard. |
| Frontier and candidates awaiting commit | At the same 800,000-state KVS Large prefix, 5,703,129 generated and 511,907 queued, reducing the ordinary eight-worker batch from 4,096 to 128 sources per worker cut RSS from 343 to 174 MiB. Wall time went from 3.03 to 3.29 s. The original frontier-spill path used 154 MiB and 4.87 s. | Candidate buffering accounts for much of the apparent benefit of spilling this prefix. The flat frontier itself still grows with layer width. |
| Parent and ordinal storage | Four million three-child BFS entries occupied 50,331,648 bytes of resident capacity before and 5,242,880 after delta coding with a 64-record block index. Wide ordinals and arbitrary parent order round-trip. | Regular discovery order compresses. The application can still seek to a predecessor for trace reconstruction. |
| Temporal capture scheduling | CoffeeCan3000 at eight workers went from 107.3 to 49.4 s with worker and claim-table reuse. One worker took 50.2 and 48.8 s. CoffeeCan1000 isolated pool reuse at 5.65 s, claim reuse alone at 10.29 s, and the original at 10.75 s. | Thread creation at every bounded wave accounts for much of the regression. Ordered graph commits and temporal work still limit parallel speedup. |
| Independent receipt delivery | Ten disjoint reachable regions of the pinned `c1cs` model contain 671,088,640 distinct postdecision states and 8,724,152,320 delivery edges. An unused four-way binder generates 34,896,609,280 witnesses for those edges. Twenty concrete paths validate, and a damaged path fails validation. | Materializing the full graph explicitly charges for every combination and duplicate witness. This construction covers those regions only. Termination and fairness obligations remain open. |

The ordinary eight-worker candidate reduced sampled peak memory on PConProof from 1,862 to 512 MB, with elapsed time increasing from 67.8 to 75.2 s. KVS Medium went from 1,335 to 812 MB and from 108 to 106 s. Each comparison used the same CPU affinity, input and golden, with other work on the host. At one worker the candidate took 574 s on PConProof and 585 s on Medium. Both benefit from parallel expansion. Extending the persistent pool to ordinary safety increased the two PConProof eight-worker measurements from 84–101 s to 99–109 s in a separate interleaved experiment. The measured candidate retains the pool for temporal capture.

Sharding alone reached **fewer** states in a sampled 650 MiB whole-process run, 4.76 million versus 6.07 million. Combining sharding, packed parents and shorter batches reached 6.38 million. Threshold samples depend on allocation timing. The isolated resize result is the reproducible claim about the table itself.

### When extra capture workers cost time

CoffeeCan starts with an unusually wide frontier and each action generates at
most one successor per source. Ordered commits still run on the capture thread.
For this shape Core now expands each source on that thread, avoiding worker
dispatch and collection. Requested widths of four and eight report one worker
used and give the actual number of retained initial states.

| Original configuration | Distinct / generated | Before, 1 / 4 / 8 workers (s) | After, 1 / 4 / 8 requested (s) |
|---|---:|---:|---:|
| CoffeeCan1000Beans | 501,500 / 2,000,002 | 9.68 / 14.69 / 18.61 | 9.60 / 9.60 / 9.58 |
| CoffeeCan3000Beans | 4,504,500 / 18,000,002 | 94.04 / 139.35 / 177.47 | 93.23 / 93.02 / 93.34 |

These are median process wall times from three trials per cell on the same
isolated CPU at 2.4 GHz. All 54 observations across the four models agreed
with their pinned TLC verdict, counts, depth and final queue. Flash and
Dijkstra have much smaller initial frontiers. Their baseline eight-worker
medians were 1.26 s and 8.77 s versus 1.13 s and 7.01 s at one. This change
targets the two CoffeeCan models. [Every sample, worker width and input digest](../../conformance/reports/perf/2026-11-20-temporal-worker-selection.json)

### Successor ordinals in temporal graphs

Replay needs the exact original successor ordinal on every generated edge,
including duplicate destinations. The graph also holds a stutter entry per
expanded state. Core stores ordinals below 65,534 in two bytes per entry.
One reserved value marks stutters, the other points to a sparse table with
the full ordinal indexed by physical edge. A source with many successors
continues to support full-width ordinals. The checked graph verifies the
table and source order before temporal analysis or reuse.

| Two-million-state prefix | Generated | Collector buffers, before / after | Peak process RSS, before / after | Wall seconds, before / after |
|---|---:|---:|---:|---:|
| BlockingQueuePoisonApple | 6,009,057 | 344 / 232 MiB | 699 / 601 MiB | 17.83 / 17.90 |
| c1cs | 41,819,682 | 2,152 / 1,256 MiB | 2,500 / 1,584 MiB | 76.95 / 78.18 |

These are three-trial medians on an isolated EC2 c7i.4xlarge. The dense
graph gains more because it retains more transition occurrences per state.
At these prefixes the measured benefit is memory capacity. The original
and compact representations stopped at identical counts, depths and queue
sizes. [All 18 measurements, controls and input hashes](../../conformance/reports/perf/2026-11-20-temporal-edge-ordinals.json)

At equal memory budgets, the original and compact c1cs captures reached
1,522,699 and 2,645,165 distinct states at 2 GiB (1.74×). At 20 GiB they
reached 20,394,916 and 23,794,977 (1.17×). All four stopped on the sampled
memory limit. The larger compact run generated 526,836,645 transitions and
still had 13,760,692 states queued. The same record includes the four
individual observations under `capacity`.

### Action labels in temporal graphs

Core retains one action ID per edge so it can distinguish named transitions during replay. For up to 255 action names, it stores a byte per edge, with 255 reserved for an unnamed entry. Larger action tables retain full-width IDs. Duplicate destinations, successor ordinals and implicit stutter edges keep their identities.

| Two-million-state prefix | Generated | Collector buffers, before / after | Mean peak RSS, before / after | Mean wall seconds, before / after |
|---|---:|---:|---:|---:|
| BlockingQueuePoisonApple | 6,009,057 | 232 / 176 MiB | 614 / 558 MiB | 18.04 / 18.53 |
| c1cs | 41,819,682 | 1,256 / 808 MiB | 1,582 / 1,224 MiB | 79.47 / 79.31 |

Both binaries already used narrow successor ordinals. The two rotations per binary agreed on every search counter within each model. Under the same 2 GiB sampled process limit, c1cs reached 2,644,879 distinct and 56,343,707 generated before, and 3,130,942 distinct and 65,470,460 generated after. Both stopped with a nonempty queue. The configuration with more transitions per state sees the bigger gain because each retained edge saves seven bytes. [Individual readings, terminal statistics, machine and hashes](../../conformance/reports/perf/2026-11-20-temporal-edge-ordinals.json)

At the 20 GiB sampled limit, the final action-label binary reached **38,066,281 distinct states** on c1cs. It generated 863,440,173 states and stopped at depth 18 with 22,309,692 queued. The earlier narrow-ordinal binary reached 23,794,977 distinct on the same instance type and core count. This 60% capacity gain still leaves the published model open. [20 GiB commands, full stop details and memory accounting](../../conformance/reports/perf/2026-11-20-temporal-edge-ordinals.json).

The native graph also stores original row IDs in blocks of 256 physical edges.
A block initially uses the representative row of each destination. On the first
edge whose persistent original differs, Core records explicit IDs for that
block, including its earlier edges. Seen aliases, generated self-loops and
synthetic stutters retain their original identities. The directory costs one
machine word per block. Each explicit entry costs four bytes.

| Two-million-state prefix | Retained original rows | Collector capacity, dense / blocks | Mean peak RSS, dense / blocks | Mean wall seconds, dense / blocks |
|---|---:|---:|---:|---:|
| BlockingQueuePoisonApple | 2,000,400 | 176 / 144 MiB | 560 / 539 MiB | 17.74 / 16.38 |
| c1cs | 2,000,000 | 808 / 554 MiB | 1,227 / 1,071 MiB | 78.67 / 78.68 |

Two trials per binary on the same eight physical cores produced identical
counts, queue sizes and depths at each prefix. The dense control already used
byte action IDs and compact ordinals. [Inputs, individual timings and collector capacities](../../conformance/reports/perf/2026-11-20-temporal-edge-ordinals.json)

### What occupies memory at the same discovery prefix?

Each of the six previously memory-limited configurations reached the same
300,000-state discovery limit at eight workers. For ordinary safety models,
a second process wrote the frontier to disk. Its generated, distinct, queued
and depth counters matched the resident run exactly. The persistent value
store interns TLA+ values shared by states. Its measured component bytes
are a lower bound before worker teardown. Peak RSS covers the whole process.
The measurements have different scopes and sampling times. Subtracting them
would give a misleading frontier size. [Ten fixed-prefix cells and component counts](../../conformance/reports/perf/2026-11-20-six-fixed-prefixes.json).

| Model | Queued | Value-store lower bound, MiB | Resident peak RSS, MiB | Disk-frontier peak RSS, MiB | Resident / disk wall, s |
|---|---:|---:|---:|---:|---:|
| KVS Large | 198,106 | 3.8 | 164 | 158 | 1.00 / 2.06 |
| DiskSynod | 147,238 | 1.5 | 230 | 177 | 2.12 / 3.47 |
| BPConProof | 299,867 | 179.2 | 767 | 778 | 3.60 / 4.55 |
| TLCSailfish2 | 217,815 | 91.1 | 234 | 231 | 9.28 / 10.21 |
| c1cs, temporal capture | 138,028 | 21.4 | 401 | — | 4.42 / — |
| BlockingQueuePoisonApple, temporal capture | 249,637 | 2.7 | 168 | — | 0.91 / — |

To repeat a prefix, run the pinned module and CFG from the data row with
`dive check MODULE --config CFG --max-states 300000 --workers 8 --max-memory 2g --time-limit 60s --compat=tlc --fp 0 --seed 0 --output json` on CPUs 8–15. For the disk arm add `--frontier-memory-bytes 0 --frontier-directory DIR` with a new directory on the local SSD.

For DiskSynod, spilling reduced peak RSS by 53 MiB at this prefix, at a cost
of 1.35 s. BPConProof queued almost every state and wrote 48 MB of frontier
records, yet its peak RSS rose by 11 MiB. Its interned-value slabs alone
occupied 145 MiB. A wide queue therefore warrants looking at both the
frontier references and the shared values those references keep alive.
For TLCSailfish2 the value store already accounted for at least 91 MiB.
These single samples support keeping the ordinary resident frontier as the
default and selecting the [existing spill option](../user/storage-recovery.md) for workloads where a
matched prefix demonstrates a useful reduction. They leave open the
component breakdown at tens of millions of states and the retained temporal graph.

Ordinary safety also offers [disk-backed seen-state membership](../user/disk-seen-safety.md) for fingerprints and exact keys. The indexed store uses one worker. Sorted fingerprint runs also support parallel expansion. Live TLA+ values and other search structures still consume memory. The paired measurements above changed only frontier placement.

### Resident frontier rows

Ordinary breadth-first safety search queues one handle for each variable in each discovered state. In the first million SlateDB admissions, many consecutive rows point to the same compound values. Storing 999,424 rows as 19 full-width handles took 152 MB. Encoding each variable relative to its first handle in a block took 30 MB in the offline layout experiment. A six-variable BPConProof prefix fell from 48 MB to 3.6 MB by the same measure.

Core now packs consecutive blocks of 256 rows in RAM. Each variable gets the smallest of zero, one, two, four or eight bytes needed for its differences. An incompressible block keeps full-width rows. Expansion recovers the original handles in discovery order. Store replacement remaps them before releasing their owner. The product effect depends on how much of the process those rows occupy.

| Original configuration | Distinct limit | Queued at stop | Mean peak MiB, original / packed | Mean seconds, original / packed |
|---|---:|---:|---:|---:|
| SlateDBWAL | 5,000,000 | 1,043,043 | 714 / 414 | 21.83 / 21.97 |
| MC_HDiskSynod | 3,000,000 | 1,177,825 | 338 / 196 | 44.78 / 44.22 |
| BPConProof | 3,000,000 | 2,997,546 | 1341 / 1110 | 11.77 / 11.84 |
| TLCSailfish2 | 1,000,000 | 772,636 | 523 / 508 | 39.85 / 39.73 |

Each pair averages two runs on eight physical cores of an isolated EC2 r7i.8xlarge. Both representations stopped with identical verdict, depth, generated, initial, distinct and queue counts in all 16 runs. TLCSailfish2 saved about 3% of whole-process RSS despite a much smaller representation for its rows. The fixed prefixes establish memory cost per search and leave the full configurations open. [Every sample, binary, input hash and layout comparison](../../conformance/reports/perf/2026-11-20-high-fanout-waves.json).

Each ordinary resident level also has consecutive predecessor IDs in admission order. Core checks this as it appends each ID and stores the first ID and count for the level. A separate eight-core comparison measured average peak RSS for the per-row ID vector and the checked range.

| Fixed prefix | Per-row ID / range, MiB | Per-row ID / range, seconds |
|---|---:|---:|
| SlateDBWAL, 12,000,000 distinct | 746 / 695 | 51.94 / 52.28 |
| BPConProof, 8,000,000 distinct | 1554 / 1462 | 30.32 / 30.25 |
| MC_HDiskSynod, 3,000,000 distinct | 202 / 180 | 44.99 / 44.87 |

Both builds used the same compressed frontier rows and reached identical search counters. The predecessor log still retains the parent and successor ordinal used to regenerate a concrete trace. Checkpoint recovery retains explicit IDs. [All 12 measurements](../../conformance/reports/perf/2026-11-20-high-fanout-waves.json).

At a **6 GiB sampled process budget**, the packed queue raised BPConProof's memory stop from 25,089,035 to 33,560,338 distinct states (34%) and TLCSailfish2's from 13,991,908 to 14,414,379 (3%). DiskSynod stopped on memory at 100,884,308 before packing. After packing it reached 124,170,333 when the 1,500 s budget expired, at 4.55 GiB peak RSS. SlateDBWAL stopped on memory at 94,702,286 before packing. Raising the experiment’s state cap from 100 million to 500 million let the packed run reach the same memory limit at 201,344,793 distinct states (2.13×), 749,768,207 generated and depth 60 in 604.66 s. Its queue still held 30,670,132 states. [All individual measurements and stop reasons](../../conformance/reports/perf/2026-11-20-high-fanout-waves.json).

## High-fanout parallel safety

A worker expands each source before the coordinator commits its successors in discovery order. The amount awaiting commit scales with **sources per batch × successors per source**. On BPConProof, 478 of the first million discovered states had left the queue, yet the checker had generated 1,119,749 states. Spilling frontier rows cannot release successor batches already under construction. Core now uses the previous completed level's mean fanout to target 65,536 successor occurrences per batch, with at least one source per requested worker. One source can still exceed that target.

| Model and discovery limit | Original resident, s / MiB | Adaptive resident, s / MiB | Original disk frontier, s / MiB | Adaptive disk frontier, s / MiB |
| --- | ---: | ---: | ---: | ---: |
| BPConProof, 300,000 | 4.43 / 1,068 | 0.90 / 318 | 4.99 / 1,038 | 1.38 / 282 |
| BPConProof, 1,000,000 | 4.64 / 1,143 | 2.68 / 473 | 6.35 / 1,034 | 4.28 / 420 |
| DiskSynod, 300,000 | 3.19 / 189 | 3.22 / 192 | 4.76 / 168 | 4.78 / 168 |
| TLCSailfish2, 300,000 | 12.72 / 228 | 12.88 / 231 | 13.60 / 212 | 13.74 / 216 |

Each cell is the median of three interleaved runs on the same isolated eight-core allocation. All arms reached identical generated, distinct, depth and queue counts. These were state-limited prefixes. DiskSynod and TLCSailfish2 showed little change.

On the unchanged BPConProof input, the 18 GiB sampled process limit stopped the resident checker at 83,955,924 distinct states in 289.73 s. Independent chunks reached the 100-million state limit in 583.00 s at 1.87 GiB peak RSS. Both shared the same 24 GiB hard limit including file cache. The uncompressed frontier wrote 49.69 GB. An ordinary resident rerun under the original 24 GiB budget was kernel-killed with no final counters. The independent uncompressed prototype reached 662,795,501 distinct, 1,411,214,748 generated, depth 4 and 662,194,098 still queued. It stopped after 4,453.30 s when its 320 GiB root volume filled. The checker reported a frontier write failure after committing 329.40 GB. At 501 million discoveries the cgroup had about 10.85 GiB of anonymous memory and 11.89 GiB of file cache. [Limits, counters, file-cache pressure and binary identities](../../conformance/reports/perf/2026-11-20-high-fanout-waves.json).

On a second node, storing BPConProof rows as independently decodable 64-row chunks and reclaiming the value store between batches used 355 MiB and 4.83 s at one million states. Its matched adaptive disk-handle baseline used 428 MiB and 4.31 s. Reclaiming only at level boundaries used 1,039 MiB and 6.74 s. The measured independent-chunk prototype underlies the opt-in [independent frontier](../user/storage-recovery.md#independently-stored-frontier-values). [All 132 trials, source pins, binary identities and counters](../../conformance/reports/perf/2026-11-20-high-fanout-waves.json).

At 100 million discoveries on BPConProof, independent chunks with LZ4 wrote **10.55 GB** versus **49.69 GB** with the same chunks uncompressed. That is 78.8% less disk traffic measured as bytes committed to the frontier. In one matched pair on an isolated c7i.4xlarge, process wall time was 534.03 s versus 553.51 s and peak RSS was 1.88 versus 1.89 GiB. Both produced 201,557,438 generated states, depth four and 99,914,031 queued, and both collected the value store 185 times. The state limit stopped each check. [Block samples, binary identities and paired measurements](../../conformance/reports/perf/2026-11-20-high-fanout-waves.json).

### Where the thirteen historical non-finishes stand

The original set contains six memory stops and seven feature refusals. The
[per-model outcomes](../../conformance/reports/perf/2026-11-20-large-model-outcomes.json)
preserve the original input identities, profiles, budgets, available counts
and termination reasons. Each row also includes its eight-worker ordinary
Core observation on integrated main, in both the default and nominal TLC
profiles, at 30 s and 2 GiB. These 26 short runs preserve the original
missing-golden grades. A pinned TLC golden is still unavailable for these
inputs. The compact graph, fixed-prefix and symbolic records linked above
provide comparisons at the same work or against an independent calculation.

| Original model or CFG | Outcome | What determines the next step |
|---|---|---|
| KVS Large | Ordinary Core exhausted 594,616,815 distinct states within the original 24 GiB / 7,200 s limits. The source-driven symbolic result agrees. | Fingerprint-table resize peaks and predecessor size mattered for explicit search. Key-local field dependencies matter for symbolic compression. |
| BPConProof | The isolated 24 GiB cgroup killed the checker. At the measured 300,000-state prefix, 299,867 states were queued and the persistent-value lower bound alone was 179 MiB. | A frontier on disk still references persistent values. Bound the live value store or give queued states an independent representation. |
| TLCSailfish2 | The 24 GiB cgroup killed the checker. Spilling the frontier at the fixed prefix changed peak process RSS from 234 MiB to 231 MiB. | Measure and reduce retained value graphs and evaluation working sets independently of frontier placement. |
| DiskSynod | At 7,200 s, Core had reached 299,192,687 distinct, 4,260,111,630 generated and depth 30, with 65,518,609 queued. The cgroup peak was 18.31 GiB. | At 300,000 distinct states, disk-frontier placement cut peak process RSS from 230 MiB to 177 MiB. The larger run includes membership and predecessor growth as well. |
| BlockingQueuePoisonApple, c1cs | Both ordinary 24 GiB runs ended in a kernel OOM. At equal work the compact ordinal column reduced RSS on both. | Edges per state and the width of each retained edge field dominate dense prefixes. c1cs still had 13.76 million states queued at the compact 20 GiB stop. |
| EWD998, EnvironmentController | Default Core reached 19,573,292 and 1,332,012 distinct states respectively before the 900 s capture deadline. | The earlier startup refusals are gone. Success would require exhausting the graph and finishing fair temporal analysis, or an independently validated counterexample. |
| Kafka KIP-996 liveness | Core lowered the original quantified fairness conditions with the default work budget. At the 180 s limit it was checking a periodic boundary at depth 4, with 181,983 distinct states, 36,103,626 generated and 165,409 queued. | Earlier runs exhausted predicate lowering before exploration. Reusing value-independent checks across equivalent lexical bindings cleared that limit. The graph remains incomplete. [Inputs, planner profile and bounded runs](../../conformance/reports/perf/2026-11-20-large-model-outcomes.json). |
| etcdraft | Default Core reached 70,655,639 distinct and 266,068,245 generated, depth 13, at the 18 GiB sampled limit. | 55,290,518 states were queued. Queue width and retained aggregate values are practical targets. |
| MCetcdraft | The original default run refused mixed record keys after 679 s. Canonical bag construction reached 47,609,227 distinct and 257,868,368 generated, depth 15, before the 18 GiB memory limit. | Message bags use structurally different records. Distinct keys and their multiplicities must survive construction and independent replay. |
| TestMCReachability | The released CFG reads the host environment. Six ordinary closed CFGs on the unedited source match TLC in both profiles, including the graph 1a deadlock. Graph 7 exceeded a 10 s observation. | The graph selected as input and the reachable checker states of the graph algorithm are separate quantities. |
| CarTalk Model 3 | In the default profile the ordinary CLI reports an assumption evaluation error at its finite enumeration ceiling. The nominal profile refuses oversized enumeration. The source-bound puzzle calculation returns the single solution 1, 3, 9, 27, 81. | This is a constant constraint-solving problem. An ordinary checker would need a general exact counting or constraint-solving route for this expression. |

Each 24 GiB rerun used a dedicated EC2 instance with eight physical cores.
For a kernel OOM there is no final checker report. The 900 s runs used an
18 GiB sampled limit inside a 24 GiB cgroup, leaving room for the checker to
report a stop. The two envelopes and the fixed-prefix study answer different
questions.

### Car Talk evaluates a constant expression

Lamport’s five-piece `CarTalkPuzzle.toolbox/Model_3` asks TLC to print
`AllSolutions` for a 121-pound stone. It configures neither Init nor Next.
The expensive expression filters 25,937,424,601 functions of five indices
before testing the ability to measure every target weight. BFS frontier or
fingerprint storage cannot improve this calculation.

The sole solution is **1, 3, 9, 27, 81**. The original predicate allows
placing each piece on either pan or on neither, giving coefficients −1, 0, 1.
For pieces summing to N, shifting every coefficient by one turns full signed
coverage of −N through N into coverage of 0 through 2N with digits 0, 1, 2.
Suppose the sorted prefix sums to S and covers 0 through 2S. Its next weight
w adds two translated copies starting at w and 2w. All three intervals touch
or overlap exactly when w ≤ 2S+1. If w is larger, no later weight can fill
2S+1. This proves the prefix test in both directions. Five weights give at
most 243 sums. Achieving all 243 requires the largest admissible weight at
every position.

`python3 scripts/perf/scale/cartalk.py "$MODEL_3" --check-small` checks all
three original input hashes and evaluates this expression using the prefix
test. The control compares every solution against literal disjoint-pan
enumeration on 64 smaller domains. With seven pounds and three pieces,
pinned TLC and the prefix calculation both return exactly (1,1,5), (1,2,4)
and (1,3,3). This is a model-specific expression result. Ordinary Core
continues to refuse the oversized function enumeration. An experimental
streaming evaluator eliminated the bulk allocation but still tried every
tuple. During printing, tuple hashing forced the deferred filter through an
infallible path that could outlast the budget. The normal checker retains
its original refusal.

## Published model sizes and reproducible comparisons

A published state count is comparable across checkers when the specification,
configuration, properties, reductions and termination condition match. TLC
counts each generated successor occurrence separately from the distinct
states retained. A compact symbolic set can represent many concrete states at
once. The memory costs depend on the dependencies among the fields and the
size of intermediate sets. Specification length measures a third quantity.

| Study and configuration | What was checked | Historical result | Reproduction status |
|---|---|---|---|
| SlateDB WAL, three writers with restarts | Six safety invariants under the author’s symmetry group | About 49 billion unique states in 2 h 37 min with 40 workers on a Threadripper 9980X. Exact counts, engine version and final log are unreported. | Public MIT-licensed module and CFG. Dive and TLC controls and bounded original runs are recorded below. [Author’s model-checking notes](https://github.com/Vanlightly/s3-wal-collection/blob/35fdc537af4d226f43b4574cded1764375f264ac/slatedb/SlateDBWAL_notes.md#model-checking) |
| EmergencyCall, unconstrained | Original cellular state machine, 36 configurable variables | 10.59 billion distinct, diameter 26 transitions. The paper gives separate times for constrained cases. | Public source. The released f1–f4 runs have individual configurations and final counts. The unconstrained count lacks an identified matching configuration and final log. [Hou et al., §3](https://users.cs.northwestern.edu/~ychen/Papers/MobiSys21_ENL_OpenAccessVersion.pdf) |
| EmergencyCall, f3 | Three safety invariants, 49,152 distinct initial states | 21,255,936 distinct, 135,871,232 generated, TLC depth 27. The released TLC log records six minutes. | Reproduced by ordinary Core and pinned TLC at one and eight workers. [Authors’ configuration and raw output](https://github.com/FormalCellular/EmergencyCall/tree/093684f1bb02d733cae95d50f1690f1cb792296a/model) |
| PaxosLease, quarantine bound | Original passing consensus config with two proposers, the released constants, state constraint and invariants | 76,749,192 distinct, 391,849,185 generated, depth 43 in 10 min 55 s in the author log. | Both ordinary Core and pinned TLC reproduced all counters and the successful verdict at eight workers. [Trencséni](https://arxiv.org/abs/2609.14640) · [Released inputs and raw results](https://github.com/mtrencseni/paxoslease-revisited/tree/5ca959624c5a2560ab1ad86dc71e8b161621d9bc) |
| PaxosLease, Retry and RenewRetry | The same module with original retry configurations | The author logs report 2,179,760,458 distinct and depth 48 in 7 h 03 min for Retry, and 2,411,951,538 distinct and depth 49 in 8 h 03 min for RenewRetry. | Core and pinned TLC also completed both configurations on 32 physical cores. The tables below give the counters for each run. The author’s logs give the fingerprint polynomial and estimated collision probability for each run. [Retry](https://github.com/mtrencseni/paxoslease-revisited/blob/5ca959624c5a2560ab1ad86dc71e8b161621d9bc/results/retry.txt) · [RenewRetry](https://github.com/mtrencseni/paxoslease-revisited/blob/5ca959624c5a2560ab1ad86dc71e8b161621d9bc/results/renew-retry.txt) |
| AWS industrial example | Historical industrial TLC check | Kukovec attributes roughly 31 billion explored states over five weeks on one 16-vCPU EC2 instance to Newcombe. | The available account does not identify the generated/distinct convention or a released matching configuration. It sets a historical scale target only. [Kukovec, p. 25](https://repositum.tuwien.at/bitstream/20.500.12708/209338/1/Kukovec%20Jure%20-%202024%20-%20SMT-driven%20techniques%20for%20verifying%20distributed%20systems.pdf) · [Newcombe, ABZ 2014](https://abz-conf.org/publication/newcombe14/) |
| Pasture, node model | Invariants with parameters 2, 1, 2, 2, 2 | 4,234,887,880 distinct, depth 53 in 3,596 minutes on a 48-core Opteron with 128 GB. A repeat using another fingerprint polynomial found the same count. | The publication prints the modules. A run from a verified machine-readable original is outstanding. Its smallest published control is 47,742 distinct at depth 31. [Rodeheffer and Kotla, Table 1 and appendices](https://conf.tlapl.us/2012/rodeheffer-kotla.pdf) |
| Tunable replicated set, three replicas | Two operations per replica over three possible elements | About 9 billion generated states and 750 GB of storage when the host ran out of disk. The study reports completed times for eight smaller cases. | The thesis supplies a set specification and the author's public implementation. Identifying the exact generated module and Toolbox configuration would allow a matching replay. [Rijo, §5.5 and Table 5.1](https://run.unl.pt/server/api/core/bitstreams/d2570d62-6972-4877-a791-636d876d0096/content) · [Author’s implementation](https://github.com/AndreRijo/T-CRDTs) |
| LuPastry, simplified five-node configuration | Distributed hash-table routing | 141,786,762 distinct, 7,070,872,610 generated, depth 54 in one week. The full four-node run stopped after over five weeks and 331,300,918,096 generated. | Original configuration unverified. Later LuPastry+ specifications contain changes. [Lu, Table 5.3](https://doi.org/10.22028/D291-26550) · [Subsequent model versions](https://homepages.loria.fr/SMerz/projects/pastry/index.html) |
| Confidential Consortium Framework | Production consensus specification | 1,134 lines and 13 state variables. The paper describes model checking, trace validation and CI. | A maintenance benchmark requires pinning revisions, configuration and the exact edited property. [Howard et al., Table 1](https://www.microsoft.com/en-us/research/uploads/prod/2024/07/nsdi25spring-final392.pdf) |

These records establish a lower bound on published workloads. A global maximum
would require knowing the counting convention and input of every contender,
including proprietary models. The enlarged KVS and transaction-commit runs
above demonstrate concrete reachable cardinalities on their own programs.

### Reproduce the ZooKeeper protocol specification

Three completed no-restart configurations reproduce the state counts and diameters in Ouyang et al., Table 1. The paper’s count matches **generated** states in each. With four servers, two transactions and one timeout, Core and pinned TLC exhausted the queue at 6,634,696 distinct states, 21,393,294 generated and depth 64. Core took 623.70 s and 452 MiB peak RSS. TLC took 136.24 s and 46.3 GiB on the same host. Core used one of the 32 requested workers because the released specification contains twelve diagnostic `PrintT` calls. The completed Core run emitted no lines. With two timeouts the paper reports 1,599,588,210 states at its 24-hour cutoff. [Paper, Table 1](https://arxiv.org/pdf/2302.02703#page=7).

The release supplies the module and a Toolbox guide. The reproduction record supplies the missing wrapper and configuration template, the ten invariants, hashes, individual measurements and unsuccessful reconstructions. Restart increments the source’s timeout counter as well as its restart counter. Treating the published timeout parameter as a joint or an independent allowance produced different state spaces. Neither interpretation reproduced the two-server restart rows. [Source and reconstruction data](../../conformance/reports/perf/2026-11-20-zookeeper-reference.json).

### Reproduce the released SlateDB WAL model

Use the pinned `slatedb/SlateDBWAL.tla` and `SlateDBWAL.cfg`. All runs retain the six invariants, the garbage collector, symmetry and the original action definitions. The four comparative configurations with two values vary just the writer count and the maximum restarts. The one-writer smoke control also reduces the value domain. Each comparison used sequential fresh processes on an isolated r7i.4xlarge. The larger configurations used eight physical cores and the smoke control used one.

| Writers / values / restarts | Engine | Outcome | Distinct | Generated | Depth | Wall, s |
|---|---|---|---:|---:|---:|---:|
| 1 / 1 / 0 | Core | complete | 268 | 460 | 27 | 0.02 |
| 1 / 1 / 0 | TLC | complete | 268 | 460 | 27 | 1.21 |
| 2 / 2 / 0 | Core | complete | 74,637 | 198,123 | 69 | 0.25 |
| 2 / 2 / 0 | TLC | complete | 74,637 | 198,123 | 69 | 2.01 |
| 2 / 2 / 1 | Core | complete | 3,761,377 | 12,508,420 | 93 | 13.78 |
| 2 / 2 / 1 | TLC | complete | 3,761,377 | 12,508,420 | 93 | 31.73 |
| 3 / 2 / 0 | Core | complete | 30,114,878 | 117,007,147 | 114 | 163.11 |
| 3 / 2 / 0 | TLC | last progress before timeout | 23,054,567 | 86,651,441 | 67 | 300.19 |
| 3 / 2 / 1 | Core | stopped, time | 40,059,666 | 139,660,563 | 51 | 180.06 |
| 3 / 2 / 1 | TLC | last progress before timeout | 12,422,969 | 41,624,289 | 45 | 180.16 |

Restarts raised the completed two-writer graph from 74,637 to 3,761,377 distinct states. Adding a third writer with restarts disabled raised it to 30,114,878. A separate completed TLC run matched Core’s 117,007,147 generated, depth 114 and empty queue for that configuration. The three-writer, one-restart original remained open at the 180 s cutoff. Holding just one packed 64-bit fingerprint for each of the reported 49 billion states would occupy about 365 GiB before table overhead, predecessor records and the frontier. Under an independent uniform 64-bit hash assumption, that many distinct states would yield about 65 colliding pairs in expectation. An exact-state or symbolic control is important at this scale. [Configuration edits, input hashes and all measurements](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

Allowing two writer restarts instead of one raised the complete two-writer graph from **3,761,377 to 98,416,351 distinct states**. Core and TLC independently generated 398,235,951 states, reached depth 114 and exhausted the queue. On the same eight physical cores, Core took 396.33 s at 1.65 GiB peak RSS and TLC took 804.47 s at 5.02 GiB. With all three original writers and two restarts, Core reached a 30-million-state prefix in 135.87 s, with 102,653,606 generated and 6,184,868 still queued. That larger model's total size remains unknown. [Pinned configurations, per-engine commands and measurements](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

#### Symmetry comparison costs

A SlateDB successor changes a subset of 19 variables. Selecting its symmetry representative can compare the same function domains and nested values many times. Core now compares borrowed slices and stops reading at the first differing entry. It also reuses per-worker buffers for competing images. On the original configuration at five million discoveries, the three-run median fell from 19.97 to 17.92 s at eight physical cores, and from 10.12 to 9.62 s at 32. All twelve terminal reports agreed on counts, depth and pending queue. [Individual runs, profile and binary identities](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

#### Worker count and storage on the original configuration

Eight Core runs on one isolated r7i.16xlarge stopped at exactly 5,000,000 distinct states. All eight generated 16,458,236 states, reached depth 41 and left 1,043,043 queued. Each row gives the median of two fresh processes with one logical CPU per physical core. Bytes written are cumulative per run.

| Workers | Storage | Wall, s | Peak RSS, MiB | Spill bytes written |
|---:|---|---:|---:|---:|
| 8 | Resident | 20.72 | 714 | 0 |
| 32 | Resident | 10.91 | 1,183 | 0 |
| 8 | Only predecessors on disk | 25.18 | 701 | 80,000,024 |
| 8 | Only frontier on disk, handle rows | 63.10 | 347 | 1,840,000,656 |

At this prefix, four times as many workers gave 1.90× the throughput and used 66% more peak RAM. Spilling predecessors saved 13 MiB. Spilling the frontier saved 367 MiB at a cost of about three times the elapsed time. The two spill settings ran separately at eight workers. These figures apply to the ordinary handle frontier. The independent compressed frontier has a separate [BPConProof comparison](#high-fanout-parallel-safety).

The WAL specification has 19 state variables. Each resident queue row in this safety search holds 19 eight-byte handles and an eight-byte discovery ID, or 160 bytes. The 1,043,043 pending rows alone represent about 159 MiB of payload. BFS retains space for the current and next levels, including slots for parents already expanded during the current level. By comparison, Core reported about 2 MiB of measured retained value-store memory in both resident worker configurations. Many frontier rows therefore share underlying values while still paying for every handle in every row. [Queue implementation](../../crates/dive-explore/src/lib.rs) · [Handle representation](../../crates/dive-core/src/handle.rs).

For the full target, membership capacity changes the choice of machine. With 49 billion uniformly distributed fingerprints, the resident table's 256 shards would each require 2²⁸ eight-byte slots under the 75% growth threshold, or 512 GiB for slots alone. On a 64-bit host the packed resident parent log adds at least 57 GiB for one byte per discovery and one 16-byte index entry per 64 discoveries. The fixed-width disk parent log would write 784 GB at that count. These are size calculations. The live frontier, resize peaks and worker allocations add to the resident requirement. The indexed disk store caps membership at 4,294,967,295 entries. Sorted runs use a global 64-bit count and a streaming audit. A capacity bound alone does not establish a completed run. [Fingerprint table](../../crates/dive-explore/src/lib.rs) · [Packed parents](../../crates/dive-store/src/predecessor/packed.rs) · [Disk membership limits](../user/disk-seen-safety.md).

#### Three writers and one value to exhaustion

Changing only `Values` from two values to one retains all six invariants,
symmetry, three writers and one restart. Both checkers exhausted the queue at
**242,213,642 distinct states, 1,068,416,158 generated and depth 126**.
Core took 551.82 s at 8.70 GiB peak RSS. Pinned TLC took 2,113.87 s at
44.62 GiB. The two processes ran consecutively on the same 32 physical cores
of an isolated r7i.16xlarge. For this configuration, Core was 3.83× faster.
Both searches used fingerprints with seed and polynomial zero. TLC estimated
collision probabilities of 0.011 (optimistic) and 0.026 (actual gap).
The earlier 480-second Core run reached 208,042,815 distinct at depth 88.
[Commands, pins and individual measurements](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

#### Resident fingerprint membership

Core partitions the resident visited set into 256 shards. For a large shard, the shard number supplies the high eight bits of each fingerprint. Its home slot supplies low bits, recoverable from the physical slot and a four-bit displacement. Each primary entry stores only the remaining bits and the displacement. A cluster longer than fifteen slots retains additional full keys in an overflow table. The checker can reconstruct every existing fingerprint when a shard grows.

At 402,653,185 distinct synthetic keys, the checker’s membership table took 3.78 GiB peak process RSS, down from 5.93 GiB. The same inserts, duplicates and misses took 93.25 s versus 79.13 s with full-word slots. On ordinary Core the two SlateDB trials at 20 million discoveries averaged 1,182 MiB versus 1,261 MiB and 55.74 versus 55.04 s. DiskSynod at 10 million averaged 419 MiB versus 497 MiB and 90.67 versus 89.46 s. Every paired search agreed on its generated and distinct counts, depth and pending queue. [All trials, stream recipe and input hashes](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

At 49 billion uniformly distributed fingerprints, a shard would hold about 191.4 million keys. A separate run put exactly 191,406,250 distinct keys into one real shard and looked up every key after all resizes. With four-byte primary slots, final process RSS fell from 2,051 to 1,099 MiB and peak RSS from 3,075 to 1,703 MiB. Wall time was 45.78 and 45.18 s. This exercises the target shard size directly. Whole-model memory also includes the other shards, frontier and predecessor log. [Shard input and process measurements](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

#### External fingerprint membership

On the original SlateDB configuration, two paired trials per width reached
5,000,000 distinct and 16,458,236 generated at depth 41 with the same
1,043,043 states queued. Each cell used the same binary on an isolated
r7i.16xlarge. Elapsed time includes normal checker startup and cleanup.

| Physical cores | Store | Wall seconds | Peak RSS (MiB) |
| ---: | --- | --- | --- |
| 8 | Resident | 20.10, 19.75 | 718, 711 |
| 8 | Sorted runs | 30.46, 30.87 | 622, 618 |
| 32 | Resident | 10.16, 10.16 | 1,185, 1,187 |
| 32 | Sorted runs | 17.71, 17.79 | 1,106, 1,105 |

A replay of the 23,269,077 ordered membership operations from this prefix
isolates the tradeoff. With a 65,536-key hot set the standalone prototype
used about 8 MiB RSS versus 112 MiB for a resident hash set. It took 5.22
and 5.25 s versus 1.18 s in both resident trials. Buffered page reads and
merges cost time. In the full checker, the frontier, predecessor log and
workers remain, which explains why peak process memory fell by much less
than the standalone ratio. The prefix stopped at its discovery limit.

A negative lookup used to read a 4 KiB page even when none of its 512 fingerprints could match. Each smaller run now keeps a blocked Bloom filter for each page. A negative hint skips that read. A possible match still checks the stored key, and the closing audit streams all keys. On the same trace, page reads fell from 2,908,642 to 886,751 with identical decisions. Under ordinary parallel Core on 16 physical cores, 20 million discoveries took 118.16 s before and 74.06 s with filters. Mean peak RSS moved from 1381 to 1402 MiB. Both figures average two runs per build. The filtered comparison and the earlier resident comparison used different hardware.

The unfiltered external checker also exhausted the full one-value configuration on a separate r7i.16xlarge with 32 physical cores. It matched resident Core and TLC at 242,213,642 distinct, 1,068,416,158 generated, depth 126 and an empty queue. Its 2,322.19 s includes auditing all fingerprints and removing the run files. Peak process RSS was 3.99 GiB and peak cgroup memory was 5.64 GiB. Resident Core used 8.70 GiB and finished in 551.82 s. Before the closing audit the external key collections occupied 11.21 MiB and active runs contained 1.94 GB. The native minimum fingerprint gap was 127.

On that same machine the filtered external checker finished in **1,431.80 s**, down from 2,322.19 s. Both runs audited all 242,213,642 stored keys, found the same minimum gap and reported the same depth, generated count and empty queue. Filtering raised peak RSS from 3.99 to 4.06 GiB and peak cgroup memory from 5.64 to 5.69 GiB, including the filesystem cache. For this model, filtered disk membership took 2.59× the wall time of resident Core with less than half its peak process RSS. [Both complete runs and the resident/TLC controls](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

With no discovery limit, eight workers exhausted the two-writer, one-restart
configuration using sorted runs. Core and pinned TLC both reached 3,761,377
distinct, 12,508,420 generated and depth 93. The small exact-state control and
both fingerprint checkers also agreed on 268 distinct and 460 generated at
depth 27. These controls establish the counters on smaller complete graphs.
[Commands, all trial measurements and binary identities](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

### Reproduce the released PaxosLease runs

Use the source pin and JVM settings in the [PaxosLease record](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).
From the author's `tla/spec` directory run `dive check PaxosLeaseChecked.tla --config PaxosLeaseQuarantineEqualsProposer.cfg --compat=tlc --workers 8 --fp 0 --seed 0`.
Use the same module and `-config` with pinned TLC. For Base and QuarantineEqualsProposer,
each checker ran once on the same isolated r7i.16xlarge, pinned to the same
eight physical cores. Core finished Base in 1.77 s and the quarantine case in
243.70 s. TLC took 4.62 s and 332.45 s. On the quarantine case, peak process
RSS was 3.89 GiB for Core and 15.58 GiB for TLC. Both matched the author's
verdict, distinct, generated, depth and exhausted queue on both configurations.

The symbolic strategy checks the five ground assumptions and admits the state constraint. The unedited models exceed the ordinary 512-bit dense layout. The optional [reachable set-domain refinement](../user/symbolic.md#learning-which-members-a-wide-set-needs) can discover a smaller exact alphabet directly from the source. Pinned TLC evaluated `<<Cardinality(Msg), MaxNetwork>>` with the released constants: Base has 1,008 possible messages, QuarantineEqualsProposer 608, and Retry and RenewRetry 1,320 each. All limit the admitted network to four messages. To reproduce the diagnostic, extend `PaxosLeaseChecked, TLC`, print that pair in a ground assumption, and run the original Init with a single stuttering step. These are type cardinalities. A sparse representation must preserve set equality, duplicate message elimination, all generated witnesses, and invariant checking on emissions beyond four. [Symbolic state-constraint semantics](../user/symbolic.md#admission-and-reports).

For Retry, use the original configuration with its `StateConstraint` and
all its invariants. From the same directory, set `TLA2TOOLS_JAR` to the pinned jar and
`TLC_SCRATCH` to a fresh path on the scratch volume, then run

```sh
dive check PaxosLeaseChecked.tla --config PaxosLeaseRetry.cfg \
    --compat=tlc --workers 32 --fp 0 --seed 0 --max-memory 400g --time-limit 14400s --output json
java -Xmx192g -XX:+UseParallelGC -XX:ActiveProcessorCount=32 -jar "$TLA2TOOLS_JAR" \
    -workers 32 -fp 0 -seed 0 -metadir "$TLC_SCRATCH" -config PaxosLeaseRetry.cfg PaxosLeaseChecked.tla
```

Each process had its own isolated r7i.16xlarge in the same availability zone,
32 physical cores, a 448 GiB cgroup and zero swap. TLC had a 1 TiB gp3 volume
for its disk-backed queues. Times are single whole-process measurements at the
default guest clock settings. The author used eight workers, TLC 2.19 and
fingerprint polynomial 123. Both reproductions used polynomial 0.

| Retry run | Distinct | Generated | Depth | Wall time | Peak process RSS |
|---|---:|---:|---:|---:|---:|
| Author’s TLC | 2,179,760,458 | 12,642,614,099 | 48 | 7 h 03 min | Unreported |
| Core | 2,179,760,458 | 12,642,614,099 | 48 | 4,873.74 s | 99.73 GiB |
| Pinned TLC | 2,179,760,458 | 12,642,614,099 | 48 | 5,336.64 s | 94.16 GiB |

All three processes exhausted the queue and reported success. The two
fingerprint polynomials yielded the same final counts and depth. On the matched
instances Core finished 7 min 43 s sooner and used 5.57 GiB more peak RAM.
TLC also used disk. At 2.166 billion distinct states its two fingerprint files
occupied 16 GiB in total, in addition to its queues and trace files.

The Core report counts 12.64 billion seen-set probes and 9.33 billion
successors discarded as already seen before interning. The cost of checking
duplicates still grows with the number of generated transitions. The ordinary runs retain the authors’ state constraint and module assumptions.

For RenewRetry substitute `PaxosLeaseRenewRetry.cfg` in both commands. The only change to the original Retry configuration enables lease renewal. Each engine ran on a fresh instance with the same 32-core allocation and limits.

| RenewRetry run | Distinct | Generated | Depth | Wall time | Peak process RSS |
|---|---:|---:|---:|---:|---:|
| Authors’ TLC | 2,411,951,538 | 14,096,750,859 | 49 | 8 h 03 min | Unreported |
| Core | 2,411,951,538 | 14,096,750,859 | 49 | 5,264.86 s | 118.17 GiB |
| Pinned TLC | 2,411,951,538 | 14,096,750,859 | 49 | 5,422.06 s | 94.60 GiB |

All three runs reported success and exhausted their queues. The final counts and depth agreed. The author used fingerprint 95; the two reproductions used fingerprint 0. At 2.147 billion distinct states the pinned TLC’s two fingerprint files occupied 16 GiB. [Terminal counters, memory, collision estimates and reproduction settings](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

With `--strategy symbolic --type-invariant TypeOK --refine-domains`, one physical core learned 138 of the 1,008 possible messages on Base and 80 of 608 on QuarantineEqualsProposer. It matched all explicit counters at depths 29 and 43, in 46.97 s / 324 MiB and 405.84 s / 1.14 GiB respectively. Ordinary Core took 1.77 s and 243.70 s with eight workers. Ordering message bits with the acceptor fields reduced the quarantine reachable-set diagram from 8,532,809 nodes to 630,485. The otherwise comparable variable-order trials took 1,664.30 s and 432.51 s on separate instances of the same size. The unedited symbolic Retry run ended with `stopped` (time limit) after 1,800.03 s, with at least 3.17 GiB observed on a live worker.

For source-edit sessions, `--refine-domains --recheck MODULE CONFIG` can supply set coordinates to the next independently bound search. In the first transfer experiment, repeating Base took 41.80 s with one refinement attempt, compared with 64.16 s and eleven attempts cold. Supplying the same Base coordinates to QuarantineEqualsProposer reduced attempts from eleven to three, but wall time rose from 486.75 to 528.99 s. Its final layout contained 150 coordinates instead of 80 and peak RSS fell from 1.14 GiB to 737 MiB. Both configurations kept their exact counts and depths. On a separate c7i.4xlarge, the installed CLI repeated Base in 40.70 s after a 61.32 s cold search. The changed configuration took 520.86 s seeded and 481.88 s cold. Fewer restarts alone do not predict faster searches when an edit changes the reachable message alphabet. [Individual observations and host sharing](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

If only the obligations change, the checker can keep the completed reachability proof in its worker. On the same PaxosLease Base input, repeating the check took 0.60 s versus 47.73 s with coordinates alone. Adding the true `EmissionBound` invariant took 4.50 s versus 48.84 s. Both returned 491,037 distinct, 1,709,874 generated and depth 29. Adding the false `EmptyNetwork` obligation took 0.88 s and produced a two-state trace accepted by the independent validator. Initial cold checks took 72.52 and 72.21 s. [Per-edit measurements, source hashes and replay](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

Adding an unconditional self-loop to the released PaxosLease Base relation raised the generated count from 1,709,874 to 2,200,911. Replacing that loop with a second Tick action gave 1,804,652. Removing the duplicate restored the original count. All four revisions exhausted 491,037 distinct states at depth 29. Rechecking the three edits took 1.46, 1.67 and 0.93 s versus 43.79, 41.86 and 42.05 s with fresh searches that reused only the finite coordinates. [Model edits, exact counts, memory and per-revision times](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

### Reproduce the released EmergencyCall runs

The paper reports 10.59 billion distinct states and a maximum diameter of 26 for the unconstrained study. It gives timings for six constrained cases. The released repository contains seven original CFGs and six solution CFGs. Its `s0.cfg` uses `Init0 == Init /\ System_Constraint`; the released log ends at an F1 violation after 7,892,939 distinct states, depth seven and 203 seconds. A like-for-like comparison at 10.59 billion still needs the original initial predicate, properties and completion conditions. [Paper, §3.2](https://users.cs.northwestern.edu/~ychen/Papers/MobiSys21_ENL_OpenAccessVersion.pdf) · [Configuration inventory and log digest](../../conformance/reports/perf/2026-11-20-emergencycall-reference.json).

Check out the [pinned authors’ repository](https://github.com/FormalCellular/EmergencyCall/tree/093684f1bb02d733cae95d50f1690f1cb792296a).
Set `MODEL` to its `model` directory and `INPUT` to the same binary, JRE and
jar layout used for the KVS reference comparison. Run cells individually from
a 24 GiB cgroup on the reference hardware. Each requires a new result directory.

```sh
scripts/perf/scale/kvs-large-cell.sh core 8 "$INPUT" "$OUT/f3-core-8" \
    "$MODEL/EmergencyCall_Model_auto.tla" "$MODEL/config/f3.cfg"
scripts/perf/scale/kvs-large-cell.sh tlc 8 "$INPUT" "$OUT/f3-tlc-8" \
    "$MODEL/EmergencyCall_Model_auto.tla" "$MODEL/config/f3.cfg"
```

With f3, Core took 394.44 s versus TLC’s 580.68 s at one worker and
178.86 s versus 111.79 s at eight. Both checkers matched the authors’ verdict,
counts and depth. Going from one worker to eight sped up Core 2.21× and TLC
5.19×. Core used 1.94 GiB peak RSS at eight workers, TLC 7.24 GiB. Timings
are one process per cell on the same isolated, fixed-frequency EC2 host.

All 16 checks agreed on the verdict and final queue, counts and depth with the corresponding author log. Each timing is full process wall time in seconds. [All cells, RSS, CPU settings and input digests](../../conformance/reports/perf/2026-11-20-emergencycall-reference.json).

| Configuration | Distinct | Generated | Depth | Core 1 | TLC 1 | Core 8 | TLC 8 |
|---|---:|---:|---:|---:|---:|---:|---:|
| f1 | 3,932,160 | 23,650,304 | 19 | 150.95 | 126.91 | 113.38 | 45.56 |
| f2 | 6,248,960 | 39,053,696 | 25 | 183.39 | 185.94 | 123.11 | 53.77 |
| f3 | 21,255,936 | 135,871,232 | 27 | 394.44 | 580.68 | 178.86 | 111.79 |
| f4 | 8,872,832 | 56,734,720 | 27 | 223.76 | 259.74 | 131.62 | 63.59 |

On the historical CCF single-node commit control, ordinary Core found the original property violation after 17 distinct states, 20 generated and depth five. The three revisions with temporal obligations received a named refusal from symbolic safety checking. The [Sail evidence](https://github.com/Chelis-Lang/sail/blob/df6ffaca82b3aacdea301616411de1c0c726b12f/docs/validation/data/2026-11-20-symbolic-predicates.json) pins the production inputs and records the successful five-key KVS maintenance comparison separately.

## Reproduce the comparisons

Build the checker's exporter and validator and install the Python dependencies
using the [source-checkout instructions](../user/symbolic.md#run-a-check).
The ordinary Core comparisons used release builds with LTO disabled and 16
codegen units. Fetch the pinned corpus with
`cargo run --release --locked -p dive-conformance -- corpus fetch --corpus examples`.
The differential script resolves its default KVS directory from the corpus pin
and `DIVE_CACHE_DIR` (normally `~/.cache/dive`). For the remaining commands,
set `KVS` to that checkout's `specifications/KeyValueStore` directory.

```sh
PY=target/symbolic-python/bin/python
"$PY" scripts/symbolic/check_source.py --large --growth
"$PY" scripts/symbolic/check.py "$KVS/MCKVS.tla" "$KVS/MCKVSSafetyLarge.cfg"
"$PY" scripts/symbolic/check.py "$KVS/MCKVS.tla" "$KVS/MCKVSSafetyLarge.cfg" --prove-inductive
"$PY" scripts/symbolic/kvs_symbolic.py "$KVS" Large
"$PY" scripts/perf/scale/receipt_family.py target/release/dive "$KVS/../c1cs"
```

`--order-by-path` and `--single-partition` reproduce the layout and
simultaneous-image ablations. The hand-constructed KVS relation additionally
checks the exact source and configuration digests and compares the result
with the transaction-history formula. The separate receipt-family calculation
checks 20 concrete paths and independently rejects a damaged path. Its counts
cover the specified disjoint regions only.

Focused checks on the explicit storage/scheduling candidates included 59
store and exploration unit tests, 31 Core frontier, spill and temporal tests
and a five-model small-tier ratchet with zero regressions. Twenty CoffeeCan
and Huang worker observations matched the pinned verdict, exit class, counts
and depth. See the [performance evidence index](../../conformance/reports/perf/README.md)
for the isolated probes and source identities.

To record a bounded ordinary Core observation for a model without a pinned
TLC golden, use the normal harness with `--observe-missing-goldens`.
For example, run the following from a checkout with the pinned Examples corpus
and a release CLI. Each worker count gets its own record under `models[].runs`.
The model retains its missing-oracle outcome and contributes zero parity agreements.

```sh
cargo run --release --locked -p dive-conformance -- run --impl dive:core \
  --impl-profile tlc --corpus examples --tier all --no-sany --workers 1,8 \
  --filter '**/MCKVSSafetyLarge.cfg' --observe-missing-goldens \
  --tlc-budgets --budget-cap 600 --xmx 24g --report-dir /tmp/dive-large --no-summary
```

Four selected holdouts lack a golden because the pinned TLC ran past its deadline. The manifest marks these with `oracle_timeout = true`. The same flag records bounded non-oracle attempts for them under their existing `out-of-scope` grade.

The [13-model observation record](../../conformance/reports/perf/2026-11-20-no-golden-observations.json) contains all 26 one- and eight-worker observations from merged main. With five seconds and 2 GiB per run, six configurations emitted exploration prefixes and seven reported named refusals at both widths. The harness records zero parity agreements for these models.


The motivating measurements cover [the large non-finishes](https://github.com/Chelis-Lang/dive/issues/1034),
[slow-TLC models](https://github.com/Chelis-Lang/dive/issues/799) and
[temporal worker scaling](https://github.com/Chelis-Lang/dive/issues/1028).

The two pinned etcd Raft configurations exercise inclusive subsequence
search and a bag fold which filters pending messages by sender. In a
five-second, one-worker default-profile check, `MCetcdraft.cfg` reached
14,246 distinct states (64,037 generated) and `etcdraft.cfg` reached
51,818 (154,810 generated). Both prefixes reached depth eight on a shared
workstation using a debug build. Neither configuration has a completed
TLC golden. The nominal TLC profile still refuses the multikey fold when
the original order is unknown. See [bag fold order](../tlc-differences.md#d-031-bag-fold-order).
