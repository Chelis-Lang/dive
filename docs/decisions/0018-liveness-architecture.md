# 0018: Liveness checking in dive (`dive-liveness`)

- **Status:** proposed
- **Date:** 2026-10-09

## Context

ADR 0017 specifies what TLC's liveness checker does, and what conformance compares. This ADR is dive's design for it. It has to satisfy:

- **Charter rule 2 and goal 1.** Identical inputs give an identical counterexample at any worker count. The choice is documented, and the counterexample is validated independently.
- **ADR 0004.** Every field is compared with the single-worker golden at every worker count.
- **ADR 0015.** The search is the deterministic, level-synchronous engine with order 𝒪. Liveness must not reintroduce scheduling into results.
- **ADR 0016.** `dive-trace` validates without the search code and without the successor enumerator.
- **ADR 0006.** `ElevatorLivenessMedium` in ≤ 0.5 s (TLC 5.9 s at 8 workers; tla-mc 0.14 s), with a full tableau and no shape restrictions.
- **D-005.** Symmetry with liveness uses the sound `unreduced` mode by default; TLC's behaviour is available under `--symmetry-liveness=tlc` (ADR 0019).

TLC builds a product of the state graph and each tableau eagerly, on disk, once per order of solution (OoS). It stores state bits once per (state, tableau node) pair, and runs Tarjan with a hash-order edge list (ADR 0017 §3–§4). dive keeps TLC's semantics and changes the data structures.

## Decision

### 1. Crates and layering

No layering change. `dive-liveness` already may depend on `dive-check`, `dive-store` and `dive-model` (`xtask/src/layering.rs`).

- **`dive-check`** gains a search observer trait, `LiveHooks`, which it calls and does not implement. It covers per-state and per-successor evaluation in phase 1, id assignment after phase 2, a per-level sequential "liveness phase", and the events it may raise (§3). It knows nothing about formulas.
- **`dive-liveness`** contains:
  - the formula pipeline (`formula`, §2);
  - the behaviour graph (`graph`);
  - the observer implementation;
  - the SCC and lasso searches (`search`, `tlc_search`);
  - lasso property evaluation (`attribution`);
  - the driver that runs `dive-check`'s engine with the hooks.
- **`dive-store`'s `StateSet`** gains an optional id column: `StateSet::with_ids`, 8 bytes per slot. With it, a seen key resolves to its state id in phase 1's read-only lookup. Ids are discovery indices in 𝒪 (ADR 0015 §4), assigned to winners in phase 2.
- **`dive-trace`** gains lasso validation (§8). It still depends on none of the above.
- **`dive-api`** routes models with implied temporals to `dive-liveness`, and the rest to `dive-check` unchanged.

### 2. Formula pipeline (port of `Liveness.java`)

`dive-liveness::formula` ports ADR 0017 §2 exactly, over IR expressions and the `Scope`s `dive-model` already binds (`Model::temporals`, `Model::implied_temporals`). Steps:
1. `astToLive`: expansion of operators, `\E`/`\A` unrolling over enumerable sets, `IF`, `=>`, `WF`, `SF`, `~>`.
2. `pushNeg`, simplification, then DNF with TLC's cross-product order and its merge of `[]<>` disjuncts.
3. Classification into EA, AE-state and AE-action conditions, and the tableau formula.
4. Grouping into OoS by syntactically equal tableau formula.
5. Tableau construction (`TBGraph`, `TBPar`) with TLC's node numbering.
6. TLC's errors (2213, 2214, 2253 with exit 77), the "N branches" message, and the warnings.

The result is a `LivenessPlan`:
- **atoms**, deduplicated across all OoS:
  - state predicates, including `ENABLED <<A>>_v` and the tableau nodes' state predicates;
  - action predicates `<<A>>_v` or `A`.
  Each atom is an `(ExprId, Scope)` with its subscript.
