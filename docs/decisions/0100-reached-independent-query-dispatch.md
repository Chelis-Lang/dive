# 0100: Reached independent query dispatch

- **Status:** superseded by 0103
- **Date:** 2026-10-16

## Context

ADR 0099 preserves the actual variable namespace of an existential query.
Independent counterexample validation also has to reach the query: four pinned
initial invariant violations in the `instance-temporal` corpus were refused by
the standalone evaluator. Static effects are not sufficient because an ordinary
lazy actual can contain ENABLED behind a Boolean or data formal whose body has
no `USES_ENABLED` flag. Applying the periodic purity preflight to every ordinary
predicate would change supported host operations and evaluation order.

## Decision

Allow an evaluation invocation to provide an explicit existential resolver. The
normal interpreter calls it only when it reaches ENABLED, supplying the original
action, actual unbranched lexical scope and current evaluation state/control.
The callback does not receive a checker successor, graph or enabledness result.
Calls without a resolver retain the existing optional-enumerator behavior.

The trace validator installs its own resolver for ordinary predicate evaluation.
It uses the independent finite-cover engine, which branches the lexical query
context once and checks original relation membership against named candidates.
Sibling reached queries share expression, assignment and pure predicate work;
nested queries are interpreted by the same search. The resolver is stripped
before independent evaluation, so nested work cannot recursively re-enter its
coordinator or silently call the checker. Unsupported data-hidden queries inside
an independent search remain explicit refusals until its interpreter supports
them; this does not establish general callable or temporal INSTANCE parity.

Invocation state carries resolver identity through priming, UNCHANGED, lazy
forcing and other state projections. Thunk caches require the same resolver
identity in addition to the existing state/control checks. A cached checker or
different client's answer cannot supply an independent answer. Cached identity
is weak. The validator resolver holds a weak model reference, with its owner
kept alive by the validation run, avoiding a new evaluator/cache ownership cycle.
Foreign invocation contexts refuse before entering client code.

## Consequences

Ordinary data, argument forcing, source short circuit, errors and host operations
continue through the core interpreter. Independent query completeness and limits
remain the trace crate's responsibility. No purity certificate, admission cache,
proof receipt, new dependency or graph-based truth is introduced. Periodic
formula traversal retains its separate independent interpreter and purity rules;
its namespace, callable and fairness integration must be qualified separately.
