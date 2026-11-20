# Large-model performance and exact symbolic checking

The [source-driven symbolic checker](../user/symbolic.md) checks finite safety specifications using binary decision diagrams (BDDs). Dive resolves the original modules, binds the configuration and exports its IR. The Python compiler reads the actual state types, initial predicate, actions, invariants, symmetry group and deadlock setting. KVS runs directly from its existing source with an inferred bit layout.

The [performance evidence index](../../conformance/reports/perf/README.md) collects compact records for reuse in other reports. The tables below identify which comparisons ran on the same host, which reached a complete fixed point and which stopped at a limit.

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

## Three ways to check an edit

- **Reachability.** Enumerate bounded action witnesses, compile guards and deterministic updates into Boolean functions, and compute exact images to a fixed point. Group writes by index and consume readers before overwriting their inputs. Cyclic dependencies use a simultaneous image. Symmetry counting requires a configured full group and checks that its generators preserve Init, every property and the multiset of guarded updates. Burnside counting preserves duplicate successor witnesses. Reverse images through BFS layers recover a shortest concrete counterexample. The CLI runs `dive validate-trace` on each reported violation.
- **Reuse a completed search.** Rebind and compile the edited input into the same BDD manager. Compare the concrete layout, initial predicate, symmetry and Boolean transition functions, including witness multiplicity. A match allows checking the new property directly on the reachable set. On Large, a holding property edit took 33 ms. A false property took 29 ms including independent validation of the three-state witness. Restricting Medium to open and rollback invalidated reuse and gave three distinct states, seven generated, depth three. A fresh explicit Core search agreed. Editing the imported OpenTx guard with the original entry point and configuration gave a validated initial deadlock. Adding a synthetic module with 1,024 live helper definitions of the same local property reused Medium’s closure in 0.23–0.30 s, including parsing and binding.
- **Prove safety by induction.** Use the conjunction of the configured invariants as a candidate predicate, or load a predicate from another process. Check Init inclusion, preservation under every source action, and implication of every configured invariant and requested deadlock-freedom condition. A failed obligation is inconclusive. Success proves safety for the finite configuration and supplies no exploration counts or diameter. The original Large invariants are inductive. The proof took 28 ms, and the same proof on a configuration expanded to 16 keys took 0.50 s. The full Large reachable predicate serializes to 24,809 bytes. A fresh process verified it against the original transitions and an added invariant in 52 ms, and against a restricted Next in 34 ms. The receiver checks the predicate against its own source. This is a candidate artifact for Sail to transport across workers.

The explicit Core edit experiment also captured 3,599 exact integer rows and parents. Rebinding and scanning a second property reused the completed closure, and a rebuilt transition produced 3,658 rows. Scanning took about 0.9 ms versus 12 ms for a fresh search, excluding binding. This gives an explicit alternative when a model has poor BDD structure.

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

The motivating measurements cover [the large non-finishes](https://github.com/Chelis-Lang/dive/issues/1034),
[slow-TLC models](https://github.com/Chelis-Lang/dive/issues/799) and
[temporal worker scaling](https://github.com/Chelis-Lang/dive/issues/1028).
