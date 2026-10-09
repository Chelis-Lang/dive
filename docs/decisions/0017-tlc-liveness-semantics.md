# 0017: TLC's liveness semantics, as dive must reproduce them

- **Status:** proposed
- **Date:** 2026-10-09

## Context

The charter puts "temporal properties and liveness with fairness" in scope. Today `dive-model` binds `PROPERTY` temporals (`Model::implied_temporals`) and the behaviour specification's fairness (`Model::temporals`), and `dive check` refuses any model with an implied temporal.

The refusal costs 53 small-tier Examples models: 51 whose TLC golden is a success or a safety violation, and 2 liveness violations, `FiniteMonotonic/MCDistributedReplicatedLog` (a 6-state lasso back to state 2) and `SpecifyingSystems/RealTime/MCRealTimeHourClock` (3 states, then stuttering). `SDP_Attack_Spec` is also refused for `Bitwise!Xor`.

TLC's liveness checker has had more correctness fixes than any other part of it since 1.7 (`docs/survey/oracle-history.md` §5). Each changed a verdict on some spec:
- `4260e4712`: temporal negations were not normalized, giving spurious counterexamples (`[]~~[]P`);
- `35d40c91a`: an evaluation error in liveness on an initial state let TLC report success;
- `01cac36a8`: a bogus safety counterexample, because the possible-error model was ignored;
- `b04167104`: a behaviour-graph safety counterexample printed as a lasso;
- `c61bad9a0`: the done state of a fingerprint was overwritten;
- `773e75e48` and `1b8546882`: bogus violations of `[](FALSE ~> P)` and `<>TRUE`;
- tlaplus#1445 (fix not in the pin): a multi-worker race in value normalization that can produce bogus liveness counterexamples.

This ADR records what the pinned TLC does, from its source and from probes. It is the specification `dive-liveness` must meet under `--compat=tlc`. ADR 0018 is dive's design, and ADR 0019 covers symmetry.

**Sources.**
- TLC paths are relative to `tlatools/org.lamport.tlatools/src/` at tlaplus `94d0c50`. Abbreviations:
  - **LV** `tlc2/tool/liveness/Liveness.java`;
  - **LC** `tlc2/tool/liveness/LiveCheck.java`;
  - **LW** `tlc2/tool/liveness/LiveWorker.java`;
  - **SP** `tlc2/tool/impl/SpecProcessor.java`;
  - **MC** `tlc2/tool/ModelChecker.java`;
  - **W** `tlc2/tool/Worker.java`;
  - **AC** `tlc2/tool/AbstractChecker.java`.
- Probes are in `crates/dive-liveness/tests/fixtures/tlc-probes/`, captured with `dive-conformance dev capture` exactly as goldens are (`-workers 1 -fp 0 -seed 0 -lncheck final`). Their README tabulates every result.

## TLC's behaviour

### 1. Which parts of a property are liveness

**`PROPERTY`** (`processConfigProps`, SP:1393-1590). TLC recurses through `LET`, instance substitution, zero-arity definitions, parameterized instance definitions and `\A` over constant sets, then splits conjunctions:

| Conjunct | Becomes | Checked |
|---|---|---|
| `[][A]_v` | implied action | on every generated in-model or out-of-model transition, during the search (W:465-469) |
| `[]P`, `P` at most state level | invariant | on first occurrence of each state, during the search |
| state level | implied initial predicate | on initial states |
| any other temporal conjunct | **implied temporal** | by the liveness checker |
| an action without `[]` | configuration error | at start-up |

So a `PROPERTY` that mixes safety and liveness is checked in two places. Its safety conjuncts behave exactly as in M5 and stop the search at their first violation, before any liveness check. Probes:
- `LvMixedState` (`[](x < 2) /\ <>(x = 9)`): `Invariant line 11, col 9 to line 11, col 17 of module LvMixedState is violated.` (EC 2110), exit 12, with a 3-state trace. The liveness part is never checked.
- `LvMixedAction` (`[][x' > x]_x /\ []<>(x = 0)`): `Action property ... is violated.` (EC 2112), exit 13.
- `LvMixedLiveOnly`: only the `<>` conjunct fails. TLC names the configured property, `Temporal property Prop was violated.`

