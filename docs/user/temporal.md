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
chaos flag. Seed and worker requests are retained. Other options and randomness
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

Final checking also preserves binding errors: a missing `_PERIODIC` definition
reports TLC code 2229, and a parameterized definition reports 2228. Both exit
151 in both semantic profiles; `--exit-codes native` selects 1. Binding and
symmetry-preparation model errors report the `binding` execution stage, including
when an empty property list takes the ordinary safety route. Engine model errors
retain the `capture` stage. A valid `_PERIODIC` hook
still refuses by name until hook execution is implemented.

## Rust APIs

`dive_api::temporal` is the Rust entry point (see
[the temporal API contract](../temporal-api.md)). `dive-liveness` exposes the
kernel it uses: planning (`plan`), the tableau (`formula`), the product and
lasso search (`product`, `search`), the coordinator (`analyze`), TLC's liveness
formula classifier for `--compat=tlc` (`tlc_plan::classify`, TLC errors 2213
and 2214) and the growth trigger for future periodic checks
(`periodic_backend::Growth`).
