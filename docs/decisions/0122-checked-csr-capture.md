# 0122: Checked CSR producer evidence

- **Status:** accepted
- **Date:** 2026-10-29

## Context

L5 must retain persistent original state rows and compact adjacency without
constructing a second graph of evaluator values. The independent temporal state
source and borrowed product CSR already exist. The graph-independent
completion work separated successful producer facts from the retained reference
graph (`docs/developer/core/11-liveness.md` §7.3). Native capture
still needs a checked boundary between opaque retained rows, adjacency metadata
and those facts. A well-shaped prefix is not proof of exploration completion.

## Decision

`capture::CsrCaptureInput` borrows producer-owned arrays. Dense representatives,
original target row ids, ordinals, action labels and implicit stutters have
separate columns. Original positions are physical generated-edge positions,
excluding the final stutter; ordinal gaps and duplicate destinations survive.
Opaque row ids establish identity, not semantic equality or value admission.

`CsrCapture::checked` checks dimensions, offsets, bounds, ordered roots and
ordinals, closed source rows, stutters and the empty unexpanded frontier. Its
scan cooperatively polls the actual run's `EvaluationControl`. Only the control's
typed interruption and matching observed exhaustion can become a budget stop.
The input's borrowed arrays cannot be mutated while the checked view is alive.

`CsrComplete::from_producer` rejects the original non-success safety verdict
before interpreting metadata. It additionally requires actual statistics,
successful EOF with no queued work, matching distinct count, all source rows
closed and the configured capture limits. Reference and CSR producers share the
completion header checks and create the same sealed `CompleteEvidence` and
`GraphCompletion`. `CsrPrefix` instead requires successful committed-prefix
statistics and an unfinished frontier, and creates only `PrefixEvidence`.
Neither shape nor producer completion certifies temporal acceptance. The
temporal checker still selects a word and independently validates it.

The producer remains responsible for supplying its actual safety result and
matching source arrays, performing controlled value admission, retaining the
exact originals and enforcing the existing emitted-candidate limits. As with
`GraphBuilder`, this is an API for trusted in-process producers, not an
authentication mechanism for caller-invented search histories.

## Consequences

Completion facts can detach from either representation without cloning state
values or reconstructing a `BehaviorGraph`. Mutation tests and real reference
captures qualify the metadata boundary independently of native storage.

This decision does not enable a native temporal producer, raise entry caps,
bound aggregate bytes or claim a performance improvement. Persistent-row
retention, controlled conversion, report routing and whole-run memory evidence
remain required before native capacity or performance claims.
