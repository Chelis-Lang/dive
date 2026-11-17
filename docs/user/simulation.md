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
rendering and return values are evaluated. A reached `RandomElement`,
register, Randomization operator or other unsupported host effect refuses by
name; an untaken branch does not run it.
Core provides `TLCGet("level")` with the values the pinned TLC simulator
gives: 0 in `Init`, a state's 1-based position in its trace in invariants and
`CONSTRAINT`s, and the position of the step's source state in `Next`,
`ACTION_CONSTRAINT`s and action properties. Independent replay recomputes the
level from each state's position. A primed `TLCGet("level")`, which TLC
evaluates at the successor's level, refuses by name.
As in TLC's simulator, a step only chooses among successors that satisfy every
`CONSTRAINT` and then every `ACTION_CONSTRAINT`. A state none of whose
successors satisfies them is reported as a deadlock (exit 11) when deadlock
checking is on, even though `dive check` does not report one there: exhaustive
checking only stops exploring past such successors. Independent validation
regenerates every successor of the deadlocked state and accepts the deadlock
only if each one falls outside the constraints.
Initial states may hold compound values (sets, functions, records, sequences).
A `SPECIFICATION` with fairness conjuncts is simulated: fairness constrains only
infinite behaviors, so it cannot change a finite safety result.
Configured temporal `PROPERTY` checking, VIEW, symmetry and unproved sampling
interactions refuse. `ALIAS` is not applied, as in `dive check`: the traces
show the states themselves and the report's `notes` say so. An `ALIAS` that
names no definition refuses. Returned violations require independent trace validation;
unknown replay alignment returns `unsupported`.
Configured VIEW is rejected before assumptions, Init or sampling; exhausted budgets do not rewrite that
named refusal as a stopped run.

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
register/I/O and other unsupported effects refuse; an unexecuted
branch is not refused solely for its effect annotation.

Counts record consecutive **chosen** actions: the first edge starts at the Init
predicate that produced the selected initial state. Every chosen transition,
including a stutter, adds one edge. Generated duplicates, rejected constraints,
disabled attempts and a failing candidate do not add edges. An error or resource
limit retains only earlier chosen transitions. Empty flow counts can therefore
accompany generated candidates or a valid violation. These are sample action
statistics, not state-space or expression coverage. Collecting them does not
change the seed, RNG draws, trace selection, property checks or validation.
Full context statistics, alternative schedulers,
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
records or total process memory. Alternative schedulers, Randomization operators
and further host-effect work remain unfinished.

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

## Random choices

Simulation refuses a reached `RandomElement` in `Init` or `Next` by name, with
verdict `unsupported`:

```sh
dive simulate crates/dive-sim/tests/fixtures/random-init/InitRandomNextBoundary.tla --compat=tlc --traces 2 --depth 3 --seed 7 --output json
```

Matching TLC's individual random draws is not a parity requirement in either
profile.

The file API creates the host automatically. Low-level callers that bind source
should create `dive_sim::Runtime::new(options)`, install `runtime.host()` in
`BindOptions`, then run a backend with `runtime.simulate_with_backend`. A
runtime admits one execution and preserves startup RNG consumption.

`TLCGet("config")` is available after binding with the actual seed and settings.
Print/output bounds and evaluator ownership remain in force. Core refuses
reached random effects, such as `RandomElement` in `Init` or `Next`, by name. Broader simulation, parallel workers and
remaining effects are not complete.

## Constructive successors

`dive simulate Spec.tla --generate --traces 3 --depth 100 --seed 7` selects a
constructive successor traversal. The ordinary
simulation default still enumerates successors. Rust callers set
`SimulateRequest::generation`; low-level callers set `dive_sim::Options::generation`.
JSON reports `options.algorithm: constructive` and `probabilistic: true`.
The TLC equivalent is `dive tlc -generate num=3 -depth 100 Spec`; the shim requires
an explicit finite positive `num=N`. `-simulate num=N` retains ordinary traversal.

The generator visits actual Next disjunctions and bounded existential contexts
in a seeded cyclic order and stops after the first complete candidate that
passes the ordinary checks and constraints. Intervals use direct indices;
already stored finite sets use their existing normalized members. It does not
build every possible successor just to choose one. Disabled and constrained-out
candidates continue traversal. Partial assignments and reached errors remain
errors. All source domains of an existential binder are evaluated before its
contexts, even when an earlier domain is empty.

Init selection, predicate quantifiers, CHOOSE, function construction and ENABLED
keep their ordinary evaluator semantics and do not consume generation choices.
CASE generation, action composition and unproved opaque/lazy domains refuse by
name when reached. Generation uses one worker. Seed replay requires the same
options and source; matching TLC's random choices is not required.

Every retained finite Behavior sample is independently checked with the explicit
sample-validation operation: Init/Next, constraints, invariants and implied
initial/action properties must hold on its actual rows. This establishes the
sample's validity, not exhaustive safety, liveness or a prescribed RNG history.
Independent replay can still examine many relation choices, so a constant-space
producer does not imply constant-time end-to-end replay. `validate-trace --sample`
requests the same operation for an exported native Behavior word and labels its
JSON result `validation_goal: sample`; ordinary Behavior validation stays a
relation-only prefix check.

`--exit-codes native|tlc` selects physical process dispositions explicitly.
For example an invariant violation exits 10 under native and 12 under TLC.
The flag changes neither words nor their validation. Without the flag existing
profile-dependent simulation exits stay unchanged. The TLC shim always selects
TLC dispositions. Rust callers use `SimulateReport::exit_code_for` and
`to_json_with_exit_convention`.

## Run budgets

`dive simulate Spec.tla --traces 100 --depth 100 --time-limit 10s --max-memory 2g`
uses the same budget settings as checking: no default time limit and half
physical memory in both profiles.
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
`time_limit` and `max_memory` through `BudgetSetting`; one budget covers the run,
rather than restarting the clock for each trace or iteration.

## Execution

Core runs simulation with compiled Init, Next and predicate execution for
finite safety models in either profile:

```sh
dive simulate docs/user/examples/Counter.tla --traces 3 --depth 8 --seed 7 --output json
```

What Core does not implement, such as random choices and registers, is a
named refusal. A reached refusal is not retried and does not repeat output.
JSON reports `engine: "core"`, plus `engine_requested` and `engine_fallback`,
which is always `null`; requested and actual workers are reported separately.
Simulation uses one worker.

Native compound states, fairness-only specifications, Print and basic action
flows retain independent word validation. Unsupported native runtime effects,
unproved lazy receivers and warning/output interleaving refuse by name.
Reproduce a sample with the same seed and options; TLC's internal draw sequence is not a parity
obligation, including under `--compat=tlc`.
