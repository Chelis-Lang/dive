# 0128: Independently replayed candidate-side terminal errors

- **Status:** proposed (implementation prepared; landing requires consumer notice)
- **Date:** 2026-10-30

## Context

The fixture census at `68dd8266` refutes ordinary error words whose underlying
cause differs from the report's deciding wrapper. Plain `error` retains its
meaning: expanding the last state fails. A constraint or implied action can
instead fail while checking a generated candidate, which is already the last
row of the reported word. Replacing its underlying cause with 2103, or exempting
matching metadata from header checks, cannot establish either event.

ADR 0125's independent ordered driver already executes the actual candidate
checks and TLC's per-action enumerate-before-check diagnostic sequence. Its
Next stage deliberately refuses primary errors at other evaluation sites.

## Decision

Introduce native trace version 4, reserved for the new closed
`candidate-ordered-terminal` endpoint. The outer operation report remains schema
3. The last complete row is the generated candidate; preceding complete rows
are its source prefix. Logical length remains the complete row count. Example:

```json
{"format":"dive-trace","version":4,"variables":["x"],"states":[{"action":null,"values":{"x":0}},{"action":null,"values":{"x":1}}],"ending":{"kind":"error","ec":"GENERAL","endpoint":{"stage":"candidate-ordered-terminal","site":"constraint","source":{"module":"M","start":40,"end":49,"instance":null},"reported":1000,"deciding":2103}}}
```

The exact closed site vocabulary is `constraint`, `action-constraint`, and
`implied-action`. Source, site, underlying error, reported code and deciding code
are untrusted comparison requests. None chooses an expression, candidate,
action, scope, cached value or error during execution.

Fresh source/configuration reconstruction validates every prefix state and its
configured safety obligations. A separately rebound runtime enumerates actual
configured actions and candidate callbacks in order. Earlier completeness,
constraints, action constraints, materialized fingerprint and safety demands
retain ADR 0125's priority and refusals. Its first actual candidate error must
match the requested site, original source, underlying code and exact final
bindings. Primary wrapper identity follows the actual error constructor and
site (2113 for an implied action), never rendered exception text.

The same private terminal runtime then runs actual diagnostic generation,
enumerating each action before checking its candidates. Its first supported
error determines the deciding identity from its typed constructor. A different
secondary candidate site remains unsupported; an actual Next error can decide
instead. An unknown constructor, global newness/alias requirement, unproved
non-Boolean check, VIEW, symmetry, temporal model or executed effect remains a
refusal. Resource, interruption and internal failures transport no completed
validity report. No checker/store dependency or BFS statistics proof is added.

This is a new terminal interpretation, rather than more evidence for expansion
of the last state. Versions 1/2/3 therefore cannot carry it; version 4 cannot
carry an old ending. Existing plain error and Next stages retain their meanings
and strict report-header checks. The conformance harness compares the claimed
deciding identity with the report before mandatory fresh independent replay.

## Consequences

Readers must support version 4, its distinct candidate semantics, exact site
vocabulary and both replay obligations before advancing an immutable producer
pin. They must reject unknown versions/stages and cannot relabel version 4 as
version 1. Advance notice to known consumers is required before landing; this
ADR does not record or authorize that notice. The legacy infallible validation
entrypoints refuse the new request; fallible source-bound entrypoints read it.

The producer admits only verified deciding-2103 tool/runtime candidate errors
with original source provenance. Assertions and direct module errors, including
DivZero and Enabled controls, retain their old unproved endings.

Fifteen existing source fixtures cover Choose, Domain, NonBoolIf, NonEnum and
TypeErr at constraint/action-constraint/implied-action sites in both profiles
and engines. Their pinned primary headers remain unchanged. Mutations of
bindings, source, site, underlying cause, wrapper codes, length and versions
must fail closed; controls exercise a genuinely different diagnostic error,
unknown newness, VIEW, effects and interruption. This supplies no whole-census,
TLC classifier or generic terminal-error completion claim. In particular, VIEW,
top-level non-Boolean checks and the other #434/#490/#499/#606/#607/#620 sites
remain bounded by their existing contracts.
