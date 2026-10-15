# Temporal checking

The checker exposes bounded final/short analysis. Eligible ordinary TLC-profile
requests select it automatically; explicit selection is also available:

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

## Ordinary checking in the TLC profile

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

The route requires profile-default evaluation and fingerprint settings,
fingerprint polynomial 0, resident fingerprint storage, and no storage
budget/directory, deadlock, sequential or chaos overrides. Use `--fp 0`
explicitly: the TLC profile otherwise selects a random polynomial. Seed and
worker requests are retained. Checkpoint/recovery uses its existing separate
safety path. Randomness combined with VIEW or SYMMETRY, excluded options and
ordinary requests in the dive profile retain their existing routes and named
refusals. Real unsupported language features and resource limits remain explicit;
a refusal, bound or external timeout is never a holding result.

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
named refusals. Effects, temporal operands and ENABLED inside a pure atom,
including unused actual arguments, remain refused. Independent trace replay
checks the original configured calls separately.

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
