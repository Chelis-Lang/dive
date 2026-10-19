# 0099: Name-addressed existential successor states

- **Status:** superseded by 0102
- **Date:** 2026-10-16

## Context

The pinned TLC observations recorded by the separate `instance-temporal` corpus
distinguish an exported `ENABLED` from a query surrounding an instance.
`Pair WITH y <- x, z <- x` can assign hidden successor y and z independently
when its own exported query hides the older substitutions. A nested query can
instead retain two lazy targets naming the same hidden q. A root variable with
that canonical name shares the candidate slot. Current-state guards still read
their actual captured source context. Fresh substitutions entered after a branch
are visible; older captured environments retain their own branch depth.

These affect Boolean results and initial invariant failures, so they are language
semantics under ADR 0086. A root-only vector cannot represent all of them.
Preceding corpus/docs PR #300 records ten finite cases, 23 modules and 33
normal-harness-generated pinned goldens before this runtime change.

## Decision

Add immutable `NamedState` assignments to the core evaluator alongside indexed
`StateRef::Vals`. Named states are local to an existential query and keyed by
canonical IR variable-name text, without interning TLA+ value names. Missing
entries are unassigned. A root read and an unresolved hidden declaration use the
same core variable-read operation. Ordinary Init/Next, graph storage,
fingerprinting, trace variable lists, VIEW and symmetry retain indexed roots.

Checker ENABLED retains its existing branch placement, source-ordered search,
guards, callbacks, error behavior and work counters. Its successor assignments use
canonical names. Lazy target resolution preserves the real captured environment;
it does not flatten exported substitutions into caller aliases.

Cache inclusion requires the same named-state evaluator and compatible assigned
name/value pairs. Indexed and named representations never establish mutual
inclusion. Priming moves the complete successor, including hidden assignments,
and preserves invocation-local counters. Stored-value traversals include named
values and actual slot counts; recursive captures compare named-state identity.

Expose small checked operations for named construction/readback, dynamic query
branching and substitution projection. Projection distinguishes absence from a
present Unset slot and selects variable cutoff from actual evaluation control.
These operations do not establish purity, finite covers or successor existence.

Composition keeps its existing root-state implementation. It can project named
assignments only when every assigned name is a root name; assigned hidden-name
composition refuses explicitly instead of dropping assignments. This does not
remove any formerly supported hidden-name route. General composition over hidden
names remains unfinished language support.

## Consequences

No dependency edge, unsafe code, oracle/golden, comparison, model time limit or
resource policy changes. Both profiles use the same corrected language semantics.
This draft is not a claim of completed conformance or new original-model gains.

The independent trace evaluator may consume the plain state representation and
checked lexical operations, but must implement its own target resolution, covers
and membership search. This PR does not give the checker search to the validator.
Independent namespace-aware EXISTS, ordinary query-containing violation
validation, temporal/callable traversal and complete WF/SF words remain required
follow-up work. Full general INSTANCE parity remains open.
