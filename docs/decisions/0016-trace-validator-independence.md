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
