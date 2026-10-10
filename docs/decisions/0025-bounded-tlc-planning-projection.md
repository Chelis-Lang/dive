# 0025: Bounded TLC normalization and ordered pre-tableau grouping

- **Status:** accepted
- **Date:** 2026-10-10

## Context

The current language kernel and final/short checking path preserve temporal
truth but do not reproduce `Liveness.processLiveness`'s planning identities or
ordering. The pinned implementation tags predicate occurrences before pushing
negations, simplifies only specific syntax, moves recurrence disjunctions to the
end during DNF conversion, and groups branches by ordered syntactic equality.
Its weak-fairness AST shape differs from an equivalent formula used by the
existing kernel. Truth equality alone does not prove those operational details.

The raw development evidence in `conformance/probes/tlc-planning` includes
84 supplied-formula controls and twelve real `FastTool` models. The latter
also inspect actual `processLiveness` bin and possible-error-model arrays.
All selected failures remain captured; none is counted as a completed plan.

## Decision

Add a separate bounded diagnostic projection in `dive_liveness::tlc_plan`:

- Retain original configured property names and predicates separately from
  temporal-role labels and their bound predicate tables. The raw `TpLocal`
  projection distinguishes TLC's Action/configured name `Live` from its derived
  temporal-name vector `Identity`; dive's role label matches the latter. Keeping
  original action attribution through tableau/witness execution remains a
  separate representation obligation (#134). Tag
  predicate **occurrences**, not equivalent predicate truth, before rewriting.
- Retain tagged, negation-pushed, simplified and ordered-DNF stages. Match
  pinned `pushNeg`, `simplify`, `toDNF`, `flattenSingleJunctions`, classification
  and first-encounter bin/group ordering. Do not apply additional identities.
- Classify `<>[]predicate` into EAAction, `[]<>state` into AEState, and
  `[]<>action` into AEAction. Other action-free syntax remains for the tableau.
  Other action-containing shapes yield a named unsupported result.
- The model entry point reuses the existing scope/purity planner and its
  per-predicate bounds. A private shape selector constructs TLC's weak-fairness
  form only for this projection. Existing kernel/default verdict behavior stays
  unchanged; no callback is evaluated to discover whether it is pure.
- Retained formula-arena nodes/references and transformation work consume
  existing budgets. Group/PEM metadata is bounded by the already charged DNF
  branches and conjunct references; this is not an aggregate byte or metadata
  budget. Preflight the full DNF product before allocation and retain
  TLC's Java-int cardinality ceiling. Nested child transformations precede
  outer product calculation, matching pinned failure precedence.

This stage does **not** implement binary tableau/promise numbering, phase-L
capture, periodic scheduling, SCC/witness selection or strict integrated
liveness. Successful projection must never lift those existing refusals.

## Consequences

Field-level projections can be audited separately from temporal truth. Direct
word evaluation and the independent scoped periodic interpreter check every
retained transformation and reconstructed grouped condition, including closing
actions. Original properties remain available for independent attribution;
DNF groups are not treated as property identities.

Retaining intermediate arenas can reach the existing formula-term bound sooner
than a single-stage planner. That is an explicit construction failure, not an
empty plan or proof. Recursive/higher-order/opaque captures and unsupported
INSTANCE formals remain refused. F-019/F-020/F-021 remain partial and require
independent certification after the remaining strict execution stages land.
