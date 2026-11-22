# Temporal checking

`dive check` checks temporal properties, with fairness, on finite models
without effects. The default profile checks at completed BFS levels as the
search grows; the TLC profile checks once, after exhaustion. Every
counterexample is replayed independently before it is reported.

When the planner reaches a formula construction bound, the Core file checker reports
`DIVE-L0021`, the source predicate location and the exhausted bound. For example,
`predicate lowering steps` counts work expanding and checking scoped operands,
including fairness under finite quantifiers. The run returns `unsupported`
before a model verdict.

Under quantified fairness, Core reuses completed level and purity checks for
first-order helper calls with the same lexical binding structure when the check
is independent of concrete scalar values. A constant argument that can fail for
a particular value is still evaluated in that binding, even if the helper ignores
it. Each fairness atom keeps its original scope for evaluation on states and edges.

In an empty directory, write the counter from
[getting started](getting-started.md) and a configuration that checks one
temporal property:

```sh
cat > Counter.tla <<'TLA'
---- MODULE Counter ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = IF x < 4 THEN x + 1 ELSE x
Bound == x <= 4
StrictBound == x < 4
EventuallyFour == <>(x = 4)
====
TLA
cat > CounterTemporal.cfg <<'CFG'
INIT Init
NEXT Next
PROPERTY EventuallyFour
CFG
dive check Counter.tla --config CounterTemporal.cfg --workers 1 --seed 7 --output json
```

This returns `liveness-violation` for `EventuallyFour`, exit 13, with 5
distinct states, 6 generated states and depth 5. `Next` always increments
before four, but a TLA+ behavior can stutter at zero and the model has no
fairness condition, so the one-state word that stutters at `x = 0` is a real
counterexample, with `counterexample.validation: "valid"`. Adding
`--compat=tlc` gives the same result through final-only analysis.

