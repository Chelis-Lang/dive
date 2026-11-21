# Simulation design: temporal properties, views, symmetry, workers and scale

Handover design for lifting the remaining `dive simulate` refusals (#491). The
decision is [ADR 0162](../decisions/0162-simulation-liveness-and-scale.md),
accepted 2026-11-20, which supersedes ADR 0116 §4. This document carries the
evidence, the semantics, the complexity analysis, the owner's rulings (§15)
and the phase plan. It changes no code.

The bar: correct TLA+ semantics, at least TLC's simulation capability on every
use case below, and better where TLC is unsound, silent or limited. Soundness
comes first: a sample can refute a property, never prove one.

Contents:

1. [Current state](#1-current-state)
2. [TLC's measured behavior](#2-tlcs-measured-behavior)
3. [Use cases and parity](#3-use-cases-and-parity)
4. [Temporal properties on sampled traces](#4-temporal-properties-on-sampled-traces)
5. [SYMMETRY, VIEW and ALIAS](#5-symmetry-view-and-alias)
6. [Implied properties and constraints](#6-implied-properties-and-constraints)
7. [Workers and determinism](#7-workers-and-determinism)
8. [Random operators in Init and Next](#8-random-operators-in-init-and-next)
9. [Large and unbounded domains](#9-large-and-unbounded-domains)
10. [Report scale and trace output](#10-report-scale-and-trace-output)
11. [Complexity](#11-complexity)
12. [Divergences](#12-divergences)
13. [Phase plan](#13-phase-plan)
14. [Acceptance metrics](#14-acceptance-metrics)
15. [Owner rulings](#15-owner-rulings-2026-11-20)

## 1. Current state

Source at `9ca4c176` (origin/main when this was written).

- **Sampler:** `Native::sample` in `crates/dive-api/src/core_modes.rs`. One
  machine, one thread. Each trace draws from `Streams::simulation_trace(id)`
  (`crates/dive-stdlib/src/random/seeding.rs`), a pure function of seed and
  trace id. Every trace's states are retained in `Report::traces`.
- **Glue:** `simulate_file` in `crates/dive-api/src/simulate.rs` validates
  every retained trace independently, serializes it, and builds the report.
- **Contract:** `crates/dive-sim/src/lib.rs` (`Options`, `Report`, `preflight`).

Refusal sites:

| Site | Refuses |
|---|---|
| `dive-sim/src/lib.rs` `preflight` | temporal `PROPERTY`, implied initial properties, implied action properties with constraints, `SYMMETRY`, `VIEW`, ALIAS whose lookup fails, `POSTCONDITION` |
| `dive-model/src/reach.rs` (unsupported list) | `SYMMETRY <name>` when unreduced symmetry liveness does not apply; this is the message simulation users see, because `preflight` returns `model.unsupported` first |
| `dive-api/src/simulate.rs` | `--workers` other than 1 |
| `dive-core/src/eval.rs` `check_finite_access` | `RandomElement`, `TLCSet` and other effects in finite modes |
| `dive-model/src/randomness.rs` `transformed_randomness` (called from `dive-api/src/trace.rs`) | randomness with `VIEW` or `SYMMETRY` |
| `dive-trace/src/session.rs` (sampled behaviors) | sample validation with temporal `PROPERTY`, `VIEW`, symmetry, ALIAS or `POSTCONDITION` |
| `dive-trace/src/validate.rs` `prepare_periodic` | lasso validation under `VIEW` or `SYMMETRY` without concrete closing evidence |
| `dive-sim::Options` defaults | 1,000,000 retained states over all traces, 1,000,000 initial or successor candidates, 10,000,000 generated states |

The refusal histogram over all 235 configurations of the pinned Examples
corpus, measured with that build, is in
`conformance/reports/2026-11-20-simulate-refusals-before.json` (driver:
`scripts/simulate-refusal-histogram.py`; 10 traces, depth 100, seed 0). Each
configuration reports its first refusal only:

| Result | Configs |
|---|---:|
| sampled | 97 |
| unsupported: temporal properties | 73 |
| unsupported: SYMMETRY | 15 |
| evaluation-error | 14 |
| unsupported: implied initial properties | 11 |
| violation | 9 |
| unsupported: VIEW | 3 |
| CLI error (configuration or I/O grant) | 3 |
| unsupported: other features (CSV 2, IOEnv grant 2, TLCSet, JavaTime, Bits) | 7 |
| unsupported: sample validation with ALIAS | 1 |
| resource-limit (1,000,000 initial states, `Einstein`) | 1 |
| no module | 1 |

110 of 235 refuse. 104 of those are in buckets this design removes
(temporal 73, SYMMETRY 15, implied initial 11, VIEW 3, ALIAS validation 1,
TLCSet 1), and the resource limit is a parity gap covered in §9. Statically,
97 configurations list a `PROPERTY`, 16 a `SYMMETRY`, 10 an `ALIAS`, 6 a
`VIEW`; 113 use at least one of the four.

## 2. TLC's measured behavior

All cases are in `crates/dive-sim/tests/fixtures/temporal/` (`oracle.json`,
`capture.py`, README). The pinned TLC is `tlc-2026.10.06.014338`, one worker
and seed 1 unless stated.

### 2.1 Temporal properties

TLC checks each finished trace with `LiveCheck1`: it builds a small behavior
graph from the trace, adds a stuttering self-loop to every node, merges nodes
whose fingerprints are equal, and searches it with the tableau of the negated
property, fairness included. Observed:

| Case | Spec, property, depth | TLC | Truth | Assessment |
|---|---|---|---|---|
| C1 to C3 | cycle `0,1,2`, WF, `<>(x=5)`, `[]<>(x=5)`, `(x=1)~>(x=5)`, depth 10 | 2116, lasso back to state 1 or 2, exit 13 | violated | sound |
| C4 | same, `[]<>(x=0)` | no error | holds | sound |
| C6 | same, `[](x<2)` | 2110 "Invariant ... is violated", exit 12 | violated | sound: `[]P` runs as an invariant |
| C7 | `<>(x=5)`, depth 2 (loop never closes) | no error, exit 0 | violated | **silent**: success with liveness unchecked |
| C8 | `[]<>(x=0)`, depth 2 | no error | holds | the truncated end is not treated as stuttering |
| C9 | no fairness, `[]<>(x=0)` | 2116, stuttering after 3 states | violated | sound |
| E1 | `0` then the loop `1,2`, WF, `<>[](x>0)` | 2116, lasso back to state 2 | holds | **unsound**: BFS (B-E1) finds no violation (D-043) |
| T2, T3 | counter to 2 then no successor, `-deadlock` | 2116, stuttering | violated | sound: no successor means stuttering |
| T4 | same, deadlock checked | 2114 deadlock, exit 11 | | deadlock decides first |
| T5 | same, `-deadlock`, `<>[](x=2)` | 2116, stuttering after `x = 2`, exit 13 | holds | **unsound**: the terminal behavior `0, 1, 2, 2, ...` satisfies the property, and BFS (B-T5) finds no violation (D-043) |
| U1 to U3, U9 | `x' = x + 1`, WF, `<>`, `[]<>`, `~>`, depth 5 or 20 | no error | holds | **silent**: no trace can decide |
| U4 | `[](x<4)`, depth 5 | 2110 at `x = 4` | violated | sound bad prefix |
| U5 | `[](x<10)`, depth 5 | no error | violated | incomplete (beyond depth) |
| U7 | `[][x' = 2 * x]_x` | 2112, exit 13 | violated | sound bad prefix |
| U8 | no fairness, `<>(x=10)`, depth 5 | 2116, stuttering after 6 states | violated | sound |
| F1 to F3 | WF or SF on `Set`, `<>b` | stuttering violation without fairness; none with WF or SF | | sound |
| S1, S2 | `Set` enabled only when `x = 0`, WF vs SF | WF: lasso violation; SF: none | | sound |
| U10 | WF, `CONSTRAINT x < 4`, `<>(x=10)` | 2114 deadlock at `x = 3` | | simulation reports a constraint cut as deadlock (BFS does not) |
| U10d | same with `-deadlock` | no error | | the constraint-cut stutter is unfair: `ENABLED` sees the removed successor |

Conclusions:

- TLC's liveness on a sample is sound when fingerprints are concrete: every
  lasso it finds is a path in the trace graph, and a stutter self-loop is a
  real TLA+ step. The exception found in S4 is `<>[]P` when `P` is false
  before the part of the trace that repeats, a loop (E1) or a final state
  with no successor (T5): TLC reports a violation where `P` holds from some
  state on.
- TLC never says that a property went unchecked. A depth that never closes a
  loop (C7, U1) reports the same success as a run that checked thousands of
  lassos (C4). That is the main weakness to beat.
- `[]P` and `[][A]_v` conjuncts of a `PROPERTY` are checked per state and per
  step as invariants and action properties, so their bad prefixes are found
  on truncated traces (C6, U4, U7).

### 2.2 VIEW, SYMMETRY and ALIAS

| Case | Configuration | TLC | Truth | Assessment |
|---|---|---|---|---|
| V1 | `VIEW x`, `y` counts to 3, WF, `<>(y = 3)` | 2116, lasso of 2 states back to state 1 | holds | **unsound**: closes the loop on the view; the reported lasso is not a behavior. BFS does the same (B-V1). |
| V3, V4 | same `VIEW`, invariants | correct violation / success | | VIEW does not affect simulation safety |
| Y2 | `SYMMETRY` over `{a, b}`, `x` alternates, WF, `<>(x \notin {a, b})` | no error | violated | **missed**: `x = b` has the fingerprint of `x = a`, so the a-b cycle becomes a self-loop that looks like stuttering, which WF rejects |
| Y3 | same without `SYMMETRY` | 2116, lasso | violated | control |
| B-Y2 | Y2 under BFS | 2116 with a stuttering counterexample after `x = a` | violated | right verdict, but the counterexample is unfair under WF, so it is not a behavior |
| Y4 | `SYMMETRY`, invariant | correct violation | | SYMMETRY does not affect simulation safety |
| A1, A3 | `ALIAS` | the error trace prints the alias record | | rendering only |

### 2.3 Implied properties and constraints

- U12: `PROPERTY x = 1` (an implied initial predicate). `-simulate` never
  checks it and exits 0; BFS reports 2108 (B-U12). TLC simulation is
  **incomplete** here.
- U13 and U14: TLC checks an action property (2112) and an invariant (2110) on
  a candidate that a `CONSTRAINT` then excludes, in both simulation and BFS
  (B-U13). dive's sampler already does the same.

### 2.4 Workers and seeds

- `-simulate num=N` with `-workers W` runs `N` traces **per worker**: C11 at
  four workers and `num=50` generated 2,001 states, 200 traces of depth 10.
  TLC's help text says `num` is the total.
- W1 and W4 (`Walk`, seed 7, depth 20, invariant `x # 14`): at one worker the
  counterexample was the same in three runs; at four workers it was the same
  in six runs but different from the one-worker trace. Generated counts
  differed on every run in both settings (one worker: 11,758, 8,737, 9,229).
  Which worker reports first is decided by wall-clock time, so the result
  depends on the worker count and in general on timing.

### 2.5 Large and unbounded domains

| Case | Spec | TLC `-simulate` | TLC `-generate` |
|---|---|---|---|
| G1, G2 | `x \in 0..999999` in Init and Next | 101,000,000 generated (all successors each step), 4.9 s | 1,000,100 generated (Init enumerated, one successor per step) |
| G3, G4 | `x \in Nat` in Init | GENERAL 1000 "not enumerable", exit 255 | same |
| G5, G6 | `\E n \in Nat : x' = x + n` | GENERAL 1000, exit 255 | same |

Unbounded state spaces with finite branching (`Unbounded`, an unbounded
queue) simulate normally in both tools: only the successors of the current
state are built.

### 2.6 Trace output

- `-simulate file=PREFIX` writes every non-violating trace as a module named
  `PREFIX_<worker>_<n>` with `STATE_k ==` definitions and action comments.
  ALIAS is not applied, and the module header holds the full path, so the file
  does not parse as written (A2). The violating trace is not written (A3).
- The violating trace is printed with ALIAS applied (A1, A3). `-dumpTrace json
  FILE` and `-dumpTrace tla FILE` write it with ALIAS applied; both need the
  CommunityModules jar on the classpath, or TLC fails while parsing its own
  `_TLAPlusCounterExample` module.
- TLC also writes a trace-exploration spec `<Spec>_TTrace_<n>.tla` and `.bin`
  next to the spec on every violation.

## 3. Use cases and parity

Measured on this machine (Apple M-series, macOS) with the same build; quiet
node numbers come later (§14).

### 3.1 Configs too large to check: deep random traces and throughput

| | TLC | dive today | Design |
|---|---|---|---|
| 10,000 traces x depth 100, `WalkFree` | 0.39 s at 1 worker (25,600 traces/s incl. JVM start); 0.32 s at 4 | stops at the 1,000,000 retained-state bound after 2.25 s (4,400 traces/s); report 45 MB | retain only the deciding trace and K samples (§10); validate only those |
| 1,000 traces | | 0.23 s, every trace validated and kept | |
| bosco 10,000 x 100 (#1024) | | 16.8 traces/s, stops at a 16 GiB budget | memory O(d) per worker, not O(N d) |
| workers | per-worker streams, result depends on worker count | refused | deterministic at any worker count (§7) |
| trace count | `num` unbounded by default, per worker | `--traces N` required, total | keep total; add `--traces unlimited` that requires a time limit or interruption and ends `stopped` with full statistics |

Gaps: retained-state bound, per-trace validation cost, no workers, no
unlimited run. Phases S2 and S4.

### 3.2 Specs without a finite state space

Unbounded counters and queues already sample in both tools; nothing
enumerates the state space. The parity gaps are in the branching:

- TLC `-simulate` enumerates every successor of the chosen action; dive's
  enumerating mode does too, but stops at `max_candidates` (1,000,000) and
  `max_generated` (10,000,000) where TLC continues: on G1 dive reports
  `resource-limit` after one trace. Design: select the successor by
  reservoir sampling (one candidate kept, uniform), so memory is O(1) per
  step, and make the generated-state bound a budget, not a default stop (§9).
- Both tools enumerate Init fully, also under `-generate` and `--generate`.
  dive stops at 1,000,000 initial states (Einstein); TLC continues. Design:
  enumerate once per run with no fixed bound under the memory budget, and
  under `--generate` build each trace's initial state constructively with
  `run_action_generation`, which TLC does not do.
- Init or successor sets over `Nat` or `Int`: TLC fails with GENERAL 1000,
  exit 255 (G3 to G6). dive refuses with "unverified native finite
  evaluation-error context". Parity: report the same error class. Better:
  an opt-in option that samples unbounded sets with a stated distribution
  (D-040, §9).

### 3.3 Deep bugs

Both default to depth 100. TLC keeps only the current trace and runs depth
2,000,000 in 1.16 s. dive's retained-state bound spans all traces, so depth
2,000,000 stops at `resource-limit` after 1,000,000 states. Design: the
current trace is O(d) per worker and independent of the retention cap; the
cap applies only to traces kept for the report. Temporal checking on long
traces is bounded by §11's mitigations. `--liveness-extend` (§4.6) extends a
truncated trace only when that can decide a temporal property.

### 3.4 Traces as output

| Need | TLC | dive today | Gap and phase |
|---|---|---|---|
| every sampled trace to files | `file=PREFIX`, one TLA+-like module per trace, no ALIAS, violating trace omitted | all retained traces inside the JSON report only | S5: `--trace-dir DIR` streams one validated document per trace while running |
| violating trace | printed with ALIAS; `-dumpTrace json/tla` | JSON report, ALIAS not applied (a note says so) | S1 applies ALIAS to a separate rendered view; S5 writes TLC's json and tla formats |
| action labels | per state | `"action": null` in sampled documents | S1: record the chosen action's name and location per step |
| trace exploration spec | `_TTrace_` module by default | none | S5: named gap first, then a generated module |
| animation (ALIAS records, SVG) | ALIAS in printed and dumped traces | none | S1 and S5 |
| test generation and replay | traces depend on worker and timing | traces are a pure function of seed and id | better: S5 adds `--trace-ids A..B` to regenerate selected traces without running the others |
| trace validation | none for simulation | every reported violation is validated | kept |

### 3.5 Quick checks while writing a spec

One trace of `Walk` takes 6 ms in dive against 193 ms for TLC (medians of
five, JVM start included). The design keeps startup cheap: temporal planning
(`lower_violation`) runs once per property at startup, workers start lazily,
and the default retention keeps the human report short. A quick
`dive simulate` with a temporal `PROPERTY` reports per-property coverage
(§4.5), so a user learns at once that a depth never closed a loop instead of
seeing a silent success as with TLC.

## 4. Temporal properties on sampled traces

### 4.1 Principle

A sampled trace is evidence only for behaviors it actually contains. Each
trace `s0 .. sn` defines a **trace graph** `G`: its nodes are the distinct
states of the trace under exact value equality, its edges are the sampled
steps, and every node has a stuttering self-loop. Every infinite path of `G`
from `s0` is a behavior of `Init /\ [][Next]_vars`: each edge is a sampled
`Next` step or a stuttering step. A violation is a path of `G` on which every
fairness conjunct of the specification holds and the property is false. That
is TLC's approach (§2.1), made exact (concrete equality, no fingerprint
merging) and made explicit about what it did not check.

### 4.2 Trace classes

Each trace ends in exactly one class:

- **closed**: some sampled step reached a state already in the trace. `G`
  has a cycle through sampled steps. Sampling continues to the depth, as in
  TLC; closure is recorded, not a stop, so safety coverage is unchanged.
- **terminal**: the last state has no admitted successor and deadlock is not
  checked (`-deadlock`, or a constraint cut that is not reported as deadlock).
  The behavior stutters forever at the last state; whether that is fair is
  decided by `ENABLED` on the full specification, as in U10d and in
  `dive check`.
- **truncated**: the depth was reached without either. Its only infinite
  behaviors in `G` are stuttering at some state.
- **violated**: a safety violation, deadlock or error ended the trace.
- **cancelled**: a higher trace id stopped after a lower one decided (§7).

A trace can be both closed and terminal; closed wins in the counts.

### 4.3 What is checked, by property part

The binder already splits each `PROPERTY` as TLC does
(`dive-model/src/spec.rs` `props`): `[]P` with state-level `P` joins
`invariants`, `[][A]_v` joins `implied_actions`, a state-level formula joins
`implied_inits`, and the rest joins `implied_temporals`. For each part:

1. **Implied initial predicates** are checked on every initial state that a
   trace starts from (code 2108), as BFS does and TLC simulation does not (§6).
2. **`[]P` and `[][A]_v`** are checked on every state and step, including
   extension steps, as today (2110, 2112).
3. **Safe temporal conjuncts.** A conjunct of `implied_temporals` in the
   fragment
   `S ::= P | [][A]_v | []S | S /\ S | S \/ S | P => S | \A x \in C : S | \E x \in C : S`
   (`P` state-level, `A` action-level, `C` a constant finite set) is checked by
   formula progression on each step. The residual is a positive Boolean
   combination of `[]` subformulas, kept as a hash-consed DAG; a residual of
   `FALSE` is a bad prefix, reported as violated with the trace prefix as its
   evidence. The residual size is capped (`R_max`, default 4,096 nodes); a
   conjunct that exceeds it is checked on lassos only for that trace and
   counted as unchecked on truncated traces.
4. **Every other conjunct** (anything with `<>`, `~>`, `WF`, `SF`, or
   `ENABLED` under a temporal operator) is checked only on behaviors of `G`.

A refutation from parts 1 to 3 is a finite-prefix violation: the validator
checks it with `Ending::Invariant`, `Ending::Property` or a new
`Ending::BadPrefix(name)` (part 3), all of which already allow, or will allow,
the last state to lie outside a constraint.

### 4.4 Checking behaviors of the trace graph

For each property `p` the run plans `lower_violation(model, p, limits)` once
(`dive-liveness/src/plan.rs`), giving `Fair /\ ~p` as a tableau over state and
action atoms. Per trace:

1. Build `G`: nodes from the loop-detection map (§11.1); edges are the sampled
   steps plus one self-loop per node.
2. Evaluate state atoms once per node and action atoms once per edge.
   Fairness `ENABLED` atoms are evaluated lazily, only on nodes inside a
   non-trivial strongly connected component of `G` or on a node whose
   self-loop is a candidate (§11.4 lists the shortcuts).
   Each worker evaluates atoms on its own Core machine over the trace's
   interned rows, through the native atoms `dive check` uses
   (`core_native_atoms.rs`, audited by `DIVE_NATIVE_ATOM_AUDIT=1`). An atom
   Core does not compile, and every native fallback, goes to `dive-eval`
   (for example `ENABLED` through an `INSTANCE` that substitutes an
   expression for a variable, as `EWD840`'s `TDSpec`). A worker keeps each
   truth by row ids across its traces, since equal interned rows are equal
   values: native truths always, `dive-eval` truths only for atoms without
   side effects, and none under the audit. It also converts each distinct
   row to a value once. Truths and work charges are those of evaluating
   every demand, so results do not depend on the cache or the worker count.
3. Run the existing product search (`dive_liveness::product`,
   `dive_liveness::search`) over `G` x tableau. An accepting lasso is a
   counterexample. The search is the one `dive check` uses; it reports the
   complete set of configured properties false on the chosen word (ADR 0126)
   and a deterministic, short lasso (D-013).
4. The lasso, written as states with `Ending::Lasso { back_to }` or
   `Ending::Stuttering`, goes to `TraceDoc::with_properties` and the
   independent validator (`dive-trace`), which re-checks every step including
   the closing one. The lasso's states may revisit trace states in a
   different order than sampled; every step is a real edge of `G`.

`Formula::evaluate_word` remains the cheap path for a trace whose `G` is a
single lasso (no repeated node except the closing one) or a single stutter
candidate. It is linear in the word and needs no product.

### 4.5 Per-property verdicts on a trace

For property `p` and trace `t`:

- **refuted**: §4.3 or §4.4 found a violation (validated before reporting).
- **checked**: `G` contains at least one fair behavior (an accepting run of
  `Fair` alone exists in `G`) and no violation of `p`. The trace contributed
  real behaviors on which `p` held.
- **unchecked**: `G` contains no fair behavior. Typical for a truncated trace
  in a specification with weak fairness on its next-state action: every
  stutter is unfair and there is no cycle. Reason codes: `truncated`,
  `budget` (§11.4), `residual-cap` (§4.3).

Whether `G` contains a fair behavior is decided by the same search with the
property omitted (`lower` of the fairness conjuncts), sharing atom values; it
is only run when the violation search finds nothing.

### 4.6 Extending truncated traces (`--liveness-extend`)

TLC leaves every truncated trace undecided. dive can continue a trace that
reached the depth while some property is still unchecked on it, using the same
per-trace stream, until it closes, terminates, or has taken `E` more steps
(`--liveness-extend E`; `E = 0` disables; `auto` means `E = depth`). Extension
steps are real steps: every safety obligation is checked on them, and a
violation found there is reported like any other. The trace class becomes
`extended-closed`, `extended-terminal` or stays `truncated`.

Ruling (owner, 2026-11-20): the default is `auto` in the default profile and
`0` under `--compat=tlc`. It is result-changing relative to TLC's `-depth` (it
can find a violation, safety or temporal, that a trace cut at the depth would
not show), registered as D-039; every extra verdict is a validated real
violation that the exhaustive check also reports (ADR 0116 §6), and it never
weakens a result. The report's `options` block records the setting and the
`extended_*` trace classes show what it did.

### 4.7 Order of verdicts within a trace

Within one trace, safety, deadlock and error endings decide at the step where
they occur, before any temporal check of that trace, as in TLC (T4). The
temporal check runs when the trace ends (after any extension). Across traces,
§7 decides. Simulation never reports `holds` for any property: a run without
a violation is `sampled`, and its per-property counts say how much was
checked.

### 4.8 Report fields

Added to `statistics`:

```json
"trace_classes": {"closed": 812, "terminal": 0, "truncated": 187,
                  "extended_closed": 0, "extended_terminal": 0,
                  "violated": 1, "cancelled": 0},
"properties": [
  {"name": "Liveness", "part": "temporal", "refuted_by_trace": null,
   "checked_traces": 812, "unchecked_traces": 187,
   "unchecked_reasons": {"truncated": 187},
   "behaviors_checked": 812},
  {"name": "TypeOK", "part": "invariant", "refuted_by_trace": null,
   "checked_states": 99901}
]
```

`behaviors_checked` counts traces whose `G` held a fair behavior (it is not a
count of distinct lassos). The human summary prints one line per temporal
property, for example
`Liveness: no counterexample on 812 of 999 traces; 187 truncated traces could not decide it (not a proof)`.
The exit code is unchanged: 0 for a sample without violation, as in TLC.

## 5. SYMMETRY, VIEW and ALIAS

**Safety.** Simulation does not deduplicate states, so `SYMMETRY` and `VIEW`
change nothing (ADR 0019 §5; V3, V4, Y4). Both are accepted. The declaration
is still bound and evaluated at startup exactly as `dive check` does, so its
errors and warnings are the same; the report adds a note "SYMMETRY and VIEW
do not affect simulation". Changes: `preflight` drops both refusals; the
symmetry entry in `model.unsupported` (`reach.rs`) carries a kind that
simulation skips; `transformed_randomness` is not consulted for simulation,
because no fingerprint or canonical form is taken; the sampled-behavior
refusal in `dive-trace/src/session.rs` stops listing VIEW, symmetry and ALIAS,
which are not behavior obligations.

**Liveness.** Loops close only on concrete equality (§11.1). The view and the
symmetric canonical form are never used for closure or node identity, so V1's
spurious lasso and Y2's missed violation cannot happen. This differs from TLC
in both directions and is registered as D-037. Under `--compat=tlc` the same
concrete rule applies: TLC's view closures are not behaviors, so they could
not pass validation; the gap is documented.

**Validator.** `prepare_periodic` refuses under VIEW or SYMMETRY because a
checker lasso under those may close on an equivalent state. The validator
already checks the closing step `Next(s_last, s_back)` on concrete states.
Change: `TraceDoc` gains `closure: "concrete"`, which the simulator sets.
With it, `prepare_periodic` admits VIEW and SYMMETRY and validates exactly as
without them; the flag carries no trust, since the closing step is still
checked. Without it the refusal stays, so `dive check` is unaffected. The
notes path in `validate.rs` that skips the loop step under VIEW is not taken
for concrete closures.

**ALIAS.** ALIAS only renders traces. The report keeps the raw states in
`document` (validated) and adds an `alias` array with the ALIAS value per
state, evaluated on the same states; an ALIAS evaluation error is reported as
TLC reports it when printing a trace (the existing lookup-failure refusal is
lifted together with that error report). The note "ALIAS is not applied" goes
away.

## 6. Implied properties and constraints

The refusals at `dive-sim/src/lib.rs` (implied initial properties; implied
action properties with constraints) date from ADR 0020, when the validator
required every state to satisfy the constraints. Since then the validator
accepts a last state outside the constraints for an invariant, implied
initial or implied action ending (`terminal_before_exclusion` in
`dive-trace/src/validate.rs`). The missing contract is now only on the
sampler side:

1. **Implied initial properties.** For each initial state the sampler keeps,
   evaluate `plan.implied_inits` after the invariants and before the
   constraints, in the order `dive check` uses, and report 2108 with a
   one-state trace and `Ending::Property(name)` (the validator's `last == 0`
   path). The compiled `Plan` already carries `implied_inits`; the sampler
   reads only `plan.invariants`, `plan.implied_actions`, `plan.constraints`
   and `plan.action_constraints` today. TLC simulation never checks these (U12), so
   this is D-036; dive checks them in both profiles because skipping a
   configured property is a silent wrong answer, and documents the compat gap.
2. **Implied action properties with constraints.** The sampler already checks
   implied actions on each candidate before the constraints, matching TLC in
   both modes (U13, B-U13). The refusal is removed once a test shows
   `SimConstrainedAction` and U13 validate with the excluded last state.

## 7. Workers and determinism

**Model.** `W` workers share one `AtomicU64` trace-id counter. Each worker
owns a `Native` (machine, heap, memo) and a per-worker evaluator view; the
compiled `Plan` and the store are shared through `Arc` when they are `Sync`,
or compiled once per worker otherwise (S2 measures the startup cost). Trace
`id` uses `Streams::simulation_trace(id)` whatever worker runs it, so its
states, generated count and verdict are a pure function of seed and id.

**Result.** The run reports the violation of the **lowest violating trace
id**, so the report is identical at any worker count, including counts:

- A worker that finds a violation in trace `v` does `best.fetch_min(v)`.
- Workers claim no id above `best`, and poll `best` each step: a trace with
  `id > best` stops and is discarded (`cancelled`).
- Traces with `id < best` run to completion; one of them may lower `best`.
- When all claimed ids below `best` are done, `best` is final.
- Statistics sum per-trace counters over ids `0..=best` only (or over
  `0..N` without a violation), so `generated`, `traces_started` and the
  class counts equal the one-worker run.

This is stronger than ADR 0116 §3 and than TLC, whose reported trace and
counts depend on the worker count and on timing (§2.4). `--traces N` is the
total number of traces at any worker count; TLC's `num` is per worker, which
the compat profile documents as a gap rather than reproducing.

**Output and effects.** `Print` and `PrintT` output is buffered per trace and
released in id order through a reorder buffer: trace `id`'s output is emitted
once every lower id has finished. Output of cancelled traces is dropped.
The output budget (`max_output_events`, `max_output_bytes`) is charged in
commit order, so where output stops is also independent of `W`. The reorder
window is bounded (`4 W` traces); a worker that would exceed it waits, so
buffered output is O(W) traces. `TLCGet("level")` keeps its per-trace meaning.
`TLCSet` registers persist across traces in TLC, which would make a trace
depend on the traces before it on the same worker; they stay a named refusal
in simulation at every worker count (one Examples config, `TLCSet`, is
affected). A later option could reset registers per trace.

**Retention.** The retained-trace cap (§10) is applied in id order at
commit, so the kept sample is the K lowest ids, at any `W`.

**Budgets.** Time and memory budgets are shared and polled per step by every
worker; a stop ends the run `stopped` with the statistics of the committed
prefix of ids, which is the same at any `W` for a given stopping id.

## 8. Random operators in Init and Next

Today `check_finite_access` (`dive-core/src/eval.rs`) refuses `RandomElement`
in finite modes because the machine has no draw stream. #910 tracks the gap;
its recent comments list the finite-simulation refusals, and the last change
to `eval.rs` on main is the documentation pass of #994. This section is a proposal to
coordinate with whoever owns #910, not a change to `eval.rs` in this design.

- **Stream.** Install `machine.draw` for each action evaluation with a stream
  keyed by `(seed, Domain::SimulationDraw, trace id, step, action index)`,
  restarted for each action tried at that step. Draws are then a pure
  function of the trace position and do not depend on how many candidates
  earlier actions produced, which keeps them replayable and independent of
  the action order chosen by the prime stride. The choice stream stays
  `simulation_trace(id)`. This changes D-003's description ("one stream drives
  choices and draws") and is registered there.
- **Admission.** `check_finite_access` admits `TlcRandomElement` (and the
  `Randomization` operators already handled by `crate::random`) when a draw
  stream is installed; everything else stays refused. The existing
  `draw_ambiguous` refusal for lazily represented sets is kept.
- **Validation.** The validator's fresh finite sessions already prove bounded
  `RandomElement` support by rebinding per alternative
  (`dive-trace/src/validate.rs` module docs): each occurrence is checked as a
  nondeterministic choice over its argument set. A sampled step is valid if
  some choice reproduces it. This validates the trace as a behavior in which
  each draw picked some element of its set.
- **Semantics (owner ruling, 2026-11-20).** `TLC.tla` defines
  `RandomElement(S)` as `CHOOSE x \in S : TRUE`; TLC's override draws at
  random. Simulation draws at random, and validation checks each draw as some
  element of its set. Before starting this work, read #910's latest comments
  and the recent `dive-core/src/eval.rs` history, and coordinate with its
  owner.
- **VIEW and SYMMETRY.** With no fingerprints in simulation, randomness under
  VIEW or SYMMETRY needs nothing extra.

## 9. Large and unbounded domains

1. **Reservoir successor choice.** In enumerating mode, keep one candidate per
   step by reservoir sampling (the `i`-th admitted candidate replaces the kept
   one with probability `1/i`, drawn from the trace stream) instead of
   collecting all candidates. Uniform over admitted candidates, like the
   current `next_index(candidates.len())`, with O(1) memory. Draws differ from
   today's, which is not a parity obligation (ADR 0116 §5).
2. **Bounds become budgets.** `max_candidates` and `max_generated` stop being
   default stops; the run's time and memory budgets govern, as in
   `dive check`. An explicit `--max-generated` stays available.
3. **Init.** Enumerated once per run and shared by all workers, with no fixed
   count bound. Under `--generate`, each trace builds its initial state with
   `run_action_generation` over the Init actions, so Init is never enumerated.
4. **Sampling unbounded sets (D-040, on by default).** When Init or a
   successor binds a variable from a set dive cannot enumerate (`Nat`, `Int`,
   an infinite `Seq(S)`), the default profile draws one member per binding
   instead of failing: `Nat` from a geometric distribution with mean 8
   (`--sample-mean`), `Int` as a geometric magnitude with a uniform sign,
   `Seq(S)` with a geometric length and members drawn from `S` recursively.
   Draws come from the per-trace draw stream (§8), so they are seeded and
   replayable. The report's notes name the distribution, its parameters, the
   seed and every set that was sampled (source location and set expression).
   Each drawn value is a member of its set, so each trace is a real behavior
   and validates as one; nothing is claimed about values not drawn. A set
   without a supported distribution (for example `Real` or `SUBSET Nat`) keeps
   the named refusal. `--sample-unbounded=off` turns it off.
5. **Not enumerable under compat.** With `--compat=tlc` (or
   `--sample-unbounded=off`), an Init or successor set over such a domain
   reports TLC's error class (GENERAL 1000, exit 255 under TLC exit codes)
   instead of "unverified native finite evaluation-error context".

## 10. Report scale and trace output

Today every trace is retained, validated and serialized (`simulate_file`),
which costs 7 to 47 ms per trace in validation (release performance D5) and
grows memory with the trace count (#1024). An external caller (Sail #767)
reached a 16 MiB report bound after 160 s.

- **Retention (owner ruling, 2026-11-20: efficient by default).**
  `--keep-traces K` (default 10, `all` allowed): the report keeps the deciding
  trace plus the K lowest-id other traces. Only kept traces are validated; the
  deciding trace is always validated even when it is not emitted.
- **Complete output.** `--keep-traces all` keeps and validates every trace in
  the report; `--trace-dir DIR` (S5) writes every trace to its own file as it
  finishes, with memory independent of the trace count. The user guide
  (`docs/user/simulation.md`) must say, in the phase that adds each flag, how
  to get complete output for trace export, animation (ALIAS), test
  generation and trace validation. Draft text for it:

  > By default the report keeps the trace that decided the result and the
  > first 10 other traces, each validated. To keep every trace, pass
  > `--keep-traces all` (the report grows with the trace count) or
  > `--trace-dir DIR` (one file per trace, written as it finishes). For
  > animation, the files include each state's ALIAS value. For test
  > generation, a trace is fixed by the seed and its number, and
  > `--trace-ids A..B` regenerates only those traces.
- **Byte bound.** `SimulateRequest::max_report_bytes`: sample traces are
  admitted in id order while the serialized report stays under the bound;
  dropped samples are counted in `statistics.traces_dropped`. If the deciding
  trace alone exceeds the bound, it is validated, summarized (length, ending,
  SHA-256 of its document) and written to `--trace-file` if given.
- **Sizes.** `SimulateReport::json_len(convention)` returns the exact length
  `write_json` will write, using a counting writer, so a byte-limited caller
  can decide before writing. For an estimate before running,
  `dive simulate --estimate` runs one trace and reports
  `bytes_per_state` and `estimated_report_bytes = header + (K + 1) x (depth + 1) x bytes_per_state`.
- **Streaming.** `write_json` already writes trace entries without building a
  JSON tree (#1040). S5 adds `--trace-dir DIR`: each finished trace's validated
  document is written as it commits, in id order, so memory stays O(W) traces
  while every trace reaches disk. Formats: `dive-trace` JSON (default), and
  TLC's `-dumpTrace json` and `tla` layouts with ALIAS applied for the
  deciding trace.
- **Action labels.** Sampled documents record each step's action name and
  source location, which the sampler knows (`model.actions[index]`).

## 11. Complexity

Notation: `d` trace length (states), `|s|` state size, `|phi|` formula size,
`T` tableau size of `Fair /\ ~phi` (exponential in `|phi|` in the worst case,
usually O(|phi|)), `A` atoms, `F` fairness conjuncts, `E` cost of one
`ENABLED`, `B` average branching of the chosen action, `W` workers, `N`
traces, `K` kept traces, `R` residual size of a safe conjunct.

### 11.1 Loop detection

A per-trace hash map from a 64-bit state hash to the first index with that
state; a hit is confirmed by exact equality of the interned rows (handle
equality when the store hash-conses, otherwise `equal_values`). Per step:
O(|s|) to hash plus O(1) expected lookup, and O(|s|) on a confirmed or
colliding hit. Per trace: O(d |s|) time, O(d) memory. TLC fingerprints every
state (O(|s|)) and builds the same per-trace map, so the cost matches; the
difference is that a fingerprint hit counts as equality in TLC.

### 11.2 Lasso and trace-graph check

`G` has `V <= d` nodes and `<= 2d` edges. Atom evaluation is
O(V A_s c_s + d A_a c_a) for state and action atoms of cost `c`. Fairness
`ENABLED`: O(F E) per evaluated node, only on the nodes §4.4 names. The
product search is linear in the product: O(d T) per property. Per trace and
property: O(d (A c + T) + V' F E), `V'` the nodes needing `ENABLED`.
`evaluate_word` on a single lasso: O(d |phi|). TLC's `LiveCheck1` has the same
asymptotic shape per trace (tableau times trace graph), so the bound matches
TLC; dive adds the fair-behavior search for coverage, at most doubling the
search, only when no violation was found.

### 11.3 Incremental safety

Invariants and `[]P`: O(1) evaluations per state; `[][A]_v`: O(1) per step;
implied initial predicates: once per initial state. A safe temporal conjunct
by progression: O(R) per step, `R <= R_max`. Today's costs are unchanged;
checking candidates before the constraints is already O(B) per step.

### 11.4 Worst case: long lassos with expensive ENABLED

`d = 10^5`, `F = 30` per-process weak fairness, `ENABLED` as costly as an
action expansion: naive evaluation is 3 x 10^6 `ENABLED` calls per trace.
Mitigations, in order:

1. Only nodes on a cycle of `G` and stutter candidates need fairness atoms;
   prefix nodes never matter for acceptance.
2. A stutter candidate at `s_i` is fair only if every fair action is disabled
   there; evaluation stops at the first enabled one. If the sampler took a
   non-stuttering step of a fair action from `s_i`, or found such a candidate,
   that action is enabled at no cost; `WF_vars(Next)` is always free this way.
3. Memoize `ENABLED` by (state row, action) within the trace.
4. On a cycle, WF needs a disabled node or an occurrence: scan the cycle's
   edges for occurrences first (cheap action atoms), then `ENABLED` only for
   actions with no occurrence, stopping at the first disabled node.
5. A per-trace temporal budget (`--liveness-budget`, default a multiple of the
   trace's own sampling cost) ends the check with `unchecked: budget`; it
   never produces a verdict.

### 11.5 Memory per worker

Current trace O(d |s|) (interned rows), loop map O(d), trace graph and product
O(d T) transient, plus the machine, heap and memo. Independent of `N` once
retention is capped: O(K d |s|) for the report, shared. Today memory is
O(N d |s|) because every trace is retained (about 1.7 MB per bosco trace).

### 11.6 Throughput

Traces are independent, so throughput scales with `W` until memory bandwidth
or the commit order dominates. The lowest-id protocol wastes at most the
in-flight traces above `best` after a violation, `< W` partial traces. The
reorder buffer holds O(W) traces' output. Per trace: O(d B) generation, plus
§11.2 when a temporal property is configured. Removing per-trace validation of
non-kept traces removes the largest measured per-trace cost (§10).

### 11.7 Report size

O(1) summary plus O((K + 1) d |s|) for kept traces, with per-property
counters O(P). `write_json` streams entries with O(1) extra memory;
`json_len` costs one pass over the kept entries.

## 12. Divergences

Registered in `docs/divergences.md` with status `recorded` and described in
`docs/tlc-differences.md`; they become `implemented` with their code. The
numbers start at D-036 because D-031 was already registered for bag fold
order.

| Id | Behavior | TLC | dive default | Kind |
|---|---|---|---|---|
| D-036 | implied initial properties in simulation | never checked by `-simulate` (U12) | checked, 2108, both profiles | changing |
| D-037 | loop closure in simulation liveness | on view or symmetric fingerprints (V1 spurious, Y2 missed) | concrete equality only, both profiles | changing |
| D-038 | temporal coverage of a sample | silent success when nothing could be decided (C7, U1) | per-property checked and unchecked counts; never "holds" | preserving (report fields only) |
| D-039 | `--liveness-extend` | traces end at `-depth` | `auto` (default profile); `0` under `--compat=tlc` | changing, owner-approved default |
| D-040 | `--sample-unbounded` | GENERAL 1000 on `Nat` domains | `geometric`, named in the report (default profile); `off` under `--compat=tlc` | changing, owner-approved default |
| D-003 (update) | workers | per-worker streams; `num` per worker; result depends on worker count | lowest violating trace id; `--traces` total; identical at any `W`; draws keyed by trace position (§8) | changing (existing entry) |

## 13. Phase plan

Each phase is one PR (or a short series) with its tests, the Core small-tier
ratchet on touched models, and simulation conformance evidence. Exhaustive
goldens decide validity (ADR 0116 §6); `crates/dive-sim/tests/fixtures/temporal`
holds the TLC controls.

### S1: SYMMETRY, VIEW, ALIAS and implied properties for safety

- `crates/dive-sim/src/lib.rs` `preflight`: remove the SYMMETRY, VIEW,
  implied-initial and constrained implied-action refusals; keep POSTCONDITION.
- `crates/dive-model/src/reach.rs`: give the symmetry entry an
  `UnsupportedKind` that simulation skips (it stays for `dive check`).
- `crates/dive-api/src/trace.rs` / `dive-model/src/randomness.rs`: do not
  apply `transformed_randomness` to simulation loads.
- `crates/dive-api/src/core_modes.rs` `Native::sample`: check
  `plan.implied_inits` on initial states (2108); record action labels.
- `crates/dive-trace/src/session.rs`: stop refusing sampled behaviors for
  VIEW, symmetry and ALIAS.
- `crates/dive-api/src/simulate.rs`: ALIAS rendering array; notes for
  SYMMETRY/VIEW.
- `crates/dive-trace/src/validate.rs` and `TraceDoc`: `closure: "concrete"`
  admission in `prepare_periodic` (used from S3).
- Tests: `crates/dive-api/tests/simulate_*`: V3, V4, Y4, A1 (alias array),
  U12 (2108 at depth 0), U13 and U14 (validated excluded last state),
  `SimConstrainedAction`; validator unit tests for concrete closure under
  VIEW and SYMMETRY, and that a non-concrete lasso still refuses.
- Parity gaps closed: ALIAS rendering, action labels.
- Docs: `docs/user/simulation.md`, `crates/dive-sim/CONTRACT.md`, D-036 to
  implemented, CHANGELOG.

### S2: workers, retention and scale

- `crates/dive-api/src/simulate.rs`: lift the `--workers` refusal; run W
  sampler threads; commit protocol of §7; `--keep-traces`, `max_report_bytes`,
  `json_len`, `--traces unlimited`.
- `crates/dive-api/src/core_modes.rs`: split `Native::sample` into
  `sample_trace(id) -> TraceOutcome` (pure per trace) and a driver; reservoir
  successor choice; Init enumerated once without the fixed bound; constructive
  Init under `--generate`; not-enumerable error class.
- `crates/dive-sim/src/lib.rs`: `Options` gains `workers`, `keep_traces`,
  `max_report_bytes`; bounds become budgets; `Report` gains per-trace
  counters and `traces_dropped`.
- `crates/dive-sim/src/host.rs`: per-trace output buffers, commit-order output
  budget.
- Tests: identical JSON (minus timing) at 1, 2, 4 and 8 workers on `Walk`, a
  Print spec and a violating Examples model; cancellation of higher ids; output
  order; `BigNext` without resource-limit; depth 2,000,000 on `WalkDeep`-like
  spec within O(d) memory; `--keep-traces 0` still validates the violation.
- Parity gaps closed: §3.1 throughput and retention, §3.2 bounds, §3.3 depth.
- Profile first (CLAUDE.md): name the dominant cost before optimizing
  validation or serialization further.

As built (S2): successor choice draws `next_index(n)` over up to 1,024
admitted candidates and switches to reservoir sampling above that, so
existing seeded samples are unchanged and a step's memory stays bounded.
Output is buffered per trace in the machine (`Machine::take_output`), so
`dive-sim`'s host needed no change. `max_report_bytes` lives on
`SimulateRequest`; a deciding trace that does not fit is summarized in the
report, and writing it to a file is left to S5. Under `--generate`, an
existential domain the constructive generator cannot index (`\E n \in Nat`,
G6) stays a named refusal rather than GENERAL 1000. The `dive tlc` adapter
still passes `num=N` as the total. Constructive Init is opt-in
(`--constructive-init`, D-041) rather than the `--generate` default: by
default TLC's `-generate` enumerates Init and so finds every initial
violation, which constructing one state per trace would miss.

### S3: temporal PROPERTY

- `crates/dive-sim/src/lib.rs` `preflight`: lift the temporal refusal; keep a
  named refusal for whatever `lower_violation` refuses.
- New module `crates/dive-api/src/simulate_temporal.rs`: per-run plans
  (`dive_liveness::plan::lower_violation`, `lower` for fairness alone), the
  per-trace loop map, trace graph, lazy atoms, product search via
  `dive_liveness::product` and `search` (public API only; no change to
  `plan.rs` or `analyze`), `Formula::evaluate_word` fast path, the safe-fragment
  progression monitor, budgets, `--liveness-extend`.
- `crates/dive-api/src/simulate.rs`: lasso documents with
  `TraceDoc::with_properties`, `Ending::Lasso` / `Ending::Stuttering` and
  `closure: "concrete"`; `Ending::BadPrefix`; trace classes and per-property
  report fields; human summary lines.
- `crates/dive-trace`: `Ending::BadPrefix(name)` validation (prefix plus a
  finite check that the residual is false).
- Tests against `oracle.json`: C1 to C9, T1 to T4, U1 to U10d, F1 to F3,
  S1, S2 (verdict class and property names; validated lasso), V1 and Y2 (dive
  answers the truth, D-037), C7 and U1 (unchecked counts, D-038), extension on
  C7 and U1 with `--liveness-extend auto` (D-039), both profiles; budget stop
  yields unchecked, never a verdict.
- Gate: Core small-tier ratchet; ewd687a `EWD687a_anim` and the temporal
  Examples configs that sample.

Status: implemented in `crates/dive-api/src/simulate_temporal.rs`, with these
differences from the plan above. `dive_liveness::product` and `search` are not
public, so the module runs its own product search over the public kernel: the
negation-normal form of `Fair /\ ~p` is split into acceptance clauses
(`[]<>B` and `<>[]B` disjunctions, as `WF` and `SF` lower) and a residual
compiled with `formula::compile`, and a strongly connected part accepts by an
Emerson-Lei check. `Formula::evaluate_word` computes the complete
false-property set of the found word rather than serving as a fast path.
Finite refutation (§4.3 part 3) is a three-valued evaluation of each property
on the trace when it ends, reporting the shortest refuting prefix, instead of
per-step progression; it has no residual cap, so `residual-cap` is not a
reason code. It runs in the default profile only.

### S4: corpus, histogram and performance

- Rerun `scripts/simulate-refusal-histogram.py` and commit the after report.
- Simulation conformance over the Examples configs: every dive violation
  validates and is in the exhaustive golden's verdict class; coverage rate
  where TLC simulate finds a violation within the same traces and depth.
- `dive-bench`: traces/s against TLC on a quiet AWS node at 1, 4 and 8
  workers, with and without temporal properties; startup for one trace.

As built (S4): `scripts/simulate-refusal-histogram.py` over the 235
configurations now refuses 10, against 110 before
(`conformance/reports/2026-11-21-simulate-refusals-after.json`, one
assessment per refused or unsampled configuration). Eleven of the sixteen
refused or unsampled configurations are correct refusals (spec I/O grants,
`TLCSet`, `JavaTime`, Apalache-only configurations). Five are gaps: `\cdot`
replay in the validator (`MCCRDT`, `MCReplicatedLog`), validation cost with a
`RandomElement` constant (`SpanTreeRandom`), and the Core operators
`RandomSubset` and `Bits!And`. `dive-conformance study simulate` runs both
simulators over the corpus, ten seeds of ten traces at depth 100
(`conformance/reports/2026-11-21-simulate-conformance.json`). It found no
invalid trace, no dive class outside the golden's or TLC's, and no
configuration where TLC's simulator finds a violation in 9 of 10 seeds and
dive does not, other than TLC's unsound reports. All 116 deciding traces
validated. `MCtcp`, `MCInnerSequential` and `MCLamportMutex` stop on a
deadlock in every seed in both simulators: a state with no successor inside
the `CONSTRAINT`. TLC's simulator reported a `<>[]` violation on three
models where model checking finds none; the smallest receipts are E1 and T5
(D-043). Of 141 temporal properties on 72 models, 119 were checked on every
trace; 89 % of trace-property pairs were checked with `--liveness-extend`,
88 % without. Throughput is in
`conformance/reports/perf/2026-11-21-simulation-throughput-d04a3a88.json`:
without temporal properties dive is ahead of TLC at every worker count; with
them, the atoms run on the slower `dive-eval` evaluator, and `EWD840` is at
0.48x TLC.

### S5: trace output

- `--trace-dir DIR` streaming per-trace documents; TLC `json` and `tla`
  formats with ALIAS for the deciding trace; `--trace-ids A..B` to regenerate
  selected traces; TTrace-style exploration module (named gap until done).
- User guide: the complete-output section drafted in §10, covering
  `--keep-traces all` (from S2) and `--trace-dir`.

As built (S5): a kept or exported trace is never built as a JSON tree.
`dive_trace::stream` defines a `StateSource` that produces one state at a
time; the writer produces the same bytes as `TraceDoc::to_json`, and the
validator decodes each state into its own interner as it arrives, so the
independence boundary is still the JSON text of each state. On a deciding
trace of two million states (two integers), peak RSS fell from 8.5 GB to
0.95 GB and wall time from 13.3 s to 4.2 s, with byte-identical reports.
Writing 8,000 traces to a directory at 4 workers peaks at 16 MB, as 2,000
does. Export runs on the worker, before the trace's rows are released, and
the committer renames the hidden partial file in id order. The TLC `json`
and `tla` layouts match the pinned oracle's dumps byte for byte under
`--compat=tlc`, except that state fields are in declaration order (TLC's
follow its name table) and the `tla` module is `trace_<id>` (TLC writes the
whole output path, which does not parse); both are registered as D-042.
A temporal lasso or stuttering word (S3) is the one trace validated as a
whole document, since its check needs fairness and the complete
false-property set; a bad prefix streams like an invariant violation.
Known gap: TLC's `<Spec>_TTrace_<n>.tla` exploration module is not written;
`--trace-format tla` gives the `CounterExample` definition it is built on.

### S6: unbounded domains

- `--sample-unbounded=geometric|off` (D-040): default `geometric` in the
  default profile, `off` under `--compat=tlc`; distributions, report notes
  naming every sampled set, validation of drawn members; TLC's GENERAL 1000
  class when off. Depends on S2 (reservoir choice, error class) and on the
  §8 draw stream.
- Tests: `InfInit`, `InfNext` (G3 to G6) in both profiles; the same seed
  reproduces the same draws at 1 and 4 workers; report notes list the sets.

As built (S6): draws come from a separate stream,
`Streams::simulation_draws(trace, step, action, attempt)`, so a spec that
draws nothing samples exactly the traces it sampled before (measured overhead
on a finite spec is within noise). The sampler lives on the Core `Machine`
(`dive_core::unbounded`) and draws only at generator positions: `x' \in S`,
`\E` contexts that assign, and constructive `\E` domains, which also closes
S2's `--generate` refusal for `\E n \in Nat` (G6). Init with a draw is
detected by a probe enumeration at startup and then enumerated per trace,
with reservoir choice. A step that drew and admitted nothing is drawn again
up to 8 times and then ends the trace (`traces_stalled`), never a deadlock.
`\E` draws are recorded as per-state `witnesses` in the trace document; the
validator tries the members among them to prove an `\E` over an infinite
set true and never uses them to make one false. Known gap: an `\E` at the
very top of `Next` over a record or product set with an infinite component
is refused by the action splitter at load time, as in `dive check`.

## 14. Acceptance metrics

1. **Refusal histogram** (driver above, all 235 configs). Before: 110
   unsupported, 104 of them in buckets this design removes. After S3: none in
   the temporal, SYMMETRY, VIEW, implied-property or ALIAS-validation buckets;
   TLCSet stays a named refusal; the remaining feature refusals (CSV, IOEnv
   grants, JavaTime, Bits) belong to other work. After S2: no
   `resource-limit` from fixed candidate or initial-state bounds.
2. **Verdict-class agreement with TLC simulate.** For each Examples config,
   run TLC `-simulate` and dive with the same traces and depth: every dive
   violation validates and names a property the exhaustive golden reports
   (ADR 0116 §6); dive reports a violation on every config where TLC's
   simulation does in at least 9 of 10 seeds, except where TLC's answer is
   one of §2's unsound cases.
3. **Coverage.** Per temporal property, the fraction of traces checked, with
   and without `--liveness-extend`.
4. **Throughput.** traces/s against TLC on a quiet node at 1, 4 and 8 workers
   (S4); result identical across worker counts.

## 15. Owner rulings, 2026-11-20

The owner accepted the design and answered its four questions:

1. **`--liveness-extend` (D-039):** default `auto` in the default profile, `0`
   under `--compat=tlc` (§4.6).
2. **`RandomElement` in sampled traces:** draw at random and validate each draw
   as some element of its set, coordinated with #910 (§8).
3. **Unbounded sets (D-040):** sampling is on by default in the default
   profile, because it is coherent with TLA+; the distribution is stated,
   seeded and named in the report notes together with the sets sampled.
   `--compat=tlc` keeps TLC's GENERAL 1000 error (§9).
4. **Retention:** efficient by default (the deciding trace plus a bounded,
   validated sample), with documented flags for complete output
   (`--keep-traces all`, `--trace-dir`); the user guide must explain how to get
   full output for export, animation and test generation (§10).