- **for each OoS:**
  - an optional tableau (nodes, successors, the initial set, accepting nodes, the promises each node fulfils, the atoms each node's consistency needs);
  - its PEMs, as index lists into the atom table, in TLC's order.

The pipeline is the same in both profiles: it is semantics.

### 3. The behaviour graph, built by the deterministic search

The graph is stored **once**, as the state graph labelled with atom bits. The product with each tableau is not stored. It is explored on the fly by the searches (§5, §6), with pure bit operations.

**What is evaluated, and where in ADR 0015's phases:**

| Atom kind | Evaluated on | When |
|---|---|---|
| State atoms | each expanded state `s` (the explored concrete state) | phase 1, by the worker expanding `s` |
| Action atoms | each pair `(s, t)` for every in-model successor `t`, seen or new, and for the stutter pair `(s, s)` | phase 1 |
| Tableau consistency atoms (state predicates) | the **concrete successor** `t` | phase 1, for every in-model successor; reused when `t`'s concrete state is the one stored for its key |
| Same, for initial states | each new in-model initial state | at initial-state generation |

Successors outside the model produce no edge (ADR 0017 §9). `ENABLED` is evaluated by the evaluator's `ENABLED`, which ignores the constraints, as TLC's does.

**Events, in 𝒪.** ADR 0015 §1 orders the events within a state by check phase. Two kinds are added:
- **Phase L, after deadlock (9).** For an expanded state `s`, "liveness evaluation of `s`" comes after its deadlock event, matching `Worker.java:125-136`. An evaluation error in any atom of `s`, of a pair `(s, t)`, or of a successor's tableau consistency is an event at `(s, L)`. Its error class is the error's own, and the trace ends at `s`. Probe `LvEvalErrorLater`: complete counts 4 / 3 / 0, trace 0, 1, 2. Within the phase, the first error is the first in TLC's evaluation order:
  - OoS in order;
  - in each OoS, action atoms over the successors, then the state atoms of `s`, then tableau consistency.

  Successors are in `SetOfStates` order under `--lasso=tlc`, and in generation order otherwise. The two can differ only when two distinct liveness predicates fail on the same state; that difference is flagged.
- **Initial-state consistency.** It is evaluated after the fingerprint put and enqueue, and before the invariants (`ModelChecker.java:1176-1190`). An error there is reported as TLC does, wrapped in `GENERAL` (probe `LvEvalError`: counts 1 / 1 / 1, depth 1).

**Finite-prefix violations** must be found at TLC's event, not after the search, because they stop the search and fix its counts (ADR 0017 §5).
- For each OoS whose PEM list is empty, dive keeps **incremental tableau reachability**: one bitset per state over the tableau nodes. The phase-L step of each state in 𝒪 propagates new bits along that state's edges. It also propagates along the stored edges of already-expanded states whose bitsets grew, which is TLC's recursive variant (`LiveCheck.java:877-977`).
- The first state whose step makes an accepting tableau node reachable is the event. It reports 2116 with the prefix, and exits 12.
- This step is sequential per level, after phase 3, in 𝒪. It is pure bit work (the atoms were evaluated in phase 1), so it costs little.

**Storage.** ids are 𝒪 discovery indices, and a level's states are expanded in id order. So the graph is written **append-only, already sorted by source**: the adjacency lists form a CSR with no sort.

| Item | Size |
|---|---|
| Per state: edge offset | 8 bytes (u64; u40 packing possible) |
| Per state: state-atom pattern | 4 bytes: an index into an interned table of distinct bit patterns. In practice patterns are few |
| Per edge: target id | 4 bytes (8 above 2^32 states) |
| Per edge: action-atom pattern | 2–4 bytes (interned, as above) |
| Per edge, only when the concrete successor differs from the state stored for its key (`VIEW`, `SYMMETRY`): the successor's tableau-atom pattern | 4 bytes, in a side table |
| Stutter loop | implicit, one per state. Its action pattern is a per-state field (4 bytes), since `(s, s)` is evaluated once |
| Store id column (`StateSet::with_ids`) | 8 bytes per state |

That is about **24 bytes per state and 6–8 bytes per edge**, for every OoS together, on top of the safety search. TLC's figures are about 16–20 bytes of pointer record plus 21–43 bytes of in-memory table per node, plus 12 bytes and the bits per edge, all **per OoS** and per (state, tableau node) pair.

**Memory bound.** The graph has a budget (`--liveness-memory`, defaulting to a share of the run's memory budget).
- Edge blocks spill to an append-only file, in id order, through the positioned-read spill layer of ADR 0015 §5.
- The searches read it through a page cache. Their working sets (Tarjan index and low-link, BFS parents) are dense `u32`/`u64` arrays per (state × tableau node) of the OoS being searched. Above the budget they spill too.
- A graph that cannot be held or spilled within the limits ends the run as a **resource limit**, naming the budget. It is never an approximate check.

**Worker invariance.** Atom bits are functions of states and pairs. Ids and edge order are fixed by 𝒪. Blocks append in block order. So the graph is byte-identical at any worker count and under chaos scheduling (ADR 0015 §2), and everything after it is a function of the graph.

### 4. Constraints, `VIEW`, symmetry

- **Constraints:** no edge to an out-of-model successor (ADR 0017 §9; probes `LvConstraint`, `LvConstraintHolds`, `LvActionConstraint`).
- **`VIEW`:** nodes are view keys. A node's state atoms come from the state explored for the key; action atoms from the concrete pair; tableau consistency from the concrete successor (side table, §3).
  - **Duplicate edges.** dive reproduces TLC's duplicate-edge rule. An edge to a target already expanded (target id < source id) is dropped when an edge to the same target already exists, keeping the first edge's bits. An edge to a target not yet expanded, or to `s` itself, is always kept. This follows from TLC writing a node only after its transitions (`LiveCheck.java:562-579`). Without `VIEW` or symmetry, two successors with one key are equal states, `SetOfStates` keeps one, and the rule never applies.
- **`SYMMETRY`:** refused by default (§9; ADR 0019 §4). Under `--symmetry-liveness=tlc` it is the same construction as `VIEW`, with canonical keys.

### 5. dive's lasso search (`--lasso=short`)

After the search (`-lncheck final`), or at a periodic check (§7), the graph is searched for each (OoS, PEM) pair. The pairs are independent, read-only tasks and run in parallel. The reported violation is the one from the **smallest (OoS index, PEM index)** that has one. That is the violation TLC's single liveness worker reports, since it processes them in that order and stops at the first.

For one (OoS, PEM):

1. **The product.** Nodes are `(state, tableau node)`. Initial nodes are initial states paired with every consistent initial tableau node, in TLC's order. An edge `(s, n) → (t, m)` exists when `s → t` is a graph edge (or the stutter loop) and `m` is a successor of `n` consistent with `t`'s bits. Edges that fail the PEM's EA conditions are pruned. Without a tableau, the product is the graph.
2. **Accepting components.** First find vertices reachable from the initial nodes in the **unpruned** product. Compute SCCs of the EA-pruned product over all those vertices: the finite prefix may fail EA, which only constrains recurrence. A component is **accepting** if all of these hold (ADR 0017 §4):
   - it is non-trivial, or it is one node with a self-loop that satisfies EA (the stutter loop included);
   - each AE-state condition holds at one of its nodes;
   - each AE-action condition holds on one of its internal edges;
   - each promise is fulfilled by one of its nodes.

   **Implementation evidence, 2026-10-09:** probe `LvEAPrefix` starts at
   `x = 0`, where the EA condition `x > 0` is false, then reaches a violating
   stuttering loop at `x = 1`. The pinned TLC exits 13 with two states.
   Pruning before reachability would miss this violation. TLC's `LiveWorker`
   schedules destinations of EA-failing edges as additional roots;
   `Graph::filter_for_cycles` instead uses every unpruned reachable vertex as
   a root for decomposition. The canonical prefix search remains unpruned.
   Iterative SCC tests cover all directed four-vertex graphs against mutual
   reachability and a 50,000-vertex cycle without recursion. This is an SCC
   foundation. `dive-liveness::search::find_lasso` additionally tests supplied
   acceptance labels, preserves parallel-edge positions, and constructs the
   shortest prefix and deterministic covering walk described below. It is
   independently checked against finite closed-walk search over all two-vertex
   edge/visit labels. Temporal planning, product construction, concrete-state
   projection and property attribution remain outstanding.
3. **The choice. This is dive's documented, deterministic choice.**
   - Let `d(v)` be the breadth-first distance of product node `v` from the initial nodes, ignoring EA pruning, as TLC's prefix search does.
   - The reported component is the accepting component containing the node of smallest `d`. Ties go to the smallest state id, then the smallest tableau index.
   - Call that node the **entry** `e`. The definition does not mention any search order. A parallel SCC algorithm may therefore replace Tarjan without changing the result.
4. **The prefix.** A shortest product path from an initial node to `e`. Breadth-first parents are fixed by "first discovered from the smallest-id frontier node, in edge order", so the path is canonical.
5. **The cycle.**
   - If the stutter loop of `e` alone satisfies every condition, the counterexample ends in **stuttering**.
   - Otherwise, from `e`, repeat: take a shortest path inside the component (EA-pruned edges, canonical parents as above) to the nearest node or edge that discharges an outstanding condition, with ties to the smallest (state id, tableau index, edge position). When nothing is outstanding, take a shortest path back to `e`. The ending is **back to state k**, where `k` is `e`'s position in the printed trace.
   - The cycle is short but not guaranteed minimal: the minimum cycle covering several conditions is a covering-tour problem. The prefix is shortest.
6. **Projection.** Project the product path onto states, as TLC does (states may repeat when the tableau node differs: probe `LvDisjunct`). Collapse consecutive equal states only where TLC does (equal keys on a stutter step).
7. **Properties named.** Each configured property is evaluated on the lasso (a port of `findViolatedProperties` / `evalOnLasso`), and dive names those it violates. This is `dive-liveness`'s own evaluation, and the validator repeats it independently (§8).

**Under `VIEW`** the trace also carries the concrete closing successor, the state that has the same key as state `k`, so that the loop step can be validated (§8).

**What this gives over TLC's choice.** The prefix is shortest. The result does not depend on fingerprints or hash buckets: the same counterexample comes out at any `--fp`, any worker count and in both stores. TLC's changes with `-fp` (ADR 0017 §10). On the probes it is never longer than TLC's:

| Probe | dive (`short`) | TLC |
|---|---|---|
| `LvNoFair` | stuttering at `x = 1`, 2 states | 3 states |
| `LvTwoLoopsRev` | the 1/2 loop, 3 states | 5 |
| `LvLeadsTo` | 3 states | 3 or 4, depending on `-fp` |

These expectations are to be confirmed by WP-L3's tests.

### 6. TLC's lasso, exactly (`--lasso=tlc`, part of `--compat=tlc`)

This mode reproduces TLC's counterexample, its length, ending and back-to index, and the set of named properties. It needs:

1. **TLC's edge order.**
   - Each state's successor set is ordered as TLC's `SetOfStates` iterates it. The bucket is `(fp & 0x7FFFFFFF) % capacity` over TLC's state fingerprint (`StateFingerprinter::tlc` with the run's `-fp`), with equality deduplication, and the stutter state is inserted after the successors.
   - The capacity history is the single worker's: `16 × multiplier`, where `multiplier` grows whenever a set outgrew it (`Worker.java:235-242`). It is a function of successor counts in 𝒪, which dive replays sequentially.
   - Tableau edges follow `TableauLiveChecker`'s nesting: recorded tableau index, then successor in set order, then tableau successor. Edges added by the recursive variant are appended at the point TLC adds them.
2. **TLC's searches, ported step for step:**
   - `checkSccs`: roots in `initNodes` order; successors pushed and popped last-first; EA pruning that turns unvisited targets into new roots;
   - `checkComponent`;
   - `getPath`, the prefix breadth-first search, including its tableau variant;
   - `dfsPostFix` and `bfsPostFix`, the cycle;
   - state rebuilding: first matching initial state, then first matching successor by action order.

It runs sequentially after the search, on the same stored graph (with TLC-ordered edge lists built from the stored ones). It is a conformance mode: its only purpose is to equal TLC. Its cost is TLC's search cost, without TLC's disk graph.

### 7. `-lncheck`

| Mode | dive |
|---|---|
| `final`, `seqfinal` | one check after the search, "complete state space". Statistics are the complete search's |
| `default`, `seqdefault` | periodic checks at **level boundaries**, deterministically: after a level, if the graph has at least 2^16 states and at least twice as many as at the last check. A violation stops the run with that level's statistics and TLC's "current state space" message. A final check follows the search if none fired. The doubling bounds the total cost of periodic checks by about one final check. The schedule is a function of the graph, so the result is the same at any worker count |
| `off` | the graph's atoms are still evaluated (errors and finite-prefix violations are still caught, as in TLC), but no SCC search is run |

**Why level boundaries are sound but a scheduling divergence.** TLC's periodic check is triggered by the wall clock (ADR 0017 §6; probe `LvPeriodic`: three runs stopped at 418,795, 167,402 and 162,021 distinct states). Its single worker suspends between states, so "every state up to level L expanded, none of level L+1" is a point at which TLC can check. dive's result is therefore one of the results TLC can produce. It is reproducible, where TLC's is not.

**Owner decision, 2026-10-09 (D-014).** The default profile uses `--lncheck=deterministic`, the documented level-boundary schedule. `--compat=tlc` uses TLC's wall-clock schedule for `--lncheck=default`; the deterministic schedule is not called precisely TLC-equivalent. The harness passes `--lncheck final`, as goldens do. Wall-clock runs are checked for verdicts without claiming deterministic stop counts.

### 8. Validating lassos (`dive-trace`)

**Implementation, 2026-10-09:** `dive_trace::temporal::evaluate_lasso`
provides the independent truth-vector kernel for pure boolean combinations,
`[]`, `<>`, `~>` and state/action atoms, including the concrete closing edge.
It checks word shape and refuses fairness, `ENABLED`, dynamic constant
bindings, side effects, checker introspection, opaque local/substitution
atoms and unsupported constructors. In particular, `TLCGet("level")` is not
marked impure by the IR, but cannot inherit a search host's level when
evaluating a periodic word. A regression test installs such a host and
requires an explicit refusal. Exhaustive short-word
tests check every suffix against direct reachability semantics; six pinned
TLC liveness words falsify their reported property. This API evaluates a
formula only. It does not certify the behavior, fairness, claimed property
set or a `VIEW`/symmetry closing edge; complete L4 validation remains pending.

The trace format (ADR 0016 amendment 1, §6) already has `lasso` (`back_to`) and `stuttering` endings. It gains:
- `properties`: the claimed violated properties;
- `closing`: the concrete closing successor, under `VIEW`;
- `permutation`: under `--symmetry-liveness=tlc`, the model-value permutation mapping the closing successor to state `k`'s class.

The validator checks, with the core evaluator only (no enumerator, no search):

1. **The behaviour.** ADR 0016 checks 1–5 over every step. This includes the loop step `(s_{n-1}, s_k)`, or `(s_{n-1}, closing)` with `VIEW(closing) = VIEW(s_k)`. For a stuttering ending, the step `(s_{n-1}, s_{n-1})` is a stutter and needs no check. Every state, the last included, must be inside the model. Generated transitions must satisfy the action constraints. Implicit full-state stuttering steps are always permitted and bypass the action constraints, as TLC adds them after successor filtering (`Worker.doNextCheckLiveness`, lines 171–172). Probe `LvStutterActionConstraint` requires `x' # x` but TLC still reports the one-state stuttering violation at `x = FALSE` (exit 13, 3 generated / 2 distinct / depth 2). The validator must preserve that distinction on the prefix and closing edge.
2. **Fairness.** Every temporal conjunct of the behaviour specification (`Model::temporals`) must be **true** on the ultimately periodic behaviour.
3. **The properties.** Every claimed property must be **false** on it. Under `--lasso=tlc`, every unclaimed property must be true, since TLC names all the violated ones.

**Evaluation on a lasso.** Steps 2 and 3 use a direct temporal evaluator over the lasso's positions `0 … n-1`, with `succ(n-1) = k`, or `n-1` for stuttering.
- Each subformula gets a truth vector over positions, computed bottom-up.
- State predicates are evaluated on `s_i`; actions on `(s_i, s_succ(i))`.
- `[]F` at `i` is true when `F` holds at every position reachable from `i`; `<>F`, when at some. `~>`, `WF` and `SF` are expanded by their definitions.

This evaluator shares nothing with the tableau, the DNF or the SCC search. It is an N-version check of the whole liveness pipeline, not only of the trace.

**`ENABLED`.** `WF` and `SF` on a lasso, and properties that mention `ENABLED`, need `ENABLED <<A>>_v` on cycle states.
- **True `ENABLED`** is checked with a **witness**: the checker supplies a successor `t`, and the validator evaluates `<<A>>_v` on `(s, t)` as a predicate.
- **False `ENABLED`** has no witness. It is needed, for example, for a fair stuttering ending (probe `LvStutterEnd`). It needs a successor-existence search. dive-trace gains its own, independent of `dive_eval::next`: `dive_trace::exists_successor`. It is written from TLA+ semantics, without TLC's enumeration order or fast paths, and refuses by name any construct it cannot decide.

  This is ADR 0016 amendment 1's deferred option (b). It also turns deadlock validation from "refused" into "checked". Until it exists, a lasso whose validity depends on a false `ENABLED` is refused by name, `dive_trace::ENABLED_UNSUPPORTED`, and the harness reports `agree-pending-trace-validation`, never `agree`.

**Under `--symmetry-liveness=tlc`.** TLC's quotient lasso need not be a behaviour of the spec (probe `SymLiveSymProp`). The validator checks the steps and the closing permutation, then evaluates fairness and properties on the quotient word. It reports `ValidModuloSymmetry` or `InvalidUnderSymmetry`. dive reports either as TLC would, with a note that the counterexample is TLC's quotient artefact and may not be a real behaviour. That is the honest form of TLC's warning.

**Harness.** As ADR 0016 §3: the harness validates dive's lasso itself, and runs every golden lasso (the two goldens and the probes' `trace.json`) through the validator as positive controls. It also mutates lassos (drop a cycle state, move `back_to`, claim stuttering for a lasso, claim an unviolated property) as negative controls.