The name kept for each implied temporal is the cfg's `PROPERTY` name (SP:1560, 1580), which is what the violation message prints (`6b13ef4e9`, `2ee132cd7`). dive's binder already makes this split (`dive-model`, `spec.rs`); liveness only consumes `implied_temporals`.

A property that cannot be split goes to the liveness checker whole. Probe `LvDisjunct`, `[](x < 2) \/ <>(x = 9)`: TLC reports a liveness violation (2116, exit 13) with a 5-state lasso.

**`SPECIFICATION`** (`processConfigSpec`, SP:1167-1385):
- `[][N]_v` is the next-state relation.
- `[]S` with `S` state- or constant-level fails with `TLC_LIVE_CANNOT_HANDLE_FORMULA`.
- Every other temporal conjunct, `WF`, `SF` and any temporal formula, is a "temporal" of the spec (`Model::temporals`). These are the fairness assumptions.

**When liveness runs at all.** Only when there is at least one implied temporal (`AC:97`; `Spec.java:358-360`). Fairness with no temporal `PROPERTY` is ignored entirely.

### 2. From formulas to orders of solution

`parseLiveness` (LV:576-620) builds `F /\ ~(P₁ /\ … /\ Pₙ)`, where `F` is the conjunction of the spec's temporals. Concretely it is `F /\ (~P₁ \/ … \/ ~Pₙ)`, or just the negation when there is no fairness.

`astToLive` (LV:49-570) translates the formula:
- constant-level formulas are evaluated (`LNBool`);
- state-level formulas become state predicates; action-level formulas become action predicates;
- non-recursive user operators are expanded; recursive ones only at temporal level (`4ba7d8811`);
- `\E` and `\A` over enumerable sets are unrolled into disjunctions and conjunctions. Empty `\E` is `FALSE`; empty `\A` is `TRUE`;
- `IF` with a constant guard takes one branch, otherwise `(g /\ a) \/ (~g /\ b)`. `CASE` at temporal level is not handled;
- `=>` becomes `~a \/ b`;
- `<<A>>_e` is an action predicate that is false on steps leaving `e` unchanged (`LNAction.java:66-85`).

The temporal operators expand as follows (LV:498-541):

| Formula | Expansion |
|---|---|
| `WF_e(A)` | `[]<>(~ENABLED <<A>>_e \/ <<A>>_e)`. One `[]<>` over a mixed state/action disjunction |
| `SF_e(A)` | `<>[]~ENABLED <<A>>_e \/ []<><<A>>_e` |
| `F ~> G` | `[](~F \/ <>G)` |

`ENABLED <<A>>_e` is evaluated with the successor enumerator on the current state only (`LNStateEnabled.java:44-58`). It ignores `CONSTRAINT` and `ACTION_CONSTRAINT`: an action whose successors are all outside the model is still enabled.

`processLiveness` (LV:668-934) then does five things.

1. **Normalize and expand.** It pushes negations inward (`pushNeg`, the `4260e4712` fix), simplifies, and takes the disjunctive normal form (DNF).
   - `LNConj.toDNF` takes the cross product, with the first disjunct varying slowest, and fails with `TLC_LIVE_CANNOT_HANDLE_FORMULA` on overflow.
   - `LNDisj.toDNF` merges every `[]<>X` disjunct into a single `[]<>(X₁ \/ X₂ …)` at the end.
   - So `k` strong-fairness conjuncts give up to `2^k` disjuncts.
