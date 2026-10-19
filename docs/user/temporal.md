# Temporal checking

The checker exposes bounded final/short analysis. Eligible ordinary requests
select it automatically in both profiles; explicit selection is also available:

```sh
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterTemporal.cfg --lncheck final --lasso short --seed 7 --output json
```

The example reports a real liveness violation. Although `Next` always increments
before four, TLA+ temporal behaviors can stutter at zero; there is no fairness
condition in this model requiring the increment action to occur.

This mode explores the supported graph, checks once at the end, chooses a
deterministic short periodic witness and independently replays it in memory and
as a lossless document. It checks the complete configured property claims,
including the closing edge. This does not implement periodic
scheduling or all evaluation-error timing. Explicit requests retain their
existing `tlc+overrides` label when combined with TLC value semantics.

## Ordinary checking

```sh
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterTemporal.cfg --compat=tlc --fp 0 --seed 7 --output json
```

This ordinary request uses the same bounded pure final analyzer and independent
word validation. No lasso-policy flag is needed. It finishes graph exploration
before checking temporal properties and chooses a deterministic short valid word;
TLC's particular SCC, lasso, timing and queue order are not reproduced.
The report records the actual `lncheck: final`, `lasso: short` and temporal
bounds, with profile `tlc`. This is partial temporal support, not general TLC
parity or an implementation of periodic checking.

Ordinary requests in the default (dive) profile take the same route with dive
value semantics; their report records profile `dive` with `lncheck: final`.
The approved deterministic level-boundary schedule (D-014) is not implemented
yet, so until it is the default profile checks temporal properties once, after
complete exploration. Verdicts, counts and violated properties are those of a
final check; no request stops early on a liveness prefix.

