# 0065: Service a genuine request after its actual committed prefix

- **Status:** accepted
- **Date:** 2026-10-11

## Context

Issue #204 publishes returned contiguous work under real pause authority, but its
legacy controller is source-ordinal and can stop before append. A genuine empty
request has no source callback and therefore no controller service. Issue #208
needs a separate service transaction without inventing source/EOF authority.
Pinned real ModelChecker.doPeriodicWork controls preserve 18 empty and 9 written
selections plus every failed adapter/EOF-race epoch. They are controlled method
observations, not ordinary schedule parity.

## Decision

Add `Snapshot::execute_service` without changing the old APIs. Source callbacks
only publish graph data. Shared commit_sources completes ordered safety/counter/
output/source publication and physical frontier append, then services the actual
request once under the existing borrowed mutex. Retain request-owned metadata,
real initial/current stats and optional last graph source. Empty requests inspect
real existing registrations; do not create source zero, a written node or EOF.

Pending request admission is producer-owned and not filtered again by source elapsed
time. Request initial-force and controller liveness-force remain separate. Preserve
candidate-before-hook and error/stop cleanup order. Original shared quotas are charged
before retention and no clock/default/profile/ordinary checker behavior changes.
Exact requested evaluator options must match the retained TLC evaluator.

## Consequences

This diagnostic closes whole-prefix and empty service composition only. No Holds,
ordinary temporal verdict, queue/count/schedule certificate or family upgrade.
Candidate words require fresh independent memory/JSON replay. Ordinary `_PERIODIC`,
checkpoint and effects stay refused. See the [Cargo-run user contract](../request-owned-periodic-service.md).

## Controlled scheduling addendum

The new candidate-before-hook control uses the explicit `dive-check/test-rendezvous`
dev feature. A scoped thread-local guard is propagated only to its exploration
thread. After an actual `BlockOut` is stored, the first returned worker signals
the real timer and waits at most five seconds for its internally produced request.
The guard supplies neither a Request nor a clock sample. Unwind/closed-lane
controls restore scoped ownership and do not fabricate pending work. Default
production/release dependencies and standalone trace builds exclude the feature.
Workspace test feature unification is separately qualified, not ordinary schedule
evidence. Natural timing/resource failures remain preserved alongside this
controlled nonempty prefix and its fresh independent replay.