### 9. Symmetry with liveness (D-005)

- **`--symmetry-liveness=unreduced`** (default): disable symmetry for the whole run and report it (ADR 0019 §4). Check both safety and liveness on the full graph.
- **`--symmetry-liveness=refuse`**: refused before the search with `DIVE-L0010` (ADR 0019 §4).
- **`--symmetry-liveness=tlc`** (in `--compat=tlc`):
  - warning 2279;
  - canonical keys as nodes, with node atoms from the explored representative;
  - action atoms and tableau consistency from the concrete pair and successor (§3, §4), and the duplicate-edge rule.
  - Exit criteria: probes `SymLiveWarn` (success), `SymLiveMissed` (success, a missed violation) and `SymLiveSymProp` (TLC's false violation, one state then stuttering).
- The owner approved the sound `unreduced` mode on 2026-10-09.

### 10. Sequential mode

ADR 0015 §6's effects analysis covers liveness predicates too: the plan's atoms, including fairness formulas and `ENABLED` arguments. A side effect there (`TLCSet`, randomness, I/O, `TLCGet` counters) forces sequential mode. Atoms are then evaluated inline in TLC's order (§3, phase L). Otherwise nothing changes: phase 1 evaluates atoms in parallel, and the phase-L step and the searches are sequential or independent per task.

### 11. Stores

The graph is keyed by ids, so both stores give the same graph. In fingerprint mode, a collision merges two states into one node, as in TLC. The run's collision bound covers it. Exact mode makes the liveness verdict unconditional, as it does the safety one (charter rule 4).

### 12. Output

- **Human output** follows TLC's messages and order (ADR 0017 §5, §7): 2279, 2212, 2192, 2116, 2264, states, 2122 or 2218, 2259 or 2257, 2267, 2199, 2194.
- **JSON:** `counterexample.ending` (`lasso` with `back_to`, or `stuttering`), `counterexample.properties`, `counterexample.trace` (the validator's document, §8), `options.lasso` and `options.lncheck`.
- Under `--lasso=short`, whenever the golden's choice might differ, the output notes that TLC's counterexample can differ in length and named properties (ADR 0010 §3).

### 13. Performance targets

Against the pinned TLC, measured as in ADR 0006:
- **`ElevatorLivenessMedium` ≤ 0.5 s** at 8 workers (ADR 0006), with the full tableau path.
- **Overhead.** A liveness run is at most 1.5× the time of the same model's safety-only run at one worker, and at most 32 bytes per state plus 8 bytes per edge more memory. TLC's liveness typically multiplies its run time several times. On `ElevatorLivenessMedium` TLC spends most of its time in liveness checking.
- **Scaling.** Atom evaluation scales with the safety search (≥ 3.5× from 1 to 8 workers on ≥ 0.4 M-state specs, ADR 0006). The SCC phase scales across (OoS, PEM) tasks. Within a task it is sequential until a parallel SCC algorithm is justified by measurements; §5's declarative choice permits one.
- **tla-mc** (0.14 s on Elevator) restricts formula shapes. dive does not. The goal is to be within 2× of tla-mc on its supported shapes.

## Consequences

- Liveness reuses the safety engine unchanged except for the observer hooks and one sequential per-level step. Determinism carries over by construction. Chaos-mode tests (ADR 0015) extend to liveness: the graph's bytes and the counterexample must be identical under chaos.
- Formulas are evaluated once per state and per edge, in parallel. The searches are bit operations on one shared graph, not one disk graph per OoS. That is where the performance target comes from.
- `--lasso=tlc` is a substantial port (`SetOfStates` emulation, `LiveWorker`), but it is isolated: nothing else depends on it.
- The validator becomes a second implementation of temporal semantics on lassos, plus an independent successor-existence search. Both are new trusted code, and the positive and negative controls exercise them from the start.
- **Owner questions** (`docs/open-questions.md`):
  - `--lasso=short` as the default-profile counterexample (registry entry D-013, result-changing in length and named properties only);
  - `--symmetry-liveness=unreduced`.
- **Revisit:**
  - after WP-L3's measurements (pattern interning, edge widths, whether a parallel SCC algorithm is needed);
  - when simulation-mode liveness is designed (TLC checks each simulated trace with `LiveCheck1`);
  - if the owner chooses `--lasso=tlc` as the default.

## Owner decisions, 2026-10-09

`--lasso=short` is the intended default. Symmetry with liveness uses `--symmetry-liveness=unreduced` by default: disable reduction for the whole run. Keep `refuse` and `tlc` modes. The periodic schedule amendment above supersedes §7's earlier proposal; TLC's wall-clock path is required for precise compatibility. All options remain planned until implemented and proven.
