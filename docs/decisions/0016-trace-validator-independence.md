# 0016: Trace validator independence (`dive-trace`)

- **Status:** proposed
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
- **A standalone build in CI.** CI builds and tests `dive-trace` on its own (`cargo check -p dive-trace --all-targets` and `cargo test -p dive-trace`). Features are unified only across the packages selected, so a stray call into the enumerator fails to compile there, even though the full workspace build enables the feature through `dive-check`.
- **Proposed layering change** (an owner decision; `layering.rs` is not edited here):
  - allow `dive-trace → dive-model`;
  - extend `cargo xtask check-layering` to check that `dive-trace`'s manifest does not enable `dive-eval/enumerate`, directly or through `dive-model`;
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
