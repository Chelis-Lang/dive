# Depth-first iterative deepening

```sh
dive dfid docs/user/examples/Counter.tla --max-level 8 --seed 7 --output json
```

`dive dfid` explores the model depth first, one iteration per depth bound, up
to `--max-level`. It checks pure safety models with one worker and reports each
iteration and how the run ended.

Read `model_proved` and `termination` in the report. Finishing an iteration or
reaching `--max-level` does not prove safety, and in the TLC profile a
depth-bounded run can exit 0. 

The default seed drives a replayable native selection. TLC instead draws fresh,
unrecorded worker seeds for each iteration. `--worker-seeds S,...` records the
replay choices explicitly and labels a TLC-profile request as mixed; dive does
not treat TLC's unrecorded seeds as deterministic.

VIEW, symmetry, liveness and unsupported runtime or storage combinations refuse
by name. Every counterexample must pass independent validation.

Rust callers use `DfidRequest` and `dfid_file`. `to_json()` carries the
completion evidence and `exit_status()` the process status. From Rust, an
operator argument with effects passed through a captured scope refuses before
any output or exploration; pure ones work. See
[reports and errors](reports-and-errors.md) and
[how DFID differs from TLC](../tlc-differences.md#d-018-a-dfid-zero-exit-may-mean-only-a-depth-bounded-check).

## Run budgets

```sh
dive dfid Spec.tla --max-level 100 --time-limit 10s --max-memory 2g
```

DFID uses the same budgets as checking: no default time limit and half of
physical memory, in both profiles. `none` removes a limit, and memory also
accepts `auto`. An explicit time limit in the TLC profile labels the profile
`tlc+overrides`.

The clock starts after the model loads. Assumptions, Init and the search loops
check the budget as they run, including inside evaluator loops. Counterexample
validation runs under the same budget; when it runs out, counterexamples not
yet validated are skipped and not reported. Initial model loading and encoding
or decoding a single value cannot be interrupted. A final resource sample runs before the report is
written. Memory is sampled process usage, not a hard cap at allocation.
`options.budget` records these settings.

When a budget runs out the result is `stopped`, exit 5, with the resource,
limit, observed value, phase and progress reached. A stopped run proves neither
safety nor a violation; retained prefix traces and earlier observations are
diagnostic only. Rust requests set `time_limit` and `max_memory` through
`BudgetSetting`. One budget covers the whole run; the clock does not restart for
each trace or iteration.

## Storage and memory

DFID stores fingerprints of seen states and checks every fingerprint match
against the full state: a collision stops the run without a
conclusion. Rust
callers can set `DfidRequest.exact` to store full states instead. The report's
`store` field says which (`fingerprint+exact-collision-guard` or `exact`).
Recovery and disk membership are not supported, and a refusal reached during
the search never restarts DFID. Reports carry `engine`, `engine_requested` and
`engine_fallback` (always `null`).

Successor candidates are freed level by level, but seen states, the search
stack and stored values use memory of their own, so peak memory is not
bounded by depth. `--max-memory` is the bound.
