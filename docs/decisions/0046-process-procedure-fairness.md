# 0046: Bounded process-owned procedure fairness

## Status

Accepted for a bounded native translator increment. Full translator and liveness
execution certification remain pending.

## Context

A fair PlusCal process needs fairness of its body and of every procedure it can
call. Procedure clauses belong to the calling process's ID; procedure actions
remain shared. The pinned translator collects direct calls in source order and
extends those lists by declaration-ordered transitive reachability. Reordering
these clauses or flattening their junction indentation can change the generated
TLA expression tree.

The capture-first selection in
`crates/dive-pluscal/procedure-fairness-acceptance/selection.json` retains both
source grammars, explicit weak/strong declarations, translator fairness options,
label modifiers, nested/cyclic calls and invalid-reference artifacts.

## Decision

Keep the existing finite, disjoint process/domain and namespace admission.
Enable process/procedure fairness only within that proved ownership context.
Sequential procedure label modifiers stay explicitly unsupported.

Compute reachability from the admitted source AST without executing conditions
or unrolling calls. Preserve direct-call source order, then append transitively
reachable procedures in declaration order. Retain cycle reachability, including
a reachable procedure calling itself; ignore unused procedures in fairness
clauses while still translating their actions.

Match `PcalTLAGen.GenSpec`, `FormulaPair` and `ProcessFairness` grouping, including
its UTF-16 width accounting. Use an explicit generated-byte budget while
constructing formulas and the aggregate Spec, before retaining amplified text.
`-nof` suppresses process and procedure clauses. `-wf` and `-sf` apply only to
otherwise unfair processes. `-wfNext` and algorithm fairness add the existing
whole-Next clause.

No runtime Java delegation is introduced. Generation of a fair Spec does not
prove a liveness property. Reference outputs rejected by SANY remain diagnostic
raw evidence and never earn artifact agreement. Strict TLC witness replay remains
subject to the existing runtime-name provenance refusal (#124); separately
labelled canonical-profile replay cannot certify that profile.

## Consequences

Shared procedures receive per-caller fairness clauses without duplicated action
or storage ownership. Reachability is syntactic, so a call under `FALSE` still
contributes as in the reference. Scoped renaming, nonliteral process domains,
macro/procedure interactions and unproved multiline fairness domain layout stay
named refusals. No default liveness behavior changes.

## Provenance

Pinned `94d0c5070852b37b801bb8998c436383ab7ee37b`:
`pcal.ParseAlgorithm.GetCall`, `pcal.PcalFixIDs.FixMultiprocess`, and
`pcal.PcalTLAGen.GenSpec`, `FormulaPair`, `ProcessFairness`.
