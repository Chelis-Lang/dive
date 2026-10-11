# ADR 0044: Bounded process-owned procedure slots and continuations

Status: accepted for the documented single finite process slice; full parity pending.

## Context

Sequential procedure frames were scalar. Pinned `PcalTLAGen.GenProcedure`,
`GenVars`, `GenInit`, `GenNext` and `AddSubscriptsToExpr` give every procedure
slot and stack a process index, even for a constant-ID process. Procedure actions
bind `self`; constant callers use their literal ID. Source references to the
synthetic `stack` remain explicitly written, unlike generated transfer expressions.

## Decision

Normalize one finite process through the proven typed procedure continuations,
then retain process ownership in the generated model. Order global, procedure
and process declarations as the reference does. One private expanded source
arena feeds every borrowed procedure scope. Aggregate action/node/output quotas
remain global, and runtime recursion is never unrolled.

Emission uses owner-aware reads and whole-function EXCEPT updates for procedure
slots, stack and pc. Call arguments remain simultaneous, local initialization
ordered, ordinary frames owner-specific, and self-tail calls frame preserving.
Procedure wrappers quantify over ProcSet separately from process wrappers.
Multiple process blocks with procedures, scope disambiguation, procedure reads
of process-local variables and process-declared/option fairness paths remain named refusals.
Algorithm-level fair syntax generates global WF_vars(Next) through the existing
model.fair path; separate two-row SANY/CST controls prove formula generation,
not liveness execution or procedure fairness.
No dependency graph changes or new crates/dependencies.

## Evidence and consequences

All 112 capture-first C/P controls, raw primary/normalized/SANY outputs and
invalid artifacts remain selected. The predecessor's refusals and the initial
four explicit-stack substitution mismatches are preserved. Native admitted
artifacts require complete token/CST/SANY controls; generated-model checking
and independent validation are separate contracts. Invalid primary artifacts
receive no credit. Strict #124 witnesses remain pending; canonical replay is
qualified evidence only. F-034/F-035 remain partial/audit-required.

A readable executable CLI/API guide accompanies the implementation. Source
provenance: pinned `94d0c50`, `PcalTranslate.ExplodeCall`, `ExplodeReturn`,
`ExplodeCallReturn`, `ExplodeCallGoto`; `PcalTLAGen` ownership and emission methods
above; `PcalSymTab` process/procedure declaration order and `ParseAlgorithm`
call-tail normalization. Native namespace admission/budget helpers do not claim
source-structure porting.
