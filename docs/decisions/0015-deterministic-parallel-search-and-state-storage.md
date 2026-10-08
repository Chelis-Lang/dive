# 0015: Deterministic parallel search and state storage (`dive-check`, `dive-store`)

- **Status:** accepted (2026-10-08)
- **Date:** 2026-10-08

- **Owner decision (2026-10-08):** canonical exploration order in the default profile, and TLC's order under `--compat=tlc` (D-010; open question 9).

## Context

ADR 0010 makes deterministic parallel search the default. The owner confirmed parallel workers on by default. Any worker count must give the same results as TLC's single-worker run:
- verdict and violated property;
- distinct, generated and queued counts;
- depth;
- the same shortest counterexample.

ADR 0004's harness rule enforces this: dive is compared with the `-workers 1` golden on every field at every worker count. Any variance is a `disagree`.

TLC paths are relative to `tlatools/org.lamport.tlatools/src/` at tlaplus `94d0c50`. Probes are in `crates/dive-check/tests/fixtures/tlc-probes/` (see its README for the transcripts).

### TLC's single-worker search, exactly

**Queue discipline.**
- The state queue is a single FIFO (`DiskStateQueue`, the default; `queue/IStateQueue.java:21-40`; `DiskStateQueue.java:86-165`).
- `size()` counts states enqueued and not yet dequeued. A state being expanded is no longer counted (`StateQueue.java:134-141`, `:410-411`).

**Initial states** (`ModelChecker.doInit` and `DoInitFunctor`, `ModelChecker.java:364-404`, `:1151-1234`). States are generated in `getInitStates` order, and each is counted as generated first (`:1155`). Then:
1. Check that every variable is assigned; otherwise `TLC_INITIAL_STATE` (2102).
2. If it satisfies `CONSTRAINT`:
   - put its fingerprint into the set;
   - if new, record it in the trace and **enqueue it before checking**.
3. If it was new or is outside the model, check the invariants, then the implied initial predicates.

A violation aborts initial-state generation. In that case TLC **prints no statistics at all** (`ModelChecker.java:152-160`; probe `InitViolation`).

