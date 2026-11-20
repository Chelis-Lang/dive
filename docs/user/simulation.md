# Seeded finite simulation

```sh
dive simulate docs/user/examples/Counter.tla --traces 3 --depth 8 --seed 7 --output json
```

`dive simulate` samples a finite number of traces instead of exploring the
whole state space. The example returns `result: sampled`, with
`exhaustive: false` and `model_proved: false`, although every sampled trace is
valid.  Exit 0 is never proof of safety or liveness.

Record the seed and the complete options to reproduce a sample. dive samples
with its own per-trace stream in both profiles, so the same seed gives
different traces than TLC, including under `--compat=tlc`. A nonzero
`--aril N` is refused in the default profile. Under `--compat=tlc` it skips
the first N draws of each trace's stream, and it has no effect without an
explicit `--seed`. Simulation runs one worker on finite
safety models.

## What a sample covers

`Print` and `PrintT` run through captured operators, with their arguments,
rendering and return values evaluated. A `RandomElement`, register,
Randomization operator or other unsupported host effect refuses by name when
the simulation reaches it; a branch that is never taken does not run it.

`TLCGet("level")` returns what the pinned TLC simulator returns: 0 in `Init`,
the state's 1-based position in its trace in invariants and `CONSTRAINT`s, and
the position of the step's source state in `Next`, `ACTION_CONSTRAINT`s and
action properties. Independent replay recomputes the level from each state's
position. A primed `TLCGet("level")`, which TLC evaluates at the successor's
level, refuses by name.

As in TLC's simulator, each step chooses only among successors that satisfy
every `CONSTRAINT` and then every `ACTION_CONSTRAINT`. When no successor of a
state satisfies them and deadlock checking is on, the simulation reports a
deadlock (exit 11). `dive check` reports no deadlock there, because exhaustive
checking only stops exploring past such successors. Independent validation
regenerates every successor of the deadlocked state and accepts the deadlock
only if each one falls outside the constraints.

Initial states may hold compound values: sets, functions, records and
sequences. A `SPECIFICATION` with fairness conjuncts is simulated, since
fairness constrains only infinite behaviors and cannot change a finite safety
result.