2. **Tautology.** If the DNF is `FALSE`, there is nothing to check (see §8).
3. **Classify each conjunct of each disjunct** (`classifyExpr`, LV:975-1012):

   | Conjunct | Class |
   |---|---|
   | `<>[]x`, x not temporal | **EA** ("eventually always", stored in the action bin) |
   | `[]<>x`, x at most state level | **AE-state** |
   | `[]<>x`, x action level | **AE-action** |
   | anything else that mentions an action | `TLC_LIVE_WRONG_FORMULA_FORMAT` |
   | anything else | the tableau formula `tf` |

   A weak-fairness conjunct contributes one AE-action. Each strong-fairness branch contributes one EA (`~ENABLED`) or one AE-action.
4. **Group by tableau formula.** Disjuncts with syntactically equal `tf` (including "none") form one **order of solution** (OoS). Each OoS has an optional tableau, `TBGraph(tf)`, with its promises (the `<>r` subformulas), and a list of **possible-error models** (PEMs), one per disjunct. Each PEM is a set of indices into the OoS's shared, deduplicated predicate bins (`checkState`, `checkAction`; LV:895-965).
5. **Report** `TLC_LIVE_IMPLIED` (2212), "Implied-temporal checking--satisfiability problem has N branches.", where N is the number of OoS. Every probe with one property prints 1; `LvTwoProps` (`PROPERTIES P1 P2`) prints 2.

A tableau exists only when some conjunct is not a top-level `<>[]` or `[]<>`:

| Property | Its negation | Handled as |
|---|---|---|
| `<>[]P` | `[]<>~P` | AE-state, no tableau |
| `[]<>P` | `<>[]~P` | EA, no tableau |
| `<>P` | `[]~P` | tableau, no promise |
| `P ~> Q` | `<>(P /\ []~Q)` | tableau, one promise |
| `[](P => []Q)` | `<>(P /\ <>~Q)` | tableau, two promises |

**Tableau construction** (`TBGraph`, `TBPar`, `TBGraphNode`):
- Node indices are assigned in creation order, and the initial nodes come first.
- A node's state predicates include its constant-level parts. This is what makes `<>TRUE` and `[](FALSE ~> P)` correct (`773e75e48`).
- A node is *accepting* when its particle is empty and its only successor is itself.
- A node *fulfils* the promise `<>r` when the promise is not in its particle, or `r` is.

### 3. The behaviour graph, built during the search

**Initial states** (MC:1176-1190). Each new in-model initial state is added after the fingerprint put and the enqueue, and before its invariants are checked.
- Without a tableau, it is added as an initial node.
- With one, it is paired with every initial tableau node consistent with it, in index order.

**Each expanded state `s`** (W:92-158, 168-232; LC:95-140, 545-793). In order:
1. Its successors are generated and checked as in M5 (ADR 0015).
   - Each in-model successor (inside `CONSTRAINT` and `ACTION_CONSTRAINT`), seen or new, goes into a `SetOfStates`. That set deduplicates by state **equality**, keys on the fingerprint, and iterates in hash-bucket order (`SetOfStates.java:137-165`, `:273-283`).
   - **Successors outside the model produce no edge** and never become nodes.
2. The deadlock check (W:125).
3. **`s` itself is added to the set: the implicit stuttering self-loop** (W:171-172).
4. Every OoS evaluates its action predicates on every pair `(s, t)` in the set and its state predicates on `s`. It does this outside any lock (LC:121-128).
5. Under a per-OoS lock:
   - **Without a tableau,** node `s` stores its state bits and one edge per successor, each carrying that pair's action bits. Edges come in set order.
   - **With a tableau,** for each recorded pair `(s, n)`, each successor `t` and each tableau successor `m` of `n` consistent with `t`, an edge `(s, n) → (t, m)` is stored.
     - When `(t, m)` is new and `t` has already been expanded, LC:877-977 expands `(t, m)` immediately. It regenerates `t`'s successors per action, filtered by the constraints.
     - That recursion is also where a **finite-prefix violation** is detected: an accepting tableau node reached by an OoS whose PEMs are empty. TLC reports it during the search (§5).