**Main loop** (`Worker.run` and `Worker.addElement`, `Worker.java:92-158`, `:418-491`). Dequeue `s` (at level `L`) and enumerate its successors in TLC's order (ADR 0014 §3). For each successor `t`:
1. `generated += 1`, before anything else (`:420`).
2. If `t` leaves a variable unassigned: `TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT` (2109).
3. `inModel` means `t` satisfies `CONSTRAINT` (evaluated with `t`'s predecessor set, so `TLCGet("level")` is `L+1`) and `(s, t)` satisfies `ACTION_CONSTRAINT` (`:434-435`).
4. If `inModel`, `seen = fpset.put(fp(t))`, which inserts. If `t` is new, record `(s, t)` in the trace and update the depth (`:438-441`, `:522-546`). Outside the model, `t` is never inserted, recorded or enqueued.
5. **If `t` is new or outside the model**, check the invariants in cfg order (`:459-463`). Probe `ConstrainedViolation`: a successor outside `CONSTRAINT` is reported as a violation.
6. **Always** check the implied actions on `(s, t)`, whether `t` was seen or not (`:467-469`; probe `ImpliedActionSeen`).
7. If `t` is in the model and new, enqueue it (`:471-475`).

After the enumeration:
- **Deadlock** means that no successor was generated at all. This counts successors outside the model (`:114`, `:125-130`). Probes `ConstraintNoDeadlock` and `ActionConstraintNoDeadlock` are successes. Probe `DeadlockMidLevel` finds the deadlock when the deadlocked state is dequeued.

**Errors and statistics.**
- **Errors.** An evaluation error while enumerating ends the run. Successors generated before it keep their counts (probe `EvalErrorMidLevel`: 4/3/1).
- **Depth.** The depth is the maximum of `level(s) + 1` over new in-model successors recorded so far. Its minimum is 1, and it is reported as 0 when nothing was generated (`Worker.java:303`; `ConcurrentTLCTrace.java:76-82`; `ModelChecker.java:889-890`). The depth can therefore be smaller than the counterexample (probe `ConstrainedViolation`: depth 2, trace 3) or larger (probe `DeadlockMidLevel`: depth 3, trace 2).
- **What a violation prints.** At a violation TLC prints `TLC_STATS` (generated, distinct, queue) and `TLC_SEARCH_DEPTH`, but no collision probability (`ModelChecker.java:250-255`, `:321-324`, `:885-890`).

**The counterexample.**
- **The path.** It is the path of first-discovery predecessors. Each state's recorded predecessor is the state whose expansion first inserted it (`Worker.java:301-318`).
- **Reconstruction.** The concrete states are rebuilt by replaying `Init` and `Next` and taking the **first** successor whose fingerprint matches (`Tool.java:3719-3800`). The step labels come from the matching action.
- **Ends.** For a violating successor, the trace ends with that successor. For a deadlock or an error while expanding, it ends with the state being expanded (probes `DeadlockMidLevel`, `EvalErrorMidLevel`).

**Where order comes from.**
- With one worker, all of the above is a deterministic function of three things:
  - the order of initial states;
  - the order of successors;
  - FIFO order.
- The fingerprint set affects only membership, never order. Its hash layout and the random polynomial cannot change which state, which counterexample or which counts are reported, except through a fingerprint collision.
- The **order of successors depends on value order**: `\E`, `x' \in S`, filters and maps enumerate sets in normalized order (probes `ExistsSetOrder`, `InSetOrder`, `FilterSetOrder`, `MapSetOrder`), and normalized order for strings and model values is TLC's intern order. Probe `StringOrderViolation`: TLC reports `InvZeta` because it enumerates `"zeta"` before `"alpha"`. Probe `StringOrderPreinterned` shows that intern order includes names from standard modules TLC always loads.

### What multi-worker TLC and competitors do

- **TLC with several workers** shares one queue. The worker that first locks the checker wins the error. Counts, depth and the violated property then vary (ADR 0004; tlaplus#883).
- **tla-mc** has level barriers but races for each state's parent and for the reported failure: it inserts into a global set during expansion, and the first failure to take a mutex wins (`check.rs:255-258`, `:510-516`, `:567`). Its counts at a violation vary with workers (survey probe P8).
- **tla2** steals work without levels. Its parallel traces are empty.

## Decision

### 1. dive's reference order

**The order.** Call it 𝒪: a total order on everything the search does.
- Levels in increasing order.
- Within level `L`, states in frontier order. The frontier is TLC's queue order for that level.
- Within a state, successors in enumeration order (ADR 0014 §3).
- Within a successor, the check phases in TLC's order:
  - 0: completeness, `2109`;
  - 1: constraint evaluation;
  - 2: invariants, in cfg order;
  - 3: implied actions, in cfg order.
- An evaluation error raised after a state's `k`-th successor comes after that successor's checks.
- A state's deadlock event comes after all its successors.

**The frontier.** Level 1 is the in-model, first-occurrence initial states in `Init` order. Level `L+1` is the in-model, first-occurrence successors of level `L`, in 𝒪 order.

**The stop and the tree.**
- dive stops at the **first event in 𝒪**: a violation, an error or a deadlock. Call it E\*.
- Every state's parent is the predecessor of its first occurrence in 𝒪. This is the BFS tree.

**What dive reports.**
- The verdict and property of E\*.
- The counts and depth defined in §3.
- The counterexample is the tree path to E\*'s state, followed by E\*'s successor where E\* is a successor event.

**Equality with TLC.**
- With TLC's value order (`--compat=tlc`), 𝒪 is exactly TLC's single-worker execution order, so every reported field equals TLC's `-workers 1` output.
- The only exception is a fingerprint collision, which neither tool can rule out in fingerprint mode (§5). Under exact storage, dive explores what TLC would have explored without collisions.
- This depends on dive's intern order being TLC's. Names must be interned in the order TLC meets them, in the modules the spec loads (probe `StringOrderPreinterned`). TLC parses no standard module that the spec does not load; an earlier version of this paragraph said it did, but that observation was an artefact of the harness passing `-dumpTrace json` (issue #35). That is the `dive-value`/`dive-sema` interner's contract for `--choose-order=tlc`, and the string-order probes test it.
- This is the definition of "same counterexample" (charter rule 2, ADR 0004): the same states, labels and length as TLC's single-worker trace, modulo how TLC reconstructs concrete states (§4).

### 2. Level-synchronous search with a deterministic merge

Each level `L` runs in three phases separated by barriers, on `std::thread::scope` workers. There is no global thread pool, so embedding stays clean.

**The frontier.**
- `F_L` is a sequence of blocks. Each block is a contiguous range of the frontier in 𝒪 order, held as encoded states (§8).
- Block size adapts to the level: about 16 blocks per worker, between 1 and 256 states. tla-mc's measurement showed that larger blocks idle workers at the end of a level.
- **Narrow levels.** A level smaller than `2 × workers` states is split by action instead: block = (state, action range). Positions stay totally ordered, because successors are ordered by action and then by inner order. This keeps the first, narrow levels of a search parallel. Each state's per-action counts are summed before its deadlock check.

**Phase 1: expand.**
- Workers claim blocks through an atomic counter (work stealing within the level). Each block's output goes into a slot indexed by block id (`OnceLock`), so the merge never depends on who ran the block.
- For each state, the worker runs the enumerator and, for each successor, in order:
  1. counts it;
  2. checks it is fully assigned;
  3. evaluates `CONSTRAINT` and `ACTION_CONSTRAINT`;
  4. evaluates the implied actions on `(s, t)`;
  5. computes the dedup key (fingerprint, or exact key) for in-model successors.
- **Pre-filtering.** In-model successors whose key is already in the global set are recorded only as counts: the set is read-only during the phase, so this needs no locks. Successors that may be new become **candidates** `(key, position, encoded state, action label)`. Each candidate is appended to a per-worker, per-shard buffer.
- **Outside the model.** Successors outside the model are invariant-checked here, every time they occur, as TLC does.
- **Per state, the block records:**
  - the generated count;
  - the position and kind of any event (completeness, constraint error, implied-action violation or error, enumeration error, deadlock).

**Phase 2: resolve.**
- Each shard of the global set is owned by exactly one worker for the phase. Ownership comes from splitting the shard array with `chunks_mut`, so the shard tables need no atomics or locks.
- For its shard, a worker:
  1. gathers the candidates from every worker's buffers;
  2. sorts them by `(key, position)`;
  3. keeps the minimum position per key, excluding keys already present;
  4. inserts the winners;
  5. marks each winner in its block's winner bitmap (atomic bit sets; safe Rust).
- Winners are exactly TLC's new in-model states. Ties go to the earliest position.

**Phase 3: check and assemble.**
- Workers claim blocks again. For each block's winners, in position order, they evaluate the invariants and record any violation's position.
- Each block's winners, in position order, form its part of `F_{L+1}`. Concatenating blocks in block order gives `F_{L+1}` in 𝒪 order.
- Parent links are recorded (§4).

**After the level.** E\* is the minimum event position over all blocks and phases. If there is none, the level's counts are added and the search continues with `F_{L+1}`.

**Early cut.**
- An `AtomicU64` holds the smallest event position found so far. A worker skips a block whose first position is beyond it, since such a block can only add events after E\*.
- Blocks before E\* are always finished. Wasted work at a violation is bounded by the blocks in flight.
- Correctness never depends on the cut.

**Why this equals the sequential order.**
- A candidate is a first occurrence in 𝒪 exactly when it has the smallest position among the level's candidates with its key, and its key was not in the set before the level. Phase 2 computes precisely that.
- Invariants are a function of the state alone. Checking them on winners only finds exactly the violations TLC finds: TLC checks first occurrences.
- Implied actions and constraints are functions of `(s, t)`, checked on every pair as in TLC.
- The deadlock check is a function of `s`.
- Every event is therefore computed regardless of scheduling, and the minimum is unique.

**Small levels.** A level with fewer states than one block per worker runs inline on the coordinating thread, through the same code. Sequential mode (§6) uses this path for every level.

### 3. Counting, including at a violation

**Notation.**
- `G_L`, `D_L` and `depth_L` are the totals before level `L` is expanded.
- Level `L` has states `s_0 … s_{m-1}`, and `s_i` generates `g_i` successors.
- `W` is the set of winners (new in-model first occurrences), with their positions.

**A level that completes:**
- `G_{L+1} = G_L + Σ g_i`;
- `D_{L+1} = D_L + |W|`;
- the depth becomes `L+1` if `W` is non-empty.

**A stop at E\*** on state `s_i`. `c` is the number of `s_i`'s successors counted by TLC at the stop:
- `j+1` for an event on successor `j`;
- `k` for an enumeration error after `k` successors;
- 0 for a deadlock.

| Field | Value at E\* |
|---|---|
| generated | `G_L + Σ_{i' < i} g_{i'} + c` |
| distinct | `D_L + \|W ∩ {position ≤ E*}\|`. E\*'s own state counts if it is a new in-model state and the event is in phase 2 or 3, because TLC inserted it before checking |
| left on queue | `(m − i − 1) + \|W ∩ {position < E*}\|`. E\*'s own state is never enqueued |
| depth | `max(depth_L, L+1)` if `W ∩ {position ≤ E*}` is non-empty, else `depth_L`. 0 if nothing was generated |
| counterexample | path to `s_i`, then E\*'s successor (successor events); path to `s_i` (enumeration error, deadlock) |

**Initial states** are generated sequentially, in `Init` order, with positions equal to their generation index.
- The same definitions apply. Successors here are initial states; there is no predecessor.
- At an initial-state violation the conformance adapter reports **no counts**, matching TLC. dive's own JSON may report its counts, labelled as dive's.

**Checked against the probes.** Every probe in the fixture set reproduces from these formulas. Four examples:

| Probe | Derivation |
|---|---|
| `MidLevelViolation` | E\* = (s₀, successor 1). Generated 1+2 = 3; distinct 1+2 = 3; queue 0+1 = 1; depth 2 |
| `EvalErrorMidLevel` | E\* = (s₀ of level 2, error after 1). Generated 3+1 = 4; distinct 3+0 = 3; queue 1+0 = 1; trace 2 |
| `DeadlockMidLevel` | E\* = (s₁ of level 2, deadlock). Generated 3+1 = 4; distinct 3+1 = 4; queue 0+1 = 1; depth 3; trace 2 |
| `ConstrainedViolation` | The violating successor is outside the model. Generated 3; distinct 2; queue 0; depth 2; trace 3 |

**Worker invariance by construction.** Every term is a sum over positions before E\*, or a set defined by 𝒪. None depends on which worker ran which block.

### 4. Parents and counterexamples

**The trace log.**
- dive numbers distinct states in 𝒪 order: state id = discovery index.
- For each one it appends `(parent id, successor ordinal)` to a trace log, 12 bytes per state, in memory and spilled to disk append-only.

**Reconstruction is a replay.**
1. Regenerate the initial states, or the successors of the parent's concrete state.
2. Take the successor at the recorded ordinal.
3. Take its label from the action that produced it.

Randomness replays exactly, because generation is seeded per state (ADR 0014 §7). No fingerprint matching is involved.

**Compared with TLC.**
- TLC instead takes the *first* successor whose fingerprint matches (`Tool.java:3785-3800`).
- With neither `VIEW` nor `SYMMETRY`, the two coincide, barring collisions.
- With `VIEW` or `SYMMETRY`, dive prints the concrete states it actually explored, which are a valid behaviour. TLC may print a different member of the same class. The harness compares length and validity, not content (ADR 0004), so this is result-preserving.
- In exact mode, the stored state itself can be used instead of a replay.

**Validation.** Every counterexample is validated by `dive-trace` before it is reported (ADR 0016). A counterexample that fails validation is an internal error, reported as a bug, never as a verdict.

### 5. Storage: fingerprint mode, exact mode, disk, and the collision bound

**The fingerprint set.**
- `dive-store` keeps `2^k` shards (k chosen from the worker count and the memory budget), selected by a key's top bits.
- Each shard is an open-addressing table of `u64` with linear probing. Its load factor is at most 0.75, and 0 marks an empty slot (a key of 0 is remapped). That is 8 bytes per slot, about 11–16 bytes per state.
- Linear probing over contiguous `u64`s compiles to vectorized comparisons without intrinsics.
- In phase 2, candidates are sorted by key, so probes walk the table in near-sequential order.

**The fingerprint function.**
- **Default profile: two levels.** A state's fingerprint is FP64 over the per-variable FP64 value fingerprints (`dive-value`'s TLC-compatible FP64), in the IR's variable order, under the reported polynomial index (`StateFingerprinter::new`).
- **Caching.** Value fingerprints are cached in shared compound values, so a successor that keeps most of its parent's values reuses their fingerprints. The cost of fingerprinting is proportional to what changed.
- **Not TLC's bits.** These are not TLC's fingerprints: TLC hashes every value's bytes in one stream, in TLC's variable order. Fingerprint values are not part of any compared result.
- **Under `--compat=tlc`: TLC's own.** The store keys on TLC's state fingerprint, bit for bit: `TLCStateMut.fingerPrint`, one FP64 stream over the values in TLC's variable order (`StateFingerprinter::tlc`, given `Ir::tlc_var_order`). It cannot reuse cached value fingerprints, so it costs more per state. The fingerprint set, and therefore the "actual" collision figure and any collision, are then TLC's. TLC's fingerprints are also what TLC semantics observe:
  - for each *expanded* state, to seed randomness under `--compat=tlc` (ADR 0014 §7);
  - for each distinct state, to print TLC's "actual" probability under `--compat=tlc`.
- **What the scheme changes.** Only which fingerprints are produced. Verdicts, counts and traces are the same under both schemes, except through a collision. The default profile keeps the cheaper two-level scheme in the IR's order; nothing found while implementing WP-A3 argues for changing that.
- **`VIEW` and symmetry.** With `VIEW`, TLC fingerprints only the view's value (`val.fingerPrint(FP64.New())`, `TLCStateMut.java:262-270`), so the variable order does not enter. With `SYMMETRY`, it does twice: the smallest permuted state is chosen by comparing values in variable order, then fingerprinted in that order.
- **The polynomial index.**
  - In the default profile it is fixed at 0 and reported, so identical inputs give identical runs. `--fp N` selects another.
  - Under `--compat=tlc` it is random per run unless `-fp` is given, as in TLC (`TLC.java:230`), and it is always reported.
  - This is result-preserving. Only a collision could make it observable.

**TLC's variable order (WP-A3).** TLC keeps a state's values in an array indexed by each variable's `varLoc`, walks it in that order to fingerprint (`TLCStateMut.fingerPrint`) and to print states (`TLCStateMut.toString`). The order, established from the pinned source (`94d0c50`):
1. `SpecProcessor.processSpec` takes `rootModule.getVariableDecls()` and gives the `i`-th declaration `varLoc` `i` (`varNames[i].setLoc(i)`).
2. `ModuleNode.getVariableDecls` (`ModuleNode.java:400-408`) is the *reverse* of `Context.getVariableDecls`, and is cached on first use.
3. `Context.getVariableDecls` (`Context.java:320-331`) filters `table.elements()`: the enumeration of the root module context's `java.util.Hashtable<Object, Pair>`. The context holds every name visible in the module: the built-in operators (copied by `Context.duplicate`), every extended module's non-`LOCAL` symbols (`mergeExtendContext`), the module's own definitions and declarations, `I!Op` names of instances, and inner-module names (keyed by `SymbolTable.ModuleName`).
4. Keys hash with `String.hashCode` of the name: `UniqueString.hashCode` and `ModuleName.hashCode` both return it. **Intern tokens play no part in the order.**
5. The JDK 17 `Hashtable` (the pinned runtime is Temurin 17.0.20.1+1): capacity 11, load factor `0.75f`; when `count >= threshold` an insert first rehashes to `2·capacity + 1`, moving old buckets from last to first, each chain from its head, each entry to the head of its new bucket; a new entry goes to the head of bucket `(hash & 0x7FFFFFFF) % capacity`; `elements()` walks buckets from last to first, chains from the head.

So the order depends on the variables' names, on how many keys the context ends up with (which fixes the capacity), and on when each variable was inserted relative to the resizes. Nothing else matters: two keys' relative position in the table is decided by their own hashes and insertion times and the resize schedule alone, so, for example, the order in which the built-ins were copied cannot change it.

`dive-sema` ports this as `Model::variable_decls` (and `constant_decls`, the same structure, which is TLC's order for processing constants), over its context, which records SANY's insertion order (`crates/dive-sema/src/hashtable.rs`). `dive-ir` records the result as `Ir::tlc_var_order`, a permutation of the `VarId`s, which stay in the context's insertion order. The computation lives in `dive-sema` because it needs the whole context, including built-ins and module names that the IR does not keep; it is a port of a SANY method, not of anything in TLC's evaluator.

**Evidence.** Against the pinned jar (`crates/dive-ir/acceptance/VarOrderDump.java`, which reads `SpecProcessor.getVariablesNodes()` and checks each `getVarLoc()`, or `ModuleNode.getVariableDecls()` through SANY):
- 29 generated specifications, all matching (`crates/dive-ir/tests/var_order.rs`): 1, 2, 5, 11, 12, 13, 25 and 50 variables, each with no standard module, with `Naturals`, and with five standard modules; names with equal Java hashes (`AaAa`, `BBBB`, `AaBB`, `BBAa`) in both declaration orders; variables inserted before and after 300 definitions; `EXTENDS` diamonds; named, unnamed and `LOCAL` instances and an inner module. The root contexts have 75 to 393 keys. The 72 built-ins alone pass the first four resize thresholds (8, 17, 35, 71), so every variable is inserted at capacity 191 or more; the declarations of `VarsStd25` and `VarsStd50` straddle the resize at 143 keys, and `Epochs` has variables inserted two resizes apart (capacities 191 and 767).
- 234 repository modules SANY accepts, all matching.
- All 424 Examples modules (`6a7b26a`), 303 of them with two or more variables (1,639 variables, up to 40 in one module), all matching (`examples/var_order_oracle.rs`).
- **End to end:** every state of TLC's `-fp 0 -dump dot` output for the 29 specifications (116 states) has its full 64-bit fingerprint reproduced from the specification text: `dive-sema` and `dive-ir` give the order, `dive-value` the values, `StateFingerprinter::tlc` the fingerprint (`crates/dive-store/tests/tlc_state_fingerprints.rs`). The IR's own order gives different fingerprints on the 24 specifications whose orders differ. TLC prints each state's variables in the same order.

**What the fingerprints still need from interning.** The variable order needs nothing from the interner. The values do (ADR 0012): a model value fingerprints its absolute intern token, and sets of strings or model values, and records' fields, are fingerprinted in their normalized order, which is token order. So TLC-identical state fingerprints for such states also need `dive-sema`, `dive-config` and the evaluator to intern names in TLC's encounter order, starting from TLC's pre-interned names (ADR 0012, "For the front end"). Integers, booleans, strings, intervals, tuples and functions over these do not depend on it.

**The collision bound.** Every fingerprint-mode result reports it.
- **Optimistic:** `distinct × (generated − distinct) / 2^64`, TLC's formula (`AbstractChecker.java:206-208`).
- **Actual:** `1 / (minimum gap between sorted fingerprints)`, rounded half-up to two significant digits, TLC's `checkFPs` (`DiskFPSet.java:624-648`). TLC's default set is a `MultiFPSet` of two sub-sets split by the top bit (`fpBits` = 1), and each sub-set computes its own minimum (`MultiFPSet.checkFPs`), so pairs that straddle the split do not count. This is observable: on `nbacg_guer01` the closest pair straddles it, and TLC prints `1.5E-11`, not `1.7E-10` (fixture `crates/dive-store/tests/fixtures/tlc-fingerprints`).
- **When:** TLC computes and prints the actual figure only when the optimistic one is at least `1E-10` (`ModelChecker.reportSuccess`). The shards hold contiguous fingerprint ranges, so the gap is computed at the end by sorting each shard in turn.
- **Where it appears:**
  - human output prints it where TLC does (on success);
  - JSON always carries it, with `store: "fingerprint"` and the polynomial index.
- dive also states plainly that the bound is an estimate assuming fingerprints behave as random 64-bit values. That is TLC's assumption too.

**Exact mode (`--store exact`).**
- **Storage.** Shards hold full states in a canonical key encoding (`dive_value::codec::encode_canonical`): within a run, two states have equal keys exactly when they are equal, whatever the representation of their values (`1..3` and `{1, 2, 3}` have one key). A 64-bit hash of the key bytes indexes them, independent of the fingerprint function, and byte comparison decides equality.
- **Memory.** The key, a one-byte length, and a 16-byte slot at a load factor of 3/8 to 3/4. Measured at about 36 bytes per state on top of the key.
- **Exhaustiveness.** Results say `store: "exact"` and claim exhaustiveness unconditionally (charter rule 4).
- **Equivalence with fingerprint mode.** Both modes use the same 𝒪 and the same merge. Their results are identical unless fingerprint mode had a collision. The harness runs both on the standard tier, and any difference is reported as a collision finding.

**Disk spill.**
- **When.** Each shard has a memory budget. When it is exceeded, the shard's table is written as a sorted run file, with:
  - a Bloom filter of about 10 bits per key and 4 probes;
  - a sparse index holding the first key of every 128-key block (tla-mc `store.rs:118-166`).
- **Lookups, without random reads.** Level synchrony makes spilled lookups sequential. In phase 2 the shard's sorted candidates are **merge-joined** against its sorted runs (delayed duplicate detection).
- **Runs and filters.** Runs accumulate and are compacted occasionally, rather than rewriting one file per spill (the quadratic I/O noted in tla-mc). The phase-1 pre-filter consults the in-memory table and the Bloom filters only, never the disk.
- **Exact mode** spills in the same way: hash-sorted runs of keys with offsets into append-only state files.
- **Frontier spill.** Frontier blocks spill to disk once the level exceeds its memory budget. A sticky "spilled" flag keeps the read order equal to the write order (tla-mc `store.rs:639-644`; the order matters for 𝒪).
- **Trace log.** The trace log is always append-only.
- **I/O.** All of it uses `std::fs` with positioned reads (`FileExt`). There is no `mmap`.
- **Checkpoints and resume.** These come at level boundaries in a later milestone: the set's runs, the frontier, the trace log, the counters, the interner, the seed and a spec hash. Resuming against a changed spec is refused.

### 6. Constraints, deadlock, `VIEW`, symmetry, and sequential mode

**Constraints and deadlock.**
- `CONSTRAINT` and `ACTION_CONSTRAINT` are evaluated in phase 1 with TLC's semantics (Context, main loop, step 3). Successors outside the model are counted, invariant-checked on every occurrence, never inserted or enqueued, and they prevent deadlock.
- Deadlock checking follows the cfg's `CHECK_DEADLOCK` and TLC's `-deadlock` flag.

**The canonicalization hook.** The dedup key comes from one hook in `dive-check`:
- **Identity.** The state's fingerprint or encoding.
- **`VIEW`.** The view value's fingerprint or encoding, evaluated on the state (`TLCStateMut.java:262-270`). Counterexamples still show full states.
- **`SYMMETRY`.** The lexicographically smallest permuted state under TLC's `compareTo`, in variable order (`TLCStateMut.java:205-248`), with permutations generated as in `MVPerms.permutationSubgroup`. Then `VIEW`, if any, is applied to it.
  - The *explored* state stays the first-found concrete state, as in TLC. Because 𝒪 is TLC's order, the explored representatives, and so the counts of non-symmetric specs, also match TLC.
  - Symmetry is scheduled after M5. Until it is implemented, `SYMMETRY` is refused as `unsupported`. Symmetry with liveness is refused by default (ADR 0010).

**Sequential mode.** When the effects analysis (ADR 0013 §6) finds a checking path that writes a register, reads a live counter, calls `TLCSet("exit")` or performs CommunityModules I/O, dive runs every level inline with one logical worker. These specs depend on TLC's sequential side effects (ADR 0014 §7).
- **Reported.** The run says so: `workers: { requested, used: 1, reason }`.
- **Result-preserving.** It gives TLC's single-worker results, which is the contract.
- **Output.** `Print` output is ordered in every mode (ADR 0014 §7).

### 7. Value order and exploration order in the default profile

Under the owner-approved default `--choose-order=canonical` (ADR 0010, D-006), sets of strings and model values are stored, and therefore enumerated, in canonical order.

**What does not change.** Results of complete searches: the reachable state set, distinct and generated totals, and depth (the BFS depth of each state) do not depend on successor order.

**What can change.** At a violation, TLC's order and canonical order can stop at different events. The canonical-order run can differ in:
- which violation, and so which property or error class, is reported among events in the same level;
- the counts at the stop;
- which shortest counterexample is reported. The length is always the same.

Probe `StringOrderViolation` shows TLC reporting `InvZeta`, where canonical order would report `InvAlpha`.

**Recommendation.**
- Exploration follows the run's single value order, which keeps the default profile text-independent: the purpose of D-006.
- dive also detects when it matters. Atoms carry both orders (`dive-value` interns names in TLC's encounter order as well, as `--choose-order=tlc` needs). dive notes when a set it enumerated had elements whose two orders disagree. If such a run stops at a violation, the output says the result may differ under `--compat=tlc`, as ADR 0010 §3 requires.
- The harness would then classify a default-profile disagreement confined to at-violation fields on such a run as `expected-divergence`, under D-006.

This needs an owner decision (open question 9), because it widens D-006's documented effect. Until it is decided:
- `--compat=tlc` is unaffected;
- the default profile carries the flag.

### 8. Performance strategy

Targets are ADR 0006's, at one and eight workers. Ingredients:

**Allocation.**
- One reusable successor buffer per worker, with bind and undo (ADR 0014 §3).
- A successor is materialized only when emitted. Its values are shared with its parent by reference count.
- Per-worker scratch arenas for candidates are cleared per level, not freed.

**Sharing without contention.**
- Frontier blocks are encoded. Each worker decodes a private copy, so hot shared values are not reference-counted across cores. tla-mc measured spilled, encoded frontiers faster than shared in-memory trees (tla-mc `store.rs:584-589`).
- The encoding is compact: varints, interned atom ids, tagged values. It comes in two forms. The frontier, disk spill and checkpoints use the lossless form, which keeps each value's representation, because TLC's printing and comparability depend on it. The exact store uses the canonical key form (§5).

**Fingerprints.**
- Two-level FP64 with cached value fingerprints (§5).
- Successors outside the model are never fingerprinted.
- Duplicates already in the set are filtered in phase 1 by a lock-free read.

**Sets.**
- Sharded `u64` tables with exclusive ownership in phase 2: no CAS, no locks, and sequential probe patterns from sorted candidates.
- Spill uses sorted runs and merge-joins.

**Checks.**
- Invariants are evaluated once per new state, on winners only.
- Constraints and implied actions are evaluated once per generated pair, as TLC requires.

**Scaling.**
- Work stealing within a level by block claiming. Narrow levels are split by action.
- The early cut bounds wasted work at a violation.
- Barriers cost three per level. Levels are typically tens to hundreds, and tiny levels run inline.

**What is measured.** `dive-bench` reports states per second, peak RSS, start-up time and 1→8 scaling against the pinned TLC. It uses the survey's method and specs: `CoffeeCan1000Beans`, `MCBakery`, `MCbtree`, `MCLamportMutex`, `GameOfLife`, `SimpleRegular`.

### 9. No `unsafe`

The design needs no `unsafe` code. It uses:
- `std::thread::scope`;
- atomics (`AtomicU64`, `AtomicUsize`, atomic bitmaps);
- `OnceLock`;
- exclusive shard ownership through slice splitting;
- positioned file reads.

Every crate keeps `#![forbid(unsafe_code)]`.

**Future candidates that would need `unsafe`.** These are recorded here and not assumed:
- explicit SIMD intrinsics or prefetch for probing;
- carry-less-multiply intrinsics to combine FP64 values faster;
- `mmap` for spill files.

Each would need its own ADR, an isolated module, a safety argument, Miri tests and owner approval (charter). None is requested now.

## Consequences

- **Every result is the same at every worker count, by construction.** dive's results equal TLC's single-worker results under `--compat=tlc`. The harness's worker-invariance rule (ADR 0004) becomes a regression test of the construction, not a statistical hope. It runs at 1, 3, 4 and 8 workers, plus a chaos mode: test-only random block sizes and claim orders, under which results must stay byte-identical.
- **The barriers** cost little on wide levels and are avoided on narrow ones. Specs with very deep, narrow state graphs (tens of thousands of single-state levels) gain nothing from workers. They run inline and are no slower than sequential.
- **Phase 2 holds every candidate of a level**, which bounds memory by the widest level's new-candidate count rather than by the state space. Spilling per-shard candidate buffers is part of the disk work package.
- **Fingerprint values** differ from TLC's. They are not compared. TLC-identical fingerprints, needed for randomness under `--compat=tlc` and for TLC's printed probability, wait for variable-order emulation.
- **The default profile's** at-violation results can differ from TLC's when canonical and TLC orders disagree. That is pending open question 9, and always flagged.
- **Revisit:**
  - when symmetry is implemented (representative selection and its probes);
  - when liveness needs the behaviour graph (its edges come from phase 1);
  - after the first benchmark round (block sizing, shard count, the encoded-frontier trade-off).

### Store measurements (D1)

Measured on an Apple M3 (8 cores, 24 GB), rustc 1.99.0, with
`cargo bench -p dive-store --bench store` and
`cargo run -p dive-store --release --example memory`. The machine was shared (load
average about 15), so treat these divan medians as indicative.

| Operation | 1 thread | 2 | 4 | 8 |
|---|---|---|---|---|
| Fingerprint resolve, 2^21 candidates into 2^20 states (M candidates/s) | 22 | 31 | 46 | 59 |
| Exact resolve, 2^19 candidates with 48-byte keys (M candidates/s) | 10 | 17 | 22 | 28 |

| Per state | Cost |
|---|---|
| Fingerprint set | 13–18 bytes (16.7 at `CoffeeCan1000Beans`'s 501,500 states) |
| Exact store, 48-byte keys | 85 bytes |
| Trace log | 12 bytes |
| Phase-1 lookup, fingerprint set of 2^20 | 30 ns, one thread |
| Sequential insert | 34 ns |
| Two-level fingerprint, four variables, compound values cached | 75 ns (TLC's single stream, uncached: 1.6 µs) |
| Encode, decode of that state (121 bytes) | 0.7 µs, 2.9 µs |

Against ADR 0006's baselines for `CoffeeCan1000Beans`: at one worker TLC spends about 11 µs per generated state (22.2 s for 2,000,002), and at eight workers its peak RSS is about 2.6 KB per distinct state (1287 MB), against about 550 bytes for tla-mc (277 MB). The store's share in dive is about 29 bytes of fingerprint set and trace log per state, or 15 MB in all, and about 0.15 µs per generated successor at one thread.
