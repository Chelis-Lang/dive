# 0052: Native periodic lifecycle, clock and controller seam

- **Status:** accepted (bounded diagnostic boundary; ordinary CLI remains unsupported)
- **Date:** 2026-10-11

## Context

Issue [#189](https://github.com/Chelis-Lang/dive/issues/189) requires more than the
source-owned explicit checkpoints in [ADR 0051](0051-source-qualified-periodic-backend.md).
The pinned controller and runtime-ratio primitives have separate, closed primary
observations. Selected real `DiskStateQueue` suspension/finish controls also
exist, but joined native source waves do not establish Java queue suspension.

Native workers and their coordinator share a mutex/condition-variable job pool.
The coordinator alone dispatches jobs. A completed job barrier establishes that
no submitted job remains active; it does not establish that the logical frontier
is finished, nor that an ordinary timer requested a check. Later source results
in the fixed wave can already exist while the logical prefix is incomplete.

## Decision

Connect internally sampled producer-owned wall-clock lifecycle and the pinned
controller to real source-owned growth and partial SCC operations. No caller
clock, hook, graph or claimed validation flag may establish a model result.
Operational quiescence must retain the actual pool dispatch authority through
the controller callback and distinguish it from a mere copied joined receipt.
Finished/empty-frontier and stopped execution paths remain separate from a
successful suspension. Source-prefix counters exclude speculative later work.

Initially refuse PERIODIC expressions and checkpoint transactions by name;
do not install successful placeholders for those hooks. Preserve original
bounds, model/arena ownership, source error precedence and worker facts.
Internally obtained wall-clock samples must preserve Java subtraction/ratio
arithmetic and return typed failures when the relevant clock contract cannot be
established. Caller-supplied samples remain diagnostic arithmetic only.

Each deciding candidate requires fresh independent periodic-word, in-memory and
JSON replay. Complete safety exploration must execute actual final temporal
checking; an empty queue or a successful partial check cannot alone report
Holds. Ordinary CLI admission stays unchanged until those execution contracts
are implemented and exercised against the pinned source.

## Consequences

The implementation retains the exact f4c3ed0 controller/ratio component epoch,
765 source-owned backend delivery, and their earlier failures separately. New
native lifecycle/clock controls will record their own source and observations.
The external queue controls are selected real queue-method evidence, not model
or unrestricted scheduler proof. Timer-driven stop counts are not inferred from
joined-wave metadata, and no broad family certification is awarded by this seam.
