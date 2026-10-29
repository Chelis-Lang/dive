# 0120: Independently replayed Next errors before the first successor

- **Status:** accepted (owner-approved bounded implementation)
- **Date:** 2026-10-29

## Context

Issue #434 retains finite words whose final source state raises an error during
successor generation. An anonymous error code does not identify its source or
distinguish Next from constraints, implied actions, VIEW, or diagnostic replay.
Matching checker reports cannot establish these obligations. ADR 0103 permits
sharing the reference evaluator, including its successor operations, while
keeping checker, store, simulation and liveness dependencies forbidden.

## Decision

Use ADR 0113's optional error endpoint pattern, retaining native trace version 1
and report schema 3. The closed stage `next-before-successor` carries an original
source context and the underlying error code:

```json
{"kind":"error","ec":"TLC_MODULE_DIVISION_BY_ZERO","endpoint":{"stage":"next-before-successor","source":{"module":"M","start":40,"end":49,"instance":null}}}
```

Only a typed native Next error with an observed source origin can emit this
request. The descriptor is untrusted comparison input. It selects no action,
expression, scope, cache, or error to evaluate. The validator reconstructs the
configured actions from immutable inputs and runs them in their actual order.
Its first actual evaluation error must match the requested code and source.
Instantiated error origins remain unsupported in the first slice.

First validate the supplied finite path and all earlier safety obligations in
a private source-bound session. Terminal generation uses another freshly bound
runtime and cache/name owners, decoding the last state into that runtime. It
uses the existing evaluator's ordered successor generator with a sink that
stops at the first complete or partial candidate. Any candidate or intermediate
composition event before an error remains unsupported. Complete generation
without an error refutes the claim. This lane therefore needs no visited set,
checker functor, successor retention, or duplicate search engine.

Retain existing effect, random, register, runtime-name and composition refusals.
The first lane also refuses VIEW, symmetry and temporal requests whose earlier
obligations it cannot establish. Reuse the evaluator's cooperative controls and
resource limits, with fallible endpoint-owned allocations and polls at phase
boundaries. This is no hard process-memory ceiling. Unproved allocation paths
remain unsupported when identified; no replacement finite-cover interpreter is
introduced. Resource, interruption and internal execution failures retain the
fallible validator's typed transport and produce no completed validity report.
Borrowed-model and legacy infallible validation cannot mint this fresh-source
proof.

Independent word validity supplies no certificate for secondary diagnostic
replay, primary wrapper classification, BFS counts, or exploration completion.
The harness may compare an identical reported cause only after mandatory
independent replay. It gains no blanket 2103 exemption or classifier override.
Unclassified oracle streams remain unknown. Constraint/action/VIEW errors and
missing partial-state words (#607) retain their existing boundaries.

## Consequences

Existing trace meanings and versions remain unchanged. Old readers retain the
generic unsupported error; unknown endpoint stages fail closed in current
strict readers. No new consumer migration process is needed. Positive pinned
controls and mutations of state, code, source, first-candidate priority, missing
errors, host effects, cache warmth and interruption must exercise the actual
fresh source-bound entrypoint. Any agreement gain requires its own measured
observable and word comparison; implementing the endpoint alone earns none.
