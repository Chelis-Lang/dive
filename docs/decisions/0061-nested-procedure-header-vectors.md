# 0061: Typed nested procedure header vectors

- **Status:** accepted
- **Date:** 2026-10-11

## Context

ADR 0058 proved top-level sequential procedure checksum serialization. Its six
C/P nested If/With/Either controls remained named refusals after an earlier
serializer produced the wrong original/header AST bytes. The new selection
retains all 132 rows and adds 48 paired control-flow, CallGoto, return and While
controls. Clean reference captures precede native changes.

The reference generator generates from a separate exploded AST, but that AST
shares child objects and vectors with the stored original AST. Hashing the
exploded root would be wrong. `CopyAndExplodeLastStmt` mutates shared child
vectors; `ExplodeLabeledStmt` copies its outer vector, as does `ExplodeWhile`
for its unlabelled loop body. Direct transfers stay source nodes in the header,
while transfers inside shared controls become assignments.

## Decision

Use an internal typed header overlay, never mutate the public parse AST and
never substitute serialized text to guess a CRC. Overlay nodes distinguish
source statements from synthetic assignments; RHS variants distinguish source
expressions, saved frames, quoted continuations, Head(stack) slots and Tail(stack).
Continuation labels come from source label groups and ownership, including Done,
Error and loop-body successors. Branch needsGoto propagation follows the pinned
If/Either/With cases. The generator's assignment sorting is applied to the
synthetic simultaneous call assignment before header serialization.

Charge source-vector work, generated assignment/frame names and continuation
bytes before allocation. Keep existing global node/depth/output ceilings and
writer byte checks; no quota increase. Unsupported virtual elsif transfer chains,
branch labels, call graphs, recursion, multiple/process-owned procedures,
renaming and other unproved checksum combinations remain named refusals.
No crate edges, raw generator behavior or checksum defaults change.

## Consequences

Exact original-header bytes/CRC, separate exploded ownership, whole generated
CST/SANY/Validator admission and finite execution/witnesses remain distinct.
Reference-invalid artifacts receive no public admission or model credit. D-020's
fair-header-only CRC qualification and #124's strict saved-frame replay guard
remain. Canonical witness checks cannot certify TLC-profile replay. Families
F-034/F-035 remain partial.
