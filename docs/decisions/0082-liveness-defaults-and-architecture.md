# 0082: Liveness defaults and architecture

- **Status:** accepted
- **Date:** 2026-10-12

## Context

ADRs 0017 and 0018 define TLC's liveness semantics and dive's liveness
architecture. Between them and the 2026-10-12 rescue reset, about thirty
increment ADRs added projections of TLC internals: planning normal forms, tableau
particle numbering, phase-L receipts, SCC visitation and witness order, periodic
controller timing, request and queue quiescence, and terminal-service tokens.
Most of that work reproduced how TLC arrives at an answer, not the answer itself.
The rescue reset (ADR 0086) defines parity as observable results: verdict,
counts, violated property, error class and exit code, depth, and an
independently validated counterexample. This ADR keeps the liveness decisions
that still matter under that definition.

## Decision

**Semantics and comparison.** ADR 0017 remains the semantic reference, and ADR
0018 the architecture: an observer on the deterministic safety search builds the
behaviour graph, and `dive-liveness` owns the formula pipeline, product, SCC and
lasso search. Conformance compares the verdict, error class, exit code, the
complete search's counts and the named violated property where it is well
defined. Every reported counterexample must replay in `dive-trace`. TLC's exact
lasso, SCC choice, tableau numbering and edge order are not parity obligations.

**Schedules.** Final checking remains the reference schedule for ordinary
TLC-profile and explicit `--lncheck final --lasso short` requests. Eligible
ordinary dive-profile requests use the owner-approved deterministic
completed-level schedule (D-014, ADR 0018 §7): a check after a completed level
once the graph has at least 2^16 distinct states and at least twice as many as
at the previous check. Exhausted frontiers skip prefix analysis and receive a
final check. A prefix never proves `Holds`; a violation requires
fresh independent typed and serialized validation. Prefix statistics and
completion/exhaustion truth describe the actual boundary, without copying final
counts or reproducing TLC's wall clock. Goldens stay final; a model whose
violation is found on a prefix differs from them in its counts only, and is
registered as a D-014 expected divergence.

**Counterexamples.** dive's default lasso is short and deterministic
(`--lasso short`). A violation is reported only when the candidate word passes
independent validation, both in memory and after JSON serialization. An
unvalidated or undecidable candidate is reported as `unsupported` and never as a
violation. Original configured property names stay separate from derived
temporal role labels, so a violation names the user's property.

**Temporal expressions.** Pure temporal expression arguments, including operator
formals, are kept with their lexical context (LET, substitutions, operator
arguments) and translated at each use. They are not forced as constants or
evaluated to guess their truth. Effectful, opaque or checker-reading operators
are refused by name. Temporal arguments inside a retained INSTANCE substitution
are refused until supported.

**Errors.** Evaluation failures during planning keep their original TLC error
class and message, and are never swallowed by a fallback branch. Unsupported
shapes and resource limits are named refusals, not errors in the spec.

**Store and identity.** Temporal checking over the exact store is refused until
the exact store can provide a temporal proof. Fingerprint-store results keep the
store's collision qualification. Symmetry with liveness follows ADR 0019 §4 and
ADR 0085. Amended 2026-11-18 by ADR 0158: the exact store now names seen
targets by discovery id, so Core checks temporal properties with `--store
exact`; spilled or reclaimed temporal storage is still refused.

**Evaluator contexts.** `Evaluator::new` and `Evaluator::with_sink` validate
their context and return `Result<_, ContextError>`, with codes `DIVE-EVAL-C001`
to `C005`. Scopes are tied to their IR and binding context, and a foreign scope
or cached value is refused rather than silently reused. Independent trace,
enabledness and planning entry points check this ownership before they analyse
anything.

**Bounds.** Formula, tableau, graph and recursion limits are explicit named
resource results. They are entry and work bounds, not total memory guarantees.
A resource stop never reports `Holds`. Capture bounds are cumulative per run;
planning is performed once, and label/product/formula bounds retain their actual
per-property/per-analysis-check scopes. Repeated checks add work; there is no
total-run work/byte budget or speedup claim.

## Consequences

- Diagnostic projections of TLC's planner, tableau, phase-L, periodic
  controller and terminal service are no longer product commitments. They were
  removed on 2026-10-12, together with the explicit `--lasso tlc` mode and its
  `dive-final-tlc` harness mode. What remains of them is TLC's liveness formula
  classifier (`dive_liveness::tlc_plan::classify`, errors 2213 and 2214, for
  `--compat=tlc`) and TLC's size-based growth trigger
  (`dive_liveness::periodic_backend::Growth`). Deterministic periodic checks use
  `dive_liveness::analyze::periodic::Schedule` instead. Amended 2026-11-08:
  Core never used the growth trigger, so `periodic_backend` was deleted with the
  reference checker's remaining hooks (issue #791, ADR 0155); only the classifier
  remains.
  `plan.rs` keeps one lowering entry point, `lower`, for final/short checking
  and the classifier. It follows TLC's conversion rules for `IF`, `~ENABLED`,
  numeric operands and evaluation errors; unsupported scopes and effects remain
  explicit refusals before evaluation.
- `--compat=tlc` still selects TLC's observable behaviour where it is
  deterministic (ADR 0010). It does not promise TLC's internal visitation order.
- A remaining liveness gap is an explicit refusal tracked in
  `docs/tlc-differences.md`, not a reason to rebuild TLC's scheduling.

Supersedes: 0023, 0028, 0034, 0036, 0040.
