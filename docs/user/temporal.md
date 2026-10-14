# Temporal checking

The exposed checker is an explicit bounded final/short mode:

```sh
dive check docs/user/examples/Counter.tla --config docs/user/examples/CounterTemporal.cfg --lncheck final --lasso short --seed 7 --output json
```

The example reports a real liveness violation. Although `Next` always increments
before four, TLA+ temporal behaviors can stutter at zero; there is no fairness
condition in this model requiring the increment action to occur.

This mode explores the supported graph, checks once at the end, chooses a
deterministic short periodic witness and independently replays it in memory and
as a lossless document. It checks the complete configured property claims,
including the closing edge. The flags are explicit because this is not TLC's
periodic schedule, exact selected lasso or all evaluation-error timing.
A TLC value profile plus these flags is reported as `tlc+overrides`.

Pure supported boolean/temporal operators, finite quantification and WF/SF
predicates have scoped limits. Unknown ENABLED/fairness covers, runtime effects,
dynamic/exported scopes and other unproved constructs refuse. Ordinary strict
`dive check` retains its temporal refusal. A diagnostic planning/tableau/product
API that completes has not issued a temporal verdict.

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

Schema 2 distinguishes opt-in temporal checking from ordinary safety schema 1.
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
The process exits 255 under `--compat=tlc` and 13 in the default profile. Effects
and unsupported scopes still refuse before planning executes them.

Final checking also preserves binding errors: a missing `_PERIODIC` definition
reports TLC code 2229, and a parameterized definition reports 2228. Both exit
151 under `--compat=tlc` and 1 in the default profile. A valid `_PERIODIC` hook
still refuses by name until hook execution is implemented.

## Rust APIs

`dive_api::temporal` is the Rust entry point (see
[the temporal API contract](../temporal-api.md)). `dive-liveness` exposes the
kernel it uses: planning (`plan`), the tableau (`formula`), the product and
lasso search (`product`, `search`), the coordinator (`analyze`), TLC's liveness
formula classifier for `--compat=tlc` (`tlc_plan::classify`, TLC errors 2213
and 2214) and the growth trigger for future periodic checks
(`periodic_backend::Growth`).
