# Depth-first iterative deepening

```sh
dive dfid docs/user/examples/Counter.tla --max-level 8 --seed 7 --output json
```

DFID explores finite iterations with increasing depth. The pure safety slice
supports one actual worker and reports its iterations and termination.
Inspect `model_proved` and `termination`; merely finishing an iteration or
reaching `--max-level` cannot establish safety. TLC-profile depth exhaustion can
exit zero, so exit status is especially insufficient in this mode.

The default seed drives replayable native selection. TLC's fresh per-iteration
worker seeds are a separate qualification. Explicit `--worker-seeds S,...`
records replay choices and makes a TLC request mixed; it does not pretend that
TLC's unrecorded iteration seeds are deterministic.

VIEW, symmetry, liveness and unsupported runtime/storage combinations are named
refusals. Real counterexamples must pass independent validation. Rust callers
cannot inject effectful operator arguments through a captured scope: DFID
checks the actual callable and refuses before output or exploration. Pure
captured operator scopes remain supported. Callers use `DfidRequest` and `dfid_file`; `to_json()` carries the completion evidence
and `exit_status()` the process disposition.

See [DFID reference controls](../../crates/dive-check/tests/fixtures/dfid/README.md),
[ADR 0021](../decisions/0083-storage-and-checkpoint.md) and [report interpretation](reports-and-errors.md).

## Run budgets

`dive dfid Spec.tla --max-level 100 --time-limit 10s --max-memory 2g`
uses the same budget settings as checking: 30 minutes by default in the dive
profile, no time limit under `--compat=tlc`, and half physical memory in both.
`none` removes a limit; memory also accepts `auto`. An explicit TLC-profile
time limit labels the profile `tlc+overrides`.

The clock starts after model loading. Assumptions, Init and execution loops
receive cooperative control, including evaluator loop checkpoints. Retained
traces are serialized with state checkpoints and independently replayed on fresh
runtimes under the same budget. Exhaustion skips unprocessed retained traces.
Initial model loading and individual value encoding/decoding remain
uninterruptible. A final resource sample runs before publication. Memory is sampled process usage,
not an allocation-time hard cap. `options.budget` states these boundaries.

Exhaustion gives `result: stopped`, exit 5, actual resource/limit/observation,
phase and reached progress. It never certifies safety or a violation. Retained
prefix traces and earlier observations remain diagnostic. Rust requests expose
`time_limit` and `max_memory` through `BudgetSetting`; one owner covers the run,
rather than restarting the clock for each trace or iteration.

## Execution engines

`--engine reference|core|auto` and `DIVE_ENGINE` also select DFID execution.
The default and `dfid_file` remain reference. The core runs the supported pure
finite slice through compiled Init/Next and predicate plans in both profiles,
with fingerprint-plus-exact-collision-guard or exact membership. Actual
normalization/view demand stays separate from canonical membership keys.

```sh
dive dfid docs/user/examples/Counter.tla --max-level 8 --seed 7 --engine core --output json
```

Reports name `engine`, `engine_requested` and any static `engine_fallback`.
Core never falls back; Auto selects a whole reference run before execution only.
A reached refusal never restarts DFID. Native candidate batches release their
transients after real checking/key demand and retention of required DFS rows;
persistent values, seen keys, DFS frames and reports remain separate consumers,
so this is not a total-memory bound. Unsupported raw/lazy lifetimes refuse.
One actual worker, depth-bound incompleteness and independently validated
concrete violations retain their existing contracts. Native recovery and disk
membership integration remain separate work.