6. An evaluation error while evaluating these predicates is reported through the usual error path (W:187-232, MC:629-683), with the error's own class and a trace ending at `s`.

Edges into the same target are deduplicated only once the target has been written to disk (`transExists`), and the deduplication ignores the bits. Under `VIEW` or `SYMMETRY`, two concrete successors with one fingerprint can therefore leave two edges with different bits, or one edge with the first one's bits.

**Storage** (per OoS, under the metadirectory):
- **Pointer records,** 16 or 20 bytes: fingerprint, tableau index, file pointer.
- **Node records,** in `nodes_<n>`: 12 bytes per edge (fingerprint and tableau index), then the state bits and each edge's action bits. State bits are duplicated for every `(state, tableau node)` pair.
- **In memory,** a `NodePtrTable` of about 21–43 bytes per node, or a `TableauNodePtrTable`.

**Nodes are fingerprints.** Under `VIEW` they are view fingerprints. Under `SYMMETRY` they are canonical fingerprints (ADR 0019). Check bits always come from concrete states:
- state bits from the state explored for the class;
- action bits and tableau consistency from the concrete successor.

### 4. The search for a violation (`LiveWorker`)

**Threads.** With `-workers 1` there is exactly one liveness worker, so the OoS are processed in order (LC:278-313). With more workers there are `min(#OoS, workers)`, and the first to set a static "error found" flag wins (LW:57-117). That is a race.

**Per OoS, per PEM in order, a full `checkSccs`** (LW:151-476): an iterative Tarjan search.
- **Roots** are the initial nodes in initial-state order, then tableau index. Nodes never expanded are skipped.
- **EA conditions prune edges.** An edge that fails a PEM's EA bits is not followed. Its unvisited target is queued as a new root (LW:428-445).
- **Successors are visited last edge first** (they are pushed, then popped).
- When an SCC root closes, `checkComponent` (LW:532-768) runs:
  1. A **trivial SCC** is skipped: one node without a self-loop satisfying EA.
  2. Otherwise each AE-state condition must hold at some node of the SCC, and each AE-action condition must hold on some edge inside it that also satisfies EA. Every promise must be fulfilled by some node's tableau particle.
  3. If all three hold, that SCC is the violation and the search stops.

So TLC reports **the first accepting SCC to close in its Tarjan order**. That order depends on edge order, which is hash-bucket order of fingerprints. It is neither the shortest counterexample nor the first in exploration order. The probes show this:

| Probe | Available counterexamples | TLC reports |
|---|---|---|
| `LvTwoLoops` | lasso through 1–4 (enumerated first) or through 10–11 | 0, 10, 11, back to state 2 |
| `LvTwoLoopsRev` | lasso through 1–2 (enumerated first, shortest) or through 10–13 | 0, 10, 11, 12, 13, back to state 2 |
| `LvNoFair` | stuttering at `x = 1` (2 states) or at `x = 2` (3 states) | 3 states, then stuttering |

### 5. The counterexample

**Prefix** (LW:836-884). `getPath` does a breadth-first search over the graph from the initial nodes, in `initNodes` order and then edge order, to the SCC's root. It ignores EA.
- Without a tableau, it is a shortest path.
- With a tableau, it searches over `(state, tableau node)` pairs, so after projecting to states it need not be shortest (`TableauDiskGraph.java:404-419`).

**Cycle** (LW:909-974, 1019-1281). From the root, a greedy depth-first walk (`dfsPostFix`) discharges every AE-state, AE-action and promise, then a breadth-first search inside the SCC (`bfsPostFix`) returns to the root. States may repeat, and the cycle is not minimal.

**Rebuilding states.** States are rebuilt from fingerprints:
- the first initial state with the fingerprint;
- then, from the previous state, the first successor (actions in order, successors in generation order) with the next fingerprint.

Consecutive equal fingerprints are collapsed, and each step is labelled with the action that produced it.

