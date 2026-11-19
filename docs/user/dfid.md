# Depth-first iterative deepening

```sh
dive dfid docs/user/examples/Counter.tla --max-level 8 --seed 7 --output json
```

`dive dfid` explores the model depth first, one iteration per depth bound, up
to `--max-level`. It checks pure safety models with one worker and reports each
iteration and how the run ended.

Read `model_proved` and `termination` in the report. Finishing an iteration or
reaching `--max-level` does not prove safety, and in the TLC profile a
depth-bounded run can exit 0. In this mode the exit status alone tells you
little.

The default seed drives a replayable native selection. TLC instead draws fresh,
unrecorded worker seeds for each iteration. `--worker-seeds S,...` records the
replay choices explicitly and labels a TLC-profile request as mixed; dive does
not treat TLC's unrecorded seeds as deterministic.

VIEW, symmetry, liveness and unsupported runtime or storage combinations refuse
by name. Every counterexample must pass independent validation. A Rust caller
cannot pass an effectful operator argument through a captured scope: DFID
checks the callable and refuses before any output or exploration. Pure captured
operator scopes work.

Rust callers use `DfidRequest` and `dfid_file`. `to_json()` carries the
completion evidence and `exit_status()` the process status.

See the [DFID reference controls](../../crates/dive-api/tests/fixtures/checker/dfid/README.md),
the [storage and checkpoint design](../decisions/0083-storage-and-checkpoint.md)
and [reports and errors](reports-and-errors.md).

## Run budgets

```sh
dive dfid Spec.tla --max-level 100 --time-limit 10s --max-memory 2g
```

DFID uses the same budgets as checking: no default time limit and half of
physical memory, in both profiles. `none` removes a limit, and memory also
accepts `auto`. An explicit time limit in the TLC profile labels the profile
`tlc+overrides`.

The clock starts after the model loads. Assumptions, Init and the search loops
check the budget as they run, including inside evaluator loops. Retained traces
are serialized with state checkpoints and replayed independently on fresh
runtimes under the same budget; when the budget runs out, unprocessed retained
traces are skipped. Initial model loading and encoding or decoding a single
value cannot be interrupted. A final resource sample runs before the report is
written. Memory is sampled process usage, not a hard cap at allocation.
`options.budget` records these settings.

When a budget runs out the result is `stopped`, exit 5, with the resource,
limit, observed value, phase and progress reached. A stopped run proves neither
safety nor a violation; retained prefix traces and earlier observations are
diagnostic only. Rust requests set `time_limit` and `max_memory` through
`BudgetSetting`. One budget covers the whole run; the clock does not restart for
each trace or iteration.

## Engine

DFID runs on Core, the default of `dive dfid` and `dfid_file`;
`--engine reference` was removed. Core runs compiled Init, Next and predicate
plans in both profiles, with fingerprint membership guarded against exact
collisions, or exact membership. Normalization and VIEW evaluation stay
separate from the canonical membership keys.

Reports name `engine`, `engine_requested` and `engine_fallback`, which is always
`null` because Core is the only engine. A refusal reached during the search
never restarts DFID. Native recovery and disk membership are not supported.

Core keeps only the current Init or action candidate batch live, including its
generator continuations, until its deferred checks finish. It then keeps the
DFS rows it needs and frees the batch before the next one. Persistent values,
seen keys, DFS frames and the active DFS path use memory of their own, so this
does not bound total process memory. Unsupported raw or lazy value lifetimes
refuse.
