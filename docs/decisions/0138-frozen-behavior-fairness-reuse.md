# 0138: Reuse one frozen behavior fairness pair within an analysis

- **Status:** accepted
- **Date:** 2026-11-01

## Context

Original ABA still needs TLC/W8 completion and useful TLC/W1 headroom. ADR 0134
already reuses successful input validation for identical retained rows; all twelve
ABA atoms qualify without native fallback. The three configured properties each
consume the same bound behavior WF obligation. Native execution still evaluates
that obligation separately for each property.

Two instrumented original N=4/T=1/F=1 W8 windows at source `b1a5972d`, with all
three properties and unchanged native 900-second/16-GiB limits, establish an
implementation opportunity only. Independent conservative instruction/source
inspection identifies 347/2,972 TLC and 508/2,971 default samples in direct
interpreter work. It excludes shape/heap/allocator ambiguity and concrete control
callsites. Retained control/proof/selection counts are 453/770 and demanded reads
60/98. Most exported DWARF chains lack sufficient ancestry; the frozen chain
reduction's weak result is preserved. Broader author symbol counts have a
separate, weaker scope. These windows establish no whole-run cost share, saved
time, ENABLED cost share, temporal verdict or original capacity acceptance.

## Decision

Lower the actual bound behavior prefix once, then literally clone that prefix
into the original configured property plans. Startup temporal-role conversion
remains separate. The planner privately issues tokens only for the first WF
pair in that frozen prefix. Property-owned WF, later WF pairs and SF receive no
reuse token. Expression IDs and `Scope::same_analysis_context` are never identity
or equivalence proofs. A checked process-unique planner namespace prevents
cross-preparation token reuse; exhaustion declines this optional optimization
rather than wrapping. Each retained atom still independently passes the existing
native stability/compiler admission and exact object-identity checks.

A private cache belongs to one `analyze_graph` invocation and its immutable graph
and plan batch. It may retain only the completed packed action and ENABLED state
columns of that pair. A column is installed only after every demanded evaluation
completed natively; errors, fallback and partially written columns never install
it. The implicit stutter and first physical original target retain their existing
meaning. No graph, edge-offset array or other property column is copied.

Every later consumption retains metadata validation, original-target selection
and required reference reads/conversion in their original order. A new optional
reader hook supplies current native *input admission*, never a boolean supplied
by the caller. Its default declines. The actual reader checks owned controls,
exact retained descriptor, current Store identity and heap poison, and source
then target through the existing independent input quotas/row proofs. Only then
may the private planner/graph proof supply its stored truth. Native interpreter
fuel remains unchanged for actual evaluations. Reference fallback, errors and
owned stop authority remain in place.

The optional allowance is `min(16 MiB, requested memory bytes / 256)`; an unlimited
memory request permits at most 16 MiB. Checked layout includes cache metadata and
actual retained vector capacities. Reservation is fallible; cap/overflow or
allocation refusal uses the original path. Copies poll the existing control and
publish only a complete column. Searches of fewer than two properties, including
current periodic prefix searches, allocate no columns. Cache drop precedes fresh
independent candidate replay; every analysis starts with fresh storage.

## Consequences

Additive `fairness_reuse` metrics distinguish validated state/action consumptions
from actual interpreter evaluations, plus stored columns, cap/allocation bypasses
and peak accepted storage bytes (including an interrupted construction copy).
Existing evaluation counters remain actual evaluations;
row-proof counters still describe attempted input demands, including reuse. No
counter establishes time saved or completion.

This adds no CLI flag, semantic divergence, backend or dependency. Both profiles
retain their comparison semantics and all configured property/false-word checks.
Whole-process allocation accounting remains incomplete. Adoption requires focused
positive/negative controls and bounded unchanged-original capacity confirmation;
the eleven-of-twelve prior matrix and TLC/W1's thin margin remain unresolved until
new source-bound evidence establishes the required completion/headroom.

## Amendments

- **Amendment 1, 2026-11-07 (issue #820): every frozen prefix atom.** The
  planner issues one token for the whole bound behavior prefix that it lowers
  once and clones into every original configured property plan, instead of
  only its first WF pair. Every prefix atom (each WF and SF pair, and any other
  behavior temporal atom) may therefore keep one completed, entirely native
  packed column per analysis. Property-owned atoms still receive no token, and
  the other rules above are unchanged: completion-only installation, input
  validation on every consumption, the same storage cap (now including the
  per-atom column table), fresh storage per analysis. The cause was
  `SDP_Attack_New_Solution_Spec/MC`: six properties over fifteen behavior WF
  pairs evaluated 1,087,181 native atoms, 93% of the run's instructions; with
  the amendment it evaluates 206,091.
