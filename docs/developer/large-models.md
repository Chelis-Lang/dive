# Large-model performance and exact symbolic checking

The [source-driven symbolic checker](../user/symbolic.md) checks finite safety specifications using binary decision diagrams (BDDs). Dive resolves the original modules, binds the configuration and exports its IR. The Python compiler reads the actual state types, initial predicate, actions, invariants, symmetry group and deadlock setting. KVS runs directly from its existing source with an inferred bit layout.

The [performance evidence index](../../conformance/reports/perf/README.md) collects compact records for reuse in other reports. The tables below identify which comparisons ran on the same host, which reached a complete fixed point and which stopped at a limit.

## What sets the limit

| Model structure | What to measure and what helped |
|---|---|
| Large source trees versus many concurrent protocol operations | Dive analyzed the 34,596 project lines in the LuPastry+ proof module and its imports in 0.40 s. On its four-node join model, changing the initial membership from three ready nodes to one grew the exact graph from 71 to 555,691 states with the same code, constants for the ring size, and properties. [Source and graph comparisons](#check-a-35029-line-pastry-source-tree). |
| Wide frontiers and wide state rows | Keep distinct states, pending queue entries and generated occurrences separate. On the one-billion-state SlateDB prefix, packing the queue cut peak RSS from 51.88 to 30.13 GiB at identical work. On the complete 242-million-state control, packing full mathematical identities cut exact-store RSS from 89.4 to 35.2 GiB. [SlateDB data](../../conformance/reports/perf/2026-11-20-slatedb-wal.json). |
| Function-valued actions that preserve most entries | An action in Microsoft CCF chooses three server logs, then requires two to stay fixed. Enumerating only the writable entry cut a complete run from 44.56 to 0.12 s. [Original and framed CCF runs](#function-valued-actions-in-ccf). |
| Many small actions and fairness predicates | A state-count limit misses the cost of edges and of evaluating predicates on each edge. Kafka completed its second periodic check at 1.57 million distinct states with paged action truth under 8 GiB. The next layer exhausted the memory budget during graph capture after 1.94 billion generated occurrences. [Kafka boundaries](../../conformance/reports/perf/2026-11-20-large-model-outcomes.json). |
| Coupled fields in a symbolically checked state | The final diagram can be tiny even when computing an intermediate image dominates the run. The four-key, three-transaction KVS closure has 3,226 decision nodes and represents 592.3 billion symmetry classes. Field order and the choice of image source affect the cost of reaching that closure. [Symbolic growth](../../conformance/reports/perf/2026-11-20-isolated-symbolic-growth.json). |
| Fixed choices across an execution | Complementary initial partitions can be checked independently if the unrestricted transition relation preserves each partition. On EmergencyCall, 23 of 38 variables stay fixed. A finite-value decision diagram checked 49.91 billion reachable states on the reconstructed full Init. Ordinary exact-store runs independently check disjoint cells. [Source, counts and limits](#check-the-full-emergencycall-initial-predicate). |
| Changes to a maintained specification | A reset to an initial state preserves shortest paths. Adding a network drain to PaxosLease enlarged the reachable graph by 634,904 states. Reusing the unaffected breadth-first layers cut the edit from 433.48 to 97.49 s. [Edits and fresh Core comparisons](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json). |
| Ordered parallel admission | Workers can finish evaluating wide successors before the coordinator has committed them. Retaining the evaluator's canonical rows and confirmed resident hits removes repeated work there. Keeping threads alive produced additional throughput at 96 workers while raising peak RSS. [Matched worker comparisons](../../conformance/reports/perf/2026-11-21-safety-worker-commit.json). |

## Next experiments

- **Reproduce the published originals.** Core exhausted the released SlateDB CFG and all six invariants. Complete the identical pinned TLC run and compare the reduced and unreduced counts with the author’s report. Record both terminal queues, distinct and generated counts, depth, elapsed time, memory, worker count and fingerprint limitations. The billion-state prefix gives a repeatable checkpoint for comparing builds. Compare the unedited three-barrier Flint model with its published 192-core completion. For ZooKeeper, compare the four-server, two-timeout configuration against the paper’s 24-hour cutoff after reproducing the one-timeout control. Keep the reconstructed restart rows separate. [SlateDB controls](#reproduce-the-released-slatedb-wal-model) · [ZooKeeper source recipe](#reproduce-the-zookeeper-protocol-specification).
- **Close historical memory stops.** Use matched 24 GiB runs on DiskSynod, BPConProof, TLCSailfish2, BlockingQueuePoisonApple and c1cs, then raise the budget on the same inputs. Record the queue and stopping reason at each limit. Use the 242-million-state exact-store control to test reductions in collision-bucket and retained-row memory. The resident fingerprint table has also been tested at the shard occupancy implied by 49 billion states. [Historical outcomes](#where-the-thirteen-historical-non-finishes-stand) · [Frontier and value measurements](#resident-frontier-rows).
- **Make maintenance cheaper.** Additions and shortcuts now reuse the old layers through the 77.4-million-state PaxosLease drain. The remaining cost is deletion of a reachable transition. A selective deletion proof gave mixed timings. Test edits that remove the last path to a state. Keep the false edit and independently validated trace as a control. Five BDD orderings on Retry exhausted their time budgets; use the completed explicit runs to judge any new relation representation. [PaxosLease runs and edits](#reproduce-the-released-paxoslease-runs) · [Installed Sail measurements](#what-an-installed-sail-worker-saves).
- **Finish temporal checks.** For original Kafka, reduce the retained graph at the 8 GiB capture stop and the action-label allocation at the 24 GiB dense third-check stop. Measure both at the same depth-six prefix. For the two memory-heavy temporal originals, compare capacity at the same budget after each graph layout change. Keep independent counterexample validation and defer the 17-model worker campaign to integrated qualification. [Historical inputs and limits](#where-the-thirteen-historical-non-finishes-stand).
- **Grow a checked program.** Increase one source parameter at a time, retain the original properties and cross-check the last tractable configuration with both explicit engines. Report generated edges, distinct concrete states, symmetry representatives and symbolic cardinalities using their own units. On EmergencyCall, compare all 64 symbolic cell closures with ordinary exact Core. Four one-domain edits now have full fixed points. Check the remaining ordinary cells and use the ordinary-number F1 controls when comparing growth: the local-number edits make the published constrained initial predicates empty. Transfer the finite-value representation to the general checker only with an admission proof for all initial values and generated witnesses. [Literature and reproducibility](#published-model-sizes-and-reproducible-comparisons).

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

The shape of the *search layer* also controls the cost. In a complete BFS on the original KVS Large, layer 21 had 138,561 decision nodes in its frontier and 121,723 in all admitted states reached so far. The latter set includes earlier layers whose successors are already known. Expanding it and removing reached states gives exactly the same next layer. Choosing the smaller diagram cut two uninstrumented runs from 134.64/134.88 s to **35.50/35.20 s**, with peak RSS rising from 606 to 619 MiB. Both runs returned 594,616,815 representatives, 13,385,471,536 generated and depth 30. TCommit with 64 managers took 6.76/6.81 s using the old/new policies. [Per-layer image costs and matched runs](../../conformance/reports/perf/2026-11-20-isolated-symbolic-growth.json).

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

Growing **the released Large configuration** from three to four keys retains all three transactions, two values, both safety invariants and transaction symmetry. The installed symbolic checker exhausted it in **1,997.42 s** at **1.06 GiB** peak RSS, depth 37: **592,287,802,857 symmetry representatives and 16,831,060,919,470 generated**. The independent transaction-history formula matches both counts. Its separate concrete-state prediction is 3,552,493,680,513. The smaller-source search completed this same four-key CFG in 303.49 s at 615 MiB on a separate c7i.4xlarge, with identical counts and depth. The earlier seven-key experiment uses two transactions. [Inputs and reproduction](../../conformance/reports/perf/2026-11-20-isolated-symbolic-growth.json).

On a single isolated c7i.4xlarge, three instrumented runs of this four-key configuration used the same binary, core, inputs and resource limits. Expanding the current breadth-first layer took 1,691.38 s and 1.06 GiB peak RSS. Choosing the smaller source diagram took 195.37 s and 615 MiB, an **8.66× speedup**. Always expanding the accumulated reachable set took 195.42 s and 615 MiB on this input. All three reached depth 37 with the same counts and final 3,226-node diagram. The record gives the two source sizes, chosen operand and image time at every depth. [Matched four-key trials](../../conformance/reports/perf/2026-11-20-isolated-symbolic-growth.json).

At **five keys and three transactions**, the installed checker exhausted **571,514,403,867,075 symmetry representatives**, generated **19,613,909,230,737,652**, and checked both original invariants through depth 44. One isolated core took 1,431.39 s and 629 MiB peak RSS. The final diagram used 4,491 nodes. The independent history calculation agrees on both counters and predicts 3,428,937,742,115,475 distinct concrete states with symmetry disabled. [Configuration hash, command and result](../../conformance/reports/perf/2026-11-20-isolated-symbolic-growth.json).

At **six keys and three transactions** the checker exhausted **539,910,848,473,937,721 symmetry classes**, generated **21,725,268,712,106,423,230** occurrences and reached depth 51. Both original invariants held. The process took **4,578.19 s** and peaked at **1.10 GiB** on a single EC2 logical CPU. The reachable-set diagram had 5,756 nodes. An independent calculation over transaction histories agrees on both integers. The generated count exceeds the range of a 64-bit unsigned integer. The record includes decimal strings for tools that parse JSON numbers as floating point. The same calculation predicts 3,239,447,207,251,422,105 distinct labeled states for the six-key configuration. [Process measurement, independent formula and input hashes](../../conformance/reports/perf/2026-11-20-isolated-symbolic-growth.json).

For the five-key comparison above, the earlier frontier-only worker exhausted a 7,200 s deadline at 2.13 GiB peak RSS. The smaller-source worker completed in 1,431.39 s. These single-core runs used separate hosts of the same instance type. The stopped worker did not emit counts. [Five-key control](../../conformance/reports/perf/2026-11-20-isolated-symbolic-growth.json).

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

For an edit that changes the transition relation, a completed ordinary symbolic search also supplies its breadth-first layers. A saved successor layer is sufficient when the enabled transitions removed from its source only reached already visited states. The checker unions the images of the added transitions, preserves new constrained-out emissions for property checking, and resumes a fresh breadth-first search at the first changed frontier. This maintains shortest-path depth even when an edit introduces a shortcut or removes the only predecessor of a state. [Edit reports and trace behavior](../user/symbolic.md#check-edits-and-prove-properties).

On the 594,616,815-state KVS Large input, adding a transition that resets the store and every transaction to their initial values took 3.81 s versus 118.82 s for fresh search. Removing it took 3.68 s versus 111.98 s. Both edits reused all 30 layer images, and the added transition raised the generated count from 13,385,471,536 to 13,980,088,351. A second edit opened every transaction at once. It changed the first successor layer and took 144.03 s versus 144.19 s fresh. On Medium the same edit shortened depth from 22 to 21. [Paired results and edit definitions](../../conformance/reports/perf/2026-11-20-isolated-symbolic-growth.json).

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
expanded state. When a generated ordinal matches its position in the source row, Core
derives it directly. The last physical edge of a closed source is its
stutter. A block of 256 edges acquires a two-byte ordinal column only if
it contains an exception. Full-width exceptions use a side table keyed
by physical edge. A source hint per block bounds random ordinal lookup;
consumers that already know the source read it directly. The checked
graph verifies each block, extended ordinal and source range.

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

Core retains the action ID of every physical transition, including duplicate destinations. It uses byte IDs for up to 255 action names and two-byte IDs for up to 65,535. The maximum value at each width marks an unnamed occurrence. Each sealed block of 256 IDs uses either a dense array or sorted pairs of inclusive end position and action ID, whichever occupies fewer cells. One directory lookup and a search within the block recover a random ID. Action tables above 65,535 names retain full-width optional IDs.

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

At the same 20 GiB sampled cap, deriving ordinary edge originals raised c1cs capacity another 12.8%, from 38,066,281 to 42,930,269 distinct states. The latter run generated 989,943,733 states and stopped at depth 18, with 25,181,577 still queued. Wall time was 1,556.23 s for that larger prefix. [Matched control, binary hashes and admission counters](../../conformance/reports/perf/2026-11-20-temporal-edge-ordinals.json).

At Kafka KIP-996’s first periodic check, 36,120,199 stored edges used 292
action names. The original action column reserved 512 MiB, although only
225,593 runs of equal IDs occurred. In all 16,574 completed rows, each
generated ordinal equalled its source-local position. The table isolates each
change at fixed work. Kafka stopped one state before the check, with the
same 36,103,626 generated. The two other original configurations stopped
at two million states. The binary tree exhausted its graph and passed.

| Model | Original buffers | Two-byte IDs | Adaptive action blocks | Implicit ordinals alone | Combined |
|---|---:|---:|---:|---:|---:|
| Kafka KIP-996 | 943.0 MiB | 559.0 MiB | 435.0 MiB | 818.0 MiB | 310.0 MiB |
| c1cs | 554.0 MiB | 554.0 MiB | 508.0 MiB | 429.0 MiB | 383.0 MiB |
| BlockingQueuePoisonApple | 144.3 MiB | 144.3 MiB | 144.6 MiB | 128.7 MiB | 128.9 MiB |
| Binary tree | 12.0 MiB | 12.0 MiB | 12.1 MiB | 10.1 MiB | 10.1 MiB |

On inputs with interleaved action IDs, a dense block uses one directory
entry beyond its 256 IDs. At a later Kafka stop of 200,000 distinct and
38,232,770 generated, both binaries had completed the first periodic check
and evaluated 71,333,304 actions and 47,497,563 state predicates. Peak process
RSS was 2,258 MiB before and 1,903 MiB after; wall times were 302.12 s and
303.03 s. The run stopped with 182,506 states queued. On the binary tree, both layouts also returned the same
two-state, independently validated lasso for the false recurrence property. [All capacities,
individual runs, outcomes and input hashes](../../conformance/reports/perf/2026-11-20-temporal-edge-ordinals.json)

### Destination IDs in temporal graphs

At the pre-periodic Kafka cutoff, the graph held 36,120,197 destination IDs.
The dense target vector reserved 256 MiB. Core now packs 64 destinations at a
time using the minimum ID and enough bits for the largest difference.
When IDs repeat, a bitmap marks the starts of runs. Counting the set bits
through a position selects the corresponding value. Blocks with a wider
range can keep the original full-width IDs.

| Graph at the same cutoff | Dense targets | Base-relative values alone | Run starts alone | Combined |
|---|---:|---:|---:|---:|
| Kafka KIP-996 | 310.0 MiB | 118.0 MiB | 142.0 MiB | 110.0 MiB |
| c1cs | 383.0 MiB | 255.0 MiB | 219.0 MiB | 199.0 MiB |
| BlockingQueuePoisonApple | 128.9 MiB | 124.9 MiB | 127.9 MiB | 124.9 MiB |
| Binary tree | 10.1 MiB | 8.0 MiB | 8.7 MiB | 8.0 MiB |

These are whole collector capacities. On 4,194,304 random full-width IDs, the
encoder kept a dense prefix and a 256-byte tail. Reading the captured Kafka
targets in 2.5 million pseudorandom positions took 0.033 s from the dense
array and 0.112 s from the production encoding. The full 200,000-state Kafka
check took 303.03 s with dense targets and 306.80 s with packed targets.
Peak RSS fell from 1,903 to 1,793 MiB. Both completed one periodic check,
made the same 118,830,867 native predicate evaluations, and resumed capture
before reaching the state limit. [Individual timings, capacities, decoded
checksums and input hashes](../../conformance/reports/perf/2026-11-20-temporal-edge-ordinals.json)

### Action predicate truth at a periodic boundary

At Kafka's first periodic check, the first configured property had 255 action
predicates on 36,120,199 physical edges. A dense bit per predicate and edge
used 1,098 MiB. Only 95,167 bits were true. The paged representation reserved
28.84 MiB for action words and directories, with 49,764 nonempty pages.
Both figures exclude state predicates, column headers and the graph itself.
A dense-action control with 1,049,600 edges used 133,640 bytes for the paged
representation and 131,200 for the dense representation. Both preserved all
1,048,576 direct action evaluations and the completed verdict.

On the unchanged Kafka configuration, both eight-worker runs reached the second
periodic check with 1,568,061 distinct states and 364,992,824 generated. The
dense build stopped on the 8 GiB memory limit at 760.39 s, before another atom
evaluation. With paged actions, the run made 82,432,049 additional state and
229,798,438 additional action evaluations before the 1,800 s time limit. Its
peak RSS was 1.88 GiB. The second check was still labeling the graph, with all
four properties awaiting a verdict. [Inputs, binary identities, per-arm counters
and allocation probe](../../conformance/reports/perf/2026-11-20-large-model-outcomes.json).

With a 12-hour time allowance and the same paged binary and 8 GiB limit,
the second check returned after 49 min 29 s. Exploration resumed and stopped
on memory during graph capture at 8,388,609 distinct states and 1,935,626,353
generated, with 7,359,346 still queued at depth 6. The collector reported
6.16 GiB of buffer capacity. The sampled stop was 8,606,912,512 bytes and
process peak RSS was 8.96 GiB. The run took 5,980.25 s.
[Periodic check times and terminal resource measurements](../../conformance/reports/perf/2026-11-20-large-model-outcomes.json).

With dense action labels and a 24 GiB limit, the unchanged Kafka model
returned from its second periodic check. The third check began with 11,313,431
distinct states, 2,809,683,123 generated and 9,745,370 still queued at depth 6.
Graph buffers occupied 6.88 GiB. The process hit its memory limit during atom
labeling after 6,835.26 s, with all four properties awaiting a final verdict.
[Matched 8 GiB and 24 GiB runs](../../conformance/reports/perf/2026-11-20-large-model-outcomes.json).

### Fairness actions on unchanged destination variables

At the first Kafka periodic check, three actions accounted for 64,248,424 of
71,333,304 native action evaluations. For example,
`VoterFetchTimeout` tests the voter's destination state. When that test
requires the state to change, a destination retaining its original state
cannot satisfy the action. Core evaluates the leading condition once for
the source and its bound action. Other destinations still use the original
edge predicate, including its error path.

On the unchanged configuration with a 200,000-state limit, this answered
39,843,715 action demands and left 31,489,589 direct
evaluations. Time fell from 315.78 to 188.52 s
on the same eight cores. Both runs evaluated 47,497,563 state
predicates, used zero fallback and reached 38,232,770 generated with
182,506 states still queued. All four configured properties await a final
verdict. [Binary identities, controls and per-action
counts](../../conformance/reports/perf/2026-11-20-large-model-outcomes.json).

Without the state cutoff, the destination guard answered 319,126,840
action demands within the 1,800 s run. The checker ran 305,199,122
direct action and 210,575,907 state evaluations. The run reached
1,568,061 distinct states and 364,992,824 generated at depth
5, with 1,386,078 still queued. The time limit stopped atom labeling at the second periodic
check, with all four properties still open. Peak RSS was 1.99 GiB. [Terminal report and resource
limit](../../conformance/reports/perf/2026-11-20-large-model-outcomes.json).

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

### Ordered commit on wide states

Flint’s three-writer checker expands 31-variable successors across workers and commits them in discovery order. Each worker has already interned its complete candidate in the shared value store. With repeated interning on the coordinator, ordered commit took 9.37 s on the first ten million discoveries with 192 workers. Copying the retained canonical row directly into the resident frontier reduced commit time to 6.01 s. Both builds reached 37,302,268 generated, depth 17 and the same 4,675,720 pending states. Reported commit time covers completed parallel levels.

| Model | Workers | Scope | Previous wall time, s | Retained row, s |
|---|---:|---|---:|---:|
| Flint original | 192 | 10M distinct | 21.68, 21.73 | 17.77, 18.01 |
| Flint original | 32 | 10M distinct | 26.96, 26.96 | 23.19, 23.21 |
| Flint two-barrier control | 192 | 7,924,117 distinct, complete | 21.03, 21.00 | 17.91, 18.41 |
| SlateDB one value | 32 | 10M distinct | 15.79, 15.67 | 12.71, 12.78 |
| KVS Medium | 8 | 2M distinct | 7.55, 7.56 | 7.31, 7.34 |


An in-process worker that finds a fingerprint in the resident table can carry that observation to ordered commit. The table only grows during the wave. This avoids a second coordinator lookup for that occurrence. A candidate observed only by another worker still needs its ordinary ordered check. On a dedicated 96-core Graviton4 node the 50-million-state SlateDB prefix took 64.42–64.59 s versus 66.39–66.45 s at 32 workers. The two-million-state KVS prefix fell from 2.92–2.94 s to 2.69–2.70 s at 32 workers. The full two-barrier Flint run at 96 workers fell from 13.97–13.98 s to 13.57–13.60 s. All paired counters and queues matched. [All runs, including pooled workers and allocator settings](../../conformance/reports/perf/2026-11-21-safety-worker-commit.json).

Keeping 96 worker threads alive saved wall time on four Graviton4 workloads, but also retained more process memory. The complete two-barrier Flint run fell from 13.57–13.61 s at 1.16–1.17 GiB to 11.94–12.03 s at 1.96 GiB. Giving each exploration machine a fixed pool worker took 11.84 s at 1.99–2.10 GiB. Replacing the pool at each BFS level brought peak RSS down to 1.83–1.85 GiB and took 12.02–12.03 s. All 32 trials matched their control on verdict, distinct and generated counts, depth and queue. The fixed-worker experiment leaves OS thread placement to the scheduler. The shipping policy still creates scoped workers. These measurements support testing pool lifetime on a complete high-worker original where visited-state storage dominates RSS. [Rows, inputs and binary digests](../../conformance/reports/perf/2026-11-21-safety-worker-commit.json).

A separate experiment kept a Rayon pool alive across ordinary safety batches. On Flint at 192 workers, host tasks created per run fell from about 124,000 to 314. Wall time fell from 21.97–22.24 s to 17.44–17.90 s, but peak RSS increased from 1.39 to 2.83 GiB. At 32 workers PConProof slowed from 34.55–34.89 s to 36.72–37.52 s. These tradeoffs kept ordinary worker scheduling out of the retained-row change. [All paired runs, inputs, binary identities, CPU time and memory](../../conformance/reports/perf/2026-11-21-safety-worker-commit.json).

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
| TLCSailfish2 | The original 24 GiB cgroup killed the checker. With the revised store, the same 24 GiB budget produced a measured memory stop at 43,224,883 distinct states, 89,157,072 generated and 35,714,020 queued. Spilling the frontier at the fixed prefix changed peak process RSS from 234 MiB to 231 MiB. | Measure and reduce retained value graphs and evaluation working sets independently of frontier placement. |
| DiskSynod | At 7,200 s, Core had reached 299,192,687 distinct, 4,260,111,630 generated and depth 30, with 65,518,609 queued. The cgroup peak was 18.31 GiB. | At 300,000 distinct states, disk-frontier placement cut peak process RSS from 230 MiB to 177 MiB. The larger run includes membership and predecessor growth as well. |
| BlockingQueuePoisonApple, c1cs | Both ordinary 24 GiB runs ended in a kernel OOM. At equal work the compact ordinal column reduced RSS on both. | Edges per state and the width of each retained edge field dominate dense prefixes. c1cs still had 13.76 million states queued at the compact 20 GiB stop. |
| EWD998, EnvironmentController | At the isolated 18 GiB / 900 s capture deadline, Core reached 19,779,576 distinct states on EWD998 and 2,173,413 on EnvironmentController. | The earlier startup refusals are gone. Success would require exhausting the graph and finishing fair temporal analysis, or an independently validated counterexample. |
| Kafka KIP-996 liveness | Under 8 GiB, paged action labels allowed the original to finish two periodic checks before capture reached its memory limit at 8,388,609 distinct and 1,935,626,353 generated. With dense labels and 24 GiB, the original reached 11,313,431 distinct and 2,809,683,123 generated. | The 8 GiB paged run stopped while expanding depth 6. The 24 GiB dense run stopped during the third periodic check at that depth. All four properties await a final verdict. [Inputs, counters and earlier bounded runs](../../conformance/reports/perf/2026-11-20-large-model-outcomes.json). |
| etcdraft | Default Core reached 70,655,639 distinct and 266,068,245 generated, depth 13, at the 18 GiB sampled limit. | 55,290,518 states were queued. Queue width and retained aggregate values are practical targets. |
| MCetcdraft | The original default run refused mixed record keys after 679 s. Canonical bag construction reached 47,609,227 distinct and 257,868,368 generated, depth 15, before the 18 GiB memory limit. | Message bags use structurally different records. Distinct keys and their multiplicities must survive construction and independent replay. |
| TestMCReachability | The released CFG reads the host environment. Six ordinary closed CFGs on the unedited source match TLC in both profiles, including the graph 1a deadlock. For graph 7, Core simulation and pinned TLC found `Live` false. Standalone replay accepted their different fair stuttering words. With the published property order, `--lncheck periodic-all` found a separately validated 42-state lasso after 25,376,696 distinct states in 833.18 s (8.09 GiB peak). Final-only BFS stopped at 67.1 million states under its 18 GiB limit. | The graph selected as input and the reachable checker states of the graph algorithm are separate quantities. |
| CarTalk Model 3 | In the default profile the ordinary CLI reports an assumption evaluation error at its finite enumeration ceiling. The nominal profile refuses oversized enumeration. The source-bound puzzle calculation returns the single solution 1, 3, 9, 27, 81. | This is a constant constraint-solving problem. An ordinary checker would need a general exact counting or constraint-solving route for this expression. |

The original 24 GiB reruns used a dedicated EC2 instance with eight physical cores.
For a kernel OOM there is no final checker report. The 900 s runs used an
18 GiB sampled limit inside a 24 GiB cgroup, leaving room for the checker to
report a stop. The two envelopes and the fixed-prefix study answer different
questions.

The original etcdraft configuration exposed a worker-creation failure at 32 workers
and a 420 GiB budget. The process exited after 4,461.21 s at 338.05 GiB RSS
without a verdict. A 129-state model reproduced the panic under a Linux task
quota. Core now completes an interrupted worker launch on the calling thread,
preserving already claimed work. Five different quotas gave the same counts,
including when successor candidates spilled to disk. All ten failing-property
traces replayed independently. The [measurements](../../conformance/reports/perf/2026-11-20-large-model-outcomes.json)
include the worker counts and the original model and configuration digests.

On a 32-physical-core host with a 24 GiB sampled RSS limit, DiskSynod reached 835,858,711 distinct states and 12,639,577,756 generated in 6,500.61 s. It had 144,840,332 states queued at depth 35. BPConProof hit the same budget in 181.95 s with 134,831,096 distinct states. Of those, 134,714,941 were still queued. At 16 workers, BlockingQueuePoisonApple hit 24 GiB during graph capture with 117,440,513 distinct and 569,933,982 generated. At eight workers and 18 GiB, etcdraft reached 94,073,470 distinct before memory stopped it. MCetcdraft instead hit the 900 s time limit at 39,256,817 distinct and 8.89 GiB peak RSS. These runs use different worker counts and budget enforcement from the original cgroup trials. [Per-input commands, limits, binary identities and terminal queues](../../conformance/reports/perf/2026-11-20-large-model-outcomes.json).

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
| SlateDB WAL, three writers with restarts | Six safety invariants under the author’s symmetry group | About 49 billion unique states in 2 h 37 min with 40 workers on a Threadripper 9980X. Exact counts, engine version and final log are unreported. | Core exhausted the released CFG at 4,910,385,789 distinct fingerprints in 14,122 s. The roughly tenfold difference from the author’s rounded count remains open. [Author’s model-checking notes](https://github.com/Vanlightly/s3-wal-collection/blob/35fdc537af4d226f43b4574cded1764375f264ac/slatedb/SlateDBWAL_notes.md#model-checking) |
| EmergencyCall, unconstrained | Original cellular state machine, 36 configurable variables | 10.59 billion distinct, diameter 26 transitions. The paper gives separate times for constrained cases. | Public source. The released f1–f4 runs have individual configurations and final counts. The unconstrained count lacks an identified matching configuration and final log. [Hou et al., §3](https://users.cs.northwestern.edu/~ychen/Papers/MobiSys21_ENL_OpenAccessVersion.pdf) |
| Flint LeanP1, three writers and three barriers | Eight invariants and two step properties on the released S3 storage model | Compiled tlc-rs completed 9,821,039,047 distinct, 44,056,598,280 generated, depth 52 in 5,925 s on 192 Graviton4 cores. | Core and pinned TLC reproduced the authors’ smaller control with the same obligations. [Original terminal log](https://github.com/ddalton/flint/blob/8feba0c78f22e3ecdee0648c6b2897eac95ab1b4/lean/formal/results/2026-10-06-ec2/RESULTS.txt) · [Pinned inputs and control runs](../../conformance/reports/perf/2026-11-21-flint-leanp1.json) |
| EmergencyCall, f3 | Three safety invariants, 49,152 distinct initial states | 21,255,936 distinct, 135,871,232 generated, TLC depth 27. The released TLC log records six minutes. | Reproduced by ordinary Core and pinned TLC at one and eight workers. [Authors’ configuration and raw output](https://github.com/FormalCellular/EmergencyCall/tree/093684f1bb02d733cae95d50f1690f1cb792296a/model) |
| PaxosLease, quarantine bound | Original passing consensus config with two proposers, the released constants, state constraint and invariants | 76,749,192 distinct, 391,849,185 generated, depth 43 in 10 min 55 s in the author log. | Both ordinary Core and pinned TLC reproduced all counters and the successful verdict at eight workers. [Trencséni](https://arxiv.org/abs/2609.14640) · [Released inputs and raw results](https://github.com/mtrencseni/paxoslease-revisited/tree/5ca959624c5a2560ab1ad86dc71e8b161621d9bc) |
| PaxosLease, Retry and RenewRetry | The same module with original retry configurations | The author logs report 2,179,760,458 distinct and depth 48 in 7 h 03 min for Retry, and 2,411,951,538 distinct and depth 49 in 8 h 03 min for RenewRetry. | Core and pinned TLC also completed both configurations on 32 physical cores. The tables below give the counters for each run. The author’s logs give the fingerprint polynomial and estimated collision probability for each run. [Retry](https://github.com/mtrencseni/paxoslease-revisited/blob/5ca959624c5a2560ab1ad86dc71e8b161621d9bc/results/retry.txt) · [RenewRetry](https://github.com/mtrencseni/paxoslease-revisited/blob/5ca959624c5a2560ab1ad86dc71e8b161621d9bc/results/renew-retry.txt) |
| AWS industrial example | Historical industrial TLC check | Kukovec attributes roughly 31 billion explored states over five weeks on one 16-vCPU EC2 instance to Newcombe. | The available account does not identify the generated/distinct convention or a released matching configuration. It sets a historical scale target only. [Kukovec, p. 25](https://repositum.tuwien.at/bitstream/20.500.12708/209338/1/Kukovec%20Jure%20-%202024%20-%20SMT-driven%20techniques%20for%20verifying%20distributed%20systems.pdf) · [Newcombe, ABZ 2014](https://abz-conf.org/publication/newcombe14/) |
| Pasture, node model | Invariants with parameters 2, 1, 2, 2, 2 | 4,234,887,880 distinct, depth 53 in 3,596 minutes on a 48-core Opteron with 128 GB. A repeat using another fingerprint polynomial found the same count. A third run found five fewer distinct states. | A transcription from the printed modules reproduced the first five published counts and depths with both checkers. [Inputs and measurements](#reproduce-the-pasture-node-model). [Rodeheffer and Kotla, Table 1 and appendices](https://conf.tlapl.us/2012/rodeheffer-kotla.pdf) |
| Tunable replicated set, three replicas | Two operations per replica over three possible elements | About 9 billion generated states and 750 GB of storage when the host ran out of disk. The study reports completed times for eight smaller cases. | The thesis supplies a set specification and the author's public implementation. Identifying the exact generated module and Toolbox configuration would allow a matching replay. [Rijo, §5.5 and Table 5.1](https://run.unl.pt/server/api/core/bitstreams/d2570d62-6972-4877-a791-636d876d0096/content) · [Author’s implementation](https://github.com/AndreRijo/T-CRDTs) |
| Tunable set CRDT, three replicas with two operations each and three elements | Safety invariants replay each replica’s operation history. | About 9 billion generated states before TLC exhausted about 750 GB of disk. The same hardware completed the two-element case in 2 d 10 h. | The author provides TLA+ definitions in the thesis. The public implementation tree contains Java sources. The three-element run stopped with work remaining. [Rijo, Table 5.1 and Appendix E](https://run.unl.pt/server/api/core/bitstreams/d2570d62-6972-4877-a791-636d876d0096/content) · [Author’s repository](https://github.com/AndreRijo/T-CRDTs/tree/1792fbaa22c8f89cff4db5f886866b9795b9a7df) |
| llingr worker coordination and offset committer | Separate safety and fairness/liveness configurations for Kafka consumer coordination. | Worker coordination reports 626 million distinct states for safety and 170 million for liveness. The offset committer reports 67 million and 253 million respectively. | The reported billion-state total sums four runs. The report omits individual elapsed times and CFGs. The inspected public engine tree contains no TLA+ modules. [Authors’ correctness report](https://llingr.io/correctness) · [Implementation tree](https://github.com/llingr/llingr-demux/tree/f0eaa0857c86650150e6f0cb937d87d75d1748e5) |
| LuPastry, simplified five-node configuration | Distributed hash-table routing | 141,786,762 distinct, 7,070,872,610 generated, depth 54 in one week. The full four-node run stopped after over five weeks and 331,300,918,096 generated. | The thesis identifies the original VeriDis release. The archived project index links a formal-models page that was absent from the available capture. The later public LuPastry+ archive has different source and no CFG. [Archived author index](https://web.archive.org/web/20130725090936/http://www.mpi-inf.mpg.de:80/~tianlu/fmPastry/). [Lu, Table 5.3](https://doi.org/10.22028/D291-26550) · [Subsequent model versions](https://homepages.loria.fr/SMerz/projects/pastry/index.html) |
| Confidential Consortium Framework | Production consensus specification | 1,134 lines and 13 state variables. The paper describes model checking, trace validation and CI. | Two published abstract model revisions now have matched complete runs. The concrete pre-vote revision pair also has pinned TLC results and a named Core refusal. [Howard et al., Table 1](https://www.microsoft.com/en-us/research/uploads/prod/2024/07/nsdi25spring-final392.pdf) |

These records establish a lower bound on published workloads. A global maximum
would require knowing the counting convention and input of every contender,
including proprietary models. The enlarged KVS and transaction-commit runs
above demonstrate concrete reachable cardinalities on their own programs.

### Check a 35,029-line Pastry source tree

The LuPastry+ authors distribute 16 TLA+ modules, including a 21,764-line invariant proof. The top-level `ProofCorrectness.tla` resolves 15 of those modules (34,596 source lines) and 13 library modules. On one EC2 logical CPU, three separate `dive analyze` processes each took 0.40 s. The corresponding pinned SANY processes took 5.13, 5.18 and 5.15 s. Replacing one operator in the routing table gave medians of 0.40 and 5.16 s. Both front ends diagnosed an intentional undefined operator in that dependency when invoked on the top-level file. SANY wrote the semantic error to its output and exited zero. These times cover semantic analysis including process startup. Executing a TLAPS proof has a different completion condition. [Author’s archive and proof history](https://homepages.loria.fr/SMerz/projects/pastry/index.html) · [Inputs and raw measurement fields](../../conformance/reports/perf/2026-11-21-pastry-source-scale.json).

Running the protocol uncovered a separate limit. `SharedDigitsAux` chooses an entire function whose definition refers to earlier entries of that same function. Both TLC and Core stop at this unbounded choice. The comment in the released operator supplies a finite bound, `[RTRows -> 0..RTRowCount]`. A second equivalent definition constructs the function directly:

```tla
SharedDigitsAux(i, j) ==
  [n \in RTRows |-> Cardinality({k \in 1..n :
    IthDigit(i, RTRowCount-k+1) = IthDigit(j, RTRowCount-k+1)})]
```

For row one this counts exactly the last digit comparison. Each successive row adds the next comparison to the preceding count, so induction gives the unique solution of the published function equation. This preserves the actual definition, including its behavior when nonconsecutive digit positions agree. Both engines checked equality of the entire functions for every ordered ID pair at ring sizes 2, 4, 8 and 16, totaling 340 pairs per checker. At 32 IDs, a separate check of every row of each resulting function took 19.07 s on Core with bounded choice and 0.08 s with direct counting. TLC took 6.98 and 1.28 s. At 64 IDs the counted check finished in 0.36 s on Core and 1.39 s on TLC. Both bounded implementations exceeded their 155 s process deadlines. These tests verify routing arithmetic for all ID pairs before exploring the protocol.

The archive includes `Next` and a separate `NextMC` that omits lookup actions. It supplies no CFG. With `A={0}, B=1, M=1, L=1` and `NullNode=NullNode`, both implementations of the function and both engines exhausted 190 distinct states, 1,268 generated and depth 17 using `Next`. With `NextMC` they exhausted 14 distinct states, 40 generated and depth 11. The reconstructed CFGs check `TypeInvariant` and `CorrectDelivery` with deadlock checking disabled. Increasing the ID domain to four nodes while starting three as ready gave 71 distinct, 654 generated and depth 18 on `NextMC` in both checkers with both definitions. Starting only one node as ready in the same four-node domain gave 555,691 distinct, 13,271,201 generated and depth 49. With the counted definition on four workers, exact Core finished in 91.23 s and pinned TLC in 125.71 s. The number of concurrent joins changes reachability by several orders of magnitude at the same ring size. These results belong to the later LuPastry+ specification. Lu’s historical four- and five-node results remain separate. [Configuration text, source changes and controls](../../conformance/reports/perf/2026-11-21-pastry-source-scale.json).

Keeping all four nodes and enabling lookup actions with `Next` changed the result again. Neither definition completed on either checker in the one-worker run. At its 420 s limit the counted Core run had reached depth 17 with 1,233,725 distinct states and 494,504 still queued. The bounded Core run reached the same depth with 1,138,948 distinct and 461,672 queued. The TLC record gives the last periodic sample before each external deadline.

### LuPastry+ message and leaf domains

A one-state experiment isolated an eight-address limit to checking whether the **empty** message pool belongs to `SUBSET Message`. The earlier Core build exhausted a 15 GiB cgroup. It imported eight record variants by enumerating their possible payloads, including routing functions. Core now keeps eligible closed record families as finite set descriptors and tests the fields of the message presented to it. The explicit states, invariants and transition relation stay the same.

On a dedicated EC2 c7i.4xlarge, exact Core and pinned TLC exhausted 141 distinct states, 2,560 generated and depth 19 with seven of eight addresses initially ready. With six ready they agreed on 92,269 distinct, 3,798,734 generated and depth 38. In the four-worker comparison the default Core candidate took 0.76 s and 30.79 s. TLC took 1.64 s and 52.50 s. Making a nonempty message pool an invariant violation gave both engines the same depth-two failure. The independent validator accepted the Core witness and rejected a changed transition. [Machine, memory, commands and full measurements](../../conformance/reports/perf/2026-11-21-pastry-source-scale.json).

A second problem appears when a model builds a large candidate domain and immediately removes nearly all of it. For `L=1`, every valid left or right leaf set is the empty set or a singleton. The preparer's `--singleton-leaves` expresses that bound before forming the record product. At 16 addresses this takes the candidate count from 68,719,476,736 to 4,624, of which 4,096 pass the original predicate. On the 15-ready model TLC exhausted 222 distinct states, 7,623 generated and depth 19 with both forms.

The released `NextMC` binds two identifiers and splits nine disjuncts into 2,304 action occurrences at 16 addresses. Compiling the value expressions separately for every binding exhausted Core's four-million-node plan limit before search. Sharing instructions across matching pure bodies brings the 15-ready default plan down to 26,911 value nodes. With the singleton leaf bound and both original invariants, one-worker exact Core finished in 0.48 s at 88 MiB peak RSS and matched TLC's counts. Adding `PoolEmpty == MessagePool = {}` made both engines report a depth-two violation. The independent validator accepted both Core profiles' traces and rejected a changed transition.

With 14 addresses initially ready, two concurrent joiners produced 84,370 distinct states, 6,375,410 generated and depth 38 in both checkers. Four-worker exact Core used 241 MiB peak RSS and took 300.12 s. Four-worker pinned TLC used 2.77 GiB and took 169.76 s. [Configurations, plan sizes, commands and measurements](../../conformance/reports/perf/2026-11-21-pastry-source-scale.json).

To rebuild a measured input, download `lupastryplus.zip` from the authors’ project page.
The preparer verifies its digest and writes to a new directory. For the library
path, combine the five theorem modules listed in the measurement record from
`tlapm/library` with `Folds.tla` and `Functions.tla` from CommunityModules.
[TLAPS library](https://github.com/tlaplus/tlapm/tree/d8b9747a1dce9b4816bcc1ff130ccf7a3af2ce37/library) · [Community modules](https://github.com/tlaplus/CommunityModules/tree/78b98ae9939d2fe7a6615e9a4b21dd6c23ef8ff3/modules).

```sh
python3 scripts/perf/scale/lupastry.py lupastryplus.zip pastry-four --definition counted --bits 2 --ready 0
dive check pastry-four/Pastry.tla --config pastry-four/Model.cfg --library-path lib --store exact --workers 4 --compat=tlc --fp 0 --seed 0
```

Lu's original simplified four-node experiment removed the routing table and
completed 504,624 distinct states, 16,064,479 generated and depth 49 in
2 h 15 min. The LuPastry+ four-node join experiment retains the routing table.
The two graphs have the same depth and different counts. Lu's later paper
gives a second location for the earlier code, `tiit.lu/fmPastry/`. Its live page
returned 404, and the Wayback URL-prefix query returned no successful capture.
[Lu, Tables 5.2–5.3](https://doi.org/10.22028/D291-26550) · [Lu, 2015, reference 10](https://doi.org/10.1007/978-3-319-25942-0_19).

### LuPastry+ parallel invariant checking

The 32-address, one-join ring compiles to 32,779 memo sites with the published `NextMC`. In a four-worker exact check, a worker can fill its 28 MiB cache with action-guard tables before checking its first successor. LuPastry+’s `CorrectDelivery` quantifies over three identifiers and all pending messages. Repeating that check dominated the run. Removing both invariants reduced direct exploration to 0.66 s. Keeping `CorrectDelivery` alone took 23.66 s. Raising the worker cache allowance to 64 MiB let the full direct check explore the graph in 2.21 s.

Core now prepares the invariant tables within the worker’s existing allowance when the minimum tables for all sites would exceed it. At the original 7 GiB process ceiling, the 32-address default check fell from 25.89 s to 1.96 s, with 256 distinct states, 15,249 generated and depth 19. At 64 addresses the earlier direct run reached its 420 s limit with nine states queued. The corrected direct check finished in 16.28 s with 316 distinct, 30,493 generated and depth 19. Both profiles matched pinned TLC.

At 128 addresses the direct action still reached the compiler’s four-million-node limit before search. The diagnostic wrapper `TRUE /\ NextMC` exhausted 474 distinct states, 61,019 generated and depth 19 in 167.74 s. [Direct and grouped runs, CPU profiles and reproduction](../../conformance/reports/perf/2026-11-21-pastry-source-scale.json).

### Reproduce the ZooKeeper protocol specification

Three completed no-restart configurations reproduce the state counts and diameters in Ouyang et al., Table 1. The paper’s count matches **generated** states in each. With four servers, two transactions and one timeout, Core and pinned TLC exhausted the queue at 6,634,696 distinct states, 21,393,294 generated and depth 64. Core took 623.70 s and 452 MiB peak RSS. TLC took 136.24 s and 46.3 GiB on the same host. Core used one of the 32 requested workers because the released specification contains twelve diagnostic `PrintT` calls. The completed Core run emitted no lines. With two timeouts the paper reports 1,599,588,210 states at its 24-hour cutoff. [Paper, Table 1](https://arxiv.org/pdf/2302.02703#page=7).

With conditional diagnostics evaluated in commit order, the three-server, one-timeout control took 1.20 s with 32 workers versus 12.38 s on the prior single-worker path and 7.05 s on pinned 32-worker TLC. All three exhausted 251,332 distinct states, 610,035 generated, at depth 43, and Core emitted no output. Peak process RSS was 1.00 GiB for parallel Core, 155 MiB for serial Core and 27.4 GiB for TLC under its 128 GiB heap setting. [Paired measurements and independent diagnostic checks](../../conformance/reports/perf/2026-11-20-zookeeper-reference.json).

On the four-server control, the same worker change took 51.74 s versus 612.76 s on the prior Core and 133.97 s on pinned TLC. All three exhausted the same graph at depth 64. Peak RSS rose from 449 MiB to 3.45 GiB on Core. TLC peaked at 46.3 GiB. [Inputs, counts, times and memory](../../conformance/reports/perf/2026-11-20-zookeeper-reference.json).

The release supplies the module and a Toolbox guide. The reproduction record supplies the missing wrapper and configuration template, the ten invariants, hashes, individual measurements and unsuccessful reconstructions. Restart increments the source’s timeout counter as well as its restart counter. Treating the published timeout parameter as a joint or an independent allowance produced different state spaces. Neither interpretation reproduced the two-server restart rows. With three servers, two timeouts and no restarts, both checkers completed 17,345,585 generated at depth 52 versus 26,126,204 at depth 54 in the paper. With one restart and one timeout, pinned TLC completed 97,096,950 generated at depth 59. Core reached its 1,500 s limit, and the paper reports 84,543,312 at depth 61. [Source and reconstruction data](../../conformance/reports/perf/2026-11-20-zookeeper-reference.json).

### Reproduce the released SlateDB WAL model

Use the pinned `slatedb/SlateDBWAL.tla` and `SlateDBWAL.cfg`. All runs retain the six invariants, the garbage collector, symmetry and the original action definitions. The four comparative configurations with two values vary just the writer count and the maximum restarts. The one-writer smoke control also reduces the value domain. Each comparison used sequential fresh processes on an isolated r7i.4xlarge. The larger configurations used eight physical cores and the smoke control used one.

The published CFG permits permutations of three writers and two values, with one garbage collector. Their disjoint relabelings generate a group of order 12. For an equivariant reachable set, a symmetry representative covers between one and twelve labeled states, depending on its stabilizer. On the two-writer, two-value control, the exact store measured 74,637 representatives and 270,183 labeled states with symmetry removed, both at depth 69. Multiplying every representative by the group order would overcount. The original 4.91-billion-fingerprint run alone cannot determine the labeled total or resolve the author’s approximate 49-billion report. [Source and controls](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

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

Restarts raised the completed two-writer graph from 74,637 to 3,761,377 distinct states. Adding a third writer with restarts disabled raised it to 30,114,878. A separate completed TLC run matched Core’s 117,007,147 generated, depth 114 and empty queue for that configuration. The original three-writer, one-restart CFG later exhausted its queue on Core with 4,910,385,789 distinct fingerprints and 23,680,348,756 generated at depth 139. All six invariants held. The 40-worker run took 14,122.37 s and peaked at 128.98 GiB on an r7i.48xlarge. The public notes, CFG and module were updated together. The CFG enables writer and value symmetry. Its terminal count and the author’s rounded 49 billion are different observations, and the available evidence does not establish the source of the difference. Holding just one packed 64-bit fingerprint for each of the reported 49 billion states would occupy about 365 GiB before table overhead, predecessor records and the frontier. Under an independent uniform 64-bit hash assumption, that many distinct states would yield about 65 colliding pairs in expectation. An exact-state or symbolic control is important at this scale. [Configuration edits, input hashes and all measurements](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

Allowing two writer restarts instead of one raised the complete two-writer graph from **3,761,377 to 98,416,351 distinct states**. Core and TLC independently generated 398,235,951 states, reached depth 114 and exhausted the queue. On the same eight physical cores, Core took 396.33 s at 1.65 GiB peak RSS and TLC took 804.47 s at 5.02 GiB. With all three original writers and two restarts, Core reached a 30-million-state prefix in 135.87 s, with 102,653,606 generated and 6,184,868 still queued. That larger model's total size remains unknown. [Pinned configurations, per-engine commands and measurements](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

#### The earlier four-writer WAL protocol

The author's November release contains two programs. The main WAL model allows a fenced writer to restart. A separately named, earlier program uses four writers and has no restart parameter. The notes give 49 billion unique states for the newer program and 12 billion for the simpler one. The earlier program's shipped configuration keeps the fix enabled and checks six invariants. Its writer count differs from the three in the newer CFG, so the two published figures require separate reproductions. [Author's notes and configurations](https://github.com/Vanlightly/s3-wal-collection/tree/35fdc537af4d226f43b4574cded1764375f264ac/slatedb) · [TLAPS-Bench, Appendix D](https://arxiv.org/abs/2611.02849).

| Writers in the earlier protocol | Distinct | Generated | Depth | Exact Core | Pinned TLC |
|---:|---:|---:|---:|---:|---:|
| 2 | 58,972 | 143,482 | 55 | 0.26 s | 1.43 s |
| 3 | 9,850,105 | 34,653,324 | 84 | 54.94 s | 85.44 s |

Both checkers exhausted the queue in all four runs on one dedicated EC2 host with eight workers. The three-writer exact run peaked at 0.95 GiB on Core and 8.57 GiB on TLC. These controls preserve symmetry, both values, the collector and all six invariants. The shipped four-writer CFG is the next comparison for the older program. [Commands, memory, input digests and source history](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

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

For the author’s reported scale, membership capacity changes the choice of machine. With 49 billion uniformly distributed fingerprints, the resident table's 256 shards would each require 2²⁸ slots under the 75% growth threshold. The previous eight-byte layout needed 512 GiB for primary slots alone. The four-byte layout at that occupancy needs 256 GiB of primary slots plus full-key overflow. On a 64-bit host the packed resident parent log adds at least 57 GiB for one byte per discovery and one 16-byte index entry per 64 discoveries. The fixed-width disk parent log would write 784 GB at that count. These are size calculations. The live frontier, resize peaks and worker allocations add to the resident requirement. The indexed disk store caps membership at 4,294,967,295 entries. Sorted runs use a global 64-bit count and a streaming audit. A capacity bound alone does not establish a completed run. [Fingerprint table](../../crates/dive-explore/src/lib.rs) · [Packed parents](../../crates/dive-store/src/predecessor/packed.rs) · [Disk membership limits](../user/disk-seen-safety.md).

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

With complete state identities retained in `--store exact`, 32-worker Core independently returned the same terminal counts and depth. It took 778.53 s and 90.39 GiB peak RSS. The five-million-state control retained exactly 5,000,000 identities and stopped at its configured limit. [Exact membership, input and binary hashes](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

#### Resident fingerprint membership

Core partitions the resident visited set into 256 shards. For a large shard, the shard number supplies the high eight bits of each fingerprint. Its home slot supplies low bits, recoverable from the physical slot and a four-bit displacement. Each primary entry stores only the remaining bits and the displacement. A cluster longer than fifteen slots retains additional full keys in an overflow table. The checker can reconstruct every existing fingerprint when a shard grows.

At 402,653,185 distinct synthetic keys, the checker’s membership table took 3.78 GiB peak process RSS, down from 5.93 GiB. The same inserts, duplicates and misses took 93.25 s versus 79.13 s with full-word slots. On ordinary Core the two SlateDB trials at 20 million discoveries averaged 1,182 MiB versus 1,261 MiB and 55.74 versus 55.04 s. DiskSynod at 10 million averaged 419 MiB versus 497 MiB and 90.67 versus 89.46 s. Every paired search agreed on its generated and distinct counts, depth and pending queue. [All trials, stream recipe and input hashes](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

On the completed three-writer, one-value run, the smaller membership slots reduced peak RSS from 5.73 to 4.16 GiB (27%) and raised elapsed time from 712.54 to 727.46 s (2.1%). Both processes used 16 physical cores and finished at the same counts and depth. [Complete one-value comparison](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

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

Using the same in-memory membership on a 16-physical-core host, the uncompressed frontier completed the one-value model in 713.18 s and 7.33 GiB peak RSS. Packing the frontier completed it in 721.24 s and 5.42 GiB, a 26% reduction, with the same terminal counts. On the unedited two-value original, a separate 32-core packed-frontier run reached its **one-billion distinct state cap**, 4,114,101,313 generated and depth 71, in 2,336.42 s at 30.13 GiB peak RSS. At exactly the same counts and pending queue of 116,153,919 states, the pre-packing build took 2,264.90 s and 51.88 GiB. Packing reduced peak RSS by 42% and cost 3.2% in wall time. [Commands and terminal observations](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

The unfiltered external checker also exhausted the full one-value configuration on a separate r7i.16xlarge with 32 physical cores. It matched resident Core and TLC at 242,213,642 distinct, 1,068,416,158 generated, depth 126 and an empty queue. Its 2,322.19 s includes auditing all fingerprints and removing the run files. Peak process RSS was 3.99 GiB and peak cgroup memory was 5.64 GiB. Resident Core used 8.70 GiB and finished in 551.82 s. Before the closing audit the external key collections occupied 11.21 MiB and active runs contained 1.94 GB. The native minimum fingerprint gap was 127.

On that same machine the filtered external checker finished in **1,431.80 s**, down from 2,322.19 s. Both runs audited all 242,213,642 stored keys, found the same minimum gap and reported the same depth, generated count and empty queue. Filtering raised peak RSS from 3.99 to 4.06 GiB and peak cgroup memory from 5.64 to 5.69 GiB, including the filesystem cache. For this model, filtered disk membership took 2.59× the wall time of resident Core with less than half its peak process RSS. [Both complete runs and the resident/TLC controls](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

With no discovery limit, eight workers exhausted the two-writer, one-restart
configuration using sorted runs. Core and pinned TLC both reached 3,761,377
distinct, 12,508,420 generated and depth 93. The small exact-state control and
both fingerprint checkers also agreed on 268 distinct and 460 generated at
depth 27. These controls establish the counters on smaller complete graphs.
[Commands, all trial measurements and binary identities](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

#### Resident exact state identities

A hash bucket and a separately allocated row per discovery made exact equality expensive even when nearly every hash had only one identity. The resident safety store now keeps that first entry in the hash slot and seals groups of 256 rows by variable. A matching hash still requires mathematical equality across all variables. Additional identities sharing a hash retain their own discovery IDs.

| Resident exact layout | Peak process RSS | Wall seconds |
|---|---:|---:|
| Previous buckets and rows | 89.4 GiB | 938.27 |
| Inline first identity | 67.3 GiB | 900.22 |
| Inline first identity and packed rows | 35.2 GiB | 972.52 |

All three runs exhausted the one-value SlateDB control on the same dedicated 16-core host, storing all 242,213,642 identities, generating 1,068,416,158 states and reaching depth 126. Packing reduced complete-run peak RSS by 60.7% against the previous layout and added 3.7% wall time. A separate 10-million-state comparison of inline and packed rows used 2.84 and 1.65 GiB. On the unmodified two-value input at 100 million exact identities, inline and packed rows used 21.84 and 10.18 GiB in 368.73 and 380.86 s. Both reached 362,779,396 generated, depth 56 and 15,988,974 queued. [Inputs, binary identities, individual trials and commands](../../conformance/reports/perf/2026-11-20-slatedb-wal.json).

On EmergencyCall the hash index itself set a second memory limit. A resize of a single table could temporarily keep both generations resident. Core now splits that table into 256 shards. Each primary slot holds one hash and one packed-row position. Separate tables hold actual hash collisions and discovery IDs that differ from the row position. Looking up an existing state still compares the full mathematical row.

| EmergencyCall, 24 cores | Packed rows | Packed rows and compact index | Peak RSS reduction |
|---|---:|---:|---:|
| F3, complete (21,255,936 states) | 130.61 s / 2.15 GiB | 131.97 s / 1.27 GiB | 41.1% |
| Full Init, one cell at 100,000,000 states | 337.80 s / 8.70 GiB | 343.22 s / 6.14 GiB | 29.5% |

F3 matched the released TLC result with 135,871,232 generated, depth 27 and an empty queue. Both full-Init runs stopped at the state cap with 457,202,205 generated, depth 9 and 26,800,319 queued. Each pair ran sequentially on the same isolated Graviton4. [Inputs, commands and measurements](../../conformance/reports/perf/2026-11-20-emergencycall-reference.json).

At a 40 GiB sampled limit on full-Init cell one, the previous table stopped at 469,762,653 exact states and the compact index at 816,808,860 (+74%). The latter still had 28,063,040 states queued at depth 24. A separately provisioned larger run closed the same cell at 912,926,560. Peak RSS in the budgeted pair was 63.7 and 40.0 GiB. Resize allocations between memory samples explain why the process high-water mark can exceed the configured limit. [Both memory stops](../../conformance/reports/perf/2026-11-20-emergencycall-reference.json) · [Completed cell](../../conformance/reports/perf/2026-11-21-emergencycall-finite-values.json).

On the larger SlateDB one-value control, the same 24 cores exhausted all 242,213,642 mathematical identities. The compact hash index reduced peak RSS from 35.57 to 20.28 GiB (43.0%). Wall time was 643.65 s with the old index and 654.76 s with the compact one. Generated count (1,068,416,158), depth (126) and the empty queue agreed. [Paired exact-store runs](../../conformance/reports/perf/2026-11-20-emergencycall-reference.json).

### Reproduce the released PaxosLease runs

#### Labeled crash configurations

Refined symbolic search and exact Core both completed the two-acceptor
RenewCrash3 model with room for two network messages. Both found **34,309,860
distinct states**, generated 177,479,267 and reached depth 49. The symbolic
worker used 1.52 GiB peak RSS in 1,467.86 s on one CPU. Exact Core used
11.48 GiB in 170.64 s on eight physical cores. For ReleaseCrash at the same
bounds, the corresponding measurements were 0.90 GiB in 748.95 s and
5.63 GiB in 73.79 s. Each timing and peak comes from one run. The record
identifies the executable and worker used for each measurement.
[Measurements and commands](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

| Configuration | Acceptors | MaxNetwork | Distinct | Generated | Depth |
| --- | ---: | ---: | ---: | ---: | ---: |
| RenewCrash3 | 2 | 1 | 1,856 | 19,489 | 15 |
| RenewCrash3 | 2 | 2 | 34,309,860 | 177,479,267 | 49 |
| RenewCrash3 | 3 | 2 | 12,672 | 145,729 | 18 |
| ReleaseCrash | 2 | 1 | 832 | 6,241 | 14 |
| ReleaseCrash | 2 | 2 | 14,700,326 | 73,518,377 | 53 |
| ReleaseCrash | 3 | 2 | 4,480 | 38,081 | 17 |

All six rows passed the original invariants in both engines. Pinned TLC also
matched both two-acceptor, one-message rows and both three-acceptor,
two-message rows. The smaller state counts at three acceptors follow from
the network bound. From an empty network, `StartAcquire` broadcasts one
Prepare to each acceptor. A bound of two admits the two-acceptor broadcast
and excludes the three-acceptor broadcast from the queue. With the original
symmetry directive retained, the same model counts equivalence classes.
[Crash configurations with symmetry](#symmetry-in-records-and-message-sets).

At three acceptors and the published network bounds, two labeled symbolic
runs reached their one-hour time limit and returned no exploration counts.
The earlier worker stopped on ReleaseCrash at `MaxNetwork = 7` and refused
the overlapping action guard on RenewCrash3. The worker with guard witness
counting admitted RenewCrash3 at `MaxNetwork = 4` and reached the time limit
with a 5.05 GiB service peak on one CPU.

To reproduce a labeled row, start with `PaxosLeaseRenewCrash3.cfg` or
`PaxosLeaseReleaseCrash.cfg`. Remove `SYMMETRY Symm`, keep the indicated
number of acceptors beginning with `a1`, and set `MaxNetwork` to the table
value. Keep the other constants, `StateConstraint`, `TypeOK` and `Safety`.
Run `dive check PaxosLeaseCheckedSym.tla --config CFG --strategy symbolic --type-invariant TypeOK --refine-domains`.
[Published inputs](https://github.com/mtrencseni/paxoslease-revisited/tree/5ca959624c5a2560ab1ad86dc71e8b161621d9bc/tla/spec).

#### Base, quarantine and retry configurations

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

Five orderings reproduced Base, but none completed the original Retry configuration within two hours. The static product order reached layer 24 with 34.4 million nodes in the reached set. Continuous reordering reached layer 17 with 1.44 million nodes. Reordering once at layer 12 reached layer 27 with 25.9 million nodes. Freezing automatic reordering at layer 12 also reached layer 27, with 45.2 million nodes. A smaller decision diagram can take longer to transform on each step. The explicit checkers still provide the completed Retry reference. [All ten runs and measurement limits](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

For source-edit sessions, `--refine-domains --recheck MODULE CONFIG` can supply set coordinates to the next independently bound search. In the first transfer experiment, repeating Base took 41.80 s with one refinement attempt, compared with 64.16 s and eleven attempts cold. Supplying the same Base coordinates to QuarantineEqualsProposer reduced attempts from eleven to three, but wall time rose from 486.75 to 528.99 s. Its final layout contained 150 coordinates instead of 80 and peak RSS fell from 1.14 GiB to 737 MiB. Both configurations kept their exact counts and depths. On a separate c7i.4xlarge, the installed CLI repeated Base in 40.70 s after a 61.32 s cold search. The changed configuration took 520.86 s seeded and 481.88 s cold. Fewer restarts alone do not predict faster searches when an edit changes the reachable message alphabet. [Individual observations and host sharing](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

A prototype that stored messages in sorted slots matched exact-state Core on a reduced Retry model (1,856 distinct, 13,921 generated, depth 15). On original Base, a reachable source with four messages can emit a successor beyond six slots. Such an emission still requires invariant checking. Seven slots timed out on Base after 2,400 s, and six slots timed out on original Retry after 5,400 s. [All six trials, controls and bounds](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

If only the obligations change, the checker can keep the completed reachability proof in its worker. On the same PaxosLease Base input, repeating the check took 0.60 s versus 47.73 s with coordinates alone. Adding the true `EmissionBound` invariant took 4.50 s versus 48.84 s. Both returned 491,037 distinct, 1,709,874 generated and depth 29. Adding the false `EmptyNetwork` obligation took 0.88 s and produced a two-state trace accepted by the independent validator. Initial cold checks took 72.52 and 72.21 s. [Per-edit measurements, source hashes and replay](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

On QuarantineEqualsProposer the same edit sequence retained **76,749,192 distinct states** and 391,849,185 generated at depth 43. After a 562.05 s cold check, the unchanged repeat took 0.81 s and the added invariant 6.83 s. The coordinates-only control took 531.15 and 537.00 s on those edits. The false edit produced an independently valid two-state trace in 0.76 s. Whole-session peak RSS was 1.14 GiB for both. [Per-edit and whole-session measurements](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

Adding an unconditional self-loop to the released PaxosLease Base relation raised the generated count from 1,709,874 to 2,200,911. Replacing that loop with a second Tick action gave 1,804,652. Removing the duplicate restored the original count. All four revisions exhausted 491,037 distinct states at depth 29. Rechecking the three edits took 1.46, 1.67 and 0.93 s versus 43.79, 41.86 and 42.05 s with fresh searches that reused only the finite coordinates. [Model edits, exact counts, memory and per-revision times](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

A reset that returns *all* variables to their admitted initial values is a genuine non-stuttering edit. On PaxosLease Base, adding it took 1.44 s instead of 35.50 s with the previous checker; removing it took 1.31 s instead of 34.08 s. A second edit cleared a full network at MaxTime, enlarging the graph from 491,037 to 598,541 states. Fresh exact Core independently confirmed the reset and drain counts and depths. The drain saved nine layer images on addition and eight on removal, yet took 42.37/35.76 s versus 40.12/34.12 s previously. Across the five-revision batch, wall time fell from 194.83 to 128.86 s and peak RSS rose from 420 to 541 MiB. Retaining the layers paid off for the reset; many changed destinations can still require most of a fresh search. [Per-revision times, counts and control](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

On the larger QuarantineEqualsProposer input, adding the reset took **1.74 s** and removing it took **1.66 s**, down from 413.18 and 376.45 s. All four runs represent 76,749,192 reachable states at depth 43. The added action changed the generated count from 391,849,185 to 468,598,377. A network drain actually enlarged the reachable graph to 77,384,096 states and 401,456,611 generated. Reusing five layers on that edit took 443.54 s versus 387.58 s for fresh search. The whole five-revision batch fell from 1,950.05 to 1,257.87 s and from 1.14 GiB to 902 MiB peak RSS. [Per-revision results and controls](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

An added action can make old states reachable earlier and can introduce entirely new states. Every state in the original graph still arrives by its old depth. The symbolic checker uses that fact to retain the old image and expand the states that first arrive at a different depth. Adding the network drain to Base took **23.95 s**, down from 43.70 s. The checker used all 29 old layer images, up from nine. The edited graph had 598,541 distinct states, 2,016,104 generated and depth 29 in both runs. Reverting took 35.39 s versus 36.14 s. Across the three revisions the two processes took 104.11 s and 129.96 s. [Input digests, commands and complete observations](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

On the larger QuarantineEqualsProposer model, the drain introduced 634,904 reachable states. Counting the same 77,384,096 states and 401,456,611 generated at depth 43 took **97.49 s** with all 43 images available, down from **433.48 s** with five. Restoring the original took 412.28 s versus 395.23 s. The complete three-revision session fell from 1,225.33 to 901.83 s while peak RSS rose from 0.88 to 1.18 GiB. A fresh eight-worker Core check with the current exact store independently exhausted all 77,384,096 identities at the 22 GiB budget, in 515.30 s with 4.95 GiB peak RSS. An earlier exact-store executable stopped after 58,720,259 states at the same budget. The fingerprint control also exhausted the graph. [Both executable digests, queue counts and memory](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

A separate experiment removed the Drain action from an already explored
graph. Rechecking only destinations that might have lost their shortest
predecessor reduced the QuarantineEqualsProposer edit from 446.39 to 286.11 s
and raised the Base edit from 36.38 to 112.74 s. Both variants agreed on
the final counts and depth. The checker kept the existing deletion algorithm.
[Deletion controls and timing scope](https://github.com/Chelis-Lang/dive/issues/799#issuecomment-7057522753).

The same three transition edits on QuarantineEqualsProposer took 2.10, 2.13 and 0.86 s after a 471.49 s cold check. Fresh searches seeded with the same finite coordinates took 427.07, 417.56 and 415.67 s. All runs kept 76,749,192 distinct states at depth 43. The generated counts were 391,849,185 initially, 468,598,377 with the self-loop, 402,564,377 with the duplicate Tick, and 391,849,185 after reverting. [Paired revisions and peak memory](../../conformance/reports/perf/2026-11-20-paxoslease-reference.json).

#### Symmetry in records and message sets

The released three-acceptor modules enable permutations of both proposers and acceptors. A permutation can change an array index and the owner, sender or receiver stored inside a message. The symbolic checker now constructs the induced permutation of each encoded coordinate. It verifies the finite domains and multiset of guarded assignments before using the group to count representatives. Under domain refinement, retaining a message also retains all its renamed forms.

With two proposers, two acceptors and a one-message network cap, the source, invariants, state constraint and original symmetry expression yield **670 distinct / 7,036 generated / depth 15** for RenewCrash3 and **310 / 2,326 / 14** for ReleaseCrash. Both match ordinary exact Core and pinned TLC. Source-driven symbolic checking took 1.35 s and 1.05 s respectively. Both learned twelve concrete network members. A prototype that built the entire equality relation for fixed states before intersecting reachability exhausted 4 GiB on each case. Intersecting one coordinate at a time kept these two runs below 68 MiB. [Commands, hashes and controls](../../conformance/reports/perf/2026-11-21-paxoslease-symmetry.json).

The same procedure accepts the authors' original CFG as input. A completed smaller configuration establishes the quotient counts for that configuration alone. The number of labeled states in a class depends on which proposer and acceptor renamings fix that state.

### Reproduce the Flint three-writer model

Use Flint’s recorded module revision and the archived large CFG. The authors’ smaller three-writer control changes the barrier and mint bounds and retains all ten obligations. Both Dive and pinned TLC exhausted 449,595 distinct states, 1,954,072 generated, at depth 33 on a fresh r8g.48xlarge. With 192 workers, Dive took 1.58 s at 1.31 GiB peak process RSS and TLC took 15.62 s at 24.86 GiB. A separate eight-core exact run stored every one of the 7,924,117 two-barrier identities in 130.65 s at 3.79 GiB. Adding `BarrierBound == barriers = 0` produced a four-state violation on both Core worker widths and pinned TLC. Fresh replay accepted the Core words and rejected a corrupted final transition. The large-run comparison uses the unmodified three-barrier CFG and the same 192-core allocation. [Source revisions, commands, original timings and local comparisons](../../conformance/reports/perf/2026-11-21-flint-leanp1.json).

Increasing only the small control’s barrier bound from one to two yielded **7,924,117 distinct states**, 33,356,883 generated and depth 40. Both explicit engines exhausted the configuration on a c8g.48xlarge. With 192 workers, pinned TLC took 155.15 s and 34.41 GiB peak RSS. Core took 21.00–21.03 s on the prior build and 17.91–18.41 s with worker-interned rows. The author’s large configuration also changes other constants. [Per-process results and configuration digest](../../conformance/reports/perf/2026-11-21-flint-leanp1.json).

### Reproduce the Pasture node model

The authors printed the node specification and its model-checking instance in Appendices A and B. The [reconstruction script](../../scripts/perf/scale/pasture.py) reads the PDF, restores the TLA+ notation and verifies a digest for each output module. Use PyMuPDF 1.23.7. The configuration enables all four printed invariants and uses a fresh model value for the unspecified value of `NullTs`. The original Toolbox configuration was unavailable. [Microsoft Research report, Table 1 and appendices](https://www.microsoft.com/en-us/research/wp-content/uploads/2012/08/paper.pdf).

```sh
python3 scripts/perf/scale/pasture.py paper.pdf pasture-small --bounds 1 1 1 1 1
cd pasture-small
dive check PastureNodeModel.tla --config Pasture.cfg --compat=tlc --store exact --workers 1 --fp 0 --seed 0
```

| Bounds in publication order | Distinct | Generated | Depth | Core exact | Pinned TLC |
|---|---:|---:|---:|---:|---:|
| 1,1,1,1,1 | 47,742 | 360,066 | 31 | 1.07 s | 7.24 s |
| 1,1,1,1,2 | 106,556 | 801,933 | 32 | 2.47 s | 13.39 s |
| 1,1,1,2,1 | 966,697 | 8,031,442 | 36 | 25.39 s | 98.08 s |
| 1,1,2,1,1 | 369,750 | 2,805,496 | 33 | 8.48 s | 36.96 s |
| 1,2,1,1,1 | 283,760 | 2,718,246 | 34 | 8.20 s | 34.39 s |

Each pair ran serially on the same isolated CPU. Both engines exhausted the queue, agreed on generated counts and matched the authors’ distinct count and depth in every row. Table 1 also reports a 4,234,887,880-state run at bounds 2,1,2,2,2. That row needs its own terminal evidence. [Per-run memory, commands, hashes and original timings](../../conformance/reports/perf/2026-11-21-pasture-node.json).

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

### Check the full EmergencyCall initial predicate

The released module with `INIT Init`, `NEXT Next`, `TypeInvariant`, `SetInvariant` and deadlock checking reached **49,911,672,768 distinct states**. It started from 56,623,104 initial states, counted **328,063,574,184 generated occurrences** and exhausted breadth-first depth 44 (43 transitions). The finite-value benchmark took **116.03 s and 647.2 MiB peak RSS** on one EC2 logical CPU. The published diameter of 26 rules out equating this run with the paper's 10.59-billion-state report. The stronger `F1` fails at depth seven on full Init cell zero. [Configurations, counts and controls](../../conformance/reports/perf/2026-11-21-emergencycall-finite-values.json).

The model has 38 variables, including small sets of numbers, records of message fields, program counters and policy flags. Representing each reachable state as a separate row repeats the same policy settings and message values many times. The benchmark assigns a finite alphabet to each variable and encodes the whole tuple in 66 bits. It compiles the real initial predicate and 62 action witnesses into Boolean relations. The final reachable set uses 159,191 decision nodes. Distinct destinations can share structure while each enabled source-action witness still contributes to the generated count. On f3 alone, 3,031,040 witnesses reach destinations already emitted by another action on the same source.

The type predicate is a conjunct of Init. The one variable missing from that predicate is initially zero. Each expansion checks that all enabled assignments, including partial `CASE` expressions, have a value inside the inferred alphabets. Every layer checks the configured invariants and enabledness. The pinned initializer has one witness per initial state. Changing it to two identical alternatives would double the initial witnesses in TLC. The helper checks the bound root source and all four standard-library imports against their digests. A sibling module can shadow a standard module even when the root is unchanged. A regression with a shadowed TLC module and an undefined `CASE` invariant now gives a named refusal. Predicate operands and quantifier domains must have defined values across the admitted alphabet. The ordinary checker and the general symbolic strategy have separate admission rules.

| Released CFG | Initial | Distinct | Generated | TLC depth | Symbolic wall s |
|---|---:|---:|---:|---:|---:|
| f1 | 32,768 | 3,932,160 | 23,650,304 | 19 | 0.32 |
| f2 | 24,576 | 6,248,960 | 39,053,696 | 25 | 0.41 |
| f3 | 49,152 | 21,255,936 | 135,871,232 | 27 | 0.77 |
| f4 | 24,576 | 8,872,832 | 56,734,720 | 27 | 0.70 |

The authors' four logs and the earlier 16 ordinary Core and pinned TLC runs agree with these rows. An alternate symbolic search over the full initial predicate used strict BFS layers and simultaneous assignments in reversed action order. It shared the expression evaluator and independently reproduced both full counts, the depth and the count in each of 64 fixed-policy cells. Sixty-four separate symbolic BFS runs also agreed for every cell. Ordinary exact Core independently closed 10 of these cells, totaling 7,224,747,616 states. The JSON records each cell’s depth, counts and available native timings. Its ordinary records distinguish a memory stop with states still queued from a complete search. [Partition data and evidence scope](../../conformance/reports/perf/2026-11-21-emergencycall-finite-values.json).

Use the published module from the checkout above and the `libexec/dive-symbolic` directory from a Linux binary archive with the symbolic worker. The helper is in the Dive source tree.

```sh
cat > full.cfg <<'CFG'
INIT Init
NEXT Next
INVARIANTS TypeInvariant SetInvariant
CFG
export PYTHONHASHSEED=0 PYTHONPATH="$WORKER"
"$WORKER/python/bin/python3" scripts/perf/scale/emergencycall.py \
    "$MODEL/EmergencyCall_Model_auto.tla" full.cfg --exporter "$WORKER/symbolic_input"
```

Use `"$MODEL/config/f3.cfg"` in place of `full.cfg` for the published f3 comparison. Add `--cell 0` through `--cell 63` for the six-bit partition of the selected Init. Each partition search retains unrestricted Next and checks its defining predicate at every layer. To save a shortest counterexample from the selected initial set for a failed source invariant or a deadlock, add `--trace witness.json --validator /path/to/dive`. The helper replays the concrete states against the same module and CFG in the independent validator before it writes the file. A failed internal encoding obligation still stops the search.

#### Enlarge the EmergencyCall number sets

Adding members to either of two source sets increases the choices in messages, sets of numbers, and the number dialed by the user. The transition definitions and properties stay the same. These four modified modules use unrestricted `Init`, `Next`, `TypeInvariant`, `SetInvariant` and deadlock checking. Each row exhausted depth 44. Each process used one EC2 logical CPU. The four final runs ran concurrently with separate CPU affinities. Times include source binding and verification of all module digests. A second BFS implementation used simultaneous updates, reversed action order and 64 fixed-policy partitions. It matched the initial, distinct and generated counts and depth on every variant. Both searches use the same source expression semantics. [Run data, source digests and independent checks](../../conformance/reports/perf/2026-11-21-emergencycall-growth.json).

| Source edit | Initial | Distinct | Generated | Wall (s) | Final BDD nodes |
|---|---:|---:|---:|---:|---:|
| One extra ordinary number | 147,849,216 | 169,479,392,600 | 1,273,101,338,520 | 254.63 | 244,067 |
| Two extra ordinary numbers | 273,678,336 | 394,692,381,120 | 3,342,552,718,608 | 422.37 | 337,077 |
| One extra local emergency number | 167,772,160 | 193,953,806,512 | 1,460,900,160,048 | 302.71 | 285,740 |
| Two extra local emergency numbers | 393,216,000 | 570,694,082,176 | 4,847,988,282,736 | 559.44 | 407,330 |

The local-number edits expose a trap when using a published configuration as a control. The type invariant makes each of the home-1 and roaming-1 AMF emergency-number sets a singleton. The system constraint equates each of them with the **entire** local emergency-number set. Growing the latter makes each of the five constrained initial predicates empty. The benchmark reports the empty initial set explicitly. Both ordinary checkers reported zero initial, distinct and generated states on F1. For the ordinary-number edits, F1 still starts with 32,768 states. Its two closures contain 4,841,472 and 5,750,784 states at depth 19. On the unmodified published s0 configuration the stronger F1 property fails. The benchmark produces a seven-state counterexample that Dive independently validates.

To reproduce the two-extra-local-numbers row from the published module and the `full.cfg` above, write the changed module in a new directory:

```sh
mkdir -p local-2
sed 's/^LOCAL_EMERGENCY_NUMBER_CANDIDATES_SET == {"120"}$/LOCAL_EMERGENCY_NUMBER_CANDIDATES_SET == {"120", "local-test-2", "local-test-3"}/' \
    "$MODEL/EmergencyCall_Model_auto.tla" > local-2/EmergencyCall_Model_auto.tla
"$WORKER/python/bin/python3" scripts/perf/scale/emergencycall.py \
    local-2/EmergencyCall_Model_auto.tla full.cfg --variant local-2 --exporter "$WORKER/symbolic_input"
```

For the ordinary edits change `NON_EMERGENCY_NUMBER_CANDIDATES_SET` from `{"224"}` by appending `"ordinary-test-2"` and optionally `"ordinary-test-3"`. For a single local addition append only `"local-test-2"`. The corresponding switches are `--variant ordinary-1`, `ordinary-2` and `local-1`. Each variant verifies the complete module digest before binding. The domain and initial-count arguments cover precisely these sources. The general symbolic CLI and this specialized benchmark have different admission criteria.

On the historical CCF single-node commit control, ordinary Core found the original property violation after 17 distinct states, 20 generated and depth five. The three revisions with temporal obligations received a named refusal from symbolic safety checking. The [Sail evidence](https://github.com/Chelis-Lang/sail/blob/df6ffaca82b3aacdea301616411de1c0c726b12f/docs/validation/data/2026-11-20-symbolic-predicates.json) pins the production inputs and records the successful five-key KVS maintenance comparison separately.

### Function-valued actions in CCF

The Microsoft CCF authors released two consensus models. Their abstract model bounds three server logs to finite sequences of three possible terms. Two actions first choose an entire new log function, then require every entry except the selected server’s to equal its previous value. With the shipped `MaxExtend = 3`, a log can take 40 values. Each action can consider 40³ = 64,000 functions to find at most 40 eligible ones. A run that ends with 349 distinct states can spend most of its time rejecting candidate assignments. [Original abstract actions and smallest-term edit](https://github.com/microsoft/CCF/commit/6a2dc420f0166c93f92b773aa49c5203b0767750).

To reproduce the source edit, put the bundled `FramedFunctions.tla` on the module path, add `FramedFunctions` to the `EXTENDS` line in `abs.tla`, and replace both occurrences of the following primed membership. Keep the remaining action conjuncts, the symmetry group, the log-length constraint, both invariants and `AppendOnlyProp`.

```tla
\* Released membership
cLogs' \in [Servers -> Seq(Terms)]
\* Membership with the same fixed coordinates
cLogs' \in FramedFunctions(Servers, Seq(Terms), cLogs, {i})
```

Each timing is a complete process on one logical CPU of an isolated EC2 m7i.2xlarge. Core used its default profile and full state identity. The first two rows change only Microsoft’s smallest-term assumption. The third raises the single `MaxExtend` constant from three to four.

| Revision and bound | Distinct / generated / depth | Core original | Core with module | TLC original | TLC with module |
|---|---|---:|---:|---:|---:|
| Before assumption edit, 3 | 349 / 5,819 / 4 | 44.93 s | 0.12 s | 18.82 s | 2.27 s |
| After assumption edit, 3 | 349 / 5,819 / 4 | 44.56 s | 0.12 s | 19.22 s | 2.21 s |
| After assumption edit, 4 | 1,549 / 30,039 / 4 | stopped, 180 s | 3.26 s | stopped, 180 s | 4.78 s |

All four original-bound checks exhausted the same graph on each revision. At the larger bound, the two module runs exhausted the graph, while original Core stopped with 505 distinct and TLC last reported 601. A one-coordinate `EXCEPT` formulation with the fixed-value checks also completed in 0.08 s on Core at the original bound. The reusable module supports several writable coordinates and explicitly checks unchanged entries for membership in the new range. When a model puts an error-producing expression or output before its frame check, moving the frame can change which errors or output the checker observes. [Operator contract](../user/modules.md#enumerating-functions-with-fixed-coordinates) · [All runs, digests, commands and negative controls](../../conformance/reports/perf/2026-11-21-ccf-function-frames.json).

The original abstract model in nominal TLC mode refused a history-sensitive startup enumeration. The concrete CCF consensus model uses `IOEnv`, which currently produces a named Core refusal. Pinned TLC checked the released concrete model before and after the pre-vote change under the upstream wrapper’s defaults. Both exhausted 4,266 distinct states, 9,073 generated, at depth 39. [Upstream pre-vote change](https://github.com/microsoft/CCF/commit/59d47cd21004e646981c67d7ec613316840c2f22).

### Check disjoint safety partitions

A model with fixed per-instance choices can often be checked as several independent state graphs. Pick a total, deterministic state predicate for each choice. Each truth assignment defines one cell. Restrict **Init** to the cell, keep **Next** unrestricted, and add the cell predicate as an invariant. If a reachable transition crosses the boundary, that cell must fail. Restricting Next or adding a state constraint to enforce the boundary would hide the transition.

On the released EmergencyCall f3 model, the two binary attach-accept choices split 49,152 initial states into four groups of 12,288. Each group passed the original three safety properties, deadlock checking and its closure invariant. Full exact identity gave 21,255,936 distinct states and 135,871,232 generated in total, with maximum depth 27. All four queues emptied. The result matches both the monolithic exact run and the authors’ TLC output. On one 96-core Graviton4 node, the four concurrent 24-worker checks finished in 83.86 s; the monolithic 32-worker run took 127.71 s. Those timings use different total worker counts. Choosing the mutable `user_make_call = 0` instead failed the closure invariant at depth two, and independent replay accepted the counterexample. [Per-cell counts, wall time, memory and negative control](../../conformance/reports/perf/2026-11-20-emergencycall-reference.json).

To choose partition predicates, run the [fixed-variable analysis](../../scripts/perf/scale/fixed_variables.py) on the original module and CFG. It uses the same bound IR as the symbolic backend and can inspect `Next` even when the symbolic layout refuses the model. Use the matching `libexec/dive-symbolic/symbolic_input` in a Linux release bundle.

```sh
python3 scripts/perf/scale/fixed_variables.py "$BINDER" "$MODEL/EmergencyCall_Model_auto.tla" "$CFG"
```

The output names variables proved equal before and after every satisfying transition. A conjunction inherits the guarantees of each conjunct. An alternative must preserve the variable in every branch. Unknown expressions contribute no guarantee. Some fixed variables can have the same initial value everywhere, so use the original initial predicate to choose splits with work on both sides.

For EmergencyCall the analysis identified **23 of 38 state variables**, including all six partition coordinates. No path can enter a cell from an initial state in another cell. Cell one reached level 43 (42 transitions from an initial state) with unrestricted `Next` and an empty queue. The authors reported a maximum initial distance of 26 transitions. These distances rule out treating the reconstructed full-Init experiment as a reproduction of the published unconstrained result. [Source identities, fixed variables and completed cells](../../conformance/reports/perf/2026-11-20-emergencycall-reference.json).

For a simple safety CFG with explicit INIT/NEXT, generate each cell with the [partition script](../../scripts/perf/scale/safety_partition.py). The first predicate is the low bit of the cell number. The generator uses the complement for a zero bit, so values outside a proposed two-value domain also belong to a cell. It preserves the original invariants and deadlock setting. The generated module imports the original from its module search path.

```sh
for i in 0 1 2 3; do
  python3 scripts/perf/scale/safety_partition.py "$MODEL/EmergencyCall_Model_auto.tla" \
    "$MODEL/config/f3.cfg" "cell-$i" --cell "$i" \
    --when 'amf_0_normal_attach_accept_include_enl = 1' \
    --when 'amf_1_normal_attach_accept_include_enl = 1'
  dive check "cell-$i/DiveSafetyCell$i.tla" --config "cell-$i/DiveSafetyCell$i.cfg" \
    --module-path "$MODEL" --store exact --workers 24 --seed 0 --fp 0 --compat=tlc --output json
done
```

Combine results only after all cells exhaust their queues and pass the closure check. For exact full-state identity the distinct, generated and initial counts add, and the maximum cell depth is the whole-model depth. Every reachable state belongs to one cell and every successor of an admitted state stays in that cell. A stopped run or a cell reporting no initial states needs separate resolution before reporting any whole-model total. The script accepts simple INIT, NEXT, INVARIANT(S) and CHECK_DEADLOCK clauses. Use independent processes on a suitably sized host to run cells concurrently.

With the released `Init` and just the type and set safety invariants, the first four cells of a six-variable reconstruction each admitted 884,736 initial states. They reached 469,762,578–469,764,632 distinct exact states before the 40 GiB sampled cap. The count clusters near the growth threshold of the resident exact hash table. Each terminal queue remained open. At 84 GiB, cell zero exhausted with 600,429,632 exact identities, 3,862,251,520 generated and depth 39, in 1,884.64 s at 62.60 GiB peak RSS. Cell one exhausted with 912,926,560 exact identities, 6,006,048,236 generated, depth 43 and an empty queue in 2,913.01 s at 63.86 GiB peak RSS. Those are two of 64 cells in the reconstructed configuration. [Source, predicates and per-process peaks](../../conformance/reports/perf/2026-11-20-emergencycall-reference.json).

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