**Endings:**
- **Stuttering.** When the cycle is the root alone (including an SCC with no conditions to discharge), TLC prints `N: Stuttering` (EC 2218).
  - Unless the possible-error model is empty and the promises are box-free, it then prints a hint: `TLC_CONFIG_NO_FAIRNESS_BUT_LIVE_PROPERTY` (2259) when a `SPECIFICATION` has no fairness, or `TLC_CONFIG_NO_SPEC_BUT_PROPERTY` (2257) for `INIT`/`NEXT` models (`fbc17038b`).
  - Probes `LvNoFair`, `LvActionProp` (2259), `LvStutterEnd` (no hint: the spec has fairness). Golden `MCRealTimeHourClock` (2257).
- **Back to state.** Otherwise TLC prints `N: Back to state: <action>` (EC 2122), where N is the 1-based number of the state the loop returns to.
  - Under `VIEW`, the loop closes on the view, not the state. Probe `LvView`: states `(x=0, y=0)` and `(x=1, y=1)`, back to state 1, although the real successor is `(x=0, y=2)`. TLC's own equality assertion is disabled under `VIEW` (LW:1008-1011).
  - The lasso is the product path projected onto states, so its prefix can repeat states. Probe `LvDisjunct` prints 0, 1, 2, 0, 1 and then "back to state 3".

**Which properties are named.** `findViolatedProperties` evaluates each configured property on the lasso (`evalOnLasso`, LV:1078-1118). It names every property the lasso violates: probe `LvTwoProps` prints `Temporal properties P1 and P2 were violated.` (EC 2116). Then it prints `TLC_COUNTER_EXAMPLE` (2264) and the states (2217).

**Finite-prefix violations** (LC:795-859). When the tableau reaches an accepting node with no PEM to discharge (a pure safety formula checked by the liveness checker), TLC prints 2116, 2264 and the prefix, with no ending line. It stops the search as a safety violation, so the **exit code is 12**, not 13 (upstream test `Github710aTest`).

### 6. When the check runs: `-lncheck`

- `-lncheck` takes `default`, `final`, `seqfinal`, `off` and `seq*` (TLC.java:672-683).
- **`default`: periodic checks during the search.** The main thread waits up to 3 s, then calls `doPeriodicWork` every progress interval (60 s by default; AC:446-492). A check runs when some OoS graph has grown by more than 10% since the last check, and liveness has used less than 20% of the run time so far (MC:692-740; LC:173-202). A periodic check covers only expanded nodes. A violation it finds stops the run, with the counts of that moment, and its message says "for the **current** state space".
- **`final`:** no periodic check. One check after the search completes, over "the **complete** state space".
- **`off`:** the graph is still built (and finite-prefix violations still caught), but no SCC search is run.
- **`seq*`:** one liveness worker.

**Probe `LvPeriodic`** shows the difference. It is an N × N grid (N = 1500, 2,253,003 states) with a two-state trap loop reachable at depth 2, violating `[]<>(a = N)`.

| Run | Check | Counts at the violation (generated / distinct / queue / depth) | Trace |
|---|---|---|---|
| `-lncheck final` | complete | 4,503,005 / 2,253,003 / 0 / 3001 | 3 states, back to 2 |
| `-lncheck default`, run 1 | current, 417,880 nodes | 835,760 / 418,795 / 915 / 915 | the same |
| `-lncheck default`, run 2 | current, 166,823 nodes | 333,646 / 167,402 / 579 / 579 | the same |
| `-lncheck default`, run 3 | current, 161,452 nodes | 322,904 / 162,021 / 569 / 569 | the same |

The verdict is the same, but under `default` the statistics depend on the wall clock. Goldens therefore use `-lncheck final`, as the upstream Examples CI does. On a model that finishes in under 3 s, `default` and `final` give the same output.

### 7. Statistics and messages

