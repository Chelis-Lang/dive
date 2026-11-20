# 0160: Recheck edited safety invariants in discovery order

- **Status:** accepted
- **Date:** 2026-11-20

## Context

Maintaining a finite model often changes the properties more frequently than
the transition relation. A complete previous check certifies its old
invariants. To decide a new invariant, the checker needs the actual states.
A finite witness of a previous failure also leaves earlier sibling states
unchecked against the edit. A sibling can now raise an error, or a changed
transition can hit a search limit before reaching the old witness.

## Decision

Expose an opt-in, in-process Rust API for single-worker exact Core. Bind the
actual current source closure and configuration on every edit. Admit changed
predicate bodies only for configured, isolated zero-argument definitions whose
names occur nowhere else in the loaded source. Require all remaining source
and all transition-affecting configuration, including limits, to match.
Conservative misses run through ordinary checking.

An opaque capture owns the original discovered rows, parents, generating
actions and counters. An exhausted closure can prove all edited invariants.
A refutation prefix can prove an edited failure up to its original endpoint.
Scan in discovery order and configured invariant order. Independently validate
every returned trace, including its current named ending. Earlier evaluation
errors require a fresh verdict. Count original exploration separately from the
work performed by the scan.

## Consequences

The first implementation admits scalar integer/Boolean rows and has a caller-set
row cap. The artifact has no serialized representation or cross-build identity.
Its storage and capture overhead pay back when repeated transition generation,
queueing and identity checks cost more than evaluating the edited predicates.
Checking temporal properties needs the complete labelled edge relation.
