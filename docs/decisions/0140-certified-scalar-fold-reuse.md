# 0140: Certified scalar-fold reuse candidate adoption

- **Status:** rejected (candidate adoption)
- **Date:** 2026-11-02

## Context

S6 seeks reuse of expensive pure bodies across states whose irrelevant fields
change. The single candidate was archived at
`c4003fc63e2d886e1e8d2b6a905301ee5664a812`, tree
`8b78ac55bbef8b2ec66fc976550d48622cacb5f4`; it is not current-main code.

The proposal recognized a generic bounded recursive sum over a closed 0/1-valued
function and at most eight selected coordinates, rather than a named model.
Every original argument was evaluated and normalized eagerly, in order, before
any lookup. Full input checks made the selected body total; only a small integer
could escape. Raw, lazy, unproved or differently typed inputs retained the original
path. Existing history admission, guards and demand order were unchanged.

Before timing, independent review corrected public-Code authority and cache
isolation. Metadata was only a hint: each lookup re-proved the actual borrowed
Code/body structure, empty captures, frame/reset and recursion. Corrected private
cache access prevented public ordinary memo operations from fabricating body
results. These correctness controls did not grant performance adoption.

## Decision

Do not adopt or merge this candidate. Against `9ea1b81a`, the initial full
GameOfLife N=4 screen had three matched pairs/six correct rows, 65,536 distinct /
131,072 generated and median candidate/base runtime 0.5002306. The original-model
panel then physically completed 138 rows: GameOfLife 54, Sailfish 54 and
SimpleRegular 30. Its controller exited 10 on SimpleRegular TLC actual/requested
W8: candidate/base ratios **0.8211595695 / 1.2080699460 / 1.1794395975**,
median **1.1794395975**. Two of three pairs exceeded the predeclared **1.02**
control bar. The remaining 78 rows were correctly unrun.

Independent raw-data review verifies the source/count/pair receipts, the declared
negative stop and every member of the preserved archive. Whole-panel quiet
certification is false: telemetry records unresolved observer activity on reserved
CPUs. The data establish neither the slowdown's cause nor formal quiet performance
acceptance. The
[performance record](../../conformance/reports/perf/2026-11-02-phase6-scalar-specialization.json)
retains these limits. The initial positive screen stays separate from the stopped
adoption panel.

## Consequences

No body-cache code, runtime flag, semantic option or default change ships from
this candidate. There is no rescue variant. This rejects one candidate's adoption,
not broader S5/S6 expensive-body/read strategies or the correctness safeguards.

GameOfLife's roughly 50%/39%/67%/58% runtime reductions (default W1/W8, TLC W1/W8)
coexist with W8 RSS increases from 137.8 to 169.5 MiB default and 128.7 to
171.5 MiB TLC; W1 stayed around 55 MiB. Those gains and memory tradeoffs do not
override the failed control bar, establish current-main performance or satisfy
ADR 0119's global default floors and compatibility ceiling. Reference remains
the default. Future work must preserve the negative evidence and unchanged
history/eager-demand obligations.