| Message | When |
|---|---|
| 2279 `TLC_FEATURE_UNSUPPORTED_LIVENESS_SYMMETRY` (warning) | at start-up, with `SYMMETRY` and an implied temporal (ADR 0019) |
| 2212 `TLC_LIVE_IMPLIED` | at start-up: N branches = number of OoS |
| 2192 `TLC_CHECKING_TEMPORAL_PROPS` | per check: "Checking [k branches of] temporal properties for the {complete\|current} state space with N total distinct states". N is the sum over OoS of graph sizes: distinct in-model states times OoS for a final check without a tableau (probe `LvTwoProps`: 6 for 3 states and 2 OoS) |
| 2116, 2264, 2217, then 2122 or 2218 | a violation |
| 2267 `TLC_FINISHED_TEMPORAL_PROPS` | after each check, after a violation's trace too |
| 2199 `TLC_STATS`, 2194 `TLC_SEARCH_DEPTH` | after a violation as after a success. With `-lncheck final` these are the counts of the complete search |
| 2268 (outdegree) | on success only |

At a liveness violation found by the final check, the counts are those of the complete search: the golden `MCDistributedReplicatedLog` reports 271 generated, 37 distinct, 0 left, depth 4.

### 8. Errors and exit codes

| Situation | Message (EC) | Exit |
|---|---|---|
| Liveness violation | `TLC_TEMPORAL_PROPERTY_VIOLATED` 2116 | 13 |
| Finite-prefix violation found by the liveness checker | 2116 | **12** |
| `[]P` or `[][A]_v` part of a `PROPERTY` | 2110 or 2112 (§1) | 12 or 13 |
| DNF of `F /\ ~P` is `FALSE` | `TLC_LIVE_FORMULA_TAUTOLOGY` 2253, before initial states | 77 |
| `\A x \in {}` in a `PROPERTY` | 2253, at start-up (SP:1507-1509) | 77 |
| Formula TLC cannot translate, DNF overflow | `TLC_LIVE_CANNOT_HANDLE_FORMULA` 2213 | 255 |
| Action mixed into a temporal formula in an unsupported shape | `TLC_LIVE_WRONG_FORMULA_FORMAT` 2214 | 255 |
| Non-Boolean state or action predicate | 2250 / 2252 | 255 |
| Evaluation error in a liveness predicate on an initial state | wrapped in `GENERAL` (1000) | 255 |
| Evaluation error in a liveness predicate during the search | the error's class, nested-expression trace (2103), then the behaviour up to `s` | 255 |
| Out of memory during the liveness check | `SYSTEM_OUT_OF_MEMORY_LIVENESS` | 1 (`System.exit`) |

Probes:
- `LvEvalError`: `GENERAL` wrapping "The second argument of \div is 0.", with statistics 1 / 1 / 1 and depth 1.
- `LvEvalErrorLater`: `TLC_MODULE_DIVISION_BY_ZERO` (2179), with the trace 0, 1, 2 ending at the state whose predicates failed, and complete statistics 4 / 3 / 0, depth 3. That error came after the state's successors had been generated.

`[]<>TRUE` (probe `LvTautology`) is **not** reported as a tautology. Its negation's DNF is not syntactically `FALSE`: it becomes an EA condition that no edge satisfies, and the check reports success.

### 9. Constraints, VIEW and symmetry

- **Constraints cut the graph.** Successors outside `CONSTRAINT` or `ACTION_CONSTRAINT` are not edges.
  - A state whose successors are all outside the model has only its stuttering self-loop. That loop is a counterexample only if fairness allows stuttering there.
  - Probe `LvConstraint`: `x' = x + 1` under `WF_x(Next)`, `CONSTRAINT x < 3`, `PROPERTY <>(x = 5)`. **TLC reports success.** No state in the model has `x = 5`, but no infinite behaviour of the model violates the property either: the only loop at `x = 2` is stuttering, and fairness forbids it because `Next` is enabled there (`ENABLED` ignores constraints). Behaviours that leave the model are not considered. (Here the answer happens to be right for the unconstrained spec.)
  - Probe `LvActionConstraint`: the constrained graph's only loop is a counterexample, and TLC reports it, although the unconstrained spec satisfies the property.

  Liveness under constraints is therefore checked on the constrained graph, with `ENABLED` from the unconstrained spec. dive reproduces this exactly.