For temporal checks, compare one worker with the requested width. Core
expands and commits rows on one thread when Init produces at least 4,096
distinct roots and the compiled plan has one variable and at most eight
short, non-enumerating action branches that pass its structural check.
This avoids dispatching each small expansion to workers before its ordered
commit. The JSON report gives the requested and actual width and the reason
in `options.workers`. The `core.temporal_workers` section of multiworker reports explains the execution policy. `width_applies_to`
identifies source expansion. `serial_phases` lists the phases that use one
thread when reached, including ordered commit and temporal analysis. See [workers and stores](checking.md#workers-and-stores).

In an ordinary Core JSON report, `core.metrics.liveness.analysis_ms` measures
elapsed milliseconds per serial analysis phase. `atoms` covers labeling states
and edges, including fairness predicates. `product` covers product-graph
construction, accepting-cycle search and reconstruction of a candidate word.
`attribution` measures which properties fail on that word. The report includes
only phases entered and sums repeated calls across properties and periodic
checks. A periodic check runs during capture, so its time is already included
in `core.metrics.explore_ms`. Independent trace replay happens after analysis.

On the baseline Elevator Large run the product phase took 399.92 of 404.35 seconds. On Huang,
labeling took 10.94 of 19.37 seconds at eight workers. The
[phase measurements](../../conformance/reports/perf/2026-11-21-temporal-analysis-phases.json)
include the matching counts, host, and commands.

Fairness can make accepting-cycle search dominate the runtime. On Elevator Large, reusing shortest prefixes, reading single fairness literals directly and retaining small product rows brought wall time from **403.59 to 113.90 seconds**. Both runs exhausted 50,653 states at depth 39 and agreed on all 230,899 generated occurrences. The peak RSS moved from 121 to 129 MiB. Elevator Medium fell from 5.86–5.87 to 1.84–1.85 seconds. Large products continue to compute edges on demand. [Matched comparison, intermediate versions and controls](../../conformance/reports/perf/2026-11-21-fairness-product-search.json).

Under `--compat=tlc`, temporal capture can use every requested worker when the
model creates no names beyond its start-up name table. From a source checkout:

```sh
dive check crates/dive-api/tests/fixtures/core-liveness/FixedNames/TupleSubSeq.tla --compat=tlc --workers 8 --seed 0 --fp 0 --output json
```

This tuple `SubSeq` cycle succeeds with 3 distinct states, 4 generated states
and depth 3, as TLC does at one worker, and the default profile gives the same.
No list of models or operator names is involved: a name missing from the
start-up table, or one created inside a temporal atom, refuses before any
result is reported.

## Reading the result

A violation's `properties` lists every configured property that is false on
the reported counterexample; `property` and the human headline format the same
list. A different valid lasso can violate a different set, so dive does not
copy TLC's set onto a word on which one of those properties holds.

The counterexample is a deterministic short lasso. dive replays it in memory
and as a lossless document against a freshly bound model, checking the
original `Next`, the complete property claims, full behavior fairness and the
closing edge. A replay that fails or is interrupted publishes no violation.
TLC's own lasso choice depends on hash-table and SCC visiting order, which
dive does not reproduce, so `--lasso tlc` is a usage error.

Exit statuses under TLC's convention: success 0, temporal violation 13, a
named refusal or resource limit 3, a budget stop 5, and evaluation errors
their TLC status. Read the verdict and `execution` stage as well as the exit.
All reports use schema 4. `check_kind: "temporal"` names the report family;
it is not evidence that capture completed. See
[reports and errors](reports-and-errors.md).

When the configuration has no `PROPERTY` entries, a temporal option changes
nothing: the run is an ordinary safety check with no temporal claim. A
`PROPERTY` that binds entirely to initial predicates, invariants and implied
actions is also checked as safety, in the same pass, with its scope, order and
name. A state-only `PROPERTY P` is checked in the initial state, and `[]P`
becomes an invariant. A formula such as `[]<>P` goes through temporal
checking. Fairness alone does not make a run temporal.

## Ordinary checking in the dive profile

With the profile's default evaluation, resident fingerprint storage and no
storage, deadlock, sequential or chaos overrides, `dive check` reports
`options.lncheck: deterministic`. After each fully committed BFS level whose
frontier is not exhausted, dive checks the graph so far when it has at least
65,536 (2^16) distinct states and at least twice as many as at the last check.
The doubling keeps the total cost of these checks to about one final check,
and the schedule depends only on the graph, so results are the same at every
worker count. A model with fewer than 65,536 states is checked once, at the
end, with the same verdict and counts as a final-only check.

A check of a prefix can only find a validated violation; it never shows that a
property holds. It searches only the first configured property, because a
violation of it on a prefix is also one on the complete graph, where the final
check would name it first. Later properties wait for the final check, so the
reported property is the one a final-only check reports. A violation found on a
prefix stops the run there, skips postconditions and reports the counts with `capture_completed: false` and `exhausted: false`.

Use `--lncheck periodic-all` to search every configured property at each due
level. For example, if `Termination` holds on the prefix and `Live` already
has a fair counterexample, the run can report `Live` there. The checker
searches in configuration order, evaluates every configured property on the
selected word and independently replays it. A prefix without a witness
continues exploring. This mode can spend more time at a level when several
properties hold. It also selects the deterministic schedule under
`--compat=tlc`. The report includes `options.periodic_properties: "all"`.
Rust callers select `TemporalMode::PeriodicAll`.

Like TLC's own periodic checking, a violation found on a prefix can preempt an
invariant violation or evaluation error beyond that prefix, which a final-only
run would report instead, and it stops with fewer states than a final-only
run. This is a [registered difference](../tlc-differences.md#d-014-periodic-liveness-checking-has-a-replayable-schedule);
`--compat=tlc` gives TLC's final-only behavior.

The report's `periodic` object holds the schedule (`min_states`,
`growth_factor`), the number of prefix checks, the last checked level and size,
exhaustion and whether the final check ran. A run stopped by its budget keeps
these facts, including zero checks when no level was due. A final-only run
reports `lncheck: final` and has no `periodic`. Rust callers can choose another
schedule with `CheckRequest::periodic` (`dive_api::PeriodicSchedule`); the CLI
always uses the default.

## Ordinary checking in the TLC profile

With the two files from the start of this page:

```sh
dive check Counter.tla --config CounterTemporal.cfg --compat=tlc --fp 0 --seed 7 --output json
```

Under `--compat=tlc`, dive finishes exploring the graph, then checks temporal
properties once with TLC's value semantics. No lasso flag is needed. The report
records `lncheck: final`, `lasso: short` and the temporal bounds, with profile
`tlc`. TLC's SCC, lasso, timing and queue order are not reproduced.

This requires the profile's default evaluation and no storage budget,
directory or chaos options. Any `--store`, `--fp` polynomial, `-deadlock` and
`--sequential` setting is accepted: capture keeps concrete states, so neither
the store nor the polynomial changes the result, and the report carries no
collision bound. An excluded option refuses with a message naming it. Seed and
worker requests are kept. Checkpoint and recovery use the safety path.

The explicit `--lncheck final --lasso short` mode, and `dive tlc -lncheck
final`, refuse by name (exit 3) when the model has temporal properties, in
both profiles.

The TLC profile also accepts `--action-composition=tlc` when every property
binds to initial predicates, invariants and implied actions. From a source
checkout:

```sh
dive check crates/dive-api/tests/fixtures/ordinary-cdot/CompositionFinal.tla \
  --config crates/dive-api/tests/fixtures/ordinary-cdot/hold.cfg \
  --compat tlc --action-composition tlc --fp 0 --seed 0 --workers 1 --output json
```

This reports success with 5 generated states, 2 distinct states and depth 2.
The Rust form is `CheckRequest { compat_tlc: true, action_composition:
Some(ActionComposition::Tlc), fp: Some(0), ..request }`. Intermediate states
keep their generated-state charges and the run is sequential. A holding result
on a complete graph needs no replay, but a violation whose counterexample
needs composed `Next` or a composed closing edge refuses. Composed `ENABLED`
and fairness atoms follow TLC's composed `ENABLED`, and a violation that needs
them also refuses at replay.

With `--compat=tlc --action-composition=tlc --lncheck=final --lasso=short`,
runtime string constructors and TLC composition keep their single-worker
encounter order and intermediate-state counts; a larger worker request reports
the sequential fallback. Register and counter access, randomness, I/O and
cached lazy effects refuse. Checkpoints and DFID do not support this.

## Final checking in the TLC profile

Errors that final checking reports:

- A malformed numeric temporal operand, such as `Bad == <>1`, reports
  `evaluation-error` before exploration, TLC code 2215 (`TLC_EXPECTED_VALUE`),
  exit 255 in both profiles (13 with `--exit-codes native`).
- A single direct `<>[]A` or `[]<>A` action operand keeps an evaluator
  `GENERAL` error as `evaluation-error`, TLC code 1000, exit 255 (13 native),
  with error site `temporal-action` and the completed capture counts.
- A missing `_PERIODIC` definition reports TLC code 2229 and a parameterized
  one 2228, both exit 151 (1 native). Binding and symmetry-preparation model
  errors report the `binding` stage, and engine model errors the `capture`
  stage. A valid `_PERIODIC` hook refuses by name.
- When the temporal roles with fairness reduce to literal `FALSE` under TLC's
  negation, simplification and DNF rules, dive reports TLC's zero-solution
  startup error (2253, exit 77) after binding and before `ASSUME`, Init or
  exploration, with no counts. `<>TRUE` and `<>[]TRUE` are checked normally.

Synthetic nonboolean results and uncertain compound or multiple-property action
errors refuse. Some residual-tableau constructor errors refuse during replay
instead of reproducing TLC's model error, with no counterexample. Action
predicates use the first generated successor for each destination at a source;
later constructor aliases and the implicit stutter do not replace it, which
matters when TLC tells a function constructor from a record.

### Errors on a source row

At one worker, dive can report an evaluation error at the state where it
happens (the `MinChooseFair` and `MinChooseFairSafeNext` models in
`conformance/corpus/fairness-primary/` agree with TLC):

- TLC GENERAL (1000), exit 255, when the first source after exactly one Init
  state has only itself as successor. This covers a `Next` failure before any
  successor, a self successor followed by a fairness error, and a self
  successor before a later `Next` error. The report has no violated property
  or lasso, capture stays incomplete, and a version-5 trace carries the
  source-only endpoint.
- Division by zero (2179) or a string concatenation error (2182) in a state
  atom, at the real source row, with ending `temporal-state-row`. This needs
  the exact TLC evaluation order with no residual tableau, and dive must show
  that every predicate's inputs are stable before capture. The finite behavior
  and the error endpoint are each replayed on a fresh runtime. A row error
  claims no lasso or violated property.

A quiet earlier action may reach a later state error; an earlier failing action
takes priority. Unknown source order, targets other than the source itself,
effects, `VIEW` or symmetry, cached values, parallel source events and an
action error with pending temporal obligations refuse, with their counters,
output and context. `WF` and `SF` syntax alone gives no classification.

Of the four models in the `liveness-atom-error-timing` corpus, dive matches
`DelayedSiblingStateError` (2179, exit 255, with 4 distinct and generated
states, 2 queued and depth 3) and the bare action after `<>` that SANY rejects
(3002, exit 150). `CombinedUnusedAtom`, where TLC still evaluates the failing
atom (2179, exit 255, 2 states, depth 2), and `TableauInitAtomError` (GENERAL
1000, exit 255, 1 state, depth 1) refuse. In the
`liveness-atom-action-error-order` corpus, `<<ActionBad>>_x` makes TLC report
the state concatenation error (2182, exit 255) with 2 states, 1 queued and
depth 2; that formula sets no general priority between action and state
errors.

## Language support

Temporal operators, finite quantification and `WF`/`SF` over pure state and
action predicates work in both profiles. Unknown `ENABLED` or fairness covers,
runtime effects, dynamic or exported scopes and other constructs dive cannot
prove pure refuse by name.

Predicate domains are inspected without being enumerated, so there is no limit
on binding count: `[]<>(\A i \in 1..5000 : i >= 1 /\ x)` is accepted. Each
state still evaluates its domains and bodies. Temporal quantifiers such as
`\A i \in S : []P(i)` expand to concrete formulas within the formula and
storage limits. Finite filtered binders can read scalar constants supplied
with `=`, such as a bounded `LimitedSeq` helper reading `Max=1`; constants
supplied with `<-`, operator constants and non-scalar values cannot.

### Configured operators and instances

A configured operator constant can call a nonrecursive top-level operator with
ordinary value parameters inside a pure state or action predicate, including
primed arguments in fairness actions. From a source checkout:

```sh
dive check conformance/corpus/configured-temporal-operators/specifications/ConfiguredReplyHolds/ConfiguredReplyHolds.tla --compat=tlc --fp 0 --seed 0 --output json
```

This configures `Reply <- MCReply`, uses `Reply(0,1,x,x')` as its fair action
and checks that `x` eventually becomes 1. The paired `ConfiguredReplyFails`
checks `[]<>(x=0)` and reports a temporal violation. Configured native,
recursive, higher-order, value-overridden and temporal targets, captured
configured operators and substituted calls refuse, as do effects, temporal
operands and `ENABLED` inside a configured call, even in unused arguments.

An `INSTANCE` substitution that is absent uses the original variable or
configured constant; one explicitly unset refuses. So an unused `INSTANCE`
import does not hide a configured constant:

```sh
dive check crates/dive-trace/tests/fixtures/instance-constants/constant_absent_violation/Constant.tla \
  --config crates/dive-trace/tests/fixtures/instance-constants/constant_absent_violation/Constant.cfg \
  --compat=tlc --fp 0 --seed 0 --workers 1 --output json
```

This reports `Bad` violated, code 2116, exit 13, with 3 generated states, 2
distinct states and depth 2. Opaque bindings, effectful, recursive or
nonconstant constant redirects and missing finite `ENABLED` covers refuse.

### SelectSeq and callbacks

State predicates can call `Sequences!SelectSeq` with a unary definition or
`LAMBDA`, including a forwarded operator and lexical captures:
`SelectSeq(<<0,1,0>>, LAMBDA y: y=x)` returns `<<0,0>>` when `x=0` and `<<1>>`
when `x=1`, keeping duplicate positions and order. Nested `SelectSeq` calls, bounded binders such as
`SelectSeq(<<0,1,0>>, LAMBDA y: \E z \in {y}: z=0)`, and captured function,
filter and map sources work too; the `selectseq-captures` corpus pairs
holding and violating models. Atoms are evaluated as TLC evaluates them:
the callback runs on each element and stops at the first error, an empty
sequence calls nothing, and only reached branches run.

These refuse: definition value overrides, opaque stored functions, action or
temporal callbacks, recursive operator callbacks, native or configured
predicate targets and dynamic callback contexts. Unused captured bodies are
checked too, so an empty domain cannot hide an effect or a primed source.
Other higher-order standard operators refuse.

## State-level recursive operators

Temporal properties and fairness actions can call pure top-level `RECURSIVE`
operators whose bodies and reached definitions are at most state level, such
as `Sum(s) == IF s = <<>> THEN 0 ELSE Head(s) + Sum(Tail(s))` in
`<>(Sum(s) = 0)` or in a fair action's finite binder domain. Quantifiers,
`CHOOSE`, constructors, recursive function values and `LET` inside them keep
their usual meaning, and arguments are checked in the caller's scope, including
primed reads in action arguments. Both profiles apply this rule, as TLC treats
these calls as evaluation leaves, and the validator evaluates the atoms too.
Acceptance does not prove termination; normal limits and errors apply.

```sh
dive check crates/dive-api/tests/fixtures/recursive-temporal/Rec.tla --config crates/dive-api/tests/fixtures/recursive-temporal/Rec.cfg --compat=tlc --fp 0 --seed 0 --workers 1 --output json
```

From a source checkout, this succeeds with 3 distinct and 7 generated states in
both profiles. Action or temporal bodies, `ENABLED`, runtime effects, operator
arguments, configured bindings and overrides, dynamic `INSTANCE` contexts and
local recursive operators refuse; value-bound constants work. An evaluation
error inside an atom refuses by name.

## Finite local recursive functions

State predicates and fairness actions can use a finite local recursive
function, such as an allocator-style `PermSeqs(S)`, with its domain, formals and
enclosing state and `LET` captures in scope.

```sh
dive check crates/dive-liveness/tests/fixtures/recursive-function/PermSeqsScope.tla --config crates/dive-liveness/tests/fixtures/recursive-function/PermSeqsScope-2-Good.cfg --compat=tlc --fp 0 --seed 0 --workers 1 --output json
```

This model needs a `Schedule` action under weak fairness and checks `<>done`.
With `PermSeqsScope-2-Bad.cfg` it checks `[]<>(~done)` and reports a temporal
violation with a validated word.

The body and domains must be pure state expressions. Primed, action or temporal
captures, configured recursive actuals, higher-order or opaque forwarding,
native effects, infinite or opaque domains and exhausted bounds refuse. A
configured target whose body builds a local recursive function also refuses.
A recursive result cannot supply a temporal quantifier domain, because dive
treats it as state-dependent. A recursive function gives no successor cover for
fairness enabledness, and a missing cover never becomes a `Disabled` result.

## Nested enabledness in Boolean guards

Pure `ENABLED` queries work inside `~`, `/\` and `\/` guards, definition
bodies, `LET` expressions and bounded Boolean quantifiers, in `Next`, temporal
action predicates and fair actions. Each query looks for a fresh successor of
its current state; its primed variables do not refer to the outer transition.
A reached query error is reported even when a later guard is false, and
unreached errors stay unreached.

On a lasso, `WF_v(A)` holds when `<<A>>_v` occurs on the cycle or `A` is
disabled at some cycle state, and `SF_v(A)` when it occurs or `A` is disabled
at every cycle state. An action whose guard reads a primed variable, such as
`ack' = IF x' = 1 THEN ~ack ELSE ack`, is decided like any other (corpus
`fairness-replay`). Replay can also prove fairness from an action that recurs
on the cycle. The replayed `WF` and `SF` controls cover continuous and
intermittent starvation, a disabled stutter and an occurrence on the closing
edge.

```sh
dive check crates/dive-liveness/tests/fixtures/nested-enabled/NestedEnabled.tla --config crates/dive-liveness/tests/fixtures/nested-enabled/hold.cfg --compat=tlc --workers 1 --fp 0 --seed 0 --output json
```

The model alternates `x=0` and `x=1`. `hold.cfg` succeeds and `fail.cfg`
reports `Fail` violated; both explore 3 generated states, 2 distinct states and
depth 2. The failing word closes from `x=1` to `x=0` although a reached inner
query needs `x'=1`.

`Identity(p) == p; Identity([](ENABLED A))` forwards a temporal expression with
its caller scope, and a query behind a helper formal, as in
`Guard(p) == p /\ TRUE; Guard(ENABLED A)`, is evaluated on replay. Query-bearing
operator captures and native arguments refuse. Rust callers of trace validation
should use the `RECOMMENDED_STACK` thread size.

`ENABLED A` evaluates `A`'s conjuncts in order and stops at a false one. With
`A == FALSE /\ x' = 0 /\ D = D` and a startup filter
`D == {k \in SUBSET {0} : 1 \div 0 = 0}`, `[]<>ENABLED A` is violated (2116,
exit 13; 2 generated, 1 distinct, depth 1) with no division error. Without the
`FALSE` prefix, the TLC profile reaches the division by zero, which TLC
reports with exit 255. Two controls with a `TRUE` filter predicate hold, and
forged counterexamples for them are rejected (corpus `enabled-false-prefix`).

## INSTANCE query namespace controls

`ENABLED I!A` applies an instance inside the query, while an exported `I!Can`
defined as `Can == ENABLED A` enters the query after the instance bindings.
The two need not agree, and dive keeps them apart as TLC does. When one caller
variable instantiates both `y` and `z` and the action needs `y' = FALSE` and
`z' = TRUE`, the direct form is unsatisfiable, while the exported form violates
`P` in the initial state (TLC 2107, exit 12), even when the caller has no
variables named `y` or `z`. A nested instance that sends both through `q`
returns false. Fresh substitutions inside a query differ from bindings captured
before it. Both profiles use the same rules. The ten controls are in the
`instance-temporal` corpus.

Hidden query variables never enter graph states, fingerprints or trace
variable lists, which hold only the root variables. Composition with assigned
hidden query names refuses by name. Independent replay of instantiated query
words, general instantiated temporal actions and refinement, callable
substitution and instantiated fairness replay are not supported.

Lazy values inside queries follow TLC's caching:

- An exported query that assigns `a' = 1`, captures `LET Cut == a'` and
  filters `<<1, 2, 3>>` with `SelectSeq(..., LAMBDA e : e /= Cut)` sees `Cut`
  as the successor value. Requiring the filtered result to stay
  `<<1, 2, 3>>` makes the query false, which the invariant exposes as an
  initial violation of `Inv` (2107, exit 12); requiring `<<2, 3>>` makes it
  true, and the check succeeds with 1 distinct state, 2 generated and depth 1.
  Replay accepts the violation and rejects the same word where `Inv` holds
  (corpus `lazy-enabled-capture`).
- A lazy argument cached before a hidden choice `a' \in {1, 2}` stays cached
  across both candidates, because TLC compares only root variable names when it
  reuses a lazy value. dive compares the assigned root names the same way.
  With `b' = 42` assigned and `Step(a')` filtering `<<1, 2, 3>>` through the
  lazy `Cut`, requiring `<<1, 3>>` leaves the query disabled (2107, exit 12),
  while requiring `<<2, 3>>`, using only one candidate, inlining `a'` or moving
  the query to the root module succeeds. TLC run with
  `-Dtlc2.value.impl.LazyValue.off=true` changes only the `<<1, 3>>` case
  (corpus `lazy-enabled-cache`).
- A `LET` reached before a hidden membership choice keeps one binding across
  the choices, and one reached after it gets a binding per choice. Replay
  evaluates lazy query sources the same way. The `lazy-query-lifetimes`
  corpus has 24 controls for this, with leading and delayed helper arguments,
  nested queries, sibling Boolean expressions and false leading guards.

Rust callers can build a partial query assignment with `Evaluator::named_state`
from IR variable declaration ids and values. `StateRef::Named` keeps it for
scoped reads, and `St::enabled_candidate`, `enabled_current` and
`primed_successor` keep the complete state. An absent name is unassigned; it
does not fall back to the current state. Assignments from another evaluator
are refused, and a root projection of a query candidate is not a `Next`
transition.

A finite state or action predicate can use `Quantify(S, LAMBDA e: P(e))`
with a pure, inspectable callback, or `FoldFunctionOnSet(+, base, f, S)`
with the built-in integer addition operator. For example, a temporal
property can count the voting members that satisfy a state predicate.
A nonrecursive quantifier, set filter, set map or function constructor inside
an action predicate can range over a finite next-state domain. Both graph
capture and counterexample validation read this domain on the actual edge.
Recursive binders and expansion of a temporal formula retain their existing
scope and level restrictions.

## Fairness

Both profiles keep pure recurrence (`[]<>B`) and persistence (`<>[]B`)
obligations apart from the temporal memory a property needs, and several
strong-fairness alternatives share that memory. A finite prefix may violate a
persistence condition; the cycle must satisfy it. Recurrence may be met on the
closing transition, including a stuttering edge.

For checks with at least eight fairness alternatives, Core can rule out whole
groups of candidate cycles when there are at most 64 distinct cycle predicates.
It records which predicates hold on each original
edge, including parallel edges and stutters. Within each strongly connected
component it discards alternatives that require an absent predicate. If all
remaining alternatives require a predicate on every cycle edge, Core removes
edges that fail it and checks the smaller components. After at most eight
rounds of elimination, any surviving candidate
goes through the usual witness search and independent trace validation. The
optimization shares the optional product-row memory allowance and uses the
ordinary search when its tables would exceed that allowance.
On Elevator Medium the one-worker median fell from 1.50 s to 0.28 s. On
Large it fell from 92.33 s to 4.16 s for the same 50,653 distinct states.
Finding an accepting cycle or ruling one out took 0.89 s of those 4.16 s;
evaluation of temporal predicates on the graph took 2.57 s. Manyan ran the
same Large configuration in 1.31 s. At four floors, Manyan found a 202-state
counterexample that Dive independently validates. Ordinary Core refused that
configuration during temporal preparation in the measured run.
[Elevator comparisons and negative controls](../../conformance/reports/perf/2026-11-22-fairness-components.json)

dive can also reuse the evaluated truth of each `WF` and `SF` condition shared
by several properties, in at most the smaller of 16 MiB and 1/256 of the
memory limit. `fairness_reuse` metrics count reuse. There is no flag for this,
and it changes no result.

In the default profile, Core can label all outgoing edges of a state false
for a fairness action whose leading state conditions already rule it out.
For example, in `WF_vars(ready /\ Step)`, every nonstuttering occurrence of
`ready /\ Step` requires a source with `ready = TRUE`. Core checks the guard
once per source when the subscript is a tuple of state variables and every
retained row is materialized. A source probe that raises an error leaves the ordinary
edge evaluation in place. The JSON report counts attempts and proven false
rows under `core.metrics.liveness.atoms.action_source_guards`.

A leading condition can also read one destination variable. For example,
`x' = 1 - x` rules out a destination that keeps `x` when the source has
`x = 0`. Core tests the prefix once for that source and bound fairness action.
If it returns false without an error, warning or output, each admitted edge
with the same stored `x` value inherits that result. Other destinations take
the ordinary action check. The `unchanged_target_guards` counters report
compiled plans, source probes, successful source proofs and individual action
demands answered by those proofs.

From a source checkout, a strong-fairness model with three toggles
(`crates/dive-liveness/tests/fixtures/solutions/Solutions.tla`, 8 states, 25
generated) exercises holding and failing properties, including
multiple-property attribution, in both profiles:

```sh
cargo test -p dive-liveness --test solutions
```

## Symmetry with temporal properties

`--symmetry-liveness=unreduced` is the default profile's setting
([symmetry with liveness](../tlc-differences.md#d-005-symmetry-with-liveness-uses-the-full-state-space)).
When the configuration has a temporal property and `SYMMETRY`, the whole check,
including initial and safety checks, uses full state identities and reports
unreduced counts. `SYMMETRY` stays in the configuration and report but is not
applied. A `PROPERTY` that binds to initial, invariant or implied-action roles
does not trigger this, nor does fairness alone.

```sh
dive check conformance/corpus/probes/specifications/P4SymLive/P4SymLive.tla --workers 1 --seed 0 --fp 0 --output json
```

`--symmetry-liveness=refuse` refuses the combination. `--compat=tlc` selects
`--symmetry-liveness=tlc`, TLC's reduced liveness checking, which dive does not
implement, so it refuses instead of silently checking unreduced. For TLC value
semantics with unreduced checking, ask for both:

```sh
dive check conformance/corpus/probes/specifications/P4SymLive/P4SymLive.tla --compat=tlc --symmetry-liveness=unreduced --workers 1 --seed 0 --fp 0 --output json
```

That reports `tlc+overrides`. The flag wins over the profile in any argument
order, `dive tlc` accepts it, and there is no `.cfg` keyword for it. A
malformed symmetry declaration is still reported when safety reduction
applies. `options.symmetry_liveness` and `options.symmetry_liveness_source`
(`explicit` or `profile-default`) record the setting. After binding, the
`symmetry_liveness` object records the `declared` name, `genuine_temporal`,
`effective: "unreduced"` and `scope: "whole-run"`.

Validate a saved counterexample with the same policy:

```text
dive validate-trace conformance/corpus/probes/specifications/P4SymLive/P4SymLive.tla --trace YOUR_NATIVE_TRACE.json --compat=tlc --symmetry-liveness=unreduced
```

Native `dive-trace` stays version 1, with no quotient closing annotation.
`VIEW` with temporal properties refuses.

## Storage and limits

Temporal checks accept `--store fp|exact` in both profiles. Unless two states'
fingerprints collide, both give the same verdict, counts and counterexample.
With `--store exact`, two states whose 64-bit keys collide stay distinct, so a
holding result does not rest on the fingerprint assumption. The human report
says "Exact full-state storage." or "Fingerprint storage (64-bit); collision
probability not computed." Frontier, candidate or predecessor spill, disk seen
sets and `--store-compact-bytes` refuse by name with temporal properties, and
there is no disk-backed temporal capture.

The behavior graph keeps every captured state in memory and stops on the run's
`--time-limit` and `--max-memory`, not on a fixed state, edge, product or
counterexample count. Parallel capture expands at most eight states per wave.
State and depth ceilings stop capture without a verdict; see
[state and depth budgets](checking.md#state-and-depth-budgets). A budget stop
keeps its stage, statistics and `capture_completed`, never retries and is
reported as a stop. These limits remain:

- 8,000,000 acceptance-buffer bytes and 32,000,000 acceptance work units per
  check, in both profiles, listed in `options.temporal_bounds`. They count
  packed words, column headers, edge offsets and Boolean scratch, not total
  memory or search work. Running out is a named resource refusal.
- `usize::MAX` reachable state and tableau-node pairs, recorded as
  `options.temporal_bounds.product_vertices`. This is the machine's index
  capacity, not a memory allowance.
- Attribution-row and word-truth cell limits, under
  `options.temporal_bounds.cell_limits`; graph labels have no cell limit.
- A value cache of the smaller of 128 MiB and 1/64 of the memory budget (128
  MiB when unlimited), and 1 MiB for reused atom checks.
- 32-bit row and vertex ids.

On large native graphs, each action predicate allocates a 4,096-edge block
on the first true result in that block. An entirely false column needs no
block directory. Predicate columns share the capture's checked row offsets. The product and witness analysis read each result at its original
source and edge position.

The time limit is not a hard deadline. Polls happen between bounded units of
work and inside quantifier, `CHOOSE` and filter loops; single value operations
can finish first, and binding, `ASSUME`, symmetry preparation and rebuilding an
already decided error are not interrupted. A stop during replay reports
`stopped` at `trace-validation` with the capture flag and optional
`resources.validation` progress, and publishes no counterexample. Memory
accounting is incomplete, and sampled process memory is not a true peak.

## Metrics

`core.metrics.liveness.storage: "native-csr"` identifies the graph store.
`convert_ms`, `converted_nodes`, `memo_entries` and `graph_states` remain, with
`values` counters for admission (`"deferred"` or `"eager"`), on-demand reads,
cache hits and misses, entries, requested bytes, replacements and bypasses.
`retained_rows` is the stored row count, and `convert_ms_scope` says that
conversion timing leaves out one-variable cache hits. `memory` fields cover the
graph buffers and action labels; `accounting_complete` is false. Core packs repeated
action IDs and derives successor ordinals from their source positions when
possible. It stores destination IDs in 64-edge blocks when the IDs or their
repeated runs fit in fewer bytes. Temporal analysis can recover the original ordinal and any named
action for each generated graph edge.
`core.metrics.liveness.atoms` counts eligible atoms, direct evaluations by kind
and fallbacks, and `atoms.row_proofs` counts reused atom checks. These describe
work; they say nothing about throughput or completion.

## Measured capacity

Under explicit budgets and 8 GiB of memory, at one worker with seed and
fingerprint 0, these Examples models complete every configured property in
both profiles:

| Model | Time budget | Distinct | Generated (TLC / default) | Depth |
| --- | ---: | ---: | ---: | ---: |
| CoffeeCan1000Beans | 564,520 ms | 501,500 | 2,000,002 / 2,000,002 | 1 |
| MCCRDT | 165.8 s | 25,000 | 1,350,001 / 900,001 | 16 |
| Huang | 230.82 s | 81,256 | 1,165,101 / 1,165,101 | 21 |
| AllocatorImplementation | 361.62 s | 17,701 | 64,414 / 64,414 | 16 |

AllocatorImplementation also completes capture with a 4 GiB ceiling.
MCCRDT's default-profile count is the registered
[action composition](../tlc-differences.md#d-022-action-composition-works-by-default-with-its-tla-meaning)
difference. From the Examples CoffeeCan directory:

```sh
dive check CoffeeCan.tla --config CoffeeCan1000Beans.cfg --workers 1 --seed 0 --fp 0 --time-limit 564520ms --max-memory 8g --output json
dive check CoffeeCan.tla --config CoffeeCan1000Beans.cfg --workers 1 --seed 0 --fp 0 --time-limit 564520ms --max-memory 8g --output json --compat=tlc
```

## Rust APIs

`dive_api::temporal` is the Rust entry point; the
[temporal API contract](../temporal-api.md) has the `temporal::check` example,
supported forms, limits, source-qualified errors and indexing.

- `temporal::Outcome::NativeHolds`, `NativeViolation` and
  `NativePrefixViolation` report results. A violation exposes `word()` (states,
  edges, properties and loop position), `evidence()`, `document()` and
  `validation()`, and is built only after replay. Evidence from a prefix cannot
  prove that a property holds.
- `dive-liveness` exposes planning (`plan`), the tableau (`formula`), the
  product and lasso search (`product`, `search`), the coordinator (`analyze`),
  TLC's liveness formula classifier for `--compat=tlc` (`tlc_plan::classify`,
  TLC errors 2213 and 2214), and the periodic coordinator and schedule
  (`analyze::periodic`, `analyze::periodic::Schedule`).
- Final analysis checks every predicate, evaluator, IR and name it receives
  before evaluating constants or calling native hooks; malformed model metadata
  returns a planning refusal without capture.
- Rust callers keep their own ceilings: `product::Limits::default()` is
  1,000,000 vertices, and explicit zero is enforced. Pass `usize::MAX` for the
  file route's capacity.
- Existing analysis APIs keep `GraphLabelPolicy::Cells(limits.atom_cells)`.
  The `*_with_policies` variants choose graph admission separately from the
  acceptance policy passed to `GraphCheck::prepare`; `NoCellLimit` affects
  graph labels only.

The [Rust guide](rust-api.md) covers the low-level graph and read interfaces.
