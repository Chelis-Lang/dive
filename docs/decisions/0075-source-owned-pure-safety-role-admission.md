# ADR 0075: Source-owned pure safety role admission for disk seen-state checking

- Date: 2026-10-11
- Status: implementation in progress
- Issue: #221

## Context

The opt-in disk seen-state route refuses every configured PROPERTY and
SPECIFICATION before startup. The shared binder already decomposes those roles
into initial predicates, invariants, implied actions and temporal obligations.
A blanket syntax-level temporal check cannot distinguish `Init /\ [][Next]_x`
from an actual liveness assumption or obligation. The declared 29-model matrix
retains clean pinned runs and both-profile resident/disk outcomes before changes.
Wrapper and quantified cases remain selected even where admission is unproved.

## Decision

Prepare the actual configured tool-object tables in `dive-model`, without a
RunHost or constant-definition/ASSUME execution. A bounded source walk follows
selected definition redirects and constant bindings, inspects native dispatch
explicitly, and includes potential startup roots. Native overrides' documented
bodies are not execution authority. Effectful/checker-reading and opaque
operator contexts refuse before startup. Conservative unused-source refusals
are reported rather than granted parity credit. The walk admits at most16,384
traversal visits (shared paths count repeatedly) and depth128. Active definition
and expression paths refuse cycles; a global visited cache cannot waive the
remaining depth or establish purity of an unproved redirected cycle.

Reuse the shared `Binder::process_config` and its original `spec`/`props`
decomposition with an explicit structural-only admission mode. That mode
refuses evaluation-demanding quantified domains, literals and strict instance
substitutions rather than substituting guessed predicates or levels. Remaining
temporal role vectors refuse by name. Valid admission is only a prerequisite:
the real binder still runs once in its original order, followed by the existing
fallible inline engine. Resident execution and its startup order are unchanged.

The first increment targets state PROPERTY, `[]` invariants,
`[][A]_v` implied actions and pure safety SPECIFICATION, with aliases, LET and
INSTANCE contexts where the actual shared scope/decomposition path proves them.
Malformed role/configuration diagnostics retain their actual stage and code;
an unproved effectful route does not earn error-equivalence credit.

## Consequences

Membership and insertion stay Result operations at original source/ordinal
positions. Actual inline execution and all storage/cleanup bounds remain
reported. Pure violation words require independent validation and semantic
mutant controls. Print-dependent invariant-error replay retains ADR0072's
explicit refusal. Capture, temporal checking, checkpoint/recovery and unproved
decomposition routes remain separate; this is no total-memory, scaling,
archive-certificate or full F-031 certification claim. Crate layering is unchanged.
