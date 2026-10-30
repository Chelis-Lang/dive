# 0126: Attribute liveness property names to the actual validated word

- **Status:** accepted (owner approved, 2026-10-30)
- **Date:** 2026-10-30

## Context

ADR 0086 permits a different independently valid lasso from TLC while requiring
observable property attribution. The pinned TLC itself computes the complete
false-property set on its printed concrete lasso: `Liveness.findViolatedProperties`
visits configured properties in order, directly evaluates them on that word,
and `LiveWorker` prints the resulting list. It is not a union of every property
violated by some behavior of the model.

The primary `Liveness` control prints a six-state TLC word naming XReachesN,
AlwaysEvXN, LeadsTo and QuantProp. Dive's four-state word correctly names only
XReachesN, AlwaysEvXN and QuantProp. Both words independently validate; adding
LeadsTo to dive's word is invalid because that property holds there. The minimal
`Mutual` control has two globally violated eventualities, but TLC's word names
only one. `Identical` establishes the two-name display format. These three inputs
have normally generated TLC/SANY goldens under `word-relative-liveness`.

## Decision

For liveness, names are relative to the actual reported behavior. The JSON
`property` display, human headline and Rust word's `properties` describe the
same complete configured false-property list. Display names use the pinned TLC
singular, two-name conjunction and Oxford-list formatting. The existing Rust
`target_property` remains the analysis target, distinct from complete attribution.
No global union, copied TLC list or SCC/word-selection emulation is introduced.

A different name list from TLC may agree only after fresh source-bound independent
validation establishes the implementation's whole word and its complete exact
attribution. The reported display must equal the names in that independently
validated word. Empty, unknown, duplicate, omitted or false claims, mismatched
reported names, missing words and refused/interrupted validation provide no
exception. A generic `TraceCheck::yes` does not establish this attribution.

The context-free result comparator stays strict. The conformance pipeline may
remove only its liveness `violated_property` difference after the existing
independent trace check establishes the exact names. Shadow comparison requires
both independently validated words and matching reported-to-word attribution
before allowing different liveness name lists. Safety/invariant/action-property
names, verdicts, counts, depth, deciding error classes and physical exits retain
their existing rules. Worker determinism requirements are unchanged.

## Consequences

This clarifies the owner-approved observable-results contract rather than
registering a semantic divergence. It does not permit an unproved name, make an
unsupported word valid, relax a counterexample requirement, or certify the whole
liveness language. Reports remain schema 3 and trace formats are unchanged; the
corrected display is the intended property-attribution field, with the structured
`properties` list retained for automation. No new option or dependency is added.
