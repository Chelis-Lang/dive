# 0074: Preserve a genuinely absent configured periodic hook

- **Status:** accepted; bounded diagnostic implementation
- **Date:** 2026-10-11

## Context

Issue #220 removes mandatory hook injection from original-source diagnostics.
Pinned `SpecProcessor.processPeriodic` retains null when no role is configured;
unused FALSE or argument-taking definitions do not create a hook. The complete
26-row [primary selection](../../conformance/probes/optional-periodic-hook/primary/README.md)
retains name/arity, actual constant-role exception, empty-state evaluation,
startup and selected real controller/queue observations.

## Decision

The shared binder has Ordinary, Required and Optional internal modes. Existing
mandatory snapshot APIs stay required. An additive private-field optional token
owns the original configuration and actual pre-constant selection, including
None. No caller boolean, supplied predicate, default callback or synthetic TRUE
creates absence. Every exposed diagnostic model retains the exact ordinary-check
refusal marker, including the absent case. Execution revalidates source and bound
context, then removes only that exact producer marker from its private clone.

Use one request service and one candidate reconstruction/rehoming/fresh binding
and independent memory/JSON replay path. None omits only the original hook stage.
Real initial progress publication and forced liveness are distinct policies: an
unforced no-hook service may skip suspension/resume even when a request exists.
Empty request ranges never imply EOF. Existing ordered safety/graph/frontier
publication, candidate-before-hook and actual EOF remain authoritative.

## Consequences

Startup grammar/effect/name/ownership checks and all original policy bounds stay
unchanged. Inspection reservations are not evaluator instruction, heap or byte
bounds. The optional producer remains TLC-profile-only. Unsupported state/action/
temporal hook bodies refuse before evaluation; they earn no reference error-class
credit. Checkpoint/ordinary clock, queue, stopping-count and model-proof contracts
remain unfinished. No ordinary CLI, Holds or family-certification promotion.
See the [executable Rust guide](../optional-periodic-hook.md).

The Init division control preserves direct doInit2179 separately from actual full
ModelChecker GENERAL1000/inner2179/nested2103/exit75. Native capture already follows
the latter deciding-class contract; an initial incorrect test assumption is retained.
