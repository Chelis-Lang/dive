# 0123: Separate partial terminal bindings from complete trace states

- **Status:** accepted (owner-approved bounded implementation)
- **Date:** 2026-10-30

## Context

The pinned clean BFS stream for `ErNextIncomplete` prints three complete states
and a fourth candidate with `x = 3` and `y = null` (unassigned). Its primary
result is 2109, exit 75, eight generated/five distinct states and depth three.
Both native producers previously retained only display text for that candidate,
so serialization dropped the entire word. A null assignment cannot be treated
as an ordinary TLA value, nor can it be replaced by a fabricated complete state.

## Decision

Retain actual partial bindings and the actual generating expression in the
producer's diagnostic state, separately from its complete values. Transport a
partial terminal request in native trace version 3; version 1 and 2 retain their
existing meanings and output. The outer check and validation reports remain
schema 3. The complete `states` array excludes the partial candidate. Logical
`length` includes it, and `TraceDoc::logical_length`/`Trace::logical_length`
expose that count. The closed terminal shape is:

```json
{"kind":"partial-state","ec":"TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT","endpoint":{"stage":"next-partial-state","source":{"module":"M","start":40,"end":49,"instance":null},"assignments":{"x":3},"unassigned":["y"]}}
```

Assigned and unassigned names must form a disjoint exact partition of the
model's variables. No missing slot becomes a native value. The source context
is untrusted comparison input, never an AST/action/scope selector. Unknown
fields, stages, or inappropriate versions fail closed.

The fallible source-bound Session first checks the complete prefix, including
all its safety obligations, in a private predicate runtime. It drops that
runtime, binds another fresh runtime with actual configured actions, decodes
the source state there, and runs the existing ordered reference generator.
The first incomplete candidate must match the exact bindings and original
generating source. An earlier genuine evaluation error refutes the claim;
completed generation without the partial candidate refutes it too. In this
first slice any earlier complete candidate remains unsupported: its ordered
fingerprint, constraint, invariant and action-check demands are not skipped.
The complete prefix retains the error-witness validation goal, which checks
configured safety obligations in order. Ordinary behavior validation deliberately
checks plain membership and cannot establish this error-witness prefix.

Keep the existing host, composition, runtime-name, VIEW/symmetry and temporal
refusals. Reuse cooperative evaluator control and typed resource transport;
interruption returns no completed validation report. This is not a hard memory
ceiling. Borrowed-model and infallible APIs cannot grant fresh source authority.
No checker/store/liveness dependency or second successor algorithm is added.

## Consequences

The ten historical missing-word fixtures now preserve structured words. Eight
validate in the first slice; `RoActions` and `SplitThrows` remain unsupported
because complete candidates precede the incomplete one. Init partial words,
those earlier candidate checks, and other terminal-error sites remain separate
work. Validation proves the finite word's claim, not BFS completion, global
deduplication/counts or diagnostic wrapper classes. Primary pinned comparison
and the unclassified/unknown boundary remain indispensable. No whole-issue or
corpus parity closure follows from this format extension.
