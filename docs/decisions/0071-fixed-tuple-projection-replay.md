# 0071: Fixed projection evidence for UNCHANGED tuple aliases

- **Status:** accepted
- **Date:** 2026-10-11

## Context

ADR 0067 proves tuple histories but its original UNCHANGED rule only recognizes
direct variables and a flat literal tuple. Generated procedure actions commonly
use `UNCHANGED vars` where `vars` is a fixed tuple alias. The baseline clean
oracle finds the invariant violation while strict independent replay refuses.
A conditional tuple can preserve its value while swapping constituent variables.

## Decision

Within the same sealed history proof, equality of a fixed nested tuple implies
equality at each fixed projection. Follow actual nonrecursive, non-native,
non-overridden definitions, lazy expression parameters, LET and substitution
contexts without evaluating values, forcing providers or trusting supplied states.
Refuse conditional projections, opaque/operator slots, recursion and unsupported
constructors. Mere variable occurrence in every conditional branch is insufficient.

Charge every inspection to the existing 32,768 visit/depth64 proof budget. The
256-variable policy and operand budget are unchanged. Only already established
candidate tuple variables can use the preservation rule. Independent public
arbitrary-state APIs retain empty history hypotheses. No dependency edge or
result-changing default changes.

## Consequences

Fixed aliases may admit additional independently checked whole words; general
saved-stack or runtime-name provenance is not certified. Original generated
procedure words require separate fresh replay measurements. Preserve the actual
conditional-permutation negative and false-proof mutation. The user guide and
maintained limitation log describe the measured current boundary. Full frozen
gate and fresh CLI/JSON controls precede publication.

## Qualification

The frozen implementation passes 1,219 workspace tests without skips and the
full local gate, with the original disk-space interruption retained. Fresh fixed
aliases and their altered words are tested at 1/4/8 workers, both stores and
two scheduling seeds. A real false-proof mutation admits a conditional tuple
permutation; the focused negative catches it. The original generated procedure
study admits 32/32 strict words at this producer; this is separately
measured and does not establish general saved-stack authority.
