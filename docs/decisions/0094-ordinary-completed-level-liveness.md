# 0094: Ordinary completed-level liveness

- **Status:** accepted direction; implementation subject to integration gates
- **Date:** 2026-10-13

## Context

Issue #241 approves FinalShort as the ordinary temporal solver and deterministic
completed-level periodic checks. ADR 0086 requires observable outcomes and valid
counterexamples, without reproducing TLC timers, services or lasso identities.
K provides a coordinator Observer after complete Init and whole BFS levels.

## Decision

Ordinary default-profile temporal checking uses Growth::due with threshold 0.1,
last initially one, at completed levels only, and always checks at exhaustion.
Compat ordinary checking selects Final, the pinned golden reference. Explicit
`--lncheck deterministic` selects the same D-014 boundary schedule under compat.
`--lasso short` is optional; lasso identity is not a parity obligation.

Observer level_control runs after edge/level callbacks. Continue and deliberate
Stop are distinct from callback error/panic. Stop retains Prefix facts and skips
postconditions; it cannot create Complete or Holds, even if the queue is empty.
All waves, safety checks, frontier/predecessor work and callbacks must succeed
before a boundary is visible. A partial failing level emits no temporal candidate.
Only fresh independent typed-word and serialized-word Valid results authorize
a temporal prefix stop. Unsupported/Invalid/bounded replay is never a violation.

The graph solver prepares graph plans for all configured claims once. TLC's
2213/2214 classifier receives the original unnegated temporal-role claims and
behavior constraints before tableau construction. The solver finds an accepting
lasso and directly attributes all false claims. ENABLED is exact and independent
of captured outgoing rows.
No-claims models use the ordinary safety path and the same startup binding/host.
A dispatch callback result is not a checker verdict or generic capture admission.

Accepted original Init occurrences share the existing graph-entry bound; distinct
roots and raw counts/callbacks remain unchanged. Original target atoms run before
identity preparation. Actual state/source variants must agree on atom truth;
completed alias rows use bounded successor/constraint replay and the producer's
exact/fingerprint key-preparation kernel with actual options. Keys are discarded;
constructor flags are preserved. Unsafe/unavailable replay or incoherent word
projection yields a source-named refusal, never repaired values or Holds.
Additional cells, transport/expansion and generated work consume retained budgets.
Row/ordinal/label order is internal coherence evidence, not TLC selection parity.

## Consequences

The native temporal adapter rejects a prefix unless its schedule is deterministic,
its completed level is positive, its next frontier is nonempty and actual depth
is the completed level plus one (checked for overflow). Final has no level
consumer, and the temporal consumer skips queue-zero boundaries. These checks
do not change the generic Observer Stop contract or Final-study assessment.

Prefix reports disclose the actual level, generated/distinct/depth/queue, physical
workers and store/collision qualifications, and mark exhaustion false. An earlier
validated boundary can prevent a later safety event and differ from final counts;
D-014 is a narrow scheduling difference, never blanket conformance leniency.
Compat Final retains one-worker counts/depth/property/error/exit checks. Temporal
lasso length/ending choices are judged by independent validity; safety shortest
BFS witness checks remain.
Non-oracle temporal agreement also requires an available independently validated
document at every worker/count policy. Missing or unavailable words are pending;
an invalid word disagrees. Numeric exit codes compare exactly, including absent
observations and worker-specific goldens, independently of word validity and
count policy. The existing complete finite2116/exit12 classifier is retained;
there is no global2116-to13 rewrite. The real native adapter accepts the current
ordinary schema-2 protocol directly: actual counters are not erased by the
`reported_by_tlc` print marker; a prefix retains its boundary/queue and cannot
claim Success. Shared structural count/lasso parsers apply no study comparison
policy. Unknown routes, inconsistent metadata and unclaimed classes fail closed.
Fresh independent replay uses the actual profile/module paths. An implementation's
validation JSON is not authority.

VIEW/SYMMETRY lifting, runtime effects/RNG/registers/composition replay, storage
combinations, postconditions with temporal checks and valid `_PERIODIC` hooks
remain refused. Existing formula/capture/product/independent-existence limits are
entry/work bounds. No timer, terminal/request service, proof ladder, tableau-ID
emulation, new dependency edge or broad language delta is introduced.

## Validation

Scoped source and native controls exercise real 1/4-worker boundaries, multiple
waves, queue preservation, callback error/panic, partial-level safety priority,
fresh validation and refusal, exhaustive Holds and single startup effects. The
retained 54 planning rows are measured separately from runtime conformance.
Full gates/corpus measurements and integration review remain required; this ADR
makes no phase-completion or new corpus-agreement claim.
