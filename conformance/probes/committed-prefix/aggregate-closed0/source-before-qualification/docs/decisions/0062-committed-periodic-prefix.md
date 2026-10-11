# 0062: Publish an actual completed prefix under request quiescence

- **Status:** accepted
- **Date:** 2026-10-11

## Context

Issue #204 follows the immutable #201 producer, which retained current-wave
outputs without publication. Pinned local `Worker` returns Next output before
`doNextCheckLiveness` writes. Actual controlled suspension can observe returned
but unwritten output, later written sources, known errors and early hook/candidate
returns. Complete 24 method observations and both failed adapter epochs remain
preserved. They are selected method evidence, not ordinary scheduling proofs.

## Decision

Close claims with the existing owned request lane, join admitted work and hold
the borrowed real pool mutex. Publish only the maximal returned contiguous source
prefix. Partition candidates by original source position before seen-set resolve,
retaining original hashes/shard identities and all later buffers. Normal whole-wave
and prefix commits share one resolve/count/safety/output/source/frontier path.
Pause-owned resolve and invariants execute inline; never reenter `Pool.run` under
its dispatch lock. Resume untouched outputs and unclaimed sources without regen.

A private typed `RequestPrefix` barrier distinguishes actual published range from
the full submitted wave and expansion-pool quiescence. Inline predicate workers
are separately reported. Source/Phase-L publication precedes physical frontier
append as before; stopping callbacks do not manufacture successful append receipts.
Queue statistics after a successful prefix use its real cursor/frontier. Known
expansion cuts cancel requests while preserving earlier-source error drainage.

## Consequences

Only actually published graph data reaches the controller, owned configured hook
and SCC. Partial success never means Holds. Genuine candidate words must pass
fresh independent memory/JSON replay; unchanged validator work refusals remain
negative controls. Clock timing and asynchronous ordinary stop/count parity remain
unproved. Ordinary CLI/checkpoint/profile/effect refusals stay intact, as do all
resource defaults. See the [executable contract](../committed-periodic-prefix.md).
