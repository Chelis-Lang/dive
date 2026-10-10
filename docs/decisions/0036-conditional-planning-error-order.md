# 0036: Conditional planning conversion and deciding error order

- **Status:** accepted
- **Date:** 2026-10-10

## Context

Pinned `Liveness.astToLive` recursively converts the guard and both branches of
a nonconstant `IF` before falling back to a whole state/action atom. A constant
division by zero in either branch can therefore stop planning before any graph
or fingerprint work. The native early atom shortcut previously deferred it.

The reference also tries the guard against an empty state, converting only the
chosen branch if that succeeds. State-level conjunction, disjunction, negation
and implication convert children first; state-level equivalence does not. A
nonrecursive user call can swallow a conversion exception and fall back to the
original call. These distinctions are visible in the complete pinned selection.

## Decision

Match these conversion paths in the TLC-shaped bound planner. Preserve actual
evaluation failures as typed `PlanningError::Evaluation`, carrying the original
TLC code/message and inner expression, plus the conversion operand's source.
Temporary branch atoms are discarded before whole-expression fallback so they
cannot acquire tags or enter predicate bins. Unsupported features, malformed
contexts and resource limits never take an exception-swallowing fallback.

Guard probes remain pure and bounded, use the existing bound evaluator, and
inspect no concrete reachable states. Nonliteral nonboolean constant diagnostics
without a proved TLC semantic-node rendering remain named refusals. The ordinary
independent formula planner retains its existing behavior. No crate edge or
result-changing default is introduced.

## Consequences

Retain 24 complete current controls, all 18 original controls, the initial
SANY-invalid action wrapper capture, the native false completion and subsequent
development failures. CI checks the complete raw/source bytes. Phase-L tests
require the deciding constant error before any worker, fingerprint or predicate
work. Successful state-IF phase-L admission remains conservative; this does not
certify strict integrated liveness, all source-conversion paths or the family.
