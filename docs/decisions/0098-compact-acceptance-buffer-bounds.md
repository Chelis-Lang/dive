# 0098: Compact acceptance buffer bounds

- **Status:** accepted; amended by [ADR 0106](0106-temporal-bounds-setting.md)
  (proposed)
- **Date:** 2026-10-15

> **Amendment (ADR 0106, proposed).** The shared file-checking resource policy
> below applies to `--temporal-bounds=default`, the default profile's setting.
> Under `--temporal-bounds=unbounded`, which `--compat=tlc` selects, the
> acceptance-buffer allowances are replaced by a resident-memory ceiling of a
> quarter of physical memory.

## Context

The rescue plan requests memory-based liveness limits. Compact acceptance stores
bits rather than legacy Boolean cells, so its logical cell ceiling no longer
reflects the storage it requests. Other graph and evaluator owners remain large
and separately bounded; this decision does not create a total memory budget.

## Decision

File-based ordinary eligible TLC checking and explicit final/short checking use
`AcceptancePolicy::compact_default()`: 8,000,000 requested acceptance-buffer bytes
and 32,000,000 independent materialization work units. The byte default preserves
the former nominal 8,000,000 Boolean-cell payload allowance; it is a policy
baseline, not measured model sufficiency or the old representation's total heap.
The work default derives from the existing transition-step allowance, which is
retained separately and continues to count the same inspections.

The byte policy replaces only logical acceptance-cell admission. It covers outer
column header allocations, packed words, optional edge offsets and maximum live
recurrence Boolean scratch. Checked layouts precede requests, allocation is
fallible, and actual Vec capacities are charged after exact reservation. No
covered buffer grows afterwards. Allocator overreservation may exist transiently
before capacity-based refusal. Allocator metadata/rounding, runtime/stack, input
labels/tableaux, capture, product graph/build/renumbering, search and witnesses
are excluded. This is a covered-buffer policy, not a physical heap/RSS ceiling.

Byte mode may retain recurrence columns on behavior-source/original-edge
positions and use the product's existing behavior-state and original-edge maps
for search. Equal targets never merge distinct parallel-edge positions. Each
condition/behavior edge is then evaluated once, without specification predicate
reevaluation. A product-edge representation remains available when the behavior
domain is larger or its conservative bound does not establish a safe choice.
Checked layouts and minimum/worst-case work metadata choose once before any
evaluation or allocation; there is no speculative evaluation or retry. Behavior
storage is preferred when its full worst-case fits and is cheaper, or when the
product representation cannot fit. Otherwise product storage stays available.

Work includes metadata-node/column inspections, initialized headers/words/scratch,
promise assignments, offsets, action-row inspections, edge projections and truth
bit writes, then every executed recurrence node and actually visited child
reference. Shape-only work is precharged; fixed minimum work is checked before
allocation. Eager atom order within every executed program and Boolean short
circuiting are preserved. Existing atom-cell admission and all atom evaluation
remain unchanged; their already checked behavior edge total is reused without a
new graph scan. Every exhaustion/overflow/allocation failure is a named resource
refusal; no partial product proves a verdict. Shape validation, vertex, edge and
transition inspection limits retain their existing order.

All existing owned Product and public `analyze_final*` / `check_final*` entrypoints
retain legacy logical limits, including custom literals. New additive policy
entrypoints select buffer admission explicitly; no value sentinel or equality
with defaults changes a caller's policy. Both semantic profiles use the same
file-checking resource policy. TLC has no corresponding fixed acceptance buffer
bound: exhaustion remains an explicit implementation refusal.

## Consequences

Compact products can admit more logical acceptance labels within this byte
allowance. Header/scratch-heavy cases can refuse earlier. Neither outcome is a
performance or corpus-recovery claim. Vertex/edge, domain/formula/depth and other
limits remain; search has no new aggregate memory or work controller.
