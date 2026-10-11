# 0051: Source-qualified periodic backend checkpoints

- **Status:** accepted (bounded backend diagnostic; ordinary schedule remains unsupported)
- **Date:** 2026-10-11

## Context

Issue [#189](https://github.com/Chelis-Lang/dive/issues/189) advances the ordinary
periodic backend beyond the bounded partial SCC diagnostic in ADR 0048. The
pinned `LiveCheck.doLiveCheck` and `check` compare each OOS's node-pointer-table
size with its recorded check size. `DiskGraph.size` counts written fingerprint
keys. `TableauDiskGraph.size` counts fingerprint buckets, including registered
particles and done-only buckets; several product identities share one bucket.
These sizes are neither physical file bytes nor counts of written product nodes.

The [complete reference selection](../../conformance/probes/periodic-backend/README.md)
contains 32 normal actual-source runs, four actual closed-disk failures, and
actual pointer-table/restoration/threshold projections. The original first
32-row capture and failed first clippy source remain preserved. A forced empty
null graph can record size zero, so subsequent zero/zero growth is NaN and not
due. `LiveWorker.call` records size after its PEM loop, including a found
candidate; a thrown check bypasses that update. Later unprocessed OOSs keep their
old snapshots. Successful checks restore pointer links; a candidate stops before
the outer restoration stage.

The callback-order controller (ADR 0050) and bounded partial SCC selection do
not establish graph-growth authority, real worker suspension or ordinary
clock-based stopping. A source callback is emitted only after all submitted
jobs in the native fixed source wave have joined. That fact is useful backend
input but differs from Java's `StateQueue.suspendAll` and its finish flag.

## Decision

Add a model-owned `periodic_backend::execute` diagnostic. It accepts a bound
model and ordinary search/retention limits, never a caller graph, write mask,
claimed size or validation flag. At successful full initialization and every
whole source commit it computes source-qualified pointer sizes and executes an
explicit `LiveCheck.check`-equivalent request. It retains before/after snapshots,
OOS/PEM selections, real logical counters and private joined-wave facts. The first
candidate stops actual capture before later source commits and remains
unvalidated diagnostic evidence (`DIVE-L0054`), not a violation verdict.

The default explicit request is unforced, growth-based with threshold 0.1.
`Schedule::Final` skips unforced checks, while forcing overrides that branch.
Force applies to each supplied request; this API does not clear the outer
periodic-controller force flag. NaN/infinity and strict equality boundaries
follow the pinned comparisons.

An additive initialization-complete observer callback runs only after successful
initial enumeration/checks, before a pool exists. Source receipts carry a
producer-owned barrier recording joined submitted jobs and actual pool width.
Neither receipt certifies Java queue suspension. Existing observers preserve
all prior ordering and stopping behavior.

Graph construction, pointer projection, snapshot/receipt reservations, SCC and
witness work share the existing graph-step quota. Predicate/value work keeps its
separate unchanged Phase-L limits. Java's signed-int pointer-table count is an
explicit structural bound. These are entry/inspection limits, not aggregate
heap/byte guarantees.

## Consequences

A successful partial check never emits Holds, completion, a proof certificate or
ordinary strict-mode credit. The ordinary CLI schedule remains unsupported;
explicit final-only checking stays separate. Real clock triggering/ratio
sampling, queue suspension/finish races, PERIODIC/checkpoint transactions,
worker-dependent Java OOS assignment and independently certified ordinary stop
witnesses remain integration obligations. Controlled Java IO failures establish
snapshot timing only and are not native error-class agreements.

The [user guide](../periodic-backend.md) provides a Cargo-owned example, exact
outputs, bounds, refusals and proof qualifications. F-023 remains missing and
uncertified; F-020/F-021 remain partial/audit-required. No defaults change.
