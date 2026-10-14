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
The current mode supports one actual worker and finite safety models, including
runtime `Print` and `PrintT` through captured operators. Their normal arguments,
rendering and return values are evaluated. Reached random draws, registers and
other unsupported host effects still refuse; untaken branches do not run them.
VIEW, symmetry, temporal checking and unproved sampling interactions refuse. Returned violations require independent trace validation;
unknown replay alignment remains pending/refused rather than certified.

Use `--stats basic --output json` to collect action flows, corresponding to TLC's
`-simulate num=N,stats=basic`. The default is `--stats none`. Rust callers set
`SimulateRequest::action_stats` to `true`. The optional `action_flow` object lists
source-location groups and nonzero `{from, to, count}` transitions. Parameter
contexts at the same predicate location share a group; `bound_action_ids` lists
its original actions, and `label` describes the first context.
Unused actions remain in the inventory. A reference to a definition already
bound to a value retains its invocation name and source location, so two
unused references to `Disabled == FALSE` form two groups. A definition override
`Op <- Replacement` retains its target predicate instead. Configured operators
are checked when actually evaluated, including Print/PrintT loggers. Reached
random/register/I/O and other unsupported effects still refuse; an unexecuted
branch is not refused solely for its effect annotation.

Counts record consecutive **chosen** actions: the first edge starts at the Init
predicate that produced the selected initial state. Every chosen transition,
including a stutter, adds one edge. Generated duplicates, rejected constraints,
disabled attempts and a failing candidate do not add edges. An error or resource
limit retains only earlier chosen transitions. Empty flow counts can therefore
accompany generated candidates or a valid violation. These are sample action
statistics, not state-space or expression coverage. Collecting them does not
change the seed, RNG draws, trace selection, property checks or validation.
Full context statistics, probabilistic generation, alternative schedulers,
additional workers, DFID completion and general coverage remain pending.

```sh
dive simulate docs/user/examples/Counter.tla --traces 3 --depth 8 --seed 7 --stats basic --output json
```

Rust callers use `SimulateRequest` and `simulate_file`; `traces` is required and
`depth` is optional. `SimulateReport::to_json()` exposes actual traces,
statistics, observed/pending outcomes and validation; `exit_code()` exposes the
process disposition. See [the detailed simulation contract](../../crates/dive-sim/README.md)
and [reports](reports-and-errors.md).

Simulation reports binding output once in `startup_output` and actual backend
Print/PrintT events in `runtime_output`, in evaluation order. Init printing is
runtime initialization output, not binding output. Independent validation freshly
reconstructs the model, preserves Print's argument/rendering errors and return
values, and discards its output without calling a sink. Replay events never
appear in either producer-output field. A violation still requires a valid
independent word; sampling remains no proof of safety or liveness.

The backend retains at most 100,000 runtime Print events and 16 MiB of their
UTF-8 text by default. `dive_sim::Options` exposes `max_output_events` and
`max_output_bytes`. Crossing either limit returns `resource-limit` with retained
output and actual completed-prefix work, never `sampled`. The limits cover
retained Print text/events, not temporary rendering allocations, existing warning
records or total process memory. RandomElement execution, generation, alternative
schedulers and further M/#239 work remain unfinished.

The Print candidate preserves both `--stats none` and `--stats basic`; source
controls compare output, seed/choices, outcomes and independently validated
words with statistics toggled. Qualification on the latest main, the full gate,
fresh review and publication remain pending. Historical Print oracle/scoped
measurements retain their actual source revisions and do not certify this cut.


## Shared evaluator simulation ownership

One simulation owns a bound evaluator core through host cleanup. Low-level
`simulate(&Model, ...)` calls on that same core, including cloned models, refuse
overlap or reentry without waiting: `unsupported` names concurrent/reentrant
simulation, with preflight stage, zero work and no output or host change. Existing
preflight option/feature refusals retain priority. Separate bound cores, including
separate API loads, can run concurrently. Callers must not mix foreign evaluator
or host operations with an active simulation; ordinary evaluator APIs are unchanged.

The run lease is operational ownership of the actual shared core. It releases
after the original host is restored, including error, limit and unsupported
returns. It introduces no TLS, callback admission or cache authority. The earlier
external overlap diagnostic and each scoped qualification retain their original
source bindings. Fresh review, latest-parent transfer and full gates remain pending.
