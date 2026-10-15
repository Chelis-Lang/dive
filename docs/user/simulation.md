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
rendering and return values are evaluated. One direct scalar Init
`RandomElement` draw is supported as described below. Other reached random draws,
registers and unsupported host effects refuse; untaken branches do not run them.
VIEW, symmetry, temporal checking and unproved sampling interactions refuse. Returned violations require independent trace validation;
unknown replay alignment returns `unsupported`.

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
random outside the supported Init call, register/I/O and other unsupported effects refuse; an unexecuted
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
additional workers, DFID completion and general coverage remain unsupported.

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
records or total process memory. General random execution, generation, alternative
schedulers and further M/#239 work remain unfinished.

Print preserves both `--stats none` and `--stats basic`; source controls compare
output, seed/choices, outcomes and independently validated words with statistics
toggled. Historical Print oracle/scoped measurements retain their actual source
revisions.


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
source bindings.

## Direct scalar initial random choice

A single-variable Init predicate `x = RandomElement(S)` can select from a nonempty
finite interval or stored scalar set. The domain evaluates normally, including
Print/PrintT and their errors. In TLC mode the draw uses the startup Java stream
from `--seed`; worker selection still uses its separate seed/aril stream. Aril
therefore changes later choices without changing the initial draw. The default
profile uses its existing startup and per-trace streams. No new option is needed.

```sh
dive simulate crates/dive-sim/tests/fixtures/random-init/InitRandomInterval.tla --compat=tlc --traces 2 --depth 3 --seed 0 --output json
```

The initial choice is made once before the requested traces. An invalid or empty
domain retains the original evaluation error. An initial invariant violation
still requires independent validation. That validator checks membership in the
Init relation; it does not certify that a word was drawn from the reported seed.

The runtime host permits only one actual native invocation during Init and
refuses reached Next/property draws. The file/API boundary independently validates
the sampled words and refuses wrapped, captured, configured or correlated Init
relations it cannot decide. Low-level `dive_sim::simulate` returns raw candidates;
`Outcome::Sampled` can include a one-call captured Init expression with a deferred
domain. An untaken random branch does not consume the allowance. Output reached
before a refusal or error remains in `runtime_output`; validation output stays
discarded.

The file API refuses random startup definitions as before. Low-level
`dive_sim::simulate` callers must supply a model with no prior startup random
draws and no installed host for this Init subset. A retained caller host causes
a reached Init-random refusal because its consumed startup stream cannot be
continued. Ordinary Print remains supported with caller hosts, and every return
restores the host before releasing simulation ownership. Removing a caller host
does not reset model history or establish startup-stream continuity.

Eleven tiny pinned primary observations cover seeds, aril, domains, output and
errors; the reached Next-random observation remains a native refusal. These
controls add no corpus agreement. Broader simulation support (M/#239) remains open.