- **`VIEW`:** nodes are view classes (§3, §5; probe `LvView`).
- **`SYMMETRY`:** ADR 0019 §4. TLC both misses violations and reports false ones (probes `SymLiveMissed`, `SymLiveSymProp`).

### 10. Determinism

**At one worker, with `-fp` fixed, TLC's liveness output is a deterministic function of the input.**
- 19 liveness-violation probes and both liveness goldens ran 5 times each at `-workers 1 -fp 0 -lncheck final`. Every transcript was identical once timestamps and timings were removed: verdict, named properties, statistics, every counterexample state and label, and the ending.
- There is one source of order in TLC's liveness code: edge order. Edge order is the `SetOfStates` bucket order, a function of the fingerprints, the set's capacity history (the single worker's multiplier), and successor generation order. No `HashMap` is on the decision path (`LinkedHashSet` for attribution, LV:1083).

**It depends on the fingerprint polynomial.** The bucket order is a function of the fingerprint, so a different `-fp` can change which SCC closes first and how the prefix and cycle are built. The same 21 models were run at `-fp 0, 1, 2, 3, 7, 42`, at one worker. Verdicts and statistics never changed. The counterexample changed for 6 of the 21:

| Model | Over the six polynomials |
|---|---|
| `LvTwoLoops` | 3 states (the 10/11 loop) or 5 states (the 1–4 loop) |
| `LvTwoLoopsRev` | 5 states or 3 states |
| `LvLeadsTo` | 3 states (back to 2) or 4 states (back to 3) |
| `LvDisjunct` | 5 states (back to 3) or 6 states (back to 4) |
| `SymLiveMissedNone` | the same length, different states (a, b, c or a, c, b) |
| `MCDistributedReplicatedLog` (golden) | 5 different counterexamples of 4, 5 or 6 states, naming either `InSync` or `AllExtending and InSync` |

So even the **set of violated properties** TLC prints is an artefact of the polynomial. The golden's `InSync` is what `-fp 0` happens to give. The harness always passes `-fp 0`. TLC's default, without `-fp`, is a random polynomial per run (TLC.java:230), so a user's TLC run does not reproduce its own liveness counterexample.

**At more than one worker it is not deterministic.** With `-workers 4`, 3 of the 21 models changed against the 1-worker runs:
- `LvDisjunct`: 5 or 6 states, varying between runs;
- `LvLeadsTo`: 3 or 4 states;
- `MCDistributedReplicatedLog`: a 4-state lasso naming "AllExtending and InSync" instead of the 6-state lasso naming "InSync".

The other 18 were stable in these few runs. ADR 0004 already treats multi-worker TLC runs as invariance probes only.

## Decision

### 1. What dive reproduces

Under `--compat=tlc`, `dive-liveness` reproduces every behaviour in this ADR that is visible in TLC's output or exit code:
- the property split (§1);
- the formula translation, classification and grouping into OoS, including the number of branches printed, the tautology rule and the errors (§2, §8);
- graph construction semantics: stuttering loops, constraints, `ENABLED` ignoring constraints, `VIEW` classes, and the symmetry quotient under `--symmetry-liveness=tlc` (§3, §9);
- the accepting-SCC condition, including EA pruning and the trivial-SCC rule (§4);
- the endings, the property naming, the finite-prefix exit code, the warnings and hints, and the statistics (§5, §7);
- `-lncheck` modes (§6), as ADR 0018 §7 specifies.

Ports follow TLC's semantics, not its structure (charter). ADR 0018 replaces the disk graph and Tarjan stacks with a design that is deterministic at any worker count.

### 2. What conformance compares for liveness

ADR 0004 holds dive to the single-worker golden on every field. For liveness results the harness compares the following.

| Field | Compared? | Why |
|---|---|---|
| Verdict, error class, exit status | yes | deterministic in TLC, defined by §1–§8 |
| Violated properties (the set TLC names) | **only under `--lasso=tlc`** | it is the set of properties TLC's chosen lasso violates, so it changes with the lasso (`MCDistributedReplicatedLog` under different `-fp`). Otherwise dive names the properties its own lasso violates, and validation checks each one (ADR 0018 §8). The set must be non-empty. A golden that names one property only is still compared exactly, since with one property the set has one possible value |
| Distinct, generated, queue, depth (`-lncheck final`) | yes | the complete search's counts, which are independent of the liveness checker |
| Counts at a finite-prefix violation, or at an evaluation error | yes | events of the search in 𝒪 (ADR 0015 §1; ADR 0018 §3) |
| Counterexample validity | always | dive's validator, extended to lassos and fairness (ADR 0018 §8) |
| Counterexample length, ending kind and back-to index | **only under `--lasso=tlc`** | §4 shows TLC's choice is deterministic, but it is an artefact of its hash-bucket edge order and fingerprint polynomial. It is not shortest, nor first in exploration order |

The last row, and the property set, are the substantive decision. Charter rule 2 says "where TLC's own choice of counterexample is nondeterministic, conformance checks validity and length". TLC's liveness choice is deterministic at one worker with `-fp 0`, so in principle the length is comparable. But it is a function of the fingerprint polynomial (§10): change `-fp` and the length, the loop and the named properties change, for the same spec. The only way to match it is to reproduce TLC's exact edge order, which means its fingerprints and `SetOfStates` hashing, and its Tarjan and post-fix walks. dive therefore builds that as a mode, and does not make it the meaning of "correct".
- **`--lasso=tlc`** (part of `--compat=tlc`): dive reproduces TLC's choice. Edge order comes from TLC's fingerprints and `SetOfStates` iteration order, and the SCC search, prefix and cycle are ported step for step (ADR 0018 §6). The harness then compares length, ending kind and the back-to index with the golden, as it does today (`compare.rs` already compares `trace.length` and `trace.ending`).
- **`--lasso=short`** (dive's own, proposed as the default in the default profile; owner question): a deterministic, documented counterexample (ADR 0018 §5). Under it, the harness compares validity and ending existence, not length. A default-profile difference in length or ending kind is an expected divergence under a new registry entry (D-013, proposed), never `agree`.

Until `--lasso=tlc` is implemented, a liveness violation whose verdict, property and counts agree, and whose counterexample validates, is reported as `agree-lasso-unmatched`: a new outcome that the conformance gate does not accept as `agree`. It is never silently counted as agreement.

### 3. Goldens and probes

- Goldens keep `-workers 1 -fp 0 -lncheck final`. A liveness golden's trace length and ending are oracle facts of that exact configuration, and are recorded as such.
- `dev capture` gains `--lncheck`, `--workers` and `--fp`, so probes can record TLC's other modes as oracle facts (they are never goldens).

## Consequences

- dive's liveness checker has a precise target, including TLC's surprising cases:
  - success under constraints (`LvConstraint`);
  - `[]<>TRUE` not a tautology;
  - exit 12 for finite-prefix violations;
  - all violated properties named;
  - `-lncheck default` statistics that depend on the wall clock.
- Matching TLC's counterexample exactly is possible, but costly and tied to TLC's hashing. It is isolated in one mode, `--lasso=tlc`, so the rest of the design is not bent around it.
- The harness needs:
  - the `agree-lasso-unmatched` outcome;
  - lasso validation (ADR 0018 §8);
  - the property-set rule above.
- **Revisit:**
  - when tlaplus#1445's fix lands in a pinned oracle (multi-worker liveness may then become more stable, though it stays a race);
  - if upstream changes `SetOfStates` or `LiveWorker`, which would change `--lasso=tlc`'s target.