Every configured safety property is checked as `dive check` checks it.
Invariants and `[]P` parts of a `PROPERTY` are checked on every state, and
`[][A]_v` parts on every step, before the constraints decide whether the
state stays in the sample, so a violation can end at a state the constraints
exclude. A state-level part of a `PROPERTY`, such as `PROPERTY x = 1`, is
checked on every initial state and reported as error 2108; TLC's simulator
never checks it ([D-036](../tlc-differences.md#d-036-simulation-checks-implied-initial-properties)).

`SYMMETRY` and `VIEW` do not affect simulation, which never merges equal or
equivalent states: the traces, counts and verdicts are the ones you get
without them, and the report's `notes` say so. The `SYMMETRY` declaration is
still prepared at startup, so its warnings and errors are the ones
`dive check` reports.

With an `ALIAS`, each trace entry carries an `alias` array: the `ALIAS` value
of each state, in order, evaluated on the same states with
`TLCGet("level")` as the state's position. The `document` keeps the states
themselves, which is what validation checks. When the `ALIAS` cannot be
evaluated on a trace (an evaluation error, or `Print`, a random draw or a
register inside it) the trace's `alias` is `null` and a note names the
reason; the verdict is unchanged. An `ALIAS` that names no definition
refuses.

Each step in a trace document carries the action that produced it, with its
name and source location, as in `dive check` counterexamples. Validation
checks each label on its step.

These refuse by name: temporal `PROPERTY` checking, `POSTCONDITION`, and
sampling interactions dive has not proved sound. Every returned violation must
pass independent trace validation; if replay cannot align the trace, the
result is `unsupported`.

## Action statistics

```sh
dive simulate docs/user/examples/Counter.tla --traces 3 --depth 8 --seed 7 --stats basic --output json
```

`--stats basic` collects action flows, like TLC's
`-simulate num=N,stats=basic`. The default is `--stats none`; Rust callers set
`SimulateRequest::action_stats` to `true`. The `action_flow` object lists
source-location groups and nonzero `{from, to, count}` transitions.

Parameter contexts at the same predicate location share a group;
`bound_action_ids` lists the group's original actions, and `label` describes
the first context. Unused actions stay in the inventory. A reference to a
definition already bound to a value keeps its own name and source location, so
two unused references to `Disabled == FALSE` form two groups. A definition
override `Op <- Replacement` keeps its target predicate. Configured operators,
including `Print` and `PrintT` loggers, are checked when evaluated.

Counts record consecutive **chosen** actions. The first edge starts at the Init
predicate that produced the selected initial state, and every chosen
transition, including a stutter, adds one edge. Generated duplicates, states
rejected by constraints, disabled attempts and a failing candidate add nothing.
After an error or resource limit, only the transitions chosen before it remain,
so a report can show generated candidates or a valid violation with empty flow
counts.

These counts describe the sample. They are not state-space or expression
coverage. Collecting them does not change the seed, the random draws, trace
selection, property checks or validation, and `Print` output is the same with
`--stats none` and `--stats basic`. Full context statistics, other schedulers,
more workers, DFID completion and general coverage are not supported.

## Rust API

Rust callers use `SimulateRequest` and `simulate_file`; `traces` is required and
`depth` is optional. `SimulateReport::to_json()` holds the traces, statistics,
observed and pending outcomes and validation, and `exit_code()` the process
status. The report keeps each trace as its serialized entry;
`SimulateReport::write_json` writes the same text as `to_json()` without
building the JSON tree, which for many traces costs several times the text.
See the [simulation contract](../../crates/dive-sim/CONTRACT.md) and
[reports](reports-and-errors.md).

## Output

The report lists binding output once, in `startup_output`, and the `Print` and
`PrintT` events of the run in `runtime_output`, in evaluation order. Printing
from Init counts as runtime output. Independent validation rebuilds the model
from scratch, keeps `Print`'s argument and rendering errors and return values,
and discards its output; replay events never appear in either output field.

By default the backend keeps at most 100,000 runtime `Print` events and 16 MiB
of their UTF-8 text. `dive_sim::Options` sets `max_output_events` and
`max_output_bytes`. Crossing either limit returns `resource-limit`, never
`sampled`, with the output kept so far and the work completed. The limits
cover retained `Print` text and events only: not temporary rendering
allocations, existing warning records or total process memory.

## One simulation per evaluator core

A simulation owns its bound evaluator core until host cleanup. A low-level
`simulate(&Model, ...)` call on a core that is already simulating, including
through a cloned model, refuses at once: the result is `unsupported`, naming
concurrent or reentrant simulation, in the preflight stage, with no work, no
output and no host change. Option and feature refusals found in preflight take
priority. Separate bound cores, including separate API loads, can run
concurrently. Do not mix other evaluator or host operations with an active
simulation.

The core is released after the original host is restored, including on error,
limit and unsupported returns.

## Random choices

Simulation refuses a reached `RandomElement` in `Init` or `Next` by name, with
verdict `unsupported`:

```sh
dive simulate crates/dive-sim/tests/fixtures/random-init/InitRandomNextBoundary.tla --compat=tlc --traces 2 --depth 3 --seed 7 --output json
```

Dive does not try to match TLC's individual random draws, in either profile.
Reproduce a sample with the same seed, options and source.

The file API creates the host for you. A low-level caller that binds source
creates `dive_sim::Runtime::new(options)`, installs `runtime.host()` in
`BindOptions`, then runs a backend with `runtime.simulate_with_backend`. A
runtime runs one execution and keeps the random draws consumed at startup.
`TLCGet("config")` is available after binding, with the seed and settings in
use.

## Constructive successors

```sh
dive simulate Spec.tla --generate --traces 3 --depth 100 --seed 7
```

`--generate` builds one successor at a time instead of enumerating them all,
which is the default. Rust callers set `SimulateRequest::generation`, and
low-level callers `dive_sim::Options::generation`. The JSON reports
`options.algorithm: constructive` and `probabilistic: true`. The TLC
equivalent is `dive tlc -generate num=3 -depth 100 Spec`; the adapter needs an
explicit finite positive `num=N`, and `-simulate num=N` keeps the enumerating
traversal.

The generator walks the Next disjunctions and bounded existential contexts in a
seeded cyclic order and stops at the first complete candidate that passes the
usual checks and constraints. Intervals are indexed directly, and finite sets
already stored use their normalized members, so no step builds every successor
to choose one. Disabled candidates and candidates outside the constraints are
skipped. Partial assignments and errors reached are errors. Every source domain
of an existential binder is evaluated before its contexts, even when an earlier
domain is empty.

Init selection, predicate quantifiers, CHOOSE, function construction and
ENABLED evaluate as usual and consume no generation choices. CASE generation,
action composition and opaque or lazy domains that dive has not proved sound
refuse by name when reached. Generation uses one worker.

Each retained finite behavior is checked independently by sample validation:
Init and Next, constraints, invariants and implied initial and action
properties must hold on its rows. That shows the sample is a valid behavior. It
does not show exhaustive safety, liveness or a particular random history.
Replay can examine many relation choices, so a generator that runs in constant
space can still take longer to replay. `validate-trace --sample` runs the same
validation on an exported behavior and labels its JSON result
`validation_goal: sample`; plain behavior validation checks only the relation
on a prefix.

## Exit codes

`--exit-codes native|tlc` selects the process status convention. An invariant
violation exits 10 under native and 12 under TLC. The flag does not change the
traces or their validation. Without the flag, simulation keeps its
profile-dependent statuses. The TLC adapter always uses TLC statuses. Rust
callers use `SimulateReport::exit_code_for` and `to_json_with_exit_convention`.

## Run budgets

```sh
dive simulate Spec.tla --traces 100 --depth 100 --time-limit 10s --max-memory 2g
```

Simulation uses the same budgets as checking: no default time limit and half of
physical memory, in both profiles. `none` removes a limit, and memory also
accepts `auto`. An explicit time limit in the TLC profile labels the profile
`tlc+overrides`.

The clock starts after the model loads. Assumptions, Init and the execution
loops check the budget as they run, including inside evaluator loops. Retained
traces are serialized with state checkpoints and replayed independently on
fresh runtimes under the same budget; when the budget runs out, unprocessed
retained traces are skipped. Initial model loading and encoding or decoding a
single value cannot be interrupted. A final resource sample runs before the
report is written. Memory is sampled process usage, not a hard cap at
allocation. `options.budget` records these settings.

When a budget runs out the result is `stopped`, exit 5, with the resource,
limit, observed value, phase and progress reached. A stopped run proves neither
safety nor a violation; retained prefix traces and earlier observations are
diagnostic only. Rust requests set `time_limit` and `max_memory` through
`BudgetSetting`. One budget covers the whole run; the clock does not restart for
each trace.

## Engine

dive runs simulation with compiled Init, Next and predicates for finite safety
models in either profile. What dive does not implement, such as random choices
and registers, refuses by name. A refusal is not retried and does not repeat
output. JSON reports `engine: "core"`, `engine_requested` and
`engine_fallback`, which is always `null`, and reports requested and used
workers separately.

Compound states, fairness-only specifications, `Print` and basic action flows
are all validated independently. Unsupported runtime effects, lazy receivers
dive has not proved sound, and interleaved warnings and output refuse by name.
