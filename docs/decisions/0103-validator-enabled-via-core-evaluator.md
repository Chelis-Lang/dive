# 0103: The trace validator decides ENABLED with the core evaluator

- **Status:** accepted (owner approval, 2026-10-19)
- **Date:** 2026-10-19
- **Supersedes:** ADR 0016 §2's `enumerate` exclusion and Amendment 2
  ("bounded independent finite-cover decisions"), and the independent query
  machinery of ADRs 0099, 0100 and 0101.

## Context

ADR 0016 §2 kept `dive-eval`'s successor enumerator, and with it the core
evaluator's `ENABLED`, out of `dive-trace`. Amendment 2 then decided
`ENABLED`, deadlock and fairness through an independent bounded search,
`dive_trace::exists`: necessary finite covers for every primed variable,
assembled from a growing list of admission rules (equality, finite membership,
state-selected `IF`, `UNCHANGED`, quantifiers, constructors, `SelectSeq`
callbacks, lazy query slots, recursive domains, configured operators, ...).
ADRs 0099 to 0101 extended it to nested and lazy queries, with an
`EnabledResolver` callback in `dive-eval` and a pure-predicate budget threaded
through the evaluator's state that disables expression caching.

Every new expression shape needed a new rule, and each missing rule was a
refusal. `dive-trace` grew from 5.2k to 8.9k lines after #248, about 2.9k of
them in `exists.rs` and `ordinary_queries.rs`. Issue #321, which blocks an
external consumer, is one such refusal: weak and strong fairness on

```tla
Next == x' = 0 /\ ack' = IF x' = 1 THEN ~ack ELSE ack
```

refuse with `DIVE-T0020: ... no finite cover for primed variable ack`, because
the `IF` guard reads a primed variable. Pinned TLC reports the stuttering
liveness counterexample.

The second engine also disagreed with the core evaluator on exactly the cases
it was meant to cross-check. The replay host disabled expression caching
(`Host::reuse_expression_cache`), so lazy query values were recomputed
instead of reused as TLC's `LazyValue` caches them (the controls of #313 in
`conformance/corpus/lazy-query-lifetimes` and `lazy-enabled-cache`).

## Decision

**The validator decides `ENABLED` and successor existence with the core
evaluator.** `dive-trace` depends on `dive-eval` and `dive-model` with the
`enumerate` feature on. `dive_trace::enabled::enabled(model, action,
nonstuttering, state)` calls `Evaluator::enabled_scoped`, the same TLC port
(`Tool.enabled`) that evaluates `ENABLED` while checking. It is used for:

- deadlock endings: `ENABLED Next` on the last state;
- `ENABLED A` in temporal properties;
- `WF_v(A)` and `SF_v(A)` on a lasso without a recurring `<<A>>_v` step:
  `ENABLED <<A>>_v` at every cycle position;
- `ENABLED` nested inside any predicate the validator evaluates (invariants,
  constraints, action properties, labels and temporal atoms), which the
  evaluator handles natively.

An action with side effects, or a scope that shares lazy side effects, is
refused before the search. A refusal or evaluation error is never a decision:
a deadlock that cannot be decided is still `DEADLOCK_UNSUPPORTED`, confirmed by
the harness against the golden's deadlocked state as before.

**The new independence boundary.** Independence is kept at the level of the
replayed artifact. The validator still reads the counterexample as data with
its own reader and checks, by evaluation over the given states, the initial
predicate, every step of the next-state relation (and the loop or closing
step), constraints, invariants, action properties, labels, the claimed ending
and the temporal property on the lasso word. It still shares nothing on the
search path: `dive-check`, `dive-liveness`, `dive-sim` and `dive-store` stay
forbidden (`xtask/src/layering.rs`). What it now shares with the checker is
the whole core evaluator, including the `ENABLED` search and expression
caches. A bug there can make checker and validator agree on something TLC
would not; as ADR 0016 §2 already says for the rest of the evaluator, that
risk is covered by the harness's comparison of verdicts, counts and lengths
with TLC, by the positive controls on TLC's own counterexamples, and by the
mutation controls, which must all still be rejected.

The replay host keeps its refusals for random draws, registers, runtime names
and action composition. It no longer disables expression caching; instead
each replay host has its own cache domain (`Host::expression_cache_domain`).
Lazy argument and lazy function values that replay computes are cached and
reused within that replay, as TLC's `LazyValue` caches them, but replay never
reads a value another consumer cached (ordinary evaluation is domain 0) and
never overwrites one. Otherwise a caller could warm a retained `scope_apply`
argument through action composition and the cached `TRUE` would bypass the
composition refusal: a verdict would depend on how warm the caches were. Cold
and warm replay must agree (`replay_verdict_does_not_depend_on_cache_warmth`
and `warmed_same_owned_scopes_do_not_supply_replay_truth` in
`crates/dive-api/tests/trace_composition.rs`).

Public typed states (`dive_trace::validate`, `temporal::evaluate_lasso`,
`enabled::enabled`) are checked before evaluation for foreign names and opaque
deferred sets as before, and now also for sets, records and functions sorted in
another value order (`DIVE-V0043`). The deleted preflights refused these; the
core evaluator would read such a value in the wrong order (for example, which
element `CHOOSE` picks).

**Deleted:**

- `crates/dive-trace/src/exists.rs` and `exists/runtime.rs` (the finite-cover
  search, its admission rules and `Limits`), and `ordinary_queries.rs` (the
  `EnabledResolver` dispatch into it);
- the validator's pure-predicate budget and finite-binding preflights for
  temporal atoms, temporal actions, periodic predicates and labels: they are
  evaluated with `Evaluator::eval_scoped`, so they use the evaluator's caches;
- the replay host's `reuse_expression_cache` override (replaced by its own
  cache domain, above);
- the `enumerate`-free rule: `ENUMERATE_FREE` in `xtask/src/layering.rs`,
  `xtask/src/features.rs`, the standalone `cargo build -p dive-trace` step in
  `cargo xtask gate` and CI, and the matching sentence in `AGENTS.md`;
- tests that asserted cover rules, cover refusals and search budgets. Tests
  that compare decisions with brute force or pinned TLC observations, and
  every negative control, are kept and rewritten against `enabled`.

**Left for follow-up** (not needed by the validator any more, but still used
elsewhere):

- `dive-eval`'s pure-predicate budget (`begin_pure_predicate`,
  `eval_scoped_pure`, `check_finite_*bindings`) and the caching it disables,
  which the liveness planner (`dive-liveness/src/plan.rs`) still uses;
- `EnabledResolver`, `with_enabled_resolver` and the query memo in
  `dive-eval`, now without a production caller;
- the `enumerate` feature itself, which `dive-stdlib` and `dive-model` still
  select explicitly.

## Consequences

- #321 is fixed: both fairness kinds validate the TLC counterexample. The
  `fairness-replay` corpus records the weak and strong cases with
  harness-generated goldens.
- Every `ENABLED` the checker can evaluate, the validator can decide. New
  expression shapes need no validator change.
- The validator no longer catches a wrong `ENABLED` in the core evaluator by
  itself. The TLC comparison in the harness does, as for every other operator.
- Revisit if a core-evaluator `ENABLED` bug reaches a reported verdict that the
  harness's goldens did not cover: the remedy is a corpus control with the
  oracle's result, not a second search engine.
