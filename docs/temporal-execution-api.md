# Bounded ordered temporal execution API

`dive_liveness::phase_execution::execute(&model, &options, limits)` is a native
Rust diagnostic/execution API. It is not a CLI model-checking mode and never
issues a temporal proof. Callers supply a bound `dive_model::Model`, explicit
`dive_check::CheckOptions` and `phase_graph::Limits`; source/role ownership and
model metadata are checked before temporal planning and execution.

A completed `Complete` retains actual safety exploration, configured property
identities, derived roles, OOS groups, product registrations, written state/action
bins and transition insertion order. `model_proof()` is always false.
`product_inputs(group, pem)` projects an actual retained graph for a subsequent
bounded analysis; constructing that graph does not establish a validated verdict.

Actual source callbacks execute after the source's safety checks and before
later-source deciding errors. The pool expands fixed waves of eight sources,
independent of requested width. The coordinator commits the source prefix in
reference order. `last_commit_work.logical` records logical stopping counts;
`batch_generated` records actual speculative emissions and can be larger.
Recursive tableau re-enumeration has separate generated/retained/expansion
accounting. Do not substitute any of these figures for an unproved TLC final
counter or initialization-error replay count.

`Err(Failure)` preserves the typed deciding error, actually committed graph
prefix, phase/recursive work and available worker/source-prefix evidence.
Preflight refusal uses zero actual workers. Lost execution evidence stays absent.
Neither partial persisted groups nor an independently valid word becomes a model
proof after a failure.

The admitted slice is bounded pure fingerprint exploration with actual null and
tableau OOS groups. Named refusals include inline/sequential requests or fallback,
VIEW/symmetry, runtime effects and introspection, unproved opaque/deferred scope
interactions, accepting-sink early-witness paths and cumulative resource bounds.
State-kind conditional planning and some recursive-constraint error timing remain
unproved. Graph entry, edge, predicate-cell, inspection, successor, recursion and
worker limits are entry/work bounds, not total process-memory budgets.

Run the executable source/ordering controls from the repository root:

```sh
cargo test -p dive-check --test observed
cargo test -p dive-liveness --test phase_execution
python3 -I scripts/check-phase-execution.py
```

These controls preserve all 26 reference-selected cases: 15 completed graph
projections, three no-OOS boundaries and eight reference failures without credit.
Eight clean TLC executions pin error precedence. Additional controls check a
callback stop beyond two completed waves, both stores and 1/3/4/8 workers, actual
speculative counts, no-work inline refusal and malformed source/role admission.
See [the complete evidence contract](../conformance/probes/tlc-phase-execution/README.md),
[ADR 0034](decisions/0034-ordered-temporal-execution.md),
[exposed temporal checking](user/temporal.md) and the
[coverage ledger](tlc-parity.md). Exact SCC/lasso choice, periodic scheduling and
ordinary strict temporal verdicts require separate implementation and controls.