The route requires profile-default evaluation and fingerprint settings,
fingerprint polynomial 0, resident fingerprint storage, and no storage
budget/directory, deadlock, sequential or chaos overrides. Under `--compat=tlc`
use `--fp 0` explicitly: the TLC profile otherwise selects a random polynomial
(the default profile's polynomial is already 0). Seed and
worker requests are retained. Checkpoint/recovery uses its existing separate
safety path. Randomness combined with VIEW or SYMMETRY and excluded options retain their
existing routes and named refusals. Real unsupported language features and resource limits remain explicit;
a refusal, bound or external timeout is never a holding result.

`--temporal-bounds default|unbounded` selects the fixed bounds of file-based
temporal analysis (ADR 0106, D-025). `default`, the default profile's setting,
keeps the counts described below; exhausting one is a named resource refusal.
`unbounded` has no fixed count in planning, capture, labelling, product
construction or acceptance materialization: like TLC, only memory and time stop
it. Its memory stop stands for TLC's heap: a resident-memory ceiling of a quarter
of physical memory (the JVM's default maximum heap), sampled while the tableau,
labels and product are built and reported as
`options.temporal_bounds.resident_ceiling_bytes`. Exceeding it refuses with
`resident memory`. `--compat=tlc` selects `unbounded` unless the flag is given, and JSON reports
the setting as `options.temporal_bounds.setting`. `unbounded` also lifts the
work ceilings of pure predicate replay (100,000 finite-binding traversal visits
and 50,000 lazy query work units); the structural nesting-depth limit stays in
both settings.

With `--temporal-bounds=default`, file-based temporal checking uses 8,000,000
covered acceptance-buffer bytes and 32,000,000 independent acceptance
materialization work units.
Packed words, column headers, optional edge offsets and Boolean scratch count;
capture, labels, graph construction, search, runtime and allocator overhead do
not. This is not a total memory or search-work limit. Other graph, evaluator and
transition limits remain unchanged. JSON names the byte/work bounds under
`options.temporal_bounds`; exhaustion is an explicit resource refusal. Existing
Rust custom-limit APIs keep legacy logical acceptance cells; the additive
`check_final_with_policy` API selects byte/work admission explicitly. See
[covered allocations and exclusions](../temporal-api.md) and ADR 0098.

Binding happens once under the ordinary startup host. If the configured roles
are all safety obligations, their complete ordinary result stays on schema 1,
including startup Print output, randomness and error ordering. Genuine temporal
analysis uses schema 2. A completed holding result or violation reports actual
generated/distinct/queue/depth counters and `execution.exhausted: true`, even
though the legacy `reported_by_tlc` print marker is false. A violation names its
property and carries a lossless periodic word independently replayed against a
fresh model. TLC exit conventions give success 0, temporal violation 13,
named unsupported/resource refusal 3, and classified evaluation errors their
reported TLC status; inspect the verdict and execution stage as well as the exit.

The ordinary conformance adapter retains those actual complete counters and
requires coherent schema, class, physical status and word metadata. It still
replays the word independently: a producer's `validation: valid` field is not
proof. Periodic liveness words may differ in length or closing target while
Distinct counts, depth, property, error class and physical exit remain
strict; generated-count equality is required at one worker. Queue population is retained as raw data and as an internal final
exhaustion consistency check, rather than compared with TLC's queue population. Finite
counterexample checks retain their existing comparison rules.

When the configuration has no `PROPERTY` entries, these flags leave ordinary
safety checking in charge: invariants, initialization/successor errors,
assumptions and other safety settings retain their usual behavior. No temporal
claim or capture proof is issued.

With default resident-memory settings, configured properties whose actual bound
roles are entirely initial predicates, invariants and implied actions also use
that ordinary engine. Each obligation retains its scope, order and name; the
checker binds once and retains its startup output and random stream. This initial
route requires profile-default evaluation/fingerprints, polynomial 0, fingerprint
storage, no storage budget/directory options, deadlock or sequential override, or
chaos flag. The TLC profile also admits `--action-composition=tlc` with every
other setting unchanged, for example:

```sh
cargo run --locked --offline -p dive-cli --bin dive -- check \
  crates/dive-api/tests/fixtures/ordinary-cdot/CompositionFinal.tla \
  --config crates/dive-api/tests/fixtures/ordinary-cdot/hold.cfg \
  --compat tlc --action-composition tlc --fp 0 --seed 0 --workers 1 --output json
```

This checked Hold example reports success, with 5 generated states, 2 distinct
states and depth 2.

The Rust counterpart is `CheckRequest { compat_tlc: true,
action_composition: Some(ActionComposition::Tlc), fp: Some(0), ..request }`.
Composition intermediates retain their original generated-state charges and
sequential execution reason. This option does not add independent composition
replay: a candidate requiring composed original Next or a composed closing edge
still refuses without a published word. Holding on a complete captured graph
requires no candidate replay; genuine violations still require it. Composed
ENABLED/fairness predicates retain their bounded pure-predicate refusal. TLC's
`off` default and the default profile are unchanged; `tla` and other evaluator,
storage or explicit sequential overrides do not gain ordinary dispatch support.
Seed and worker requests are retained. Other options and randomness
under VIEW or SYMMETRY keep the existing final-mode route and error priority.
Real temporal property roles still require temporal analysis and independent
periodic-word validation. A state-only `PROPERTY P` is checked in the initial state by the ordinary engine;
`[]P` becomes an invariant. An evaluation resource limit in those safety roles
stays a resource limit, including recursive calls. A genuinely temporal formula
such as `[]<>P` still goes through temporal planning and retains its named
recursive/opaque-operator refusal. These are different requested obligations.


Pure supported boolean/temporal operators, finite quantification and WF/SF
predicates have scoped limits. Unknown ENABLED/fairness covers, runtime effects,
dynamic/exported scopes and other unproved constructs refuse. Ordinary strict
`dive check` retains its temporal refusal. A diagnostic planning/tableau/product
API that completes has not issued a temporal verdict.

Direct configured operator constants can call nonrecursive top-level user
operators with ordinary value parameters inside pure state/action predicates.
The original call and its lazy arguments are retained, including primed
arguments in fairness actions. For example:

```sh
dive check conformance/corpus/configured-temporal-operators/specifications/ConfiguredReplyHolds/ConfiguredReplyHolds.tla --compat=tlc --fp 0 --seed 0 --output json
```

This model configures `Reply <- MCReply`, uses `Reply(0,1,x,x')` as its fair
action, and checks that `x` eventually becomes 1. The paired
`ConfiguredReplyFails` model checks `[]<>(x=0)` and has a temporal violation.
Both value profiles use the same scoped structural checks and existing limits.
Configured native, recursive, higher-order, value-overridden and temporal
targets, captured configured operators and substituted operator calls retain
named refusals. Within configured calls, effects, temporal operands and
ENABLED inside a pure atom, including unused actual arguments, remain refused. Independent trace replay
checks the original configured calls separately.

Temporal preflight distinguishes an absent INSTANCE substitution from an
explicitly unset binding. An absent reference can use its original variable
declaration or configured constant instead of being rejected as opaque. Pure
zero-argument constant redirects are inspected in their actual scopes; value
bindings and lazy substitutions keep their existing meanings. An unused
INSTANCE import therefore does not hide a configured constant from checking.
For example:

```sh
dive check crates/dive-trace/tests/fixtures/instance-constants/constant_absent_violation/Constant.tla \
  --config crates/dive-trace/tests/fixtures/instance-constants/constant_absent_violation/Constant.cfg \
  --compat=tlc --fp 0 --seed 0 --workers 1 --output json
```

This model reports property `Bad` violated, code 2116 and exit 13, with three
generated states, two distinct states and depth two. Its periodic word passes
independent validation. The paired configured-redirect fixture has the same
result; the original holding fixtures report success. Explicit final checking
also supports these cases with default value semantics: omit `--compat=tlc`
and add `--lncheck final --lasso short`. Ordinary default-profile temporal
dispatch keeps its existing support boundary. Explicitly unset or opaque
bindings, effectful/recursive/nonconstant constant redirects and missing finite
ENABLED covers retain their named refusals.

Pure state predicates can call authoritative `Sequences!SelectSeq` with a unary
user definition or `LAMBDA`, including a known forwarded operator and lexical
captures. For example, `SelectSeq(<<0,1,0>>, LAMBDA y: y=x)` returns `<<0,0>>`
when `x=0` and `<<1>>` when `x=1`; duplicate positions and order are preserved.
The planner and independent trace evaluator resolve the captured scopes
separately, so a forwarded state callback is not folded as a constant.

This subset excludes callback binders, deferred functions/providers, action or
temporal callbacks, recursive/higher-order callbacks, native/configured
predicate targets and retained dynamic callback contexts. Unused captured
bodies and stored values are checked too. These combinations report a named
`unsupported` result. Other higher-order standard operators remain refused.
This includes direct and negated `ENABLED` and WF/SF enabledness atoms when the
independent validator can derive complete finite successor covers. Ordinary
unary pure state helpers enter through their original lazy lexical arguments;
callback captures and sequence dependencies remain part of the cover checks.
Missing covers, explicit primed/action callback sources, generic higher-order
action forwarding and action composition retain named refusals. Independent
replay can also prove fairness from an action recurring on the repeated cycle
without querying enabledness.
Each atom evaluation starts one fresh aggregate work budget, shared by its
original before/after/body evaluations and nested calls. An enabledness atom
shares that budget through its complete original search; the independent trace
search initializes its own counters and shares them through cover reads,
candidate relations and nonstuttering comparisons. The existing native
loop checks each actual element before invoking its original callback and stops
at the first error. Empty sequences invoke no callback. Bounded predicate
execution bypasses definition and lazy argument result caches, leaving caller
caches untouched. Authoritative configured/prebound and startup values retain
their original semantics; reading such a value does not invoke a selection
loop. During an actual selection, a captured alias of that same input is
checked by storage identity at the sequence shell; its elements are checked as
the loop reaches them. Reading the alias inside the callback still checks the
whole consumed value. Unrelated stored captures get no exemption. Source-only
capability checks on manually prebound API scopes remain strict and can refuse
such a stored input before any loop executes. These checks enforce capability and work limits, not
predicate truth or a temporal verdict. Both value profiles use this same subset.

Schema 2 reports explicit final requests and actual ordinary temporal analysis;
ordinary safety results retain schema 1.
`properties` names complete claims, and periodic counterexamples carry validation
and a closing edge. Read [the temporal API contract](../temporal-api.md) for the
Rust `temporal::check` example, supported forms, limits, source-qualified errors
and indexing conventions.

## Final checking in the TLC profile

`dive check --compat=tlc --lncheck final --lasso short` checks temporal
properties with TLC's value semantics and reports a deterministic short lasso
that passes independent replay. The profile is labeled `tlc+overrides`, because
TLC's own lasso choice is not reproduced: it depends on hash-table and SCC
visitation order, which are not parity obligations (ADR 0082, ADR 0086).
`--lasso tlc` is a usage error. Periodic scheduling is not implemented yet.

Malformed numeric temporal operands, such as `Bad == <>1`, report
`evaluation-error` before exploration, with TLC code 2215 (`TLC_EXPECTED_VALUE`).
The process exits 255 in both semantic profiles; `--exit-codes native` selects 13. Effects
and unsupported scopes still refuse before planning executes them.

Action predicates use the first generated concrete successor for each
destination identity at a source. Later constructor aliases and the implicit
stutter do not replace that selected target; exploration still counts every
generated successor. This matters when TLC distinguishes a function constructor
from a record during comparison. A single direct `<>[]A` or `[]<>A` action
operand preserves an actual evaluator `GENERAL` error as `evaluation-error`,
TLC code 1000 and process exit 255 (`--exit-codes native`: 13). The report names
the `temporal-action` error site and retains completed capture counts.

This error routing does not establish general temporal error ordering. Synthetic
nonboolean results and uncertain compound or multiple-property action errors
remain unsupported. State predicates continue to use canonical captured states;
some residual-tableau constructor errors still refuse during independent
validation rather than reproduce TLC's model error. No counterexample is
published after that refusal. Action-only constant simplification excludes
eliminated temporal operands only at graph labeling. Existing eager candidate
attribution and independent validation can still refuse eliminated-action
candidates; those refusals emit no model error or counterexample. The additional
bounded normalization uses the existing formula-node and work limits and can
itself refuse when they are exhausted. Predicate-local lazy Boolean guards
retain their existing evaluation.

Final checking also preserves binding errors: a missing `_PERIODIC` definition
reports TLC code 2229, and a parameterized definition reports 2228. Both exit
151 in both semantic profiles; `--exit-codes native` selects 1. Binding and
symmetry-preparation model errors report the `binding` execution stage, including
when an empty property list takes the ordinary safety route. Engine model errors
retain the `capture` stage. A valid `_PERIODIC` hook
still refuses by name until hook execution is implemented.

## Rust APIs

Final analysis admits every retained bound-model predicate and its evaluator,
IR and name ownership before planning evaluates constants or calls native hooks.
Malformed public model metadata returns a planning refusal without graph capture.
Its diagnostic does not resolve an unadmitted location through the evaluator.
Valid configured wrappers keep their original native callbacks and typed errors.

Finite predicate binders support structurally bounded deferred filters. Their
global configured-redirect preflight can resolve scalar constants supplied with
`=`, such as a bounded `LimitedSeq` helper reading `Max=1`, without calling the
helper or forcing unused arguments. Each inspected definition, redirect lookup,
formal/actual/binder metadata item and expression consumes the existing domain work budget; depth and materialized
component/output limits still apply, even with warm evaluator caches. This is
metadata inspection, not predicate evaluation or a liveness verdict.

Predicate and captured-context effect checks retain their existing refusals.
Definitely forbidden global redirect effects refuse before charged source
inspection, including at zero work, independently of redirect iteration order.
Constants supplied with `<-`, operator constants, deferred/non-scalar values,
recursive or higher-order redirects and unsupported lexical contexts do not
qualify for this extension. Two tiny filtered-WF models exercise success and a
genuine independently replayed violation. Coverage of original Quicksort and
other corpus models is not established by these controls.

`dive_api::temporal` is the Rust entry point (see
[the temporal API contract](../temporal-api.md)). `dive-liveness` exposes the
kernel it uses: planning (`plan`), the tableau (`formula`), the product and
lasso search (`product`, `search`), the coordinator (`analyze`), TLC's liveness
formula classifier for `--compat=tlc` (`tlc_plan::classify`, TLC errors 2213
and 2214) and the growth trigger for future periodic checks
(`periodic_backend::Growth`).

## Finite local recursive functions

Pure state predicates and fairness actions can use a finite local recursive
function, such as allocator-style `PermSeqs(S)`. The recursive self, finite
domain, formal values and enclosing state/LET captures retain their actual
interpreter scopes. Planning inspects these without evaluating state-dependent
domains; execution obtains the concrete bounded contexts in each invocation.

```sh
dive check crates/dive-liveness/tests/fixtures/recursive-function/PermSeqsScope.tla --config crates/dive-liveness/tests/fixtures/recursive-function/PermSeqsScope-2-Good.cfg --compat=tlc --fp 0 --seed 0 --workers 1 --lncheck final --lasso short --output json
```

This tiny model requires a Schedule action under weak fairness and checks
`<>done`. Its two initial elements generate two permutation alternatives,
including duplicate successor work. Changing the configuration to
`PermSeqsScope-2-Bad.cfg` checks `[]<>(~done)` and returns a named temporal
violation with an independently validated periodic word.

The supported recursive body and its domains must be pure state expressions.
Primed/action/temporal captures, configured recursive actuals, higher-order or
opaque function forwarding, unsupported native effects, infinite/opaque domains
and exhausted bounds remain named refusals. Finite domains do not prove
termination: the interpreter retains its existing recursion errors and limits.
Unused LET definitions stay lazy.

Recursive source analysis conservatively treats self and formals as state-level
unknowns, even for a closed constant call. A recursive result therefore cannot
yet supply a temporal quantifier domain that planning must establish constant.
This is an analysis limitation, not evidence of an actual state dependency.

Independent fairness enabledness still needs complete finite successor covers
and checks the original relation. A recursive function supplies no cover by
itself. Empty domains cannot hide captured or outer primed reads, and missing
covers cannot become a Disabled result. No planner approval is passed to the
validator. Both value profiles share this bounded subset; existing defaults,
options, schemas and resource caps are unchanged.

Seven tiny pinned primary cases cover empty/one/two-element domains, holds,
fails and a direct control. Original allocator observations retain their earlier
producer identities and do not qualify this transferred source. No corpus gain
or roadmap completion is claimed. Fresh review, current-main qualification and
the full gate remain pending.

A configured first-order target whose body constructs a local recursive function
also refuses, including through ordinary helpers or lazy bindings. The same
restriction applies to used or unused recursive actual arguments and actual
recursive-function slots. Each temporal/finite/independent enabledness walk
inspects that configured source before evaluating domains or using cached values;
an empty recursive domain cannot hide a captured primed read or effect. Pure
unconfigured local recursive functions retain the scoped support described above.

Sequential graph capture distinguishes stateful checking from ordering needs.
Runtime string/name constructors and TLC action composition may appear together:
`--compat=tlc --action-composition=tlc --lncheck=final --lasso=short` retains their
single-worker encounter order and intermediate-state counts. A larger worker
request still reports the sequential fallback. Register/live-counter access,
randomness, I/O and cached lazy effects retain their existing refusals, and
speculative parallel capture remains excluded. Independent validation can still
refuse a counterexample that requires composition or newly created runtime names;
complete graph capture alone does not establish a validated violation. This
change does not extend checkpoint or DFID admission.

## Nested enabledness in Boolean guards

Bounded final analysis accepts pure `ENABLED` queries in ordered Boolean
`~`, `/\` and `\/` guards, including ordinary first-order definition bodies,
local LET expressions and bounded Boolean quantifiers. Queries can appear in
`Next`, temporal action predicates, and actions used by weak or strong fairness.
Each query quantifies a fresh successor from its actual current state; its inner
primed variables do not refer to the outer transition's target.

Fairness on a lasso is validated with the same `ENABLED`: `WF_v(A)` holds when
`<<A>>_v` occurs on the cycle or is disabled at some cycle state, and `SF_v(A)`
when it occurs or is disabled at every cycle state. An action whose guard reads
a primed variable, such as `ack' = IF x' = 1 THEN ~ack ELSE ack`, is decided
like any other (issue #321; corpus `fairness-replay`).

```sh
dive check crates/dive-liveness/tests/fixtures/nested-enabled/NestedEnabled.tla --config crates/dive-liveness/tests/fixtures/nested-enabled/hold.cfg --compat=tlc --workers 1 --fp 0 --seed 0 --lncheck final --lasso short --output json
```

The tiny model alternates `x=0` and `x=1`. `hold.cfg` returns success;
`fail.cfg` reports property `Fail`, a liveness violation with a validated periodic
word. Both explore 3 generated states, 2 distinct states and depth 2. The failure
word closes from `x=1` to `x=0` even though a reached inner query needs `x'=1`.
These two pinned primary references qualify this tiny subset, not the original
Elevator model or a corpus gain.

The trace validator checks the original `Next` and closing edge on the word and
decides each query with the core evaluator's `ENABLED`
([ADR 0103](../decisions/0103-validator-enabled-via-core-evaluator.md)). A
reached query error is reported even when a later guard is false; ordinary
Boolean short circuit leaves unreached errors unreached. Use the dedicated
`RECOMMENDED_STACK` thread contract when calling trace validation directly from
Rust.

First-order temporal expression arguments retain their existing formula route:
`Identity(p) == p; Identity([](ENABLED A))` forwards a temporal expression,
including its caller scope, rather than capturing an operator. Direct queries
still undergo the usual planning and independent enabledness checks. This does
not admit query-bearing operator captures, native/provider arguments or hidden
state-helper query words.

State-level mixtures of direct queries keep their existing temporal conversion
and native INSTANCE support. Replayed queries use the actual INSTANCE and lazy
scopes, including enabled and disabled correlated assignments. Direct and instantiated WF/SF words cover continuous
and intermittent starvation, disabled stutter and recurrent closing occurrences.
General callable and planner integration remains unfinished.

Periodic word validation evaluates queries hidden behind helper formals, such
as `Guard(p) == p /\ TRUE; Guard(ENABLED A)`, with the core evaluator. A
query-bearing action label is checked like any other label; the original Next
edge, closing edge and property claims are still required independently.

Finite preflight keeps binder domains, instantiated function/set construction
bodies, filter predicates and UNCHANGED operands in data-only mode, following
actual lazy bindings before reading domains. As in existing concrete-binding
checks, a body with zero bindings may be left unvisited without evaluating its
queries. Other effect and purity guards still apply; this is not a global
syntactic query scan.
Boolean existential and universal bodies retain their bounded query support.

Finite preflight prepares all bounded domains and checks the complete binding
count before visiting a quantifier body. Concrete non-tuple quantifiers can
reuse one copy-on-write working context; every original body and guard still
runs. Captured contexts retain their old bindings. Tuple patterns and analysis
contexts keep eager construction and its original first-error order. These
entry/work bounds do not provide a runtime or process-memory guarantee.

Finite preflight visits every conjunct and every `IF`/`CASE` branch, including
ones that evaluation never reaches in the current state or edge. When a
binder's domain raises a TLC evaluation error there (for example a `CHOOSE`
with no witness behind a false guard), preflight skips that binder instead of
failing the atom: evaluation must compute the same domain before it can enter
the body, so it either raises the same error where TLC does or never reaches
it. A refusal that stands for such a TLC error is skipped the same way: applying
a multi-argument lazy function outside its domain is a TLC error whose message
dive does not reproduce, so evaluation refuses if it reaches it. Other refusals,
undefined identifiers (a missing state or successor) and resource limits in a
domain still stop preflight.

## INSTANCE query namespace controls

The separate `instance-temporal` corpus records ten tiny Boolean controls from
the pinned primary oracle before changing INSTANCE execution. These expose a
lexical distinction: `ENABLED I!A` applies an instance inside the query, whereas
an exported `I!Can` whose definition is `Can == ENABLED A` enters the query after
the instance bindings. They need not give the same answer.

The conflict action requires `y' = FALSE` and `z' = TRUE`, with both variables
instantiated by one caller variable. The direct and outside-query controls make
that conflict unsatisfiable. The exported-query controls instead violate `P`
in the initial state (TLC code 2107, exit 12), including when the caller has no
variables named `y` or `z`. These are initial invariant failures, not liveness
failures. A nested instance sends both substitutions through `q`; its query
returns false because the successor choices share that name. Flattening every
instance to the caller's root variable would lose these observed distinctions.

The guarded controls also separate an actual current-state value from a caller
variable with the same spelling as the imported declaration. Fresh substitutions
inside the query differ from captured bindings established before it. See
[the corpus guide](../conformance-corpora.md#instance-query-namespace-controls)
for the exact selection and generated oracle route.

The evaluator's ENABLED search represents its partial successor by canonical
variable name, including hidden names. It preserves the captured contexts and
branch cutoffs above. Ordinary graph states, fingerprints and trace variable
lists still contain only the root variables. Both semantic profiles use the
same query-name rules; no new flag or default divergence is introduced.

Rust callers can construct an actual partial query assignment with
`Evaluator::named_state`, using checked IR variable declaration IDs and values.
`StateRef::Named` retains it for scoped reads; `St::enabled_candidate`,
`enabled_current` and `primed_successor` preserve the complete state and shared
invocation counters. An absent name is unassigned and does not fall back to the
old current state. Public evaluation and context operations check both current
and successor named-state ownership before effects or retained captures, including
when a pure invocation already has a work budget. Named assignments from another
evaluator are refused. A root
projection of a query candidate is not an ordinary Next transition. See the
constructor's runnable rustdoc example and [ADR 0099](../decisions/0099-enabled-variable-name-states.md).

This is evaluator/checker query execution, not complete file-based INSTANCE
support. Independent INSTANCE query-word validation, general instantiated
temporal actions and refinement, callable substitution, and fairness word replay
remain required scope. Existing high-level planning/data guards and independent
replay refusals remain; the ten oracle cases do not themselves add native corpus
agreements. Composition with assigned hidden query names explicitly refuses
until its intermediate-state generator can retain them; existing root-only
composition keeps its original behavior. No new native agreement or independent
full-word validation is claimed.

## Lazy action sources in enabledness callbacks

The isolated `lazy-enabled-capture` oracle corpus records four controls for
[issue #312](https://github.com/Chelis-Lang/dive/issues/312). An exported
INSTANCE query assigns `a' = 1`, captures `LET Cut == a'`, and filters
`<<1, 2, 3>>` with `SelectSeq(..., LAMBDA e : e /= Cut)`. Requiring the result
to remain `<<1, 2, 3>>` makes that query false; requiring `<<2, 3>>` makes it
true. The outer invariant exposes the false query as an initial violation
of `Inv` (TLC 2107, exit 12). The true query completes with one distinct state,
two generated states and depth one (2193, exit 0). Constant and current-state
capture controls distinguish the creation context from the successor lookup.

These primary facts guide creation-context and query-namespace support. They
provide no native checking or independent replay acceptance. The corpus is
reported separately from the established 156-model population, and its generated
goldens retain their own source identity. Independent validation must establish
both genuine and forged words under the actual captured context and finite
covers; admitting an action-valued capture by syntax alone supplies no proof.

## Hidden-query lazy argument caching

The separate `lazy-enabled-cache` oracle corpus records six minimal controls for
[issue #313](https://github.com/Chelis-Lang/dive/issues/313). An exported INSTANCE
query assigns `b' = 42`, enumerates hidden `a' \in {1, 2}`, and calls
`Step(a')`. Its callback filters `<<1, 2, 3>>` using the lazy `Cut` argument.
Pinned default TLC reports disabledness when the final conjunct requires
`<<1, 3>>`, exposed as an initial `Inv` violation (2107, exit12). Requiring
`<<2, 3>>`, using only candidate2, inlining `a'` in the callback, or moving the
query to the root module each yields success (2193, exit0; one distinct, two
generated, depth one). Requiring the unchanged sequence remains disabled.

This result depends on observable primary caching behavior. Pinned
`LazyValue.getCachedValue` calls `TLCState.isSubset`, which compares only root
variable names; hidden `a` and `c` do not invalidate the retained lazy value.
The root collision `b` stays42 for both candidates. A paired research run with
`-Dtlc2.value.impl.LazyValue.off=true` changes only the remove2 control to
success. That JVM setting is diagnostic and supplies no oracle golden.
LazyValue captures its expression and lexical context; forcing supplies the
states and control flags, so adding creation-state snapshots is not a solution.

Native named-query lazy-cache checks compare the assigned root names, retaining
pinned TLC's observable behavior across hidden membership candidates. The six
controls check actual invariant evaluation in both profiles. Independent replay
compatibility remains open. Replay must evaluate original
source with its own bounded invocation state; checker caches or witnesses cannot
establish validity. The six inputs and generated goldens remain separate from
the established156-model population and do not increase its agreement count.

### Lazy query evaluation in replay

Replay evaluates lazy query sources with the core evaluator and its ordinary
caches, so a LET reached before a hidden membership choice keeps one binding
across the choices and one reached after it gets a binding per choice, as in
TLC. The isolated lazy-capture and lazy-cache controls do not change the
established 156-model population.

## Lazy query binding lifetime

The separate `lazy-query-lifetimes` oracle corpus records 24 primary controls
for the lexical lifetime issue investigated in [issue #313](https://github.com/Chelis-Lang/dive/issues/313).
An exported INSTANCE query assigns `b' = 42` and then enters a LET binding
`Cut == a'` before enumerating hidden `a' \in {1, 2}`. A SelectSeq callback
uses Cut to remove an element from `<<1, 2, 3>>`. Pinned TLC retains the same
lazy binding across both membership candidates: requiring `<<1, 3>>` leaves
the query disabled. Moving that LET after membership creates a binding for
each candidate and makes the same query enabled. Leading helper arguments,
delayed helper arguments, nested queries, sibling Boolean expressions and
false leading guards supply controls for this distinction.

The paired inverted invariants make the independent replay obligation explicit:
a genuine invariant-ending word must validate, and a fabricated ending must
fail. Recreating all LET contexts per complete candidate, or hoisting every
LET before all candidates, violates one of these controls. Source inspection
cannot supply the runtime context. The implementation must preserve each
reached lexical binding's lifetime across precisely the branches it encloses.

These primary-first fixtures and generated goldens establish the required
behavior before its implementation. They remain separate from the established
156-model population; no native or independent replay support is implied.
