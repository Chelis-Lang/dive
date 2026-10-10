# Seeded finite simulation

```sh
dive simulate docs/user/examples/Counter.tla --traces 3 --depth 8 --seed 7 --output json
```

Simulation samples a positive finite number of traces instead of exhaustively
exploring the model. The example returns `result: sampled`, with
`exhaustive: false` and `model_proved: false`, even though its sample is valid.
A zero exit without an observed error is never proof of safety or liveness.

Use `--compat=tlc` for the implemented Java/TLC selection stream and `--aril N`
for its supported seed adjustment. Record the seed and the complete options.
The current mode supports one actual worker and pure finite safety models.
VIEW, symmetry, temporal checking, unsupported effects and unproved sampling
interactions refuse. Returned violations require independent trace validation;
unknown replay alignment remains pending/refused rather than certified.

Rust callers use `SimulateRequest` and `simulate_file`; `traces` is required and
`depth` is optional. `SimulateReport::to_json()` exposes actual traces,
statistics, observed/pending outcomes and validation; `exit_code()` exposes the
process disposition. See [the detailed simulation contract](../../crates/dive-sim/README.md)
and [reports](reports-and-errors.md).
