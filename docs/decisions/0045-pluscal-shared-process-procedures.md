# ADR 0045: Shared procedure normalization across disjoint finite processes

- **Status:** accepted for the documented bounded slice; full parity pending
- **Date:** 2026-10-10

## Context

Pinned `PcalTLAGen.GenMultiprocess` generates each procedure before all process
bodies, once. Procedure slots and `stack` initialize over the union `ProcSet`;
every procedure action binds `self`. Constant-ID process callers retain their
literal owner while member callers bind `self`. `GenNext` orders constant
processes, quantified procedures, then member processes. Repeating procedure
normalization for each block would duplicate storage/actions and inflate quotas.

## Decision

Reuse the source-backed scalar procedure normalizer once on a private source
arena, with all admitted process declarations available for name checks. Each
additional body borrows that arena and contributes its own actions. Reorder
storage as globals, procedure slots and process locals, as the primary does.
Charge action/node budgets globally before appending each body's actions.
Runtime recursion is never expanded at translation time.

Admission requires finite pairwise disjoint bounded-integer/unescaped-string ID
domains and disjoint process/procedure/global/local/label names. Process-local
reads in procedures, cross-process local reads, process-declared/option fairness
and unproved name rewrites remain explicit refusals. Existing owner-aware
emission and global algorithm-level fairness generation are reused. No crate
dependency or new mathematical/default-profile decision is introduced.

## Consequences

Complete capture-first C/P controls retain unsupported, rejected and invalid
reference outputs alongside admitted artifacts. Token equality, enclosing-module
CST/SANY validation, finite execution counts and independent witnesses remain
separate evidence. Translation never claims model proof. SANY-invalid primary
artifacts receive no credit or silent repair. Strict #124 namespace witnesses
remain pending; canonical replay is explicitly qualified evidence only.

Source provenance: tlaplus `94d0c50`, `PcalTranslate.ExplodeCall`, `ExplodeReturn`,
`ExplodeCallReturn`, `ExplodeCallGoto`, `PcalTLAGen.GenMultiprocess`, `GenVars`,
`GenInit`, `GenProcedure`, `GenProcess`, `GenNext`, `AddSubscriptsToExpr` and
`PcalSymTab` ownership/declaration contexts. Native admission and quota mechanics
are independent design. See `docs/pluscal-multiple-procedures.md` and the complete
`crates/dive-pluscal/multiple-procedure-acceptance` study.
