# 0054: Bounded pure supplied periodic expressions

- **Status:** accepted (diagnostic slice; configured binding/ordinary schedule remain refused)
- **Date:** 2026-10-11

## Context

Issue [#189](https://github.com/Chelis-Lang/dive/issues/189) connects the pinned
controller to native source-owned graph/clock/dispatch authority. ADR0052 initially
refused PERIODIC and checkpoint callbacks. The complete59 new actual pinned
`ModelChecker.doPeriodicWork` controls show hook evaluation after successful
liveness, before checkpoint/resume, with empty states/Clear and FALSE equality.
The configured binding snapshot remains a separate model-binder obligation.

## Decision

Compose only a bounded pure explicitly supplied predicate with the actual native
lifecycle. Preserve ordinary `_PERIODIC` binding refusal, existing CLI/defaults,
model-proof=false and checkpoint refusal. Callback-free ownership/scoped grammar
proof precedes clocks/workers/evaluation. Refuse state references even in dead
branches, effects/opaque captures, configured replacements, recursion and
unbounded constructors. Admit only original bounded stored material and finite
syntax with a small TLC integer/boolean whitelist.

Charge inspected text bytes before scans within the unchanged 50k/128 grammar
and capture ceiling. Reserve checked cubic inspection weight plus one receipt
unit before each evaluation against the shared cumulative graph.steps quota.
Repeated calls accumulate; overflow and exhaustion stop before evaluation. This
is a conservative policy charge, not a proved evaluator-complexity upper bound
or exact instruction/aggregate heap bound. Nested stored construction/sorting
and equality can cost more than linear structural visits. Retain original typed errors/origin; false2104 and candidate
stops grant no resume, EOF, temporal absence-of-error or schedule credit.

## Consequences

The supplied Pred explicitly does not establish configured hook/name/snapshot
identity. Real queue/clock/checkpoint effects and whole ordinary temporal verdict
remain pending. Fresh deciding witnesses still require independent replay.
Earlier66fa/114de lifecycle receipts and failures remain their original epoch.
User and Cargo examples describe the diagnostic boundary precisely; no new
result-changing defaults or family certification are adopted.
