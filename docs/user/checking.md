# Safety checking

`dive check` explores every reachable state of a model breadth first and checks
its invariants, action properties, constraints and deadlock. It reads the
sibling `.cfg`, or the one you pass with `--config FILE.cfg`.

```sh
dive check docs/user/examples/Counter.tla --seed 7 --workers auto --output json
dive check docs/user/examples/Counter.tla --compat=tlc --seed 7 --fp 0 --workers 8 --store exact --output json
```

For finite function and set models, the optional [symbolic safety checker](symbolic.md) can reuse a complete reachable set or prove invariants by induction.

For message types written as unions of finite record sets, Core can check
membership directly against the fields. This also works inside a containing
record or a powerset, such as `MessagePool \in SUBSET Message`. Asking for the
elements of the type still requires actual enumeration. The [LuPastry+ example](../developer/large-models.md#lupastry-message-and-leaf-domains)
shows how to distinguish a large type from a large set of reachable states.

## Select a closed model input in the CFG

When a zero-argument TLA+ definition selects a host input, you can give that
definition a fixed scalar value in the model configuration. For example, copy
the Examples corpus's `TLC/TestMCReachability.cfg` to `graph1.cfg` and append
`CONSTANT GraphName = "1"`. Then run:

```sh
dive check TestMCReachability.tla --config graph1.cfg
```

This selects graph 1 with both original temporal properties. No environment
grant is needed. Core and pinned TLC found 15 distinct states and 21 generated
at depth 9. For graph `"1a"` both reported a five-state deadlock and the
independent validator accepted both Core traces. A different reachable use
of `IOEnv` still requires host input support. Plain boolean, string and
small integer assignments can flow through module instances this way.
[Six configurations and oracle results](../../conformance/reports/perf/2026-11-20-closed-graph-inputs.json)

For graph `"7"`, bind `GraphName` to `"7"` and keep the original
`Termination` and `Live` properties. With the CommunityModules directory on
the library path, this seed finds a fair 88-state stuttering counterexample
to `Live`:

```sh
dive simulate TestMCReachability.tla --config graph7.cfg --library-path modules \
  --traces 2048 --depth 120 --keep-traces 0 --seed 0 --workers 1 --output json > run.json
jq '.traces[0].document' run.json > witness.json
dive validate-trace TestMCReachability.tla --config graph7.cfg --library-path modules --trace witness.json
```

A successful witness replay establishes the violation. A sampling run that finds
no witness leaves the exhaustive question open. [Source hashes, commands and the separate TLC witness](../../conformance/reports/perf/2026-11-20-closed-graph-inputs.json).

## Four outcomes

The included counter shows the four results a check can give:

```sh
dive check docs/user/examples/Counter.tla --seed 0 --fp 0 --workers 1 --output json
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterViolation.cfg --seed 0 --fp 0 --workers 1 --output json
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterTemporal.cfg --seed 0 --fp 0 --workers 1 --output json
dive check docs/user/examples/Counter.tla --seed 0 --fp 0 --workers 1 --max-states 2 --output json
```

They return safety success (exit 0), a `StrictBound` invariant violation
(exit 12), an `EventuallyFour` temporal violation (exit 13) and a state-limit
stop (exit 5). The temporal violation is real: the specification allows
stuttering and declares no fairness.

From a source checkout, a Rust example runs the same four checks and replays
both counterexamples in a fresh validator, using `check_file`, `TraceDoc` and
`SpecModel`:

```sh
cargo run -p dive-api --example checking_preview --locked
```

The [Rust API guide](rust-api.md) covers the library interfaces.

## What a check covers

One compiled engine runs `dive check`, `dive simulate`, `dive dfid` and
`dive tlc` in both profiles. Anything it does not implement is an
`unsupported` verdict (exit 3) that names the feature; a run never falls back
to something else. `--engine core` and `DIVE_ENGINE=core` are accepted, and
any other engine name is a usage error (exit 2). The JSON report adds
`"engine": "core"` and a `core` object with strategy metrics. A check that ends
before any search, for example at a false `ASSUME`, names no engine.

- **Safety.** Invariants, constraints, `ACTION_CONSTRAINT`, deadlock, and the
  safety parts of a `PROPERTY`: `[]P`, its initial predicates and its actions
  `[][A]_v`. A refinement `PROPERTY` of another specification is of this kind.
  `-deadlock` disables deadlock checking, with TLC's spelling and meaning.
- **Symmetry and views.** `SYMMETRY` under `--compat=tlc` uses TLC's
  comparison, and a comparison TLC could fail on refuses. `VIEW` works, also
  with `SYMMETRY`, and is evaluated on the canonical state as TLC does. `ALIAS`
  is accepted but not applied, with a note. `POSTCONDITION` refuses by name.
- **Liveness.** A `PROPERTY` that needs liveness checking, with fairness, is
  checked on the behavior graph: at completed levels in the default profile
  ([periodic liveness checking](../tlc-differences.md#d-014-periodic-liveness-checking-has-a-replayable-schedule)),
  once the search is complete under `--compat=tlc`. Every counterexample is
  validated independently before it is reported. [Temporal checking](temporal.md)
  has the details and limits.
- **Results.** Verdicts, TLC's distinct and generated counts and depth,
  counterexample traces in the `dive-trace` format, and TLC's evaluation-error
  endings where dive can reproduce them.
- **Printing.** `Print`, `PrintT` and `ToString` print what TLC prints, in
  TLC's order. Ordinary in-memory safety checking with `Print` or `PrintT`
  can use several workers. If a worker reaches a print, the coordinator
  evaluates that source again at its place in the search, with the committed
  visited set. This preserves the order and number of messages, including
  prints in checks that only a newly discovered state requires. One-source
  levels run inline. Random draws, checker registers and other ordered modes
  retain their single-worker behavior. These refuse: a run that prints and then finds a violation
  or an error; a print inside a function constructor, filter or set map; and a
  set that TLC may hold as an interval and print as `1..3` (a set of
  consecutive integers, in a specification that uses `..` or `DOMAIN`).

With temporal properties, these refuse by name: `VIEW`, `SYMMETRY`, `Print`,
`RandomElement`, `TLCSet` and `TLCGet`; frontier, candidate or predecessor
spill; disk seen sets; and an explicit `--store-compact-bytes`. A liveness
check that fails instead of answering (a planning or evaluation error, or a
graph limit) also refuses. Temporal checks support `--store exact`.

Source and TLC logical coverage, granted external inputs (`--allow-io`) and
bounded final checking (`--lncheck final --lasso short`) refuse by name and
are left out of `dive check --help`; see the
[command-line surface](README.md#command-line-surface). A checkpoint archive in
an older pre-release format is rejected with `DIVE-S0400` before the model
loads.

## Workers and stores

If the operating system refuses a thread during ordinary safety expansion, Core
keeps the work already claimed and runs the remaining worker jobs on the calling
thread. The report records the highest worker width started for a wave and names
the OS limit when a wave needed this fallback. An explicit `--sequential`
request retains its own reason in the report. A width of one can therefore
occur under a multi-worker request. The search retains its parallel commit order.


Any number of workers gives the one-worker result: the same counts, verdict,
trace and output. Parallel searches commit results in a fixed order. Read
`options.workers.requested`, `used` and `reason` in the report: register reads
and some modes need an inline run, and a requested width does not mean every
step used that many threads. `--sequential` selects inline checking.
Simulation and checkpoint widths have their own limits.

Under TLC value or exploration order, creating strings at run time can also
force an inline run. Sequence concatenation does not, when dive can show from
the source that every reachable call has a tuple operand: literal tuples,
unchanged `Append` results, and transparent calls and branches establish that.
A configured literal value counts as a leaf; unknown state values, configured
native origins, operator overrides and opaque captured providers keep the
inline run. Requested and used widths stay in the report either way.

In temporal checks, `\o` is accepted when the source shows that at least one
operand cannot produce a string, following lexical aliases and every result arm
of an `IF` or `CASE`. dive does not evaluate an operand or trust a state
invariant for this. Both operands still get the usual effect and resource
checks, and a context dive cannot prove refuses with `DIVE-L0033`.

Temporal checking expands a bounded batch on a reusable worker team, then
commits its edges in discovery order. Cheap state transitions can leave little
work to do in parallel. On CoffeeCan3000Beans, eight workers took 49.4 s and
one took 48.8 s after worker reuse. Compare `--workers 1` with the desired
width on the actual model. The report's worker count describes expansion.
Ordered commit and temporal analysis can still dominate elapsed time.

A temporal check whose model has effects or run-time names, such as a constant
drawn with `RandomElement` at start-up, builds its behavior graph with one
worker at any requested width, so it gives exactly the `--workers 1` result.
The report says `"used": 1` and gives the `reason`, and the human report
prints it under the header:

```text
dive check SpanTreeRandom.tla: profile dive, store fingerprint (fp 0), seed 0, 4 worker(s) requested, 1 used
the temporal capture has effects or runtime names (start-up or configured effects): the core captures the behavior graph in order with one worker
```

Fingerprint storage is the default. `--store exact` keeps canonical concrete
keys for supported values; a fingerprint run's collision figures do not turn
into an exact-state proof. Safety checks support ordered frontier spill and
predecessor-log spill under their budgets; disk seen sets and `--chaos` are
not supported here (see [disk seen-state checking](disk-seen-safety.md)).
Inspect `statistics`, `collision`, `counterexample`, `warnings`, `notes` and
the resolved `options` in the JSON report. `core.metrics.store.memory` lists
the known allocations of the state store; its
`known_retained_bytes_lower_bound` leaves out allocator overhead and other
regions, so it is a lower bound. Core reports a permanently unsupported option even
when its startup memory sample already exceeds the requested ceiling.

`dive check --distributed-worker IP:port` (repeated, two or more) sends states
to running `dive worker` processes started with the same build, model files,
`--seed`, `--fp` and a shared `--worker-key FILE`. Results equal a local
one-worker check. Temporal properties, `SYMMETRY`, `VIEW` and runtime effects
refuse by name, and a run whose workers fail stops with exit 3, never a
success. Distribution is slower than local checking for most models. See
[distributed safety](distributed-safety.md).

## Coverage

```sh
dive check docs/user/examples/Coverage.tla --coverage 0 --compat=tlc --seed 7 --fp 0
dive check docs/user/examples/Counter.tla --source-coverage --seed 7
```

dive does not implement TLC's logical coverage (`--coverage 0`, TLC
`-coverage 0`) or source coverage (`--source-coverage`). Each request refuses
by name (verdict `unsupported`, exit 3) in both profiles, and so does
`dive tlc -coverage 0`. Coverage is off by default, which matches TLC; the
same command without the option checks the model. Rust callers that set
`CheckRequest.tlc_coverage` or `CheckRequest.source_coverage` get
`ApiError::Unsupported`.

## Language support

The default profile's
[mathematical comparisons](../tlc-differences.md#d-009-comparisons-that-tlc-refuses-although-tla-answers-them)
answer comparisons TLC refuses. `--compat=tlc` selects TLC's integer, name and
exploration switches; unsupported comparison shapes refuse by name, and dive
does not reproduce TLC's per-object construction history. See
[the compat gaps](../tlc-differences.md#compat-gaps). Unless noted, everything
below holds in both profiles.

### Instances and ENABLED

- An `INSTANCE` substitution keeps its caller's state when an arithmetic
  operator reads it: an instantiated `Double == 2 * v` with `v <- x` reads the
  current `x`. A shared definition reached through two instances is compiled
  per instance, so it can read `x` through one and `x + 2` through another,
  including nested instances and primed definitions in a step property.
- `ENABLED A` inside an action constraint or implied action property searches
  for a successor of `A` from the current state, and primed expressions read
  that query's candidate. The same holds inside callbacks such as `SelectSeq`
  in an invariant that uses `ENABLED` through an `INSTANCE`, even when an inner
  variable shares a name with an outer one.
- Pure, nonrecursive `ENABLED` queries work through `INSTANCE` constant and
  operator parameters when each instantiated variable maps to the root
  variable of the same name, including nested instances. A query body may call
  recursive operators and functions when the compiled plan has no effects and
  no call reaches a nested `ENABLED` or composition.
- These refuse: hidden or renamed instance variables, instance queries inside
  an initial predicate, state-dependent configuration operator overrides,
  query bodies with effects or nested queries, and recursive calls in a plan
  with any effect.
- Split Init/Next roles keep uncaptured top-level operator substitutions, as
  in the Examples `TwoPhase/MCTwoPhase` configuration, which checks with 5
  generated states, 4 distinct states and depth 4. Operator closures whose
  captures dive cannot resolve refuse.

From a source checkout, these minimal models show the instance cases:

```sh
dive check conformance/corpus/reference-instance-naturals/specifications/RefSubstDouble/RefSubstDouble.tla --compat=tlc --workers 4 --seed 0 --fp 0 --output json
dive check crates/dive-api/tests/fixtures/core-m0/EnabStepCache.tla --compat=tlc --workers 4 --seed 0 --fp 0 --output json
dive check crates/dive-api/tests/fixtures/core-m0/EnabConstInstance.tla --compat=tlc --workers 1 --seed 0 --fp 0 --output json
dive check conformance/corpus/core-instance-scopes/specifications/ScopeHolding/ScopeHolding.tla --compat=tlc --workers 4 --seed 0 --fp 0 --output json
```

They succeed with 1, 3, 4 and 3 distinct states (2, 4, 5 and 4 generated;
depth 1, 3, 4 and 3), exit 0. The adjacent `ScopeViolation` model fails its
invariant immediately with exit 12.

### Recursion

- LET-bound recursive functions used through `f[arg]` keep their enclosing
  values across recursive and repeated calls, including `INSTANCE`
  substitutions. An unused failing LET binding or argument is not evaluated.
- A recursive operator or LET function applied under a prime, such as
  `sumList(templates)'` from an `INSTANCE ... WITH` refinement mapping, is
  evaluated in the successor state: its arguments, body and captured locals
  are all read primed. A recursive call passed as an operator argument is
  evaluated in each disjunct's next state.
- An enclosing operator parameter used by a LET-recursive function can be bound
  to a pure nonrecursive operator, including a `LAMBDA` or forwarded parameter.
- Mutually recursive top-level operators keep separate frames, and self and
  mutually recursive operators keep different `INSTANCE` substitutions apart.
- An applied recursive Boolean function can act as a predicate in `Next` when
  its compiled plan has no effects or unknown call targets. dive counts the
  successors it emits and does not repeat identical successors for multiple
  logical witnesses: original VoteProof generates 27,727 states instead of
  TLC's 318,877, with the same 6,962 distinct states, depth 14 and verdict.
  A predicate already proved true also skips later alternatives, so the
  OrError control succeeds where TLC reports a division by zero. Both are
  [D-029](../tlc-differences.md#d-029-predicate-witness-counts) differences,
  and they stay under `--compat=tlc`.
- Calls poll run budgets and refuse beyond 64 active calls instead of
  exhausting the stack.

These refuse by name: escaping recursive function values; a prime or `ENABLED`
inside a recursive definition applied under a prime; an enclosing local read
primed by a recursive definition applied unprimed; mixed plain and primed self
calls; effectful primed functions; mutually nested recursion; mutually
recursive local definitions with lexical captures; general recursive operators
with enclosing operator parameters; substitutions that need effectful or
recursive evaluation or a caller local outside the function; and a callback
that captures a local outside its activation, reaches an effect or an unproved
recursive call, or is applied as an action inside the recursive function.

### Higher-order operators

- `SelectSeq(sequence, Test)'` works when `Test` is pure and depends only on
  its element parameter and constants; the sequence reads the successor state.
  This also works through `INSTANCE` refinement substitutions, for example
  counting messages selected by `msg.type = "pl"`. A callback that
  reads state, captures an enclosing local, reaches an effect or makes an
  unproved recursive call refuses under a prime, as do other higher-order
  operators under a prime.
- A value-only standard operator passed to a higher-order operator, such as
  `FoldFunctionOnSet(+, 0, f, S)` or `FoldSeq(\o, <<>>, s)`, is applied as a
  `LAMBDA` of its arity. An error raised inside it refuses, because TLC reports
  it through a Java method. Built-ins passed to a standard-library fold
  (`FoldSet(\cup, {}, S)`) refuse; the pinned TLC fails on them too.
- A built-in value operator passed to a TLA+ operator, as in
  `ReduceSet(\intersect, S, base)`, is applied as if written in place. This
  covers `\cup`, `\cap`, `\`, `=`, `/=`, `\in`, `\notin`, `\subseteq`, `~`,
  `/\`, `\/`, `=>`, `<=>`, `SUBSET`, `UNION` and `DOMAIN`, with TLC's
  short-circuit connectives, also inside a recursive LET function and under a
  prime. A built-in applied as an action (`Op(\/)` in the next-state
  relation) and the state, action and temporal built-ins (`'`, `UNCHANGED`,
  `ENABLED`, `\cdot`, `[]`, `<>`, `~>`) refuse.
- `DyadicRationals!Reduce` follows the Java override and refuses operands
  outside 32 bits.

From a source checkout, a small refinement example completes with 3 distinct
states, 4 generated states, depth 3 and exit 0:

```sh
dive check crates/dive-api/tests/fixtures/core-primed-selectseq/PureInstance.tla --compat=tlc --workers 4 --seed 0 --fp 0 --output json
```

The Examples `EWD998Chan` model also starts searching with its configuration
and refinement property, given the CommunityModules and dependency library
roots and its adjacent `EWD998` module. A one-second, one-GiB run stops with
exit 5 and no verdict.

### Sets, functions and CHOOSE

- Finite literals and mapped sets are built canonically. Mixed structured
  sets, such as a set holding both a tuple and a record, have an
  order-independent identity and can appear in validated counterexamples.
  Direct undefined scalar comparisons still report errors.
- Compound sets whose nested comparison shapes do not unify refuse, including
  `Cardinality({<<1>>, <<TRUE>>})`. This also covers some sets TLC can sort by
  an earlier field or size. A reversed heterogeneous set may refuse even in the
  default profile, and a forward one can give a report whose structured trace
  then fails decoding. Treat that report as an unvalidated witness.
- When an equality, set element, function key or stored state needs a symbolic
  identity dive cannot resolve, the check refuses. It never infers inequality
  or reports the limit as a specification error.
- Large symbolic powersets, function sets and sequence domains stay lazy.
  Applying a function that TLC keeps lazy, such as `F[a, b \in S] == e`
  applied as `F[x, y]`, evaluates `e` for that argument alone, so a domain such
  as `Nat` and entries whose bodies would fail do not stop the check.
- A lazy filter ignored by `VIEW` is not evaluated: `x = {i \in Nat : Print(i,
  TRUE)}`, `Next == UNCHANGED x` and `VIEW 0` finish with 2 generated states,
  1 distinct state, depth 1 and no output, as in TLC. Spilling or archiving
  such a value refuses.
- `VIEW` normalizes every variable of a concrete state before computing its
  key, including variables the view ignores. A value that cannot be normalized
  refuses, so a state that cannot be printed never becomes a counterexample.
- At startup, `CHOOSE` over finite function spaces with scalar domains and
  ranges generates candidates as needed, with the same choice order,
  cardinality limits, errors and interruption checks.

When constraints exclude every initial candidate, the check completes with
zero distinct states and no counterexample. As in TLC, the depth is one if
Init generated any candidate and zero if it generated none. If Init has states
but the model defines no next-state action, the result is TLC's model error
2115 (exit 75), with no counterexample or statistics, even with deadlock
checking off. A defined action with no successors follows the deadlock
setting.

### Large domains

`x \in lo..hi` in Init and `x' \in lo..hi` in Next stream ascending integers
without building the whole domain, and so does `ENABLED`, which stops at its
first witness. Quantifiers stream interval domains too, but every domain is
still evaluated before the first body, and the first variable varies fastest.
A filtered set over a concrete source evaluates its predicate only as
candidates are needed, so an accepted empty set reaches the action before a
later candidate's failing predicate. A powerset of an empty base yields its
empty subset without normalizing the base. For a closed filter of the form
`{A \in SUBSET B : \A b \in A : P(b)}`, dive runs the whole predicate on the
empty set and on each singleton in order, then builds larger candidates only
from base elements whose singletons passed, in the same order. Powersets, Cartesian
products, function sets and record sets of concrete finite components also
yield candidates one at a time; `SUBSET ({1, TRUE} \X {})` yields the empty
set without comparing the dead mixed column.

Candidate loops poll the run budget even when every candidate is rejected, so
a time limit can stop a search such as `x' \in 0..50000000 /\ FALSE`. A
parallel stop keeps its ordered prefix and leaves the unfinished state queued,
so a later worker's violation cannot jump ahead of an earlier incomplete
expansion. Other constructors can still build a domain before binding, and a
single long evaluation may not be interruptible.

```sh
dive check conformance/corpus/interval-streaming/specifications/WideIntervalInit/WideIntervalInit.tla --compat=tlc --time-limit 1s --max-memory 1g --output json
```

From a source checkout, this fails on `x = 0` over a billion-element domain
(TLC class `TLC_INVARIANT_VIOLATED_INITIAL`, exit 12), with a validated
witness. The rest of the interval is not checked. TLC also enumerates an
`IntervalValue` in ascending order, so `x # x` fails on the first initial state;
a constant `FALSE` invariant instead fails configuration validation before any
enumeration. The `interval-streaming` corpus holds these models.

### Randomness

Ordinary safety checking supports `RandomElement` with a closed pure `VIEW`
and pure startup and Init, including a configured route such as
`CONSTANT C <- RandomElement`. Draws use the `VIEW` value's TLC fingerprint,
and counterexamples keep concrete values. Random checking runs inline and
reports that width beside the requested workers.

```sh
dive check docs/user/examples/Stream.tla --compat=tlc --seed 7 --fp 0 --workers 8 --output json
```

This reports an invariant violation with 4 generated states, 4 distinct
states, depth 4 and values `0,80,94,100`, matching TLC, and discloses an
inline width of one although eight workers were requested.

Values and the `VIEW` value must fit 16,384 stored-value visits and depth 64:
atoms, literal tuples and records, functions with homogeneous atomic domains,
and explicit homogeneous atomic sets. These refuse by name:

- compound or mixed sets, symbolic or deferred sets and nonliteral tuple or
  record representations;
- an unproved `VIEW` or startup context, before any output;
- a random violation on the first successor of an initial state, and startup
  draws before a random Init, because dive cannot replay TLC's stream there;
- configured `Randomization` replacements, `SYMMETRY` with randomness, graph
  capture, temporal random checking and seeded recovery.

A model that replaces a constant with a pure user operator, rather than a
random one, still uses the parallel engine.

A specification counts as random when a definition in a non-standard module,
used or not, reaches a random operator. Extending `Randomization` or `TLC`
without calling one does not count: the standard modules' own helpers, such
as `TestRandomSetOfSubsets`, count only when something outside them calls
them. On the random-witness test set, the
validator accepts 144 of 180 witnesses, refuses 36 whose draw positions it
cannot prove (two draws, a discarded random branch, conditional random Init)
and rejects all 180 with a changed Init.

## TLC checking entrypoints

`dive tlc [OPTIONS] SPEC[.tla]` and the installed `tlc [OPTIONS] SPEC[.tla]`
accept TLC's checking arguments. Any executable named `tlc` or `tlc.exe`,
including a renamed binary or a symlink to `dive`, selects the same adapter.

```sh
dive tlc docs/user/examples/Counter -seed 7 -fp 0 --output json
tlc docs/user/examples/Counter.tla -seed 7 -fp 0 -workers 1
```

The adapter selects TLC evaluation and TLC statuses without `--compat=tlc`
and uses one worker unless `-workers N|auto` says otherwise; `DIVE_COMPAT`
does not change that. It accepts `-modelcheck`, `-config FILE[.cfg]`,
`-seed N`, `-fp N`, `-workers N|auto`, `-deadlock`, `-h` and `-help`,
double-dash aliases (also with inline values) and `--output human|json`.
`dive tlc` also accepts the JVM spelling `-Xmx8g`, or `--max-memory`.

Missing `.tla` and `.cfg` suffixes are added. With an absolute module path, a
relative `-config` is resolved from the module's directory; with a relative
module path, from the current directory. The default configuration is the
module's sibling `.cfg`, and an absolute configuration path stays absolute.
There is no fallback between directories, and a configuration embedded in a
`.tla` file refuses by name.

`-lncheck default` checks eligible temporal properties once at exhaustion,
reports profile `tlc` with final/short metadata and validates the word
independently. `-lncheck final` is left out of the help and, with temporal
properties, refuses by name (exit 3). See
[TLC-profile temporal checking](temporal.md#ordinary-checking-in-the-tlc-profile).

Every counterexample is replayed from fresh source and configuration before it
is reported, and `counterexample.validation` is `valid` only after that.
Replay that dive cannot perform exits 3 and an invalid replay exits 4; both
drop the trace and its violation claim. Startup output and the statistics
observed so far can stay in that JSON report; they do not show a completed
check.

Finite `-simulate num=N` and `-generate num=N` run the matching sampling
algorithm with TLC's exits and validated samples; see
[simulation](simulation.md#constructive-successors). These refuse by name:
`-tool` message framing, unbounded or file sampling, DFID, checkpoint,
storage, debugger, coverage and dump flags. Unknown or malformed arguments
exit 1, and recognized unsupported options exit 3 before reading input. As in
TLC, the configuration is read first: a missing or malformed configuration
exits 151 even when the module is also missing, and a missing module exits
150. Input errors go to stderr. Rust callers get the same order from `dive_api::check_file_tlc_inputs`,
with filenames that already include their extensions. The adapter covers
checking and finite sampling, not the whole TLC toolchain.

## Time and memory budgets

The default in-memory fingerprint store grows one shard at a time. Core keeps
trace parents and successor numbers in a packed log and can still replay a
counterexample from any discovered state. In ordinary parallel safety runs,
each worker handles at most 128 source states per batch before ordered commit.
Core reduces the batch when the previous level generated many successors per source. In local resident safety search the coordinator queues the canonical values that workers already retained. Discovery order and predecessor information are preserved.
See the [matched high-fanout measurements](../developer/large-models.md#high-fanout-parallel-safety).
A wide frontier or a single state with many successors can still require
substantial memory. [Core storage measurements](../developer/core/07-engine.md#resident-storage-measurements).

With `--store exact`, Core grows the resident hash index one shard at a time.
Each matching hash still requires equality of the full state. Peak memory also
depends on the queued states and the values shared between them.
[Exact-store measurements](../developer/large-models.md#resident-exact-state-identities).

Every `dive check` runs under two budgets:

- `--time-limit DURATION|none` bounds wall-clock time for the whole check.
  Durations are seconds by default or take a unit: `500ms`, `90s`, `10m`,
  `2h`. There is no default time limit in either profile, as in TLC.
- `--max-memory BYTES|auto|none` bounds the sampled memory of the whole
  process. Sizes take `-Xmx` units: `512m`, `8g`. `auto`, the default in both
  profiles, is half of physical memory. `none` turns the check off.

```sh
dive check docs/user/examples/Counter.tla --time-limit 2m --max-memory 4g --output json
```

When a budget runs out, the result is `verdict: "stopped"`, exit 5, with a
`stopped` block (resource, limit, observed value, elapsed time, unit, phase and
a hint) and the statistics reached so far. A stop proves nothing about the
model: it is never a success, a violation or `unsupported`. Rust callers set
`CheckRequest::time_limit` and `CheckRequest::max_memory`
(`dive_api::BudgetSetting`). `options.budget` reports each resolved limit and
whether it came from the default, an explicit setting or none. See
[D-026](../tlc-differences.md#d-026-run-budgets-stop-a-run-with-a-structured-result).

The clock starts after the specification and configuration load. dive polls
the budget for each initial and expanded state, inside quantifier, `CHOOSE`
and filter loops, while action composition collects candidates, and during
graph capture, temporal analysis and counterexample replay. This stops with
exit 5 although it never produces a successor:

```sh
dive check docs/user/examples/RejectedCandidates.tla --seed 0 --fp 0 --workers 4 --time-limit 100ms --max-memory 1g --output json
```

The phase tells you where the run stopped:

- `check-startup`: during `ASSUME` evaluation, or when the memory limit is
  already exceeded before binding or once constant evaluation, compilation and
  the `ASSUME`s finish. No search statistics are reported.
- `model-binding`: while constants and roles are bound. Output already printed
  is kept, with no search statistics or used workers.
- `graph-capture` and `trace-validation`: during temporal capture or replay of
  a candidate counterexample. No unvalidated trace is reported.
- `finalization`: the last memory sample or clock check, before a holding
  result is published. Completed progress is kept.

Errors and refusals that hold at any budget come first, and an earlier genuine
error or validated counterexample keeps priority over a later stop. These are
not interruptible: structural binding, symmetry preparation, single value
operations such as allocation and sorting, and rebuilding a diagnostic path
after the result is decided.

Memory is the whole process's: resident plus swapped pages on Linux, resident
pages on macOS, where swapped and compressed memory are not counted. `resources.memory` reports the metric (`rss+swap` or `rss`),
sample count, observed maximum, first and last sample flags and availability.
The observed maximum is not the true peak, and memory can exceed the limit
between samples. If memory cannot be measured, or physical memory cannot be
read when resolving `auto`, checking fails with `DIVE-R0002`; it never assumes
zero or drops the ceiling.

If the host cannot start the budget watchdog, checking does not begin: the CLI
prints `DIVE-R0001` on stderr and exits 4. Free host resources and retry, or pass
`--time-limit none --max-memory none`. `dive_api::check_file` returns
`ApiError::BudgetStartup`, and liveness analysis returns
`AnalysisError::BudgetStartup`.

## State and depth budgets

`--max-states N|none` bounds discovered state identities, using the same
`VIEW`, symmetry and fingerprint or exact identity as the distinct count.
`--max-depth N|none` bounds BFS depth; initial states have depth one. Both are
unset by default, as in TLC, and zero is a valid budget. TLC's simulation
`-depth` option does not bound an exhaustive search.

```sh
dive check docs/user/examples/Counter.tla --max-states 3 --output json
dive tlc --max-depth 3 --output json docs/user/examples/Counter
```

Both stop with exit 5 before the fourth state, with no counterexample. Raise
the ceiling to five to finish the counter.

The first new identity past either ceiling is not admitted, and duplicates do
not count. When constraints exclude every initial candidate, the empty graph
completes even with both ceilings at zero. Parallel workers keep their
requested width and publish candidates in reference order up to the ceiling;
speculative candidates never push the distinct count, trace log or graph past
it. States at the depth boundary can still reach already discovered
states, so a graph exactly at both limits may finish normally. A truncated
search reports `verdict: "stopped"`, exit 5, resource `states` or `depth`, the
attempted count or depth in `observed`, the progress reached, a hint and
`exhaustive: false`. An earlier error or violation still decides the result.
The limits appear in `options.budget.max_states` and `max_depth`. A finite
ceiling labels the TLC profile `tlc+overrides`; `none` restores TLC's setting.
Rust callers set `CheckRequest::max_states` and `CheckRequest::max_depth`
(`Option<u64>`).

With temporal properties, a ceiling reached during capture stops in phase
`graph-capture` with `execution.capture_completed: false`, and never reports
that a property holds. A violation already proven at a completed level is
still reported, and `periodic` counts those checks. A ceiling equal to the
model's size gives the same verdict as no ceiling. Recovery checks that the
saved graph fits the ceilings first.

```sh
dive check Spec.tla --store exact --max-states 100000 --output json
```

Single evaluations and domains built before the first state still need the
time or memory budget; state and depth limits do not interrupt them.

## Process statuses

`dive check` uses TLC's process statuses by default in both profiles:

| Result | Status | With `--exit-codes native` |
| --- | --- | --- |
| Success | 0 | 0 |
| Invariant violation | 12 | 10 |
| Initial or action property, or liveness, violated | 13 | 10 |
| Deadlock | 11 | 11 |
| False assumption | 10 | 12 |
| Evaluation error | TLC's code for the error (255 for division by zero) | 13 |
| Rejected specification | 150 | 1 |
| Rejected configuration | 151 | 1 |
| Usage error | 2 | 2 |
| Unsupported or resource limit | 3 | 3 |
| Internal error | 4 | 4 |
| Stopped by a run budget | 5 | 5 |

```sh
dive check docs/user/examples/Counter.tla --seed 7 --workers 1 --output json
dive check docs/user/examples/Counter.tla --seed 7 --workers 1 --exit-codes native --output json
```

`--exit-codes native` and `--exit-codes=native` both work, and the last one
given wins regardless of `--compat` or `DIVE_COMPAT`. Native statuses with TLC
evaluation are labeled `tlc+overrides`. JSON adds `exit_code`,
`options.exit_codes` and `options.semantic_profile`; `options.profile`
includes overrides. Human output shows the selected convention and status.
The `tlc` object describes the deciding error class whichever convention you
pick, and `verified: false` stays unverified. This applies only to `check`;
other commands keep their own statuses.

Rust callers keep `CheckReport::exit_code()`, `to_json()` and
`render_human()`, or pick a convention with `ExitConvention::Tlc` or `Native`
through `exit_code_for`, `to_json_with_exit_convention` and
`render_human_with_exit_convention`. Changing the presentation does not rerun
the check.

A few TLC error codes need care:

- Under TLC comparisons, a tuple or record equality error in an initial
  invariant can report `TLC_FINGERPRINT_EXCEPTION` (2147), exit 75. The
  recorded initial state is context, not a counterexample, and appears in
  `diagnostic_witness`. The default profile keeps its mathematical comparisons.
- For errors during Init, `tlc.code` can stay GENERAL (1000) while
  `tlc.exit_status` is 75, because TLC reports the original exception and then
  sets the status from its diagnostic replay. The Init site and where an
  override came from tell these cases apart; the error text alone does not.
- A division by zero, or values that cannot be compared, while preparing a pure
  `SYMMETRY` set reports `model-error`, TLC GENERAL (1000), exit 255 (1 with
  `--exit-codes native`), before any state is explored. In the default
  profile a mixed set can give a different explanatory message with the same
  class. Effectful,
  state-dependent and parameterized symmetry declarations refuse.
- The finite temporal 2116/exit 12 runtime case refuses; TLC statuses do not
  change that.

```sh
dive check conformance/corpus/comparison-receiver-primary/specifications/InitialStateLeft/InitialStateLeft.tla --compat=tlc --seed 19 --fp 0
dive check crates/dive-model/tests/fixtures/symmetry-group/specs/SyDivZero/SyDivZero.tla --compat=tlc --seed 0 --fp 0 --workers 1 --output json
```

## Tuning switches

`--core-sample N` compares the successors of `N` expanded states, chosen by the
seed, with an independent evaluator's, and writes `core.sample` in the JSON
report. It keeps each sampled state's level for `TLCGet("level")`. It changes
no result and has no TLC equivalent.

`DIVE_CORE_OPT` (for example `-fold,-delta-hash`) turns individual strategies
off (`-name`) or on (`+name`) for measurement; results must not change.
`DIVE_CORE_OPT=-member-index` turns off the hash index that the default
profile builds for repeated membership tests on sets of at least 16 members
(at most 4 MiB per worker, or a sixteenth of its memo allowance). TLC
comparisons never use that index. In the TLC profile, `-memo` also turns off
reuse of set-membership proofs.

## Embedding notes

These apply to Rust callers below `check_file`:

- `dive_model::bind_controlled` and `Recipe::bind_controlled` bind under the
  run's control; the boxed `bind` and `Recipe::bind` do not, and cannot carry
  an interruption. Use the controlled entry points or `Recipe::bind_fallible`
  to get the original cause of a binding failure.
- Low-level `capture::*` functions expect the caller to arm a budget with
  `budget::arm`, or to clone `watchdog.budget()` into the options, and keep the
  watchdog through capture and analysis. `Budget::start` returns `Result` and
  does not change the original budget; an unarmed budget never runs out.
- `capture::explore_checkpoints_with_evidence` exposes the Init and
  completed-row checkpoints of a one-worker capture and refuses parallel
  requests. Its diagnostic path is not a validated counterexample.
- The detailed temporal-failure APIs keep a state atom's typed cause apart
  from its predicate source; `state_evaluation()` says whether the failure
  came from final graph labels, a periodic check or attribution. A
  `CaptureRow` site comes with separate `capture_row()` evidence.
- The evaluator's `set_host` and `replace_host` always call the host's
  permission callbacks. `native_hook_scope` can select `Unrestricted` only for
  a host in shared cache domain zero; otherwise it returns `None`. A caller
  using a scope must own host replacement.

For temporal-specific errors, row errors and limits, see
[temporal checking](temporal.md). For debugging a safety check, see
[debugging](debugging.md). See also [compatibility](compatibility.md),
[storage](storage-recovery.md), [reports](reports-and-errors.md) and the
[coverage scoreboard](../scoreboard.md).
