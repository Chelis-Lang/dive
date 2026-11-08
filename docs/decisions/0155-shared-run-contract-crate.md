# 0155: The shared run contract lives in `dive-runtime`

- **Status:** accepted
- **Date:** 2026-11-07

## Context

ADR 0153 makes Core the product. Issue #791 part B retires the reference
search runtime in `dive-check`: the breadth-first engine (`engine.rs`), the
parallel pool (`parallel.rs`, `pool.rs`), DFID (`dfid.rs`), and the frontier,
seen-state and snapshot stores behind them.

Core (`dive-api`'s `core_*` modules) and `dive-liveness` reached into
`dive-check` for pieces that have nothing to do with that search:
- `CheckOptions` and `CheckResult` with its verdicts, statistics and storage
  usage;
- TLC's error classes (`class`, `classify`) and human rendering;
- budgets and `budget::EvaluationControl` (35 uses in `dive-liveness`);
- the behavior-graph types, `capture::CsrCapture`, capture evidence and
  admission (`capture::refusal`);
- checkpoint framing and `checkpoint::owned_identity` (`core_checkpoint`,
  `core_worker`);
- `engine::{run_config, startup_counters, search_counters}`.

While these lived in `dive-check`, deleting the reference search meant
editing every consumer.

## Decision

Add the crate `dive-runtime`, the run contract that every checking engine
shares. It holds:
- options, results, error classes and rendering;
- budgets and evaluation control;
- capture graph, evidence, checkpoint-observer and admission types;
- checkpoint framing, requests and receipts, and Core's archive identity;
- the run host's configuration and counters;
- pure model admission (randomness, effects, the temporal-route exclusion);
- the diagnostic replays that TLC's error classes need.

It depends on `dive-ir`, `dive-value`, `dive-eval`, `dive-stdlib`,
`dive-config`, `dive-model` and `dive-store`. It never runs a search.
`dive-check` depends on it and re-exports each item at its historical path,
so existing callers and the reference engine are unchanged. `dive-trace` may
not depend on `dive-runtime`, just as it may not depend on `dive-store`.
`xtask/src/layering.rs` records the new edges.

Some items were private to `dive-check` and are now used across the crate
boundary. These are `#[doc(hidden)]` and public. The capture evidence types
keep their guarantee: a driver can build them only through
`ControlledExploration::from_successful_search`, which runs the same
completion checks as `GraphBuilder`. The reference engine's unit-test race
hook is behind the `test-hooks` feature. Only `dive-check`'s
dev-dependency enables that feature.

This is a pure move. No verdict, count, message or exit status changes.

## Consequences

`dive-liveness` and Core now import the shared contract from `dive-runtime`.
Core no longer runs any `dive-check` code: the source-binding dispatch
(`dive_liveness::analyze::owned::dispatch_controlled`) binds the model
itself and no longer runs the reference search for safety-only properties,
DFID's request types and static admission live in `dive-api`, and Core frames
reference archives itself for their DIVE-S0400 rejection. `dive-api`
imports the shared contract from `dive-runtime` and no longer depends on
`dive-check`; `xtask/src/layering.rs` no longer permits that edge.
`dive_api::checkpoint` re-exports the archive request and format items from
`dive-runtime`, without the reference engine's `check` and
`recovery_seed_fp`.

`dive-liveness` no longer contains the reference temporal analyses
(`analyze_final*`, `analyze_periodic*` and the row-demand capture) and no
longer depends on `dive-check`; `xtask/src/layering.rs` no longer permits
that edge. Its analyzer tests build their graphs with a small test-only
producer (`crates/dive-liveness/src/test_capture.rs`), so no `dive-liveness`
target depends on `dive-check`.

The `dive-check` crate is deleted (issue #791). `dive-runtime` is the only
home of the run contract, and `xtask/src/layering.rs` no longer lists
`dive-check`. The fixtures its tests shared with other crates moved to
`crates/dive-api/tests/fixtures/checker/`; the tests that checked
`dive-runtime`'s CSR and evidence factories against actual captures moved to
`dive-liveness` (`tests/capture_csr.rs`, `tests/capture_evidence.rs`), and
the tests of the reference engine itself were removed with it.

With `dive-check` gone, its remaining hooks leave `dive-runtime` too: the
reached-checkpoint observer (`capture::CheckpointObserver` and its types),
`ControlledExploration::from_successful_search` and the `test-hooks` feature
(issue #791). Capture evidence is now built only by `GraphBuilder` and the
checked CSR producers (`CsrComplete::from_producer`,
`CsrPrefix::from_producer`), which run the completion checks themselves.
`dive-liveness` drops its unused `dive-store` dev-dependency. The permitted
edges in `xtask/src/layering.rs` are unchanged.
