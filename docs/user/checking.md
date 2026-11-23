# Safety checking

`dive check` explores every reachable state of a model breadth first and checks
its invariants, action properties, constraints and deadlock. It reads the
sibling `.cfg`, or the one you pass with `--config FILE.cfg`.

```sh
dive check docs/user/examples/Counter.tla --seed 7 --workers auto --output json
dive check docs/user/examples/Counter.tla --compat=tlc --seed 7 --fp 0 --workers 8 --store exact --output json
```

Use [`--strategy auto`](symbolic.md#choose-a-strategy-automatically) to select a representation for the bound model.

For finite function and set models, the optional [symbolic safety checker](symbolic.md) can reuse a complete reachable set or prove invariants by induction.

For message types written as unions of finite record sets, Core can check
membership directly against the fields. This also works inside a containing
record or a powerset, such as `MessagePool \in SUBSET Message`. Asking for the
elements of the type still requires actual enumeration. The [LuPastry+ example](../developer/large-models.md#lupastry-message-and-leaf-domains)
shows how to distinguish a large type from a large set of reachable states.

When an action chooses messages from a queue, Core can enumerate only those
addressed to the receiver. For example, this guard permits two choices if a
40-message queue contains one matching message.

```tla
\E selected \in SUBSET inbox :
    /\ \A m \in selected : m.destination = receiver
    /\ Deliver(selected)
```

The empty set is one choice. Every subset of matching messages is considered,
and the checker evaluates the original action for each one. The guard must
come first and compare pure values independently of the selected subset.
The number of possible choices therefore grows with the matching messages.
[Measurements and replay controls](../../conformance/reports/perf/2026-11-21-guarded-message-subsets.json).

For `CHOOSE s \in SUBSET A : P(s)` where `A` is an integer interval or an
explicit set of more than 24 distinct integers, Core can return the first qualifying empty set or singleton. It visits
singletons in base-element order. If none qualifies, the checker reports that
the search needs larger subsets. TLC integer mode requires the base size to
fit a signed 32-bit integer. [LuPastry+ neighbour choices](../developer/large-models.md#lupastry-neighbour-choice-at-32-and-64-addresses).

Core can split a next-state action such as `\E i, j \in Nodes : Send(i, j)`
into one action per pair. When the pairs bind the same slots in a pure
expression, they share compiled instructions. Each pair still has its own
parameter values, action label and guard memo statistics. Large sets of pairs
can share the same instructions close to the compiler’s constant-folding limit
when the remaining allowance covers the recorded work with fuel to spare.
[LuPastry+ measurements](../developer/large-models.md#lupastry-parallel-invariant-checking).

## Check paths through a transition bound

```sh
dive check APtcp.tla --config APtcp.cfg --strategy bounded-path --path-bound 5 --output json
```

This mode checks the configured invariants on Init at depth zero and every
reachable prefix through the requested number of Next transitions. Its layers
preserve a state reached again at another depth. It uses the bound source
operators and checks the source assumptions. It independently validates a
counterexample and reports the depth at which the invariant failed.

Transition checking uses explicit INIT and NEXT. Repeated states and explicit
stuttering transitions in Next count toward the bound. A state with no
admissible successor ends that prefix. If every prefix ends before the bound,
the result is `executions-too-short`. A reached bound with no violation yields
`bounded-safe` and states the bound in `path_bound`. Both results have
`model_proved: false`. Configurations asking for temporal, symmetry or explicit
deadlock checking require their corresponding checking mode.


For a single configured invariant that negates a conjunction, Dive can
check an Init made of independent closed finite assignments without
enumerating their Cartesian product. It checks each clause on all the
values the clause reads before using the clause to prune a partial
assignment. This preserves demanded evaluation errors. A clean depth-zero
query covers every initial assignment. A violation yields an independently
replayed initial word at any requested bound. A resolved pure
`SPECIFICATION` can supply Init for this query. Later steps under a
`SPECIFICATION` with no initial violation require a supported transition
route. The JSON report records domain sizes, definedness checks and search
work under `initial_query`.
[Einstein and permutation controls](../../conformance/reports/perf/2026-11-23-bounded-initial-assignments.json).

## Reuse a finite symbolic relation across path bounds

Linux builds with the symbolic worker can reuse one bound relation across
multiple exact-depth path queries. Select the configured invariant that
declares the finite layout of all state variables.

```sh
dive check MCKVS.tla --config MCKVSSafetySmall.cfg \
  --strategy symbolic-path --type-invariant TypeInvariant \
  --path-bound 5 --path-bound 15 --k-induction 2 --output json
```

Each requested bound records whether an execution can reach that length and
whether any prefix through that length violates a configured invariant. Only
actual `Next` steps contribute to the length. A supported `SPECIFICATION`
can also permit full-state stuttering. Stutters implicit in that formula do
not contribute to path length. Stuttering written in `Next` does contribute.
A state filtered by a state constraint is still checked on emission. Only
admitted states can continue a path.

`--k-induction K` checks the base through `K` transitions and a separate
step from arbitrary admitted valuations satisfying the invariants at the
preceding `K` positions. Only two discharged obligations yield
`inductive-safety` and `model_proved: true`. A satisfiable step yields
`induction-inconclusive` with its last valuation. A reachable violation
gets an independently validated trace and can be written with `--trace`.

`--write-path-receipt FILE` records the bound query and its result when the
worker completes without a reachable violation. `--verify-path-receipt FILE`
requires the same source, CFG, exporter, layout, bounds and induction depth
and independently repeats the check. Temporal and implied obligations,
symmetry, action constraints and explicit deadlock checks currently refuse
with a reason. The strategy uses the normal symbolic time and memory limits.

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

## Search for a fair counterexample from a cold start

```sh
dive check Elevator.tla --config ElevatorLivenessLarge.cfg --lncheck cold-fair \
  --seed 0 --periodic-min-states 1500000 --time-limit 360s --max-memory 8g --output json
```

Cold fair search chooses the first admitted state emitted by the original
Init and explores its exact reachable states. It evaluates the original
fairness assumptions and configured properties. Independent replay checks
every reported counterexample against the original source and configuration.

Add `--cold-roots 8 --cold-init-emissions 256` to explore several distinct
roots in the same exact graph. The selector tries the first two distinct
admitted roots near the start of Init and uses seed-dependent gaps for later
roots. The emission limit includes duplicates and candidates skipped between
windows. Selection, capture and independent replay share the time and memory
budget. The default with both flags omitted selects only the first root.

`initial_roots` reports the original emission ordinals, the selection cost
and the work attributable to each root. If two roots reach the same state,
its exploration work is assigned to the root of its first discovery path.
Root ordinals and counts are replayable with the same seed and completed
search boundary. A wall-time stop can occur at a different boundary.

The JSON report identifies the selected-root scope. Exhausting that graph
without a witness reports `bounded-inconclusive`. A budget stop reports the
limiting resource. Ordinary `dive check` explores all admitted initial states
and keeps its own coverage counts.


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

Core can generate and check large finite initial products on several workers.
It admits the initial states in the original order. The JSON report exposes
`core.metrics.parallel_init_workers` for runs that use this path. Use
`--sequential` to enumerate Init inline when worker buffers would exceed your
memory budget. Time and memory stops report the ordered prefix that the
explorer processed.

Ordinary multi-worker safety checking starts a worker team at its first parallel
expansion and keeps the same threads through later levels. If the operating
system refuses the team at startup, the caller runs the worker jobs for the rest
of the search. The report records one worker used and gives the OS limit as the
reason. A single-worker run with candidate spilling can also finish a refused
worker job on the caller. An explicit `--sequential` request retains its own
reason. Each wave commits its results in the same order.

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

### Finite predecessor function equations

Core can construct the function in an unbounded `CHOOSE g : g = [n \in D |-> e]`
when `D` is a finite domain of small integers and any self-lookup in `e` is exactly
`g[n - 1]`. It builds entries in ascending order and reports a diagnostic when
a demanded predecessor is missing or an entry is undefined. The unchanged
LuPastry+ routing-table definition uses this form. The TLC comparison profile
reports the unbounded-CHOOSE refusal. [Compatibility details](../tlc-differences.md#d-046-finite-function-equations-with-strict-predecessors).

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
Core can also reuse a pure computed assignment across successors that agree on every source and successor component it reads. This benefits history tables and other derived state. [Computed successor components](../developer/core/06-memoization.md#24-definitions-a-next-state-action-reads-primed).

A wide frontier or a single state with many successors can still require
substantial memory. [Core storage measurements](../developer/core/07-engine.md#resident-storage-measurements).

A parallel worker uses a bounded cache for both action guards and state invariants.
When the initial tables for all sites exceed that cache, a pure plan reserves
the invariant tables first. This lets workers reuse a checked invariant across
duplicate candidates even when the model contains thousands of action guards.
[LuPastry+ measurements](../developer/large-models.md#lupastry-parallel-invariant-checking).

Many process models select the next action with a string guard such as
`pc[p] = "idle"` or `workers[p].stage = "idle"`. During exhaustive search,
Core can reuse the selected string across alternatives that read the same
table entry. A false leading guard can rule out an action before Core enters
its frame. Enabled alternatives retain their order and multiplicity.
[Flint and Bakery measurements](../developer/large-models.md#reproduce-the-flint-three-writer-model).

With `--store exact`, Core grows the resident hash index one shard at a time.
Each matching hash still requires equality of the full state. In parallel resident
safety search, workers check step properties even on transitions to previously
visited states. Once a worker confirms full equality with an earlier committed
state, it can skip repeating that state’s invariants. Peak memory also
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

#### Count labeled states under symmetry

For a finite safety model with `SYMMETRY`, run
`DIVE_CORE_OPT=+orbit-census dive check M.tla --config M.cfg --store exact --output json`.
An exhausted search reports `core.metrics.orbit_census`. The `orbit_sizes` histogram
maps each orbit size to the number of retained representatives of that size.
Multiplying each size by its frequency and summing gives `labeled_orbit_union`.
For a model whose initial states, transition relation and constraints are preserved by
the declared permutations, this is the exact number of reachable labeled states.
The checker counts the orbits. The model author establishes the symmetry assumption.

`weighted_generated` adds the actual initial occurrences to the successor occurrences
of each expanded representative multiplied by its orbit size. It recovers ordinary
unreduced generated counts when relabeling also preserves each successor's multiplicity.
Both totals are decimal strings. Ordinary `statistics` still describes the reduced search.
The census supports groups of order up to 64 in local exact safety search without
`VIEW` or checkpoint recovery. Partial searches publish no completed census.
[SlateDB controls and commands](../developer/large-models.md#reproduce-the-released-slatedb-wal-model).

#### Compare a declared carrier symmetry on the original files

The source-checkout driver takes an unmodified TLA+ module and configuration. Choose a constant bound to two, three or four distinct untyped model values that you expect to be interchangeable. This example uses the original TCommit specification and its three resource managers.

```sh
python3 scripts/perf/scale/checked_orbit.py TCommit.tla \
  --config TCommit.cfg --carrier RM --dive /path/to/dive --workers 8
```

By default the driver runs two exhaustive exact checks on private copies of the inputs. One uses a generated module extending the original with all pair transpositions of the carrier. The other checks the original files directly. A successful `verified-observables` result means the original reached a successful verdict and its distinct count, generated count and depth matched the expanded orbit totals and quotient depth. This comparison applies to the listed source closure, configuration and properties. Different labeled graphs can have the same counts. On failure, each reported counterexample is replayed against the original module and configuration.

Use `--assume-equivariance` for a single reduced check when the model author establishes that relabeling preserves the initial states, transitions, constraints, properties and transition multiplicities. All relevant expressions must remain defined independently of enumeration order. Dive checks that adjacent swaps preserve every configured constant. Its resolved-source observer audit checks for effects, opaque operators and `CHOOSE` expressions whose selected witness may distinguish two labels. For example, `CHOOSE w \in Writers : TRUE` refuses for a nontrivial writer symmetry. The preflight covers the lowered user-file definitions and can also refuse an unused definition. A successful reduced check in this mode reports `conditional-orbit`.

`--prepare-only` returns an unsigned input manifest with the module closure hashes, configuration digest, exact adapter source and configuration suffix. A separate client can verify the original bytes and submit the assembled request through its own checker. The receiver needs its own authority for any cached verdict. A source or configuration edit requires preparing the current inputs again. `dive analyze MODEL.tla --orbit-observers --output json` exposes the preflight's choice domains and refusal locations.

Exit 0 means the selected comparison or certificate succeeded. Exit 1 carries a replayed counterexample, exit 2 means refusal or disagreement, and exit 3 identifies a prepared request or a successful conditional result. `--seconds` limits each check, and `--max-memory` sets the checker's memory limit. [Flint and Lamport controls](../../conformance/reports/perf/2026-11-22-checked-orbits.json)

`DIVE_CORE_OPT` (for example `-fold,-delta-hash`) turns individual strategies
off (`-name`) or on (`+name`) for measurement; results must not change.
`DIVE_CORE_OPT=-member-index` turns off the hash index that the default
profile builds for repeated membership tests on sets of at least 16 members
(at most 4 MiB per worker, or a sixteenth of its memo allowance). TLC
comparisons never use that index. In the TLC profile, `-memo` also turns off
reuse of set-membership proofs.

### Certify interchangeable model values

Use `--certify-equivariance graph` when you need evidence that every reachable labeled transition survives renaming. This source-checkout driver supports pure safety models with two to four distinct untyped model values in a literal configured carrier. The graph method handles ordinary compound values, translated process tables, duplicate transitions, and configured invariant and step obligations. It requires a complete original search within the supplied limits. Constraints, prior reductions and liveness yield a refusal.

Build the helper from the same checkout as the checker, then run it on the original module and CFG. For example, from the checkout root with the released transaction-commit files in `models/`:

```sh
cargo build --release --locked -p dive-cli --bin dive
cargo build --release --locked -p dive-api --example orbit_graph --example symbolic_input
python3 scripts/perf/scale/checked_orbit.py models/TCommit.tla \
    --config models/TCommit.cfg --carrier RM --certify-equivariance graph
```

The first proof runs both the original safety check and an exact capture of its reachable graph. For each adjacent carrier swap it verifies that every state maps into the graph, that the mapping is its own inverse, and that each initial state and each mapped successor occurs with the same multiplicity. Checking each outgoing multiset also covers swaps that fix the source. The fresh reduced search must agree with the original on verdict, expanded distinct and generated counts, and depth. A violation carries a word independently replayed against the original files. Stops, diagnostic disagreement and failed conjugacy yield exit 2. Successful certification returns exit 0 and `status: certified-orbit`.

For a model with a supported finite layout, select `--certify-equivariance finite --type-invariant TCTypeOK`. Install the optional [symbolic checker dependencies](../../scripts/symbolic/requirements.txt) in the Python environment that runs the driver. This method proves the finite initial occurrences, guarded updates with their multiplicities, invariant predicates, and supported boxed step predicates commute with the generators. It also checks the original directly on first use to preserve observable diagnostics. Ambiguous whole-function comparisons, unconstrained state coordinates, and unsupported value shapes or operators refuse by name. Use `--proof-memory-mib` to bound the helper address space and `--seconds` to bound each individual proof or check.

To reuse a proof in another local process, create a private 32-byte key once and retain it under your control. Python 3.11 or newer is required.

```sh
export DIVE_ORBIT_PROOF_KEY="$PWD/orbit.key"
python3 -c 'import os,sys; f=os.open(sys.argv[1],os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600); os.write(f,os.urandom(32)); os.close(f)' "$DIVE_ORBIT_PROOF_KEY"
python3 scripts/perf/scale/checked_orbit.py models/TCommit.tla \
    --config models/TCommit.cfg --carrier RM --certify-equivariance graph \
    --certificate-cache "$PWD/orbit-proofs"
```

Run the final command again to see `certificate.reuse: authenticated`. Every invocation rebinds the source and runs the reduced search. The HMAC covers the complete source and CFG identity, group, checker, proof implementation, worker count, and proof result. An edit requires a new proof. Undo can reuse the original. A damaged receipt or a different private key causes reproof. Possession of the key grants authority to sign a reusable proof. Give another checker the prepared input bytes and let it establish its own verdict and cache authority. [Complete edit cycle, memory and separate Sail measurements](../../conformance/reports/perf/2026-11-22-certified-orbits.json).

### Build a source-bound native successor worker

For repeated checks of a growing program, the Linux source checkout includes an experimental compiler for Core's successor control flow. It accepts an unmodified module and configuration. Each generated worker is an executable with the ordinary `dive check` interface. It handles guards, assignments, scopes and finite existential choices using the current Core evaluator. Unsupported generator nodes continue in the ordinary generator, including pending conjunctions. Exact and fingerprint storage retain their usual guarantees.

Preparation needs Python 3.11+, the project's Rust toolchain, Git, tar and strip. Give it a private build cache outside the checkout. The first preparation copies the committed Dive tree and builds a bootstrap checker and a model-specific executable for the current Linux platform. On the measured Flint run, preparation took 38.39 s and peaked at 1.51 GiB of charged process-family memory. The normal installed checker starts without this build.

```sh
python3 scripts/perf/scale/native-successors/prepare.py Model.tla \
  --config Model.cfg --cache "$HOME/.cache/dive/successor-builds" --jobs 4 > prepared.json
worker=$(python3 -c 'import json; print(json.load(open("prepared.json"))["binary"])')
"$worker" check Model.tla --config Model.cfg --workers 8 --seed 0 --fp 0 --output json
```

Rerun preparation after an edit. It resolves the current input, exports a fresh bound plan before exploration and checks whether the same generated instructions still apply. The receipt records the original module hashes, configuration hash, kernel identity, build time and executable checksum. Local builds and cache manifests are trusted. Colocated source modules and pure Core action plans are supported. Oversized, effectful or unavailable plans produce a named refusal from preparation.

At check time, the executable compares the entire bound generator structure with its compiled shape. A different shape, debugging, unbounded simulation or a non-exhaustive sink uses ordinary traversal. Current predicates and values are evaluated anew even when the control shape still matches. A fresh search reconstructs the graph, checks the configured obligations and retains their diagnostics. For a matched control set `DIVE_DISABLE_NATIVE_SUCCESSORS=1`. For a separate coverage run, `DIVE_NATIVE_SUCCESSOR_METER` can name a fresh file recording power-of-two counts of calls to compiled actions. A separate Sail installation accepts the underlying TLA+ inputs and maintains its own cache.

Set `DIVE_EXPERIMENT_PROJECTED_ACTION_ROWS=1` to try projected fairness
labels on eligible single-worker checks. A source with too many original
edges or an unproved result uses ordinary evaluation. Earlier source labels
remain available. Check the `projected_rows` statistics for the work this
option covered and the work it deferred.

Flint's property edit completed in 3.30 s including preparation, against 23.10 s for compiled tla-mc. Unchanged warm search on the same labeled graph took 3.07–3.18 s against 1.49 s. [Original and edited measurements, fallback and trace controls](../../conformance/reports/perf/2026-11-22-source-native-successors.json).

Ordinary `dive check` recognizes repeated updates to distinct fields of the same finite actor record, such as `[w EXCEPT ![a].pc = next, ![a].seen = stamp]`. It preserves source order, nested paths and the old value bound to `@`. A missing key, a repeated field or a different runtime shape uses ordinary evaluation. The original Flint check took 2.94–3.02 s with this lowering, and the optional compiled worker took 2.62–2.63 s plus its binding check. Compiled tla-mc remained faster on unchanged warm checks. [Full equality, growing models and edit-cycle measurements](../../conformance/reports/perf/2026-11-22-actor-table-rows.json).

The prepared worker can read one actor label for several action alternatives, pass the checked position into an update, iterate small closed scalar domains, and update only the changed cells of a guarded table of sets. Each new process binds those operations to its current source and configuration. A missing field, different domain or unrecognized value operation uses the ordinary evaluator. The Flint guard edit took 36.25 s including preparation and search against 43.28 s for the peer's edited source, with both checking all 7,387,957 states. The already compiled peer completed the same check in 22.78 s. [Matching cold and warm runs and exact-store comparisons](../../conformance/reports/perf/2026-11-22-source-value-blocks.json).

To see which operations run, set `DIVE_NATIVE_BLOCK_METER` to a new writable file when invoking the prepared binary. It records a lower bound for each operation at powers of two. For a same-worker comparison, set `DIVE_DISABLE_NATIVE_BLOCKS=1` to use the previous generated continuations, or `DIVE_DISABLE_NATIVE_SUCCESSORS=1` to use the ordinary Core generator.


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


### Consistency predicates over named processes

Independent trace validation can check a common consistency invariant in one scan of the concrete table. This invariant excludes a committed manager whenever any manager has aborted.

```tla
Consistent == \A a,b \in Owners :
    ~ /\ status[a] = "aborted"
      /\ status[b] = "committed"
```
The evaluator checks that the bound names are independent, both lookups read the same table, and the values used for the reduction have the expected primitive type. Partial tables and unsupported comparisons follow the usual evaluation rules. Counterexamples still undergo full initialization, transition and property validation. [Measured replay costs](../developer/large-models.md#independent-replay-on-large-owner-tables).

When replay checks a step such as `status' = [status EXCEPT ![i] = "prepared"]`, many candidate processes share the same source and target. For concrete tables with comparable keys and primitive entries, Dive compares the two tables once. A repeated guard such as `\A i \in Owners : status[i] # "committed"` can also share its result during that check. Other guards, updates and ending properties still run in source order. On Transaction Commit the validator rejected two simultaneous prepares among 65,536 managers in 0.80 s. [Trace construction and measurements](../developer/large-models.md#independent-replay-on-large-owner-tables).
