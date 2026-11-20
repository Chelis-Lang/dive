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

## Temporal property edits

A separate typed API retains Core's complete native CSR with representatives,
actual concrete generated-target rows, implicit stutters and labels. It records
the sizes and counters Core observed at each due completed-level boundary.
Replay borrows the original append-only arrays through each recorded boundary.
The closed source offsets are final. Every then-unexpanded source has an empty
outgoing row. The standard CSR checker validates each reconstructed view.

New scalar predicates use the newly bound model's store. Only inline integers
and Booleans can be read across the store boundary. Once all ordinary periodic
checks continue, run the ordinary final product, SCC, attribution and independent
word validation. Any terminal prefix or source-row route defers to fresh checking.
The complete graph cannot certify the final verdict of a check that stops earlier.

A complete native temporal Holds may also qualify retention of exactly its
first original property, given two freshly bound closed inputs and an
authenticated previous report. The state cap excludes scheduled prefix
analysis. Core's final search completes each product in order; the first
property starts with empty fairness-column reuse. Both inventories must pass
ordinary startup planning, and their retained bound formula, fairness atoms,
transition inputs, argument slots and name domains must agree. Requiring an
actual fallback-free, unaudited previous execution avoids treating native
compilation eligibility as an execution receipt. This contract returns the
retained name. The embedding application authenticates the previous worker,
input and report before using its verdict. See the
[Rust embedding guide](../user/rust-api.md#retain-the-first-temporal-property).
