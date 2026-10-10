# 0034: Ordered temporal execution and the strict witness boundary

- **Status:** accepted
- **Date:** 2026-10-10

## Context

Completing safety capture before temporal graph construction loses TLC's first
error order. A temporal predicate at one source can fail before a later source's
safety or enumeration error. Product nodes discovered after a fingerprint is
expanded also require immediate re-enumeration of the original concrete values.

## Decision

Attach a layering-neutral `SourceObserver` to the existing safety collector.
Initial callbacks run after insertion/enqueue, before initial invariants; source
callbacks run after that source's safety checks and before selecting later-source
stops. Safety expansion really uses the requested deterministic pool. Predicate
and graph commits use one source order and one `SetOfStates` capacity history.
Speculative batch emissions remain separate from logical stop counts.

`phase_execution::execute` binds a private Session observer. Each OOS evaluates
ordered action/state bins, then tableau consistency and graph insertion before
later OOSs. Graph records distinguish registration from actual writes. Done-state
recursive visits enumerate complete action vectors before constraint/predicate
checks, retaining original concrete representatives and separate work accounting.
These bounded graphs may supply SCC acceptance inputs; that alone grants no model
verdict. Configured/action identities and derived role names remain separate.

Preflight, retained-entry/cell/inspection and recursion ceilings stay fixed.
Opaque/effectful/introspective/runtime-name captures remain refused; predicates
are never invoked to guess purity. Ownership admission precedes planning and
execution. Typed deciding errors, actual workers and written prefixes survive
stops. Missing execution evidence remains absent. Public user observer callbacks
cannot attest to TLC equivalence.

## Consequences

The complete 26-row raw graph selection, eight clean TLC error-order controls,
15 completed field comparisons at four pool widths and independent periodic-word
mutants are retained. Initial callback-prefix counts are not TLC's doInit replay
final counts. Every previous source matrix and raw attempt remains available.

The accepting-sink/empty-PEM witness path, exact TLC SCC traversal and lasso
selection, complete error/initial replay and periodic schedule are still required
before strict integration. Each unsupported boundary remains named. F-020 through
F-022 remain partial/audit-required; F-023 stays missing. No default, conformance
claim or strict checking behavior changes in this intermediate increment.
