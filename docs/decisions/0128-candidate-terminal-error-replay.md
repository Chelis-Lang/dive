# 0128: Independently replayed candidate-side terminal errors

- **Status:** accepted
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

The version-4 [consumer notice](https://github.com/Chelis-Lang/oyster/issues/61#issuecomment-6526681302)
was posted before the initial implementation landed in #666. The extension below
uses the same endpoint, final-candidate interpretation and two replay obligations.

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
top-level non-Boolean role result, VIEW, symmetry, temporal model or executed effect remains a
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
initial notice is recorded above. The legacy infallible validation
entrypoints refuse the new request; fallible source-bound entrypoints read it.

The producer admits verified deciding-2103 tool/runtime candidate errors and
typed assertion, division-by-zero and integer-arithmetic overflow exceptions
with original source provenance. The last three families retain their actual
2132/2179/2178 identities rather than receiving the nested wrapper. Integer
overflow constructor provenance comes only from an actual integer module
operation, not a numeric code, message or general value-layer overflow conversion.
Set-size overflow remains outside that constructor proof. Fresh replay still
discovers the candidate and separately executes the diagnostic sequence.
The file producer now requires this actual controlled fresh replay before
retaining a verified result. A mismatched or unproved diagnostic sequence
returns unsupported with no formal word/class, preserving actual search facts
and the observed primary cause. It does not retag the primary to a guessed
diagnostic class. Two pinned minimal controls prove that division and overflow
can exchange primary/diagnostic roles while both physically exit 255. Canonical
shared 255 streams remain unclassified; only the strictly framed 2132/14 assertion
route gains owned-reader classification. The initial 92-agreement assertion was
rejected by independent review; 92 valid focused words do not imply 92 oracle
agreements. No endpoint shape or native version changes.

Sixteen existing source fixtures cover Choose, Domain, NonBoolIf, NonEnum and
TypeErr at constraint/action-constraint/implied-action sites, plus
`ErPropNonBool`, in both profiles/engines at one and four workers. Their pinned
primary headers remain unchanged.

The non-Boolean boundary concerns the operation actually reached, not a fixture
name or message. If a named role returns a non-Boolean value, the driver has not
proved that role's `isValid` classification and refuses it. `ErConstrNonBool`
and `ErActConstrNonBool` explicitly retain non-agreement. In contrast,
`ErPropNonBool` evaluates `[Err(x)]_<<x, y>>`; the evaluator's actual `StepBox`
constructor raises a source-qualified tool/runtime error when the inner action
is non-Boolean. The driver independently reaches that error, rather than
synthesizing a role failure from a returned value. Its pinned stream reports
2113 at that expression and diagnostic 2103 at the same source. Exact source,
wrapper and earlier-constraint-priority controls accompany this qualified case;
it does not prove every non-Boolean role error. Mutations of
bindings, source, site, underlying cause, wrapper codes, length and versions
must fail closed; controls exercise a genuinely different diagnostic error,
unknown newness, VIEW, effects and interruption. This supplies no whole-census,
TLC classifier or generic terminal-error completion claim. In particular, VIEW,
top-level non-Boolean checks and the other #434/#490/#499/#606/#607/#620 sites
remain bounded by their existing contracts.
