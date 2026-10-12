# 0016: Trace validator independence (`dive-trace`)

- **Status:** accepted
- **Date:** 2026-10-08

## Context

Charter soundness rule 2 says that every counterexample dive produces is independently replayed and validated against the spec by a separate trace validator. The architecture section adds that the validator "must not share evaluation shortcuts with the checker beyond the core evaluator, so that it remains a genuine second check".

Two mechanisms already exist:
- **Layering.** `xtask/src/layering.rs` forbids `dive-trace` from depending on `dive-check`, `dive-liveness`, `dive-sim` or `dive-store`, and a unit test asserts it.
- **The harness.** It does not compare counterexample content. An implementation other than TLC must report its counterexample as validated (`Implementation::trace_validated`), or it cannot `agree` on a violation (`crates/dive-conformance/README.md`; `compare.rs`).

Three things are still open:
- what "validated" checks;
- how the validator stays independent *inside* `dive-eval`, since successor enumeration also lives there (ADR 0014 §3);
- who computes the flag the harness consumes.

TLC facts that decide what a valid counterexample is (probes in `crates/dive-check/tests/fixtures/tlc-probes/`):
- **The last state can be outside the model.** A successor that fails `CONSTRAINT` is still invariant-checked and can be the last state of a counterexample (probe `ConstrainedViolation`). Every *earlier* state was explored, so it satisfies the constraints.
- **Invariants are reported in cfg order.** TLC checks them in cfg order on each new state and reports the first that fails.
- **Action properties apply to every step**, including steps into states already seen (probe `ImpliedActionSeen`).
- **Deadlock** means the last state generates no successor at all, before constraints (probes `ConstraintNoDeadlock`, `DeadlockMidLevel`).
- **Which action TLC explores.**
  - With `SPECIFICATION ... [][A]_v`, TLC explores `A`.
  - With `NEXT` given as `[A]_v`, it explores `[A]_v`, including stuttering steps (probes `StutterSpec`, `StutterInNext`).
- **Error traces.** For evaluation errors and assertion failures while expanding a state, the trace ends at the state being expanded (probe `EvalErrorMidLevel`; harness fixture `EvalError`).

## Decision

### 1. What the validator checks

`dive_trace::validate(model, trace) -> Validation` takes:
- the bound `Model`, from `dive-model` (ADR 0013 §8);
- a counterexample **as data**: the JSON form that `dive check --output json` emits, which is also accepted in TLC's `-dumpTrace json` form.

The counterexample is a list of states (full variable assignments), the step labels, and the claimed ending:
- invariant `P`;
- action property `P`;
- deadlock;
- assertion failure;
- evaluation error with an `EC` class;
- later, for liveness: a lasso, or stuttering.

