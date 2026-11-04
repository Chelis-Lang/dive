# 0150: Final Reference logical action-context costs

- **Status:** accepted
- **Date:** 2026-11-04

## Context

ADR 0146 deliberately measures native producer entries, not TLC costs. Pinned
`ACoverageTest` separates the same CHOOSE helper under two actions and counts
actual quantifier demand. Primary microcases additionally establish shared
predicate trees for `A(1) \/ A(2)`, separate helper paths within one action,
logical continuation costs, concrete-domain enumeration costs, prefetch and zero
rows for assignments in unvisited branches. These cannot be obtained by
relabelling static source entry totals. Pinned TLC rejects a missing `-coverage`
interval as usage exit 1 before model access; the previous unsupported-only
transport control is respecified as separate missing/nonzero interval controls.

The failed `FailDemand` control establishes a different boundary: TLC's reported
Init cost includes two invocations, although the model has one generated initial
state, because diagnostic reconstruction repeats it. Repeating producer effects
or counting the independent validator would violate dive's observation contract.

## Decision

Add explicit `check --coverage 0`, TLC `-coverage 0` transport and default-false
`CheckRequest.tlc_coverage`. Off is TLC's ordinary coverage-disabled setting;
on corresponds to final `-coverage 0` on the admitted slice. Both semantic
profiles retain their value semantics. This option and native `source_coverage`
are separate and mutually exclusive. Native reports and counters are unchanged.

Reuse the existing optional observer and binder. The logical collector prepares
an IR/source calling forest for the actual Init and split Next predicates before
generation. Repeated predicates share a tree; child and ancestor resolution
separates actual helper call paths. Exact declaration headers reuse the source
lexer and JavaCC positions, without changing the IR or trace wire schemas.
Expression demands, successful generator continuations, actual emissions and
checker-admitted identities update their respective logical counters. The
report uses the pinned source's logical tree collapse and assignment zero rows.
Assignment markers use the whole Next predicate's source locations, resolved
through formal actuals and aliases without evaluating them. The scan stops at
an equality/membership predicate and does not descend into its right-hand side;
the resulting source markers also apply to shared predicates in Init.

Literal set constructors retain only shared set identity plus their logical
source owner. Enumeration hooks count actual next-element demand, including
concrete ContextEnumerator prefetch and early termination. Observation never
hashes, compares, normalizes, prints or forces a value. Ordinary native thunk
caching is retained; the measured mode uses the existing lazy recipe instead of
pure strict/settled argument shortcuts that would erase TLC source demand.
This applies only inside a measured producer action: startup, action splitting
and paused diagnostic execution retain their ordinary recipes. Enabled/disabled
semantic fields and actual effects are compared directly.

The initial supported slice includes multiple scalar state variables, acyclic
first-order helpers and bounded scalar/quantifier safety INIT/NEXT runs with
literal/interval domains and interval-based powersets/differences. One Reference
worker is selected before work. Unimplemented roles, source constructs and
backend combinations refuse explicitly. Unsupported reached value cost maps,
collector capacity/saturation, or any non-successful run give a named coverage
refusal with no action costs; the existing model verdict, statistics, output and
independent witness stay intact. This is additive report-schema data, not an
incompatible trace migration.

## Consequences

Successful admitted logical rows are tested against complete pinned final cost
reports. Twenty-one separate primary models, including their ordinary TLC/SANY
facts, are regenerated from the same pinned oracle; no established golden or
population changes. Twenty successful controls retain the complete cost rows;
the failure control establishes the explicit reconstruction boundary.
Failed, stopped or unimplemented cost paths cannot claim
TLC cost parity. Variable-value summaries, remaining cost families, other workers,
Core/INSTANCE/PlusCal maps and DAP remain mandatory follow-up work.

The disabled route adds no observer storage allocation, atomic counter operation,
set retention or callback. Passing focused correctness tests is not evidence for
the outstanding instruction-overhead acceptance criterion or integrated
performance goals.
