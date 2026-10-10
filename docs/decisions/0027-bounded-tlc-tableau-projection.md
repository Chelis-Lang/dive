# 0027: Bounded TLC particle tableaux and promise identities

- **Status:** accepted
- **Date:** 2026-10-10

## Context

ADR 0025 preserves normalization tags and ordered OOS/PEM groups. Temporal truth
alone does not establish the binary formula, particle representatives, first-
encounter node identities, outgoing-edge order or promise order used by pinned
TLC. These identities are prerequisites for a future strict execution path.

## Decision

Add a separate diagnostic `dive_liveness::tlc_tableau` projection. It consumes
ADR 0025's tagged plan, leaving existing checking and validator algorithms
unchanged. The direct formula entry point accepts already tagged positive state
formulas; it never evaluates predicate callbacks or guesses their purity.
Bound model callers first use `tlc_plan::lower_model`'s conservative scope proof.

Match pinned `makeBinary`, `TBGraph`, `TBGraphNode`, `TBPar`, `TBParVec` and
`extractPromises` semantics: split original conjunction/disjunction children
before flattening each half; retain positive-closure encounter order and
particles' duplicates; expand alpha/inverse-alpha and beta/inverse-beta rules
in source order; use set-style particle equality while preserving TLC's chosen
right-hand union representatives. Number initial particles and subsequent nodes
in breadth-first first-encounter order. Keep duplicate edges, ordered state
predicates, preorder unique eventualities and each node's promise-fulfillment
bits. The empty-particle sole-self-loop flag is metadata, not a verdict.

Retain configured property and Action names separately from derived temporal
role labels in the parent bound projection. Group/particle identities must never
substitute for original property attribution. In particular `TpLocal` carries
configured/Action `Live` and derived role `Identity`; each field is checked
against its own pinned projection.

Use the existing formula-term, tableau-node and expansion-work limits. Source
and retained expression arenas count nodes plus child references. Retained
output nodes count cumulatively across groups. Vector copying, interning,
membership scans, particle expansions and graph lookup consume work before
allocation/expansion. Pending alternatives are bounded by the tableau-node
limit, including held left alternatives while the right branch expands. Add a
named maximum recursion depth of 256 (`TLC tableau closure depth`), without
raising any existing default. Group/edge/promise metadata is bounded by these
charged terms/nodes/work; this is **not** aggregate byte accounting. Invalid
edited group/bin/tag references, action-containing tableaux, empty binary
junctions and nonpositive direct input produce explicit typed refusals.

## Consequences

The complete selection contains 137 controls: 84 normalization projections,
35 direct particle formulas and 18 bound models. There are 126 complete exact
field matches, five normalization failures, four direct structural/action
refusals and two SANY-rejected models. The nonpositive direct negation is a
native precondition refusal even though the low-level Java constructor returns
an artifact; it is not reported as a match. Both invalid CONSTANT and corrected
VARIABLE INSTANCE controls remain present. All ten Java process failures and
all raw bytes are retained, along with the complete initial 136-case snapshot.

Independent periodic words compare the original formula against tableau
acceptance. A separate scoped periodic interpreter checks grouped fairness and
property truth at every word position and closing action. Fresh whole-witness
validation rejects altered initial/Next/closing transitions and property claims.
No validator depends on this tableau implementation.

This does not capture behavior graphs, schedule phase-L checks, execute TLC SCC
acceptance, select TLC witnesses or lift strict integrated-liveness refusals.
F-019/F-020/F-021/F-022 remain partial and audit-required; successful diagnostic
planning is not proof, certification or new corpus verdict credit.
