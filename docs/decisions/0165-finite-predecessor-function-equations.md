# 0165: Finite predecessor function equations

- **Status:** accepted
- **Date:** 2026-11-22

## Context

LuPastry+ uses an unbounded `CHOOSE` to name a finite function whose next
entry reads its strict predecessor. TLC refuses the unbounded choice. Both
the bounded and counted encodings of the published recurrence provide
reference values for the complete function. See #1334.

## Decision

The default profile recognizes an equality between the unbounded choice
variable and a single-bound function constructor in resolved IR. Each use
of the choice variable in the constructor must be an application at the
same index minus one. The bound must evaluate to a finite ordered
small-integer domain. At each index the evaluator computes the original
entry with the already computed predecessor available. The base case
and missing predecessors follow ordinary demand and lookup semantics.
The equality fixes the domain and induction over its ascending order
fixes each defined entry.

Core and independent trace evaluation use the same shape admission and
construct the value independently. The TLC comparison profile keeps its
unbounded CHOOSE refusal. See [D-046](../tlc-differences.md#d-046-finite-function-equations-with-strict-predecessors).

## Consequences

The released LuPastry+ routing table and other strict predecessor recurrences
can run without source edits. Exposed recursive function values, forward
dependencies, effects, and configured operator substitutions have named
refusals in this fragment. Undefined demanded entries surface as ordinary
evaluation errors.
