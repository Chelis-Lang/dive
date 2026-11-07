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
They still use these reference-runtime pieces of `dive-check`, which part B
must replace or delete:
- `dive-liveness`:
  - the reference capture drivers `capture::explore_with_native_hooks`,
    `explore_controlled_with_native_hooks` and
    `explore_checkpoints_with_native_hooks`, behind `analyze_final*`,
    `analyze_periodic*` and row-demand capture;
  - `final_roles::dispatch_controlled`, whose safety continuation runs the
    reference search.
- Core in `dive-api`:
  - `dive_check::check` in `core_engine`'s binding-failure path;
  - `dfid::{preflight, check, Outcome}` in `core_modes`;
  - `checkpoint::recovery_seed_fp` for reference archives in
    `core_checkpoint`.

  `core_engine.rs` and `core_modes.rs` still import shared items through
  `dive-check`'s re-exports. They move to `dive-runtime` with part A
  (#791), which edits those files.

When those uses are gone, the `dive-liveness -> dive-check` edge is removed
and `dive-api` drops `dive-check`. The re-exports go when `dive-check` does.