The validator parses values with its own reader (`dive-value`'s JSON form). It never receives dive's in-memory states.

**Checks, all of which must pass:**

1. **Shape.**
   - Every state assigns exactly the model's variables.
   - The trace is non-empty.
   - The values parse under the run's `Ctx`.
2. **Initial state.** `s_0` satisfies the conjunction of the model's initial predicates, evaluated as a predicate on `s_0`.
3. **Steps.** Each `(s_k, s_{k+1})` satisfies the next-state action TLC explores, as above, evaluated as an action-level predicate with `s_k` unprimed and `s_{k+1}` primed.
   - If the step carries a label, the labelled split action alone must also hold.
   - A label that names the wrong action is a failure.
4. **Model membership.**
   - Every state except the last satisfies every `CONSTRAINT`.
   - Every step except the last satisfies every `ACTION_CONSTRAINT`.
   - For an invariant violation, the last state may lie outside the model, as in TLC. For every other ending it must lie inside.
5. **No earlier stop.**
   - Every invariant holds on every state before the last.
   - Every implied action holds on every step before the last.
   - For an initial-state ending, every implied initial predicate is checked on `s_0`.
6. **The ending.**

   | Ending | Check |
   |---|---|
   | Invariant `P` | `P` is false on the last state, and every invariant listed before `P` in the cfg is true there |
   | Action property `P` | `P` is false on the last step, and the properties before it hold on that step |
   | Deadlock | `ENABLED` of the explored next-state action is false on the last state, evaluated by the core evaluator's `ENABLED` |
   | Assertion or evaluation error | Evaluating the action on the last state (or the failing check on the last state or step) raises an error of the claimed class |

**Not checked: minimality.** The validator does not check that the counterexample is shortest. That would need a search, which would make it a second checker. Length is compared with TLC's single-worker golden by the harness (charter rule 2, ADR 0004), and shortest-by-construction is ADR 0015's argument.

**The result.** The result is `Validated`, or `Invalid { step, check, detail }`. `Invalid` is precise enough to file as a bug.

### 2. Independence

**The validator shares only the core evaluator** (`dive-eval`'s expression and predicate evaluation, `ENABLED`), the value library (`dive-value`, `dive-stdlib`) and the binding stage (`dive-model`). It shares nothing on the search path:
- **Store and codec.** No fingerprint set and no state codec: `dive-store` is forbidden.
- **Search.** No frontier, merge or trace log: `dive-check` is forbidden.
- **Enumerator.** No successor enumerator.
  - Steps are checked by evaluating the action as a predicate over a *given* pair of states. That code path is different from `dive_eval::next`, which constructs successors.
  - Initial states are checked by evaluating `Init` as a predicate, not by `getInitStates`.

  A bug in enumeration (an order, a fast path, a binding rule) therefore cannot make the validator accept a trace the spec does not allow.

**Enforcement.**
- **The `enumerate` feature.** `dive-eval`'s enumerator sits behind a cargo feature, `enumerate`. `dive-trace` depends on `dive-eval` and `dive-model` with that feature off. `dive-model` must itself never need the feature: action splitting evaluates domains as predicates and values (ADR 0013 §4).
- **A standalone build in CI.** The workspace gate builds `dive-trace` on its own (`cargo build -p dive-trace --locked`). Dev-dependencies are not part of that build, so the check covers the validator itself, not its tests. Features are unified only across the packages selected, so a stray call into the enumerator fails to compile there, even though the full workspace build enables the feature through `dive-check`.
- **Layering change** (accepted by the lead on 2026-10-08, open question 11; implemented):
  - allow `dive-trace → dive-model`;
  - extend `cargo xtask check-layering` to check that a standalone `dive-trace` build does not enable `dive-eval/enumerate`, directly or through any internal dependency (`xtask/src/features.rs` resolves the features from the manifests, since `cargo metadata` reports them unified across the workspace);
  - add the standalone `dive-trace` build to the workspace gate.

**What is shared, and why that is acceptable.**
- Both sides share the core evaluator and `dive-model`.
- A bug there (in `\in` on a lazy set, say, or in which formula `SPECIFICATION` yields as `Next`) could make checker and validator agree on something TLC would not.
- That risk is covered by the harness's comparison of verdicts, counts and lengths against TLC (charter rule 1). It is also covered by §3's positive controls, which run TLC's own counterexamples through the validator.
- The charter permits sharing the core evaluator. Splitting it would mean two evaluators to keep TLC-exact.

### 3. Who computes "trace validated", and the harness controls

**dive itself.**
- `dive check` (through `dive-api`) validates every counterexample before reporting it.
- The JSON output carries `counterexample.validation: { "validated": true }`, or the `Invalid` detail.
- An invalid counterexample is reported as an internal error (a bug, with the trace attached), never as a violation verdict.

**The harness does not trust that flag.**
- `dive-conformance` already may depend on `dive-trace` (`layering.rs`).
- For every dive run that reports a violation, the harness:
  1. reads dive's JSON counterexample;
  2. builds the `Model` from the same spec, cfg, profile and options;
  3. calls `dive_trace::validate` in-process.
- `Implementation::trace_validated` is the harness's result.
- A violation whose counterexample is missing or fails validation can never `agree`. It is a `disagree` on the field `trace.validated`.

**Positive controls.** In the standard tier, the harness also runs every TLC golden counterexample (the `-dumpTrace json` document stored in the golden) through `dive_trace::validate`. All must validate:
- TLC's traces are valid by definition of the oracle;
- a rejection is a validator or evaluator bug, or a TLC finding to investigate.

Goldens whose trace was replaced by a hash (over 1 MB, ADR 0003) are skipped and counted.

**Negative controls (mutation).** For each validated counterexample, the harness also builds mutants that are invalid by construction, given that the original is valid:
- truncate the trace before its last state, keeping the claimed ending. The claimed invariant or property then holds at the new last state, because check 5 passed on the original;
- claim an invariant listed before the violated one in the cfg. It holds at the last state, by check 6;
- truncate before the last state and claim deadlock. The new last state has a successor: the original's last state;
- relabel a step with a split action that does not hold on that step, chosen by evaluating the candidates.

Every such mutant must be rejected. A mutant that validates is a validator bug.

**Deep tier.** Random mutants (dropped steps, swapped states, perturbed values) are only classified against an oracle. TLC's `-loadTrace` replay, available at the pin, is that oracle. The validator's verdict must equal TLC's.

### 4. Trace validation of implementation traces (later)

The charter's `dive validate-trace` checks *implementation* traces, which may be partial, against a spec. That is a different problem: TLC handles it by search, with `-loadTrace` and trace-validation specs. It reuses this validator's parser and predicate evaluation, but needs a search over unassigned variables. It is scheduled after M5 and gets its own ADR.

## Consequences

- dive's "same counterexample" claim (ADR 0015) is checked on every run, and the harness's `trace_validated` comes from an independent computation, not from dive's own report.
- The enumerator, the part of the evaluator most specific to TLC and most likely to have subtle order or binding bugs, is outside the validator's trusted base, and the build enforces it.
- Positive controls on TLC's own counterexamples test the validator against the oracle from the first day dive-trace exists, before dive produces any verdict. This makes the validator the first evaluator consumer to meet conformance (M5 plan, WP-E1).
- Checking each step costs one predicate evaluation, so validation is negligible next to checking.
- **Revisit:**
  - when liveness counterexamples (lassos, stuttering) arrive;
  - when `VIEW`/`SYMMETRY` traces are compared (dive prints explored states, ADR 0015 §4);
  - when implementation-trace validation is designed.

## Amendments

**Amendment 1, 2026-10-08, WP-E1 (the validator).** Building `dive-trace`, its API and its controls, and running them on the pinned goldens, settled these points. Where they differ from the sections above, this amendment holds.

1. **Deadlock is refused by name, and the harness confirms it with the oracle.** §1 checks a deadlock with `ENABLED` of the explored next-state relation. In dive, `ENABLED` *is* the successor enumerator (`dive_eval::next`, TLC's `enabled` port, ADR 0014's WP-C2 amendment): without the `enumerate` feature the evaluator refuses it. The options were:
   - **(a) Turn the feature on for `ENABLED`.** That puts the enumerator, which §2 keeps out of the validator's trusted base, back in. A bug in successor generation could then make the validator agree with a wrong deadlock. Rejected.
   - **(b) A second, independent successor search in `dive-trace`.** It would decide whether *some* state satisfies the action with the given state unprimed: a search over the primed variables, which is a second enumerator that must agree with TLC on which actions can be taken. It is real work, and it would make the validator a partial second checker, which §1 rules out for minimality for the same reason. Deferred; see "Revisit".
   - **(c) Trust the checker.** Forbidden by charter rule 2.
   - **(d) Refuse by name (chosen).** The validator makes every other check (§1, checks 1 to 5: every state inside the model, the last included; every invariant on every state; every action property on every step; the model must check deadlock at all, or the claim is invalid). Any failure is `Invalid`, so a deadlock trace is still refuted when it can be. If all pass, the result is `Unsupported` with the reason `dive_trace::DEADLOCK_UNSUPPORTED`. A refusal is never a verdict.

   **In the harness**, a deadlock counterexample counts as validated (`Implementation::trace_validated`) only when the validator passed everything else *and* its last state equals, as a value, the last state of the golden's own deadlock counterexample, read by the same validator. The oracle, not the implementation, vouches that the state has no successor. A golden whose document is unavailable (`trace.json_unavailable`, ADR 0003) cannot confirm it, so such a deadlock cannot `agree`. `dive check` (WP-D2) must report a deadlock counterexample's validation as `unsupported`, never as validated.
2. **Error and liveness endings are checked as far as they can be, then refused.** For an assertion failure or evaluation error, re-raising the error means generating the last state's successors: the steps, constraints, invariants and action properties are checked, and the claim is refused by name. For a lasso or stuttering ending, the steps are checked, including the loop back for a lasso, and the temporal property is refused until liveness counterexamples are validated. Under a `VIEW`, TLC closes the loop on the view's value, not the state's (Examples `MCDistributedReplicatedLog`: the last state's successor has the same view as state 2, but is not state 2), so the loop step is not checked there, with a note.
3. **Labels check the named definition, not the split action.** The split actions (`Model::actions`) are built by the enumerator's expansion, so they do not exist without `enumerate`. Instead, a label that names a definition of the root module with no parameters (after the configuration's `<-` overrides) must evaluate to `TRUE` on the step. TLC labels a step with the definition its action was split from, and a split action implies the body of the definition it came from, so this check is weaker than §1's but sound. A label that names an operator with parameters, or no definition, is noted and not checked; one whose definition cannot be evaluated on its own is noted too. A label that evaluates to `FALSE` is `Invalid`.
4. **The assumptions are checked** (`Check::Assumption`): a counterexample of a model whose `ASSUME`s fail is invalid.
5. **Endings.** An action-property ending on a one-state trace names an implied initial predicate (TLC's `TLC_PROPERTY_VIOLATED_INITIAL`). For an implied-initial ending, every invariant must hold on the initial state; for an invariant ending at the initial state, the implied initial predicates are not required (TLC stops at the invariant). A last state outside the model is accepted only for an invariant ending, as §1 says.
6. **The format** (`dive_trace::format`, version 1) is a JSON object `{"format": "dive-trace", "version": 1, "variables", "states": [{"action", "values"}], "ending", "length"?}`, documented in the crate. Values use dive's own lossless JSON form (`dive_trace::value_json`): `true`/`false`, numbers, strings, and one-key objects `{"set": [...]}`, `{"seq": [...]}`, `{"record": {...}}`, `{"fcn": [[k, v], ...]}`, `{"model": "m"}` and `{"int": "..."}` for integers outside `i64`. `dive check --output json` embeds the object as `counterexample.trace`, next to the report's own `states` (whose values are in TLC's lossy JSON form, for display). `dive check` does not yet validate its own counterexamples before reporting them (§3); the harness does.
7. **TLC's `-dumpTrace json` is read by interpretation.** TLC's JSON form is lossy: a set and a sequence are both arrays, a string and a model value are both strings, and a function's domain elements are object keys in TLC's printed form. TLC cannot read these documents back either. `dive_trace::tlc` makes each ambiguous choice once per path in the value (variable, field, element), for the whole trace. The JSON forces some choices: an array with repeated elements, integers out of ascending order, or elements of different JSON kinds is a sequence, and a string is a model value only if the configuration declares one of that name. The remaining choices are searched: the default reading first, then improving single changes (scored by the conjuncts of the initial predicate that hold and the checks passed), then, with at most 10 free choices, all of them. The result is `Valid` if some reading is a valid trace, and that exact trace is returned as the witness, validated like any other. It is `Invalid` only when the search was exhaustive, and `Unsupported` otherwise. With an `ALIAS`, TLC writes the alias's records instead of the states: if every variable is a field, the state is read from those fields (the alias expression is not evaluated; in `SlidingPuzzles_anim` and `Hanoi_anim` it runs `IOExec`); otherwise the trace is refused. The negative controls mutate the exact witness, so they never depend on the search.
8. **Implementation traces with every variable assigned are supported now.** §4's partial traces still need a search and their own ADR. A trace whose states assign every variable is validated as `Ending::Behavior`: the initial predicate on the first state and the next-state relation on every step, with no constraint, invariant or property checks. `dive_api::trace::SpecModel` exposes it (load a spec and cfg, validate a `TraceDoc` built from observed states), and so does `dive validate-trace`.
9. **The negative controls** (`dive_trace::mutate`) are the user-requested set plus §3's: drop a middle state, drop the initial state, swap two adjacent states, set one variable of a middle state to a model value that occurs nowhere in the specification (one mutant per variable), truncate before the end keeping the claim, claim another invariant or property (and one that does not exist), and relabel a step with another of the trace's actions that is false on it. Labels on the steps a mutation touches are removed, so the states, not the labels, must be rejected. Dropping and swapping are invalid by construction for a breadth-first counterexample: such a trace is a shortest path, so state `k` is at distance exactly `k`, and a step that skips or reorders states would put a state nearer than that. §3's "truncate and claim deadlock" mutant is dropped: per point 1 the validator cannot refute it. Truncation is generated only for invariant and property endings, for the same reason.
10. **Two small evaluator additions**, outside `enumerate`: `Evaluator::redirected` (a definition after `<-` overrides) and `Evaluator::is_value_override` (an `Op = v` definition), for the label check.

**Results on the pinned goldens** (`dive-conformance trace-controls --tier all`, under `--compat=tlc`). There are 17 golden counterexamples: 15 safety violations and 2 liveness violations, all with a `-dumpTrace json` document (no `trace.json_unavailable`; v1.7.4's goldens have no documents and are reference-only).
- **Valid (12 of the 15 safety violations):** `MCCheckpointCoordinationFailure`, `DieHard`, `MCDieHarder`, `MissionariesAndCannibals`, both `FourQueens`, `SlidingPuzzles`, `SlidingPuzzles_anim` (from its `ALIAS` fields), `ACP_NB_WRONG_TLC`, `MC_spanning`, `Hanoi` and `Hanoi_anim` (from its `ALIAS` fields).
- **Refused by name, none invalid:** `MCDieHardest` (the evaluator does not implement `TLCGet` yet), `SDP_Attack_Spec` (`Bitwise!Xor`), `EWD840_anim` (its `ALIAS` record does not contain the variables), and the two liveness traces (`MCDistributedReplicatedLog`, `MCRealTimeHourClock`), whose steps validate.
- **Mutants:** every mutant of every trace that validates, or whose steps validate, is rejected. The mutants of the three refused safety traces stop at the same refusal, and none validates.
- **Probes** (`crates/dive-check/tests/fixtures/tlc-probes/`): the 12 with a document validate, `DeadlockMidLevel` and `StutterSpec` up to the deadlock claim, and all their mutants are rejected. `StringOrderPreinterned`'s document is unavailable (issue #35) and is skipped.

**Harness, `run --impl dive --impl-profile tlc --tier small`.** Before E1 (main at `8dde457`): 59 `agree`, 9 `agree-pending-trace-validation`, 72 `unsupported`, 0 `disagree`. After: 68 `agree`, 0 pending, 72 `unsupported`, 0 `disagree`. All nine pending violations validate: `DieHard`, `MCDieHarder`, `MissionariesAndCannibals`, `Queens` `FourQueens`, `SlidingPuzzles`, `SlidingPuzzles_anim`, `ACP_NB_WRONG_TLC`, `MC_spanning` and `Hanoi_anim`. An `agree-pending-trace-validation` outcome now means the validator could not confirm the counterexample (a refusal by name, or no document); an invalid counterexample is a `disagree` on `trace.invalid`. `scripts/conformance-gate.sh` accepts only `agree`.

The gate runs the controls on the seven goldens whose models are vendored in `dive-eval`'s fixtures and on the probes (`crates/dive-api/tests/trace_controls.rs`); `trace-controls` runs them on the corpus.

**Revisit:** an independent successor check for deadlock (option b) if refusing deadlocks blocks agreement on goldens without documents; and `TLCGet`/`Bitwise` once the evaluator implements them, which turns the two refused controls into positive ones.

**Amendment 2, 2026-10-09, issue #52 (replaying random draws).** Under `--compat=tlc`, probe `P9Random` agreed with TLC on every compared field but stayed `agree-pending-trace-validation`: its counterexample's value comes from `RandomElement`, and the validator's host provided no random stream. Where this amendment differs from the sections above, it holds.

1. **What carries the seed.** The checking run's seed and fingerprint polynomial index, `TraceOptions::run` (`dive_api::trace`; the harness passes the goldens' `-seed 0 -fp 0`, and `dive validate-trace` takes `--seed` and `--fp`, with `dive check`'s defaults). The trace document does not carry them: they are options of the run, which `dive check` reports. `dive_trace::Replay` holds the run's `Streams` (TLC's seeding under `--compat=tlc`, dive's otherwise) and the polynomial index, and is passed to `validate`, `validate_doc`, `tlc::validate_tlc` and `mutate::mutants`.
2. **The host is `dive-stdlib`'s `RunHost`, reused.** `dive-stdlib` depends on `dive-eval` with `enumerate` off, so the validator can use the checker's own random operators (TLC's algorithms over the run's streams) without the successor generator. `dive-trace → dive-stdlib` was already a permitted edge; `check-layering` and the standalone `cargo build -p dive-trace` stay green. `RunHost` gains `stream`, `set_stream` and `skip` (advance a stream by whole generator steps). Nothing else is shared with the search: the validator computes each state's fingerprint itself, as TLC's (`TLCStateMut.fingerPrint`: FP64 over the values in TLC's variable order, `Ir::tlc_var_order`), which is what both profiles seed per-state streams with.
3. **Where draws are replayed.** The initial predicate draws from a fresh start-up stream, as TLC and dive rebuild a counterexample's initial state (D-002). The step out of state `k` (and a lasso's loop step) draws from the stream seeded with state `k`'s fingerprint. `ASSUME`s are re-checked on a fresh start-up stream; one that draws and fails there is refused, since the run drew its constant definitions from that stream first. A draw anywhere else (an invariant, a constraint, an action property, or any check under a `VIEW` or `SYMMETRY`, where TLC seeds with the view's or the representative's fingerprint) is refused by name (`dive_trace::RANDOM_UNPLACED`). A label whose definition draws is not checked, with a note.
4. **Draw order must be proved.** A predicate draws in its own evaluation order. The generator can draw for successors absent from the trace, so neither a match nor a mismatch at offset zero establishes the seed-specific claim without an alignment proof. The validator accepts a conservative subset: straight-line scalar assignments, pure RHS expressions or direct random calls on pure arguments, and a single enabled action whose alternatives are excluded by unprimed guards before any assignment. Each variable must be assigned once. Constant startup effects, shared lazy effects, quantifiers, primed guards and multiple enabled branches prevent certification. They are reported as unsupported rather than searched over a finite offset window.

   The certificate is a read-only inspection of the IR plus evaluation of pure guards; it constructs no states and does not call or compile the successor enumerator. A recursion/fuel limit can only make certification unavailable. The core predicates still check every state and step. Wrong seeds and polynomial indices are invalid for certified predicates; uncertified random predicates remain unsupported even if a draw happens to match.
5. **Negative controls.** Certified traces add `redraw-k-v`: replace a directly assigned draw with a different value from the same operator under another seed. Include the candidate only when independent predicate validation rejects it at a certified position. There is no bounded-stream assumption.

**Rejected approach (takeover audit, 2026-10-09).** The interrupted implementation searched 4096 offsets and treated failure to find a match as invalid. `RndWide` reaches a valid successor after more draws than this bound. A singleton random domain can also produce a coincidental match despite unknown placement (`RndCoincidence`). The original assertions for uncertified mismatches were incorrect; those cases now explicitly test refusal. Minimal specs and pinned TLC captures live under `crates/dive-api/tests/fixtures/random-traces/`. At seed 7, TLC itself times out after announcing the `RndWide` violation while rebuilding its counterexample (300 seconds); the capture is not used as a valid trace.

**Results.** `P9Random`: the TLC golden's counterexample (`x = 730968`, `-seed 0 -fp 0`) validates, and its `redraw-0-x` mutant is rejected. `crates/dive-api/tests/trace_random.rs` checks dive's own counterexamples with draws in `Init` and `Next` in both profiles at three seeds: they validate with the run's seed, are refused without it, and are invalid with another seed or (from the first step) another polynomial; every mutant, `redraw` included, is rejected; the out-of-order case above is refused, not judged; a draw in an invariant is refused by name. Harness and `trace-controls` numbers before and after are in the PR for issues #52 and #55.

**SpanTreeRandom-style models.** Examples `SpanTreeRandom` draws its graph in a constant definition (`RandomElement(SUBSET ...)`), which the validator binds from the run's start-up stream, as the checker does. Its configuration checks temporal properties (`PROPERTY Liveness Safety`), so `dive check` refuses it until liveness lands, and its golden is a success with no counterexample to validate. It uses no `SYMMETRY`; a `SYMMETRY` or `VIEW` variant would also need the validator to seed per-state streams with the representative's or view's fingerprint, which is refused by name today.

**Revisit:** an independent placement of draws (for example, a draw log in dive's trace document, checked against the stream) if draw-order refusals block agreement on corpus models.


## Amendment 2: bounded independent finite-cover decisions (2026-10-09)

`dive_trace::exists::exists_successor` implements part of the previously deferred
independent successor-existence option. It shares predicate evaluation and
values, never the checker generator, graph or liveness plan. Necessary covers
come from equality, explicit finite-set/interval membership, conjunction,
disjunction, state guards, state-selected IF and UNCHANGED variables/tuples.
Conjunction chooses a necessary cover; disjunction covers only variables bounded
in every possible branch, unioning their covers. All primed reads must be
covered; unreferenced variables may retain current values. The original action
checks correlations on every candidate. An optional subscript comparison requires
a non-stuttering witness. Model constraints and reachability never restrict
ENABLED.

A concrete witness proves existence; complete-cover exhaustion proves absence.
Missing covers, unsupported contexts/operators and limits refuse. Bounds cap
variable-domain entries, Cartesian assignments and expression visits, not total
memory in bytes. This is a partial independent action decision procedure.

Deadlock endings now use it. A witness refutes the claim; absence certifies it.
Other cases retain `DEADLOCK_UNSUPPORTED`, diagnostic notes and pinned-oracle
confirmation. The harness reads both certified and refused golden deadlocks.
WF/SF retain the recurring-action certificate, then check cycle enabledness
independently: WF needs a disabled position, SF needs every cycle position
disabled. Prefix occurrences do not suffice. The validator remains built without
`enumerate`, enforced by CI. Full arbitrary-action existence remains pending.


## Amendment: runtime namespace provenance (2026-10-10, #124)

Independent validation must not infer encounter-ordered runtime string tokens
from the trace it is meant to prove. Witness decoding itself interns strings;
a forged successor can therefore change CHOOSE ordering. Seed/fingerprint
metadata and eager checker configuration do not establish names created by
off-trace exploration.

Until independent namespace provenance is implemented, reachable runtime-name
constructors under TLC value or exploration order yield named `unsupported`
in both the decoded-trace API and the lossy TLC reader. The latter preflights
before interpretation heuristics evaluate predicates. Default canonical order
remains decisive. Literal-name models remain supported. This conservative
admission restriction also covers tuple uses of overloaded string constructors;
no successor enumeration or checker/store dependency is introduced.

The safe clean-oracle control and the previous forged positive are retained in
`crates/dive-trace/tests/fixtures/runtime-names/`. Tests check both formats,
direct decoded validation, metadata presence/absence and canonical controls.


Public lasso-formula and independent enabledness APIs also preflight their
supplied predicates for runtime-name constructors in TLC order. The model's
configured checking-path flag is insufficient for an unconfigured caller
predicate. Existing scoped walkers resolve redirects and retain their opaque
context refusals. No caller may establish independent truth or disabledness
by choosing an unproved runtime namespace.

### Implementation addendum: pure VIEW random streams (2026-10-11)

The first ordinary transformed stream slice permits closed pure VIEW expressions
and pure startup/Init. The checker selects `Streams::state` with FP64 of the VIEW
value after concrete deep normalization. The independent validator computes that
fingerprint itself from supplied concrete states and evaluates Next as a
predicate; it never re-enumerates successors. Both sides share only binding,
value operations and a callback-free stored-shape admission helper.

Admission is bounded to 16,384 visits and depth 64; explicit homogeneous atomic
sets and literal tuples/records are admitted recursively. General functions need
homogeneous atomic domains. Compound/mixed sets, nonliteral tuple/record shapes,
providers and symbolic sets remain refused rather than infer safe JSON replay.
The #190 decoder/normalization boundary remains open. Source walks retain bounds
512 (closed VIEW/startup) and 4096 (actual native effects); opaque contexts and
unused effectful/family definitions can conservatively refuse before binding.

The global TLC counterpart is Java random seeded with transformed fingerprint
XOR seed; default replayable streams retain the approved D-002 algorithm.
Requested workers are separate from actual inline width 1. First-successor
partial trace printing, startup draws, symmetry, Randomization families,
graph/temporal randomness and seeded recovery remain outside this proof slice.
No archive, complete family or shortest-trace certificate follows from admission.

## Amendment 3: sound replay only (2026-10-12, #241, #242)

The rescue plan (#241, stage V) reduces the validator to a replay checker.
Where this amendment differs from the sections and amendments above, it holds.

**What the validator does.** It decodes the trace against the bound model and
checks each claim by evaluation: the first state satisfies `Init`, each step
satisfies the next-state relation as an action predicate on the two given
states, constraints, invariants and action properties hold where the search
would have checked them, and the ending holds. A deadlock is decided by the
independent finite-cover search (`exists_successor`). A liveness lasso is
checked with the word semantics of `temporal.rs`. The successor enumerator is
still not compiled in (§2).

The lossy TLC reader checks the existing public model context before looking up
expressions. Its interpretation score evaluates only pure initial conjuncts,
with the replay host already installed. Scoring must never write registers,
print, or consume random draws before checking `Init` in its actual evaluation
order: otherwise `TLCGet(0)=1 /\\ TLCSet(0,1)` can be made true by the heuristic's
own earlier write. The regression controls in `crates/dive-trace/tests/tlc_effects.rs`
reject that false acceptance and keep untaken effects from running.

**Runtime names** (replaces the namespace-provenance amendment of
2026-10-10). The evaluator records how many names were interned when binding
finished (`Evaluator::startup_names`). Interning TLC's checker start-up names
and configuration strings (`intern_checker_names`, `intern_bfs_config`)
extends that count only when no run-time name came before them. The
validator validates as usual. If, under TLC's value or exploration order, any
name was interned after the start-up names (while decoding the trace, while
evaluating an `ASSUME`, a check, an `ENABLED` decision or a lasso formula, or
by a checker sharing the model), the result is `unsupported: runtime string
encounter order`, whatever the verdict would have been. Names interned at
binding have the same order as in TLC, so a verdict that only compared those
names is TLC's. The static admission proofs (`runtime_order`,
`tuple_history`, the tuple-shape proof in `exists`, `context_admission`) are
deleted. `INSTANCE ... WITH` bindings are entered only when they have no side
effects.

**Random draws** (replaces amendment 2's replay). Any check that draws a
random value makes the result `unsupported` (`RANDOM_UNSUPPORTED`). The
stream-position certificate (`random_position`) and the replay are deleted.
`dive_trace::Replay` is an uninhabited type kept only until the remaining
callers drop the argument.

**Registers.** Integer/named `TLCGet`, aggregate `TLCGet("all")` and
`TLCGet("all:named")`, and integer/named `TLCSet` are refused at evaluation
time (`REGISTERS_UNSUPPORTED`). A shared evaluator can contain values written
by checker evaluations outside the trace, even without runtime names. The
defaulted `Host::check_register_access` permits ordinary evaluator/checker
access; the replay host refuses before any read or write and remembers the
attempt, so optional label checks cannot swallow it and produce a verdict.
Untaken register calls remain supported, caller registers remain unchanged,
and non-register introspection retains its existing behavior.

**Typed state input.** Public trace, lasso and enabledness inputs check every
stored atom against the model's name table before evaluating predicates.
`Value::validate_names` checks text and token iteratively without interning,
normalizing, enumerating or calling a deferred source. Foreign tokens are
unsupported (`DIVE-V0041`), as are opaque deferred state values whose names
cannot be inspected (`DIVE-V0042`). This prevents a foreign interner's ordering
from making `CHOOSE` manufacture an invariant violation despite the model's
start-up name count remaining unchanged. Existing evaluator context checks
still validate atoms in constant bindings and captured scopes; the source IR,
binding process and its start-up name feed remain the trusted model boundary.

**Removed.** The negative-control mutants (`dive_trace::mutate`): the
harness's `trace-controls` and `SpecModel::controls` run the positive control
and the dive-format round trip only. The opt-in stored-representation trace
format version 2 (`value_repr`, `--trace-format representation-v2`): dive
writes and reads version 1 only.

**Effect.** `MCCheckpointCoordinationFailure`, whose `SubSeq` and `\o` only
build sequences of model values on the trace, validates under `--compat=tlc`
instead of being refused.
