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
rendering and return values are evaluated. Reference execution supports repeated
`RandomElement` calls at startup, in ordered Init and in Next, including finite
compound choices and captured operators. Independent replay remains bounded as
described below. Registers, Randomization operators and unsupported host effects
refuse; untaken branches do not run them.
Initial states may hold compound values (sets, functions, records, sequences).
A `SPECIFICATION` with fairness conjuncts is simulated: fairness constrains only
infinite behaviors, so it cannot change a finite safety result (ADR 0116).
Configured temporal `PROPERTY` checking, VIEW, symmetry and unproved sampling
interactions refuse. Returned violations require independent trace validation;
unknown replay alignment returns `unsupported`.
Configured VIEW is rejected by shared preflight in Reference, Core and Auto,
before assumptions, Init or sampling; exhausted budgets do not rewrite that
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

## Finite native random choices

Reference simulation owns a single host before source binding and continues its
stream through startup, assumptions, ordered Init and ordinary Next execution.
Repeated, nested, parameterized and configured RandomElement calls are supported;
finite compound states keep their actual values. Selection and constructive
traversal streams retain their existing contracts. The same source, options,
engine and seed reproduce the result. Matching TLC's individual draws is not a
parity obligation in either profile (ADR 0116).

```sh
dive simulate crates/dive-sim/tests/fixtures/random-init/InitRandomNextBoundary.tla --compat=tlc --traces 2 --depth 3 --seed 7 --output json
```

Every returned sample is still independently checked. Replay searches actual
source-defined finite choices, rebinding the entire private runtime for every
alternative. It preserves startup and lexical correlations within an attempt;
failed attempts share no values, atoms or normalization history. Actual bound
action captures persist within an attempt: a constant random action argument is
frozen by source binding, while action LET and expression LET keep their distinct
cache rules. Ordinary pure validation still omits action construction. Supplied scalar
state values only prioritize actual domain elements, without forcing compound
values or establishing membership. Ordinary predicates and the final claimed
ending must all pass. No producer RNG position or replay flag is proof.
Fresh action reconstruction keeps the pre-startup VIEW/symmetry randomness
refusal, including unused references. Native domain errors keep their diagnostics;
wrong-kind values in an otherwise pure initial state remain invalid words.

Replay bounds are 65,536 attempts, 256 source selections per attempt and 65,536
materialized domain entries. i64 intervals are indexed without materialization;
the earlier direct scalar Init membership fast path also remains. Function,
record and product domains follow native component selection, so an empty
function domain does not demand its range. Opaque deferred providers remain
unsupported. These bounds can make a valid complicated sample unproved; it is
reported explicitly, never certified. The existing run budget governs fresh
binding and replay. General long random words can require substantial search.

Random choices mixed with registers, reached random operations inside ENABLED,
and unproved Init forms remain unsupported. Ordered Init handles conjunctions,
assignments and lexical/call/substitution scopes; forms needing a different
independent assignment operation do not gain authority from the supplied state.
TLC JSON interpretation and temporal/error-endpoint replay retain their separate
support boundaries. Native trace versions and report schemas are unchanged.

The file API creates the host automatically. Low-level callers that bind source
should create `dive_sim::Runtime::new(options)`, install `runtime.host()` in
`BindOptions`, then call `runtime.simulate(&model)` or its controlled/backend
variant. A runtime admits one execution and preserves startup RNG consumption.
The legacy `dive_sim::simulate(&model, options)` still refuses reached randomness
when a foreign host is installed; it cannot recover that host's startup history.
Removing the host cannot erase consumed draws.

`TLCGet("config")` is available after binding with the actual seed and settings.
Print/output bounds and evaluator ownership remain in force. Explicit Core may
still refuse unsupported random effects; Auto may select Reference before
execution and reports that selection. Broader simulation, parallel workers and
remaining effects are not complete.

## Constructive successors

`dive simulate Spec.tla --generate --traces 3 --depth 100 --seed 7` selects a
constructive successor traversal in Reference, Core and Auto. The ordinary
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
name when reached. Reference RandomElement calls use the independent finite
choice relation described above; compiled effects retain their own support
boundaries. Both engines still use one actual worker. Seed replay requires the same engine,
options and source; matching TLC's random choices is not required (ADR 0116).

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

The default remains `reference`. `--engine core` uses compiled native Init/Next
and predicate execution for the supported finite safety slice in either profile:

```sh
dive simulate docs/user/examples/Counter.tla --traces 3 --depth 8 --seed 7 --engine core --output json
```

`--engine auto` may select the whole reference simulation before execution,
reporting its static reason. A reached native refusal never retries or duplicates
producer output. `DIVE_ENGINE` selects the same setting. JSON adds `engine`,
`engine_requested` and nullable `engine_fallback`; requested/actual workers remain
separate. Both implementations currently use one actual worker.

Native compound states, fairness-only specifications, Print and basic action
flows retain independent word validation. Unsupported native runtime effects,
unproved lazy receivers and warning/output interleaving refuse by name. Auto
can select reference for declared unavailable effects. Reproduce a sample with
the same engine, seed and options; TLC's internal draw sequence is not a parity
obligation, including under `--compat=tlc` (ADR 0116).
