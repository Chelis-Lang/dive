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
