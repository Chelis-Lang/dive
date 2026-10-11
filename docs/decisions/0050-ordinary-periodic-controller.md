# 0050: Preserve the pinned periodic controller at backend boundaries

- **Status:** accepted
- **Date:** 2026-10-11

## Context

Partial SCC checks over source-qualified graph prefixes do not reproduce TLC's
ordinary scheduling. `ModelChecker.doPeriodicWork` controls short-circuit growth
queries, suspension, forced checks, elapsed-time ratio updates, PERIODIC effects,
checkpoint transactions and early stopping. The exact runtime-ratio boundary
does not use the same inequality as the initial growth guard.

The pinned method was exercised on all 296 selected combinations through actual
`FastTool`/`ModelChecker` construction. Observing callbacks supplied queue and
SCC outcomes and projected checkpoint/ratio operations; the actual PERIODIC
expression ran through the pinned evaluator. Independent readback checks every
callback, code, force state and runtime marker. Initial wrong-keyword setup
failures and the first marker-observer failure remain retained.

## Decision

Port the control flow into `dive_liveness::periodic::run` with explicit `Hooks`.
The caller consumes checkpoint requests once. Preserve Java floating-point
branch comparisons, early-return ordering and force-clearing position. Backend
failures stay typed; negative elapsed wall-clock samples fail explicitly.
Callbacks implement graph, clock, queue, PERIODIC and checkpoint work separately.

Keep this API separate from ordinary checker verdicts until the actual backend
establishes graph-prefix authority, quiescence, ratio math, checkpoint/recovery,
counts/stopping and independently validated witnesses. No existing model-proof,
schedule or compatibility boundary is widened by this kernel.

## Consequences

The partial graph observer can integrate a controller without replacing source
authority with caller-supplied graph edits. Forced and failed checks have a tested
contract. The reference projection does not certify actual checkpoint bytes or
wall-clock math. F-023 remains missing/audit-required for the ordinary backend;
F-019 through F-022 remain partial. Bounded partial SCC diagnostics must not be
promoted into Holds results or ordinary schedule certification.
